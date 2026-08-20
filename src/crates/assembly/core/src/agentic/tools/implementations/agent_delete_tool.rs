use crate::agentic::agents::is_swarm_planner_agent_type;
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;

pub struct AgentDeleteTool;

impl Default for AgentDeleteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentDeleteTool {
    pub fn new() -> Self {
        Self
    }

    fn parse_agent_ids(input: &Value) -> BitFunResult<Vec<String>> {
        let object = input
            .as_object()
            .ok_or_else(|| BitFunError::tool("AgentDelete input must be an object".to_string()))?;
        if let Some(field) = object.keys().find(|field| field.as_str() != "agent_ids") {
            return Err(BitFunError::tool(format!(
                "AgentDelete does not accept field '{field}'"
            )));
        }
        let values = match object.get("agent_ids") {
            Some(value @ Value::String(_)) => vec![value],
            Some(Value::Array(values)) => values.iter().collect(),
            Some(_) => {
                return Err(BitFunError::tool(
                    "agent_ids must be a string or an array of strings".to_string(),
                ));
            }
            None => {
                return Err(BitFunError::tool(
                    "agent_ids is required for AgentDelete".to_string(),
                ));
            }
        };

        let mut seen = HashSet::new();
        let mut agent_ids = Vec::new();
        for value in values {
            let agent_id = value
                .as_str()
                .map(str::trim)
                .filter(|agent_id| !agent_id.is_empty())
                .ok_or_else(|| {
                    BitFunError::tool("agent_ids must contain only non-empty strings".to_string())
                })?;
            if seen.insert(agent_id.to_string()) {
                agent_ids.push(agent_id.to_string());
            }
        }
        if agent_ids.is_empty() {
            return Err(BitFunError::tool(
                "agent_ids must contain at least one agent ID".to_string(),
            ));
        }
        Ok(agent_ids)
    }

    fn ensure_context_allowed(context: &ToolUseContext) -> BitFunResult<()> {
        let agent_type = context
            .agent_type
            .as_deref()
            .ok_or_else(|| BitFunError::tool("agent_type is required in context".to_string()))?;
        if !is_swarm_planner_agent_type(agent_type) {
            return Err(BitFunError::tool(
                "AgentDelete is available only to Ultra and SwarmPlanner".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl Tool for AgentDeleteTool {
    fn name(&self) -> &str {
        "AgentDelete"
    }

    fn manages_own_execution_timeout(&self) -> bool {
        true
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok("Permanently delete one or more direct child agents and their entire descendant subtrees. Active work is cancelled before the sessions and pending results are removed.".to_string())
    }

    fn short_description(&self) -> String {
        "Permanently delete direct child agent subtrees.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agent_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Direct child agent IDs to delete."
                }
            },
            "required": ["agent_ids"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let resources = Self::parse_agent_ids(input)?
            .into_iter()
            .map(|agent_id| format!("delete:{agent_id}"))
            .collect();
        Ok(vec![PermissionIntent::new("task", resources)])
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        Self::parse_agent_ids(input)
            .map(|agent_ids| format!("Deleting agent subtrees: {}", agent_ids.join(", ")))
            .unwrap_or_else(|_| "Deleting agent subtrees".to_string())
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let result = Self::parse_agent_ids(input).and_then(|_| {
            if let Some(context) = context {
                Self::ensure_context_allowed(context)?;
            }
            Ok(())
        });
        match result {
            Ok(()) => ValidationResult::default(),
            Err(error) => ValidationResult {
                result: false,
                message: Some(error.to_string()),
                error_code: None,
                meta: None,
            },
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        Self::ensure_context_allowed(context)?;
        let agent_ids = Self::parse_agent_ids(input)?;
        let session_id = context
            .session_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("session_id is required in context".to_string()))?;
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let deleted_agents = coordinator
            .delete_direct_child_agents(session_id, &agent_ids)
            .await?;
        Ok(vec![ToolResult::Result {
            data: json!({
                "agent_ids": agent_ids,
                "status": "deleted",
                "deleted_agents": deleted_agents,
            }),
            result_for_assistant: Some(format!(
                "Permanently deleted the selected agent subtrees ({}) containing {deleted_agents} agent session(s).",
                agent_ids.join(", ")
            )),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::AgentDeleteTool;
    use crate::agentic::tools::framework::{Tool, ToolUseContext};
    use crate::agentic::tools::ToolRuntimeRestrictions;
    use std::collections::HashMap;

    fn context(agent_type: &str) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: Some("tool-call".to_string()),
            agent_type: Some(agent_type.to_string()),
            session_id: Some("session".to_string()),
            dialog_turn_id: Some("turn".to_string()),
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[test]
    fn permission_is_scoped_to_all_deleted_agents() {
        let intents = AgentDeleteTool::new()
            .permission_intents(
                &serde_json::json!({ "agent_ids": ["a2", "a3"] }),
                &context("Ultra"),
            )
            .expect("permission intent");
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].action, "task");
        assert_eq!(intents[0].resources, ["delete:a2", "delete:a3"]);
    }

    #[test]
    fn parser_tolerates_a_string_and_deduplicates_arrays() {
        assert_eq!(
            AgentDeleteTool::parse_agent_ids(&serde_json::json!({ "agent_ids": " a2 " }))
                .expect("single string"),
            ["a2"]
        );
        assert_eq!(
            AgentDeleteTool::parse_agent_ids(
                &serde_json::json!({ "agent_ids": ["a2", " a2 ", "a3"] }),
            )
            .expect("deduplicated array"),
            ["a2", "a3"]
        );
    }

    #[tokio::test]
    async fn validation_rejects_non_planner_contexts() {
        let validation = AgentDeleteTool::new()
            .validate_input(
                &serde_json::json!({ "agent_ids": ["a1"] }),
                Some(&context("SwarmWorker")),
            )
            .await;
        assert!(!validation.result);
    }
}
