use bitfun_agent_tools::{ToolContextFacts, ToolRuntimeRestrictions, ToolWorkspaceKind};
use bitfun_runtime_ports::DelegationPolicy;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct ToolRuntimeCustomDataInput<'a> {
    pub context_vars: &'a HashMap<String, String>,
    pub delegation_policy: DelegationPolicy,
    pub remote_file_delivery_key: &'a str,
    pub extension_custom_data: Option<&'a HashMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRuntimeContextFactsInput {
    pub tool_call_id: Option<String>,
    pub agent_type: Option<String>,
    pub session_id: Option<String>,
    pub dialog_turn_id: Option<String>,
    pub workspace_kind: Option<ToolWorkspaceKind>,
    pub workspace_root: Option<String>,
    pub runtime_tool_restrictions: ToolRuntimeRestrictions,
}

pub fn build_tool_runtime_custom_data(
    input: ToolRuntimeCustomDataInput<'_>,
) -> HashMap<String, Value> {
    let mut map = HashMap::new();

    map.insert(
        "delegation_allow_subagent_spawn".to_string(),
        serde_json::json!(input.delegation_policy.allow_subagent_spawn),
    );
    map.insert(
        "delegation_nesting_depth".to_string(),
        serde_json::json!(input.delegation_policy.nesting_depth),
    );

    insert_u64_context_var(input.context_vars, &mut map, "turn_index");
    insert_non_empty_string_context_var(input.context_vars, &mut map, "primary_model_provider");
    insert_bool_context_var(
        input.context_vars,
        &mut map,
        "primary_model_supports_image_understanding",
    );
    insert_bool_context_var(input.context_vars, &mut map, "acp_transport");
    insert_bool_context_var(input.context_vars, &mut map, input.remote_file_delivery_key);
    if let Some(extension_custom_data) = input.extension_custom_data {
        for (key, value) in extension_custom_data {
            map.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }

    map
}

pub fn project_tool_context_facts(input: ToolRuntimeContextFactsInput) -> ToolContextFacts {
    ToolContextFacts {
        tool_call_id: input.tool_call_id,
        agent_type: input.agent_type,
        session_id: input.session_id,
        dialog_turn_id: input.dialog_turn_id,
        workspace_kind: input.workspace_kind,
        workspace_root: input.workspace_root,
        runtime_tool_restrictions: input.runtime_tool_restrictions,
    }
}

pub fn delegation_policy_from_custom_data(
    custom_data: &HashMap<String, Value>,
) -> DelegationPolicy {
    let allow_subagent_spawn = custom_data
        .get("delegation_allow_subagent_spawn")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let nesting_depth = custom_data
        .get("delegation_nesting_depth")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(0);

    DelegationPolicy {
        allow_subagent_spawn,
        nesting_depth,
    }
}

/// Whether the session primary model accepts image inputs.
///
/// Defaults to true when unset so API listings without model metadata keep the
/// historical behavior.
pub fn primary_model_supports_image_understanding(custom_data: &HashMap<String, Value>) -> bool {
    custom_data
        .get("primary_model_supports_image_understanding")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn insert_u64_context_var(
    context_vars: &HashMap<String, String>,
    map: &mut HashMap<String, Value>,
    key: &str,
) {
    if let Some(value) = context_vars.get(key) {
        if let Ok(parsed) = value.parse::<u64>() {
            map.insert(key.to_string(), serde_json::json!(parsed));
        }
    }
}

fn insert_bool_context_var(
    context_vars: &HashMap<String, String>,
    map: &mut HashMap<String, Value>,
    key: &str,
) {
    if let Some(value) = context_vars.get(key) {
        if let Ok(parsed) = value.parse::<bool>() {
            map.insert(key.to_string(), serde_json::json!(parsed));
        }
    }
}

fn insert_non_empty_string_context_var(
    context_vars: &HashMap<String, String>,
    map: &mut HashMap<String, Value>,
    key: &str,
) {
    if let Some(value) = context_vars
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
    {
        map.insert(key.to_string(), serde_json::json!(value));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tool_runtime_custom_data, delegation_policy_from_custom_data,
        primary_model_supports_image_understanding, project_tool_context_facts,
        ToolRuntimeContextFactsInput, ToolRuntimeCustomDataInput,
    };
    use bitfun_agent_tools::{ToolRuntimeRestrictions, ToolWorkspaceKind};
    use bitfun_runtime_ports::DelegationPolicy;
    use serde_json::json;
    use std::collections::{BTreeSet, HashMap};

    #[test]
    fn materializes_provider_neutral_tool_custom_data() {
        let mut context_vars = HashMap::new();
        context_vars.insert("turn_index".to_string(), "7".to_string());
        context_vars.insert("primary_model_provider".to_string(), "openai".to_string());
        context_vars.insert(
            "primary_model_supports_image_understanding".to_string(),
            "false".to_string(),
        );
        context_vars.insert("acp_transport".to_string(), "true".to_string());
        context_vars.insert("remote_file_delivery".to_string(), "true".to_string());
        let extension_custom_data = HashMap::from([("extension_key".to_string(), json!("kept"))]);

        let custom_data = build_tool_runtime_custom_data(ToolRuntimeCustomDataInput {
            context_vars: &context_vars,
            delegation_policy: DelegationPolicy::top_level().spawn_child(),
            remote_file_delivery_key: "remote_file_delivery",
            extension_custom_data: Some(&extension_custom_data),
        });

        assert_eq!(custom_data["delegation_allow_subagent_spawn"], json!(false));
        assert_eq!(custom_data["delegation_nesting_depth"], json!(1));
        assert_eq!(custom_data["turn_index"], json!(7));
        assert_eq!(custom_data["primary_model_provider"], json!("openai"));
        assert_eq!(
            custom_data["primary_model_supports_image_understanding"],
            json!(false)
        );
        assert_eq!(custom_data["acp_transport"], json!(true));
        assert_eq!(custom_data["remote_file_delivery"], json!(true));
        assert_eq!(custom_data["extension_key"], json!("kept"));
    }

    #[test]
    fn custom_data_ignores_invalid_or_empty_context_values() {
        let mut context_vars = HashMap::new();
        context_vars.insert("turn_index".to_string(), "not-a-number".to_string());
        context_vars.insert("primary_model_provider".to_string(), "".to_string());
        context_vars.insert(
            "primary_model_supports_image_understanding".to_string(),
            "not-bool".to_string(),
        );
        context_vars.insert("acp_transport".to_string(), "not-bool".to_string());
        context_vars.insert("remote_file_delivery".to_string(), "not-bool".to_string());

        let custom_data = build_tool_runtime_custom_data(ToolRuntimeCustomDataInput {
            context_vars: &context_vars,
            delegation_policy: DelegationPolicy::top_level(),
            remote_file_delivery_key: "remote_file_delivery",
            extension_custom_data: None,
        });

        assert_eq!(custom_data["delegation_allow_subagent_spawn"], json!(true));
        assert_eq!(custom_data["delegation_nesting_depth"], json!(0));
        assert!(!custom_data.contains_key("turn_index"));
        assert!(!custom_data.contains_key("primary_model_provider"));
        assert!(!custom_data.contains_key("primary_model_supports_image_understanding"));
        assert!(!custom_data.contains_key("acp_transport"));
        assert!(!custom_data.contains_key("remote_file_delivery"));
    }

    #[test]
    fn extension_custom_data_cannot_override_runtime_owned_values() {
        let mut context_vars = HashMap::new();
        context_vars.insert("turn_index".to_string(), "7".to_string());
        context_vars.insert(
            "primary_model_supports_image_understanding".to_string(),
            "true".to_string(),
        );
        let extension_custom_data = HashMap::from([
            ("turn_index".to_string(), json!(99)),
            ("delegation_allow_subagent_spawn".to_string(), json!(false)),
            (
                "primary_model_supports_image_understanding".to_string(),
                json!(false),
            ),
            ("extension_key".to_string(), json!("kept")),
        ]);

        let custom_data = build_tool_runtime_custom_data(ToolRuntimeCustomDataInput {
            context_vars: &context_vars,
            delegation_policy: DelegationPolicy::top_level(),
            remote_file_delivery_key: "remote_file_delivery",
            extension_custom_data: Some(&extension_custom_data),
        });

        assert_eq!(custom_data["delegation_allow_subagent_spawn"], json!(true));
        assert_eq!(custom_data["turn_index"], json!(7));
        assert_eq!(
            custom_data["primary_model_supports_image_understanding"],
            json!(true)
        );
        assert_eq!(custom_data["extension_key"], json!("kept"));
    }

    #[test]
    fn derives_runtime_policies_from_custom_data() {
        let mut custom_data = HashMap::new();
        custom_data.insert("delegation_allow_subagent_spawn".to_string(), json!(false));
        custom_data.insert("delegation_nesting_depth".to_string(), json!(3));
        custom_data.insert(
            "primary_model_supports_image_understanding".to_string(),
            json!(false),
        );

        assert_eq!(
            delegation_policy_from_custom_data(&custom_data),
            DelegationPolicy {
                allow_subagent_spawn: false,
                nesting_depth: 3
            }
        );
        assert!(!primary_model_supports_image_understanding(&custom_data));
        assert!(primary_model_supports_image_understanding(&HashMap::new()));
    }

    #[test]
    fn projects_prompt_safe_tool_context_facts_only() {
        let facts = project_tool_context_facts(ToolRuntimeContextFactsInput {
            tool_call_id: Some("tool-1".to_string()),
            agent_type: Some("coding".to_string()),
            session_id: Some("session-1".to_string()),
            dialog_turn_id: Some("turn-1".to_string()),
            workspace_kind: Some(ToolWorkspaceKind::Remote),
            workspace_root: Some("/home/user/project".to_string()),
            runtime_tool_restrictions: ToolRuntimeRestrictions {
                allowed_tool_names: BTreeSet::from(["Read".to_string()]),
                denied_tool_names: BTreeSet::from(["Bash".to_string()]),
                denied_tool_messages: Default::default(),
                path_policy: Default::default(),
            },
        });

        let value = serde_json::to_value(&facts).expect("serialize facts");
        assert_eq!(value["toolCallId"], "tool-1");
        assert_eq!(value["agentType"], "coding");
        assert_eq!(value["sessionId"], "session-1");
        assert_eq!(value["dialogTurnId"], "turn-1");
        assert_eq!(value["workspaceKind"], "remote");
        assert_eq!(value["workspaceRoot"], "/home/user/project");
        assert_eq!(
            value["runtimeToolRestrictions"]["allowed_tool_names"][0],
            "Read"
        );
        assert!(value.get("customData").is_none());
        assert!(value.get("runtimeHandles").is_none());
        assert!(value.get("unlockedCollapsedTools").is_none());
    }
}
