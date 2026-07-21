use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskAction {
    Spawn,
    SendInput,
    Cancel,
}

impl TaskAction {
    pub(super) fn parse(value: &Value) -> BitFunResult<Self> {
        let action = match value
            .get("action")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|action| !action.is_empty())
        {
            Some(action) => action,
            None => {
                return Self::infer_from_input(value)
                    .ok_or_else(|| BitFunError::tool("action is required".to_string()))
            }
        };

        match action {
            "spawn" => Ok(Self::Spawn),
            "send_input" => Ok(Self::SendInput),
            "cancel" => Ok(Self::Cancel),
            other => Err(BitFunError::tool(format!(
                "action must be one of: spawn, send_input, cancel; got '{}'",
                other
            ))),
        }
    }

    fn infer_from_input(value: &Value) -> Option<Self> {
        let has_description = value.get("description").is_some();
        let has_prompt = value.get("prompt").is_some();
        if !has_description || !has_prompt {
            return None;
        }

        let has_session_id = value.get("session_id").is_some();
        let has_subagent_type = value.get("subagent_type").is_some();
        let has_fork_context = value.get("fork_context").is_some();

        if !has_session_id && (has_subagent_type || has_fork_context) {
            return Some(Self::Spawn);
        }
        if has_session_id && !has_subagent_type && !has_fork_context {
            return Some(Self::SendInput);
        }

        None
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::SendInput => "send_input",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct TaskInvocation {
    pub(super) action: TaskAction,
    pub(super) description: Option<String>,
    pub(super) prompt: Option<String>,
    pub(super) context_mode: SubagentContextMode,
    pub(super) target_session_id: Option<String>,
    pub(super) subagent_type: Option<String>,
    pub(super) model_id: Option<String>,
    pub(super) inherit_parent_model: bool,
    pub(super) timeout_seconds: Option<u64>,
    pub(super) run_in_background: bool,
    pub(super) is_retry: bool,
    pub(super) requested_auto_retry: bool,
}

impl TaskTool {
    pub(super) fn parse_invocation(
        input: &Value,
        is_deep_review_parent: bool,
    ) -> BitFunResult<TaskInvocation> {
        if input.get("workspace_path").is_some() {
            return Err(BitFunError::tool(
                "workspace_path is no longer supported; subagents inherit the current workspace. Put any non-current target path in the prompt."
                    .to_string(),
            ));
        }

        if is_deep_review_parent {
            if input.get("action").is_some() {
                return Err(BitFunError::tool(
                    "action is not supported for DeepReview Task calls".to_string(),
                ));
            }
            for field in ["fork_context", "session_id", "run_in_background"] {
                if input.get(field).is_some() {
                    return Err(BitFunError::tool(format!(
                        "{field} is not allowed for DeepReview Task calls"
                    )));
                }
            }

            let (model_id, inherit_parent_model) = Self::optional_model_id(input)?;

            return Ok(TaskInvocation {
                action: TaskAction::Spawn,
                description: Self::string_field(input, "description", "DeepReview Task calls")?,
                prompt: Self::string_field(input, "prompt", "DeepReview Task calls")?,
                context_mode: SubagentContextMode::Fresh,
                target_session_id: None,
                subagent_type: Self::string_field(input, "subagent_type", "DeepReview Task calls")?,
                model_id,
                inherit_parent_model,
                timeout_seconds: Self::optional_timeout_seconds(input)?,
                run_in_background: false,
                is_retry: input.get("retry").and_then(Value::as_bool).unwrap_or(false),
                requested_auto_retry: input
                    .get("auto_retry")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            });
        }

        let action = TaskAction::parse(input)?;
        if Self::has_deep_review_retry_fields(input) {
            return Err(BitFunError::tool(
                "DeepReview retry fields are only allowed for DeepReview Task calls".to_string(),
            ));
        }
        if input.get("timeout_seconds").is_some() {
            return Err(BitFunError::tool(
                "timeout_seconds is only allowed for DeepReview Task calls".to_string(),
            ));
        }
        let run_in_background = Self::optional_bool(input, "run_in_background")?.unwrap_or(false);

        match action {
            TaskAction::Spawn => {
                let description = Self::required_string_for_action(input, "description", action)?;
                let prompt = Self::required_string_for_action(input, "prompt", action)?;
                if input.get("session_id").is_some() {
                    return Err(BitFunError::tool(
                        "session_id is not allowed when action is spawn".to_string(),
                    ));
                }
                let context_mode = Self::context_mode_from_input(input)?;
                match context_mode {
                    SubagentContextMode::Fresh => {
                        if input.get("subagent_type").is_none() {
                            return Err(BitFunError::tool(
                                "subagent_type is required when action is spawn and fork_context is false or omitted"
                                    .to_string(),
                            ));
                        }
                    }
                    SubagentContextMode::Fork => {
                        if input.get("subagent_type").is_some() {
                            return Err(BitFunError::tool(
                                "subagent_type cannot be combined with fork_context=true when action is spawn; use either subagent_type for a fresh subagent or fork_context=true to inherit the current context."
                                    .to_string(),
                            ));
                        }
                        Self::ensure_fields_absent(
                            input,
                            &["retry", "auto_retry", "retry_coverage"],
                            action,
                        )?;
                    }
                }

                let (model_id, inherit_parent_model) = Self::optional_model_id(input)?;

                Ok(TaskInvocation {
                    action,
                    description,
                    prompt,
                    context_mode,
                    target_session_id: None,
                    subagent_type: Self::optional_trimmed_string(input, "subagent_type")?,
                    model_id,
                    inherit_parent_model,
                    timeout_seconds: None,
                    run_in_background,
                    is_retry: false,
                    requested_auto_retry: false,
                })
            }
            TaskAction::SendInput => {
                let target_session_id =
                    Self::required_string_for_action(input, "session_id", action)?;
                let description = Self::required_string_for_action(input, "description", action)?;
                let prompt = Self::required_string_for_action(input, "prompt", action)?;
                Self::ensure_fields_absent(
                    input,
                    &[
                        "fork_context",
                        "subagent_type",
                        "retry",
                        "auto_retry",
                        "retry_coverage",
                    ],
                    action,
                )?;

                let (model_id, inherit_parent_model) = Self::optional_model_id(input)?;

                Ok(TaskInvocation {
                    action,
                    description,
                    prompt,
                    context_mode: SubagentContextMode::Fresh,
                    target_session_id,
                    subagent_type: None,
                    model_id,
                    inherit_parent_model,
                    timeout_seconds: None,
                    run_in_background,
                    is_retry: false,
                    requested_auto_retry: false,
                })
            }
            TaskAction::Cancel => {
                let target_session_id =
                    Self::required_string_for_action(input, "session_id", action)?;
                Self::ensure_fields_absent(
                    input,
                    &[
                        "prompt",
                        "fork_context",
                        "subagent_type",
                        "model_id",
                        "run_in_background",
                        "retry",
                        "auto_retry",
                        "retry_coverage",
                    ],
                    action,
                )?;

                Ok(TaskInvocation {
                    action,
                    description: None,
                    prompt: None,
                    context_mode: SubagentContextMode::Fresh,
                    target_session_id,
                    subagent_type: None,
                    model_id: None,
                    inherit_parent_model: false,
                    timeout_seconds: None,
                    run_in_background: false,
                    is_retry: false,
                    requested_auto_retry: false,
                })
            }
        }
    }

    pub(super) async fn validate_invocation_input(
        input: &Value,
        is_deep_review_parent: bool,
        workspace_root: Option<&std::path::Path>,
    ) -> ValidationResult {
        let invocation = match Self::parse_invocation(input, is_deep_review_parent) {
            Ok(invocation) => invocation,
            Err(error) => return Self::invalid_input(error.to_string()),
        };
        let _ = workspace_root;
        if invocation.action != TaskAction::Cancel {
            if let Some(result) = Self::validate_prompt_size(input) {
                return result;
            }
        }

        ValidationResult {
            result: true,
            message: None,
            error_code: None,
            meta: None,
        }
    }

    fn required_string_for_action(
        input: &Value,
        field: &str,
        action: TaskAction,
    ) -> BitFunResult<Option<String>> {
        let value = Self::string_field(
            input,
            field,
            format!("action is {}", action.as_str()).as_str(),
        )?;
        if value.is_none() {
            return Err(BitFunError::tool(format!(
                "{field} is required when action is {}",
                action.as_str()
            )));
        }
        Ok(value)
    }

    fn string_field(input: &Value, field: &str, context: &str) -> BitFunResult<Option<String>> {
        match input.get(field) {
            None => Ok(None),
            Some(value) => {
                let value = value
                    .as_str()
                    .ok_or_else(|| BitFunError::tool(format!("{field} must be a string")))?;
                let value = value.trim();
                if value.is_empty() {
                    return Err(BitFunError::tool(format!(
                        "{field} is required for {context}"
                    )));
                }
                Ok(Some(value.to_string()))
            }
        }
    }

    fn optional_trimmed_string(input: &Value, field: &str) -> BitFunResult<Option<String>> {
        match input.get(field) {
            None => Ok(None),
            Some(value) => {
                let value = value
                    .as_str()
                    .ok_or_else(|| BitFunError::tool(format!("{field} must be a string")))?;
                let value = value.trim();
                Ok((!value.is_empty()).then(|| value.to_string()))
            }
        }
    }

    fn optional_model_id(input: &Value) -> BitFunResult<(Option<String>, bool)> {
        match Self::optional_trimmed_string(input, "model_id")? {
            Some(model_id) if model_id == "inherit" => Ok((None, true)),
            model_id => Ok((model_id, false)),
        }
    }

    fn optional_bool(input: &Value, field: &str) -> BitFunResult<Option<bool>> {
        match input.get(field) {
            None => Ok(None),
            Some(value) => value
                .as_bool()
                .map(Some)
                .ok_or_else(|| BitFunError::tool(format!("{field} must be a boolean"))),
        }
    }

    fn optional_timeout_seconds(input: &Value) -> BitFunResult<Option<u64>> {
        match input.get("timeout_seconds") {
            None => Ok(None),
            Some(value) => {
                let parsed = value.as_u64().ok_or_else(|| {
                    BitFunError::tool("timeout_seconds must be a non-negative integer".to_string())
                })?;
                Ok((parsed > 0).then_some(parsed))
            }
        }
    }

    fn ensure_fields_absent(
        input: &Value,
        fields: &[&str],
        action: TaskAction,
    ) -> BitFunResult<()> {
        for field in fields {
            if input.get(field).is_some() {
                return Err(BitFunError::tool(format!(
                    "{field} is not allowed when action is {}",
                    action.as_str()
                )));
            }
        }
        Ok(())
    }
}
