use crate::agentic::agents::is_swarm_planner_agent_type;
use crate::agentic::coordination::get_global_coordinator;
use crate::agentic::tools::framework::{
    PermissionIntent, Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct AgentListTool;

impl Default for AgentListTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentListTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_request(input: &Value, context: Option<&ToolUseContext>) -> BitFunResult<()> {
        let object = input
            .as_object()
            .ok_or_else(|| BitFunError::tool("AgentList input must be an object".to_string()))?;
        if let Some(field) = object.keys().next() {
            return Err(BitFunError::tool(format!(
                "AgentList does not accept field '{field}'"
            )));
        }
        if let Some(context) = context {
            let agent_type = context.agent_type.as_deref().ok_or_else(|| {
                BitFunError::tool("agent_type is required in context".to_string())
            })?;
            if !is_swarm_planner_agent_type(agent_type) {
                return Err(BitFunError::tool(
                    "AgentList is available only to Ultra and SwarmPlanner".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Tool for AgentListTool {
    fn name(&self) -> &str {
        "AgentList"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok("List direct child agents and their latest status.".to_string())
    }

    fn short_description(&self) -> String {
        "List direct child agents and their status.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        Ok(Vec::new())
    }

    fn render_tool_use_message(&self, _input: &Value, _options: &ToolRenderOptions) -> String {
        "Listing child agents".to_string()
    }

    async fn validate_input(
        &self,
        input: &Value,
        context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        match Self::validate_request(input, context) {
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
        Self::validate_request(input, Some(context))?;
        let session_id = context
            .session_id
            .as_deref()
            .ok_or_else(|| BitFunError::tool("session_id is required in context".to_string()))?;
        let coordinator = get_global_coordinator()
            .ok_or_else(|| BitFunError::tool("coordinator not initialized".to_string()))?;
        let agents = coordinator
            .direct_child_agents(session_id)
            .await?
            .into_iter()
            .map(|agent| {
                json!({
                    "agent_id": agent.agent_id,
                    "status": agent.status.as_str(),
                })
            })
            .collect::<Vec<_>>();
        let count = agents.len();
        Ok(vec![ToolResult::Result {
            data: json!({ "agents": agents }),
            result_for_assistant: Some(format!("Found {count} direct child agent(s).")),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::AgentListTool;
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
    fn schema_is_a_strict_empty_object() {
        assert_eq!(
            AgentListTool::new().input_schema(),
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })
        );
    }

    #[tokio::test]
    async fn validation_accepts_only_swarm_planners() {
        assert!(
            AgentListTool::new()
                .validate_input(&serde_json::json!({}), Some(&context("Ultra")))
                .await
                .result
        );
        assert!(
            AgentListTool::new()
                .validate_input(&serde_json::json!({}), Some(&context("SwarmPlanner")),)
                .await
                .result
        );
        assert!(
            !AgentListTool::new()
                .validate_input(&serde_json::json!({}), Some(&context("SwarmWorker")))
                .await
                .result
        );
    }
}
