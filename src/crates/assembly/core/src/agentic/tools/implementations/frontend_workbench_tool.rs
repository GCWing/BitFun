//! Creative-mode tool for safely editing the running packaged frontend.

use crate::agentic::tools::framework::{PermissionIntent, Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::frontend_workbench_host::{
    frontend_workbench_host_available, invoke_frontend_workbench, FrontendWorkbenchHostRequest,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_agent_runtime::remote_file_delivery::TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY;
use serde_json::{json, Value};

pub struct FrontendWorkbenchTool;

impl FrontendWorkbenchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FrontendWorkbenchTool {
    fn default() -> Self {
        Self::new()
    }
}

fn creative_local_context(context: Option<&ToolUseContext>) -> bool {
    context.is_some_and(|context| {
        context.agent_type.as_deref() == Some("Creative")
            && !context.is_remote()
            && !is_remote_control_context(context)
    })
}

fn is_remote_control_context(context: &ToolUseContext) -> bool {
    context
        .custom_data
        .get(TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[async_trait]
impl Tool for FrontendWorkbenchTool {
    fn name(&self) -> &str {
        "FrontendWorkbench"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Safely customize the frontend of the running packaged BitFun desktop client. Creative mode only.

Workflow: call prepare, edit only the returned draftPath using file tools, then call apply with draft_id set to the returned draftId. Apply opens immutable host recovery controls, hot-loads the candidate, waits for the real app shell to report readiness, and only then starts the authoritative 15-second confirmation countdown. The apply call resolves with the final confirmed or rolled-back outcome; it does not report success merely because navigation was requested. status exposes the confirmed active revision and any provisional preview separately. rollback explicitly restores the previous confirmed revision.

Actions:
- prepare: create an editable draft from the current active frontend.
- status: inspect active/pending revision state.
- apply: validate and provisionally activate a prepared draft; requires fresh permission.
- rollback: restore the previous confirmed revision; requires fresh permission.

This tool is unavailable for remote workspaces, remote-control turns, and non-desktop surfaces because the user must be able to inspect the visible local BitFun window."#.to_string())
    }

    fn short_description(&self) -> String {
        "Draft and hot-apply the packaged BitFun frontend with 15-second rollback protection."
            .to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["prepare", "status", "apply", "rollback"]
                },
                "draft_id": {
                    "type": "string",
                    "description": "Exact draft id returned by prepare. Required for apply."
                }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    async fn is_available_in_context(&self, context: Option<&ToolUseContext>) -> bool {
        frontend_workbench_host_available() && creative_local_context(context)
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        let action = input.get("action").and_then(Value::as_str).unwrap_or("");
        if !matches!(action, "apply" | "rollback") {
            return Ok(Vec::new());
        }

        let resource = if action == "apply" {
            input
                .get("draft_id")
                .and_then(Value::as_str)
                .map(|id| format!("bitfun-frontend:draft:{id}"))
                .unwrap_or_else(|| "bitfun-frontend:draft:<missing>".to_string())
        } else {
            "bitfun-frontend:previous-confirmed-revision".to_string()
        };
        let mut intent = PermissionIntent::new("frontend_workbench", vec![resource]);
        intent.save_resources.clear();
        intent
            .display_metadata
            .insert("requiresFreshApproval".to_string(), Value::Bool(true));
        Ok(vec![intent])
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if context.agent_type.as_deref() != Some("Creative") {
            return Err(BitFunError::tool(
                "FrontendWorkbench is restricted to Creative mode".to_string(),
            ));
        }
        if context.is_remote() {
            return Err(BitFunError::tool(
                "FrontendWorkbench cannot modify a remote workspace or remote BitFun host"
                    .to_string(),
            ));
        }
        if is_remote_control_context(context) {
            return Err(BitFunError::tool(
                "FrontendWorkbench cannot run from a remote mobile or bot controller because the changed local desktop and its recovery controls are not visible there"
                    .to_string(),
            ));
        }

        let action = input
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| BitFunError::validation("Missing required field: action"))?;
        let draft_id = input
            .get("draft_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if action == "apply" && draft_id.is_none() {
            return Err(BitFunError::validation(
                "draft_id is required when action is apply",
            ));
        }

        let result = invoke_frontend_workbench(FrontendWorkbenchHostRequest {
            action: action.to_string(),
            draft_id,
        })
        .await
        .map_err(BitFunError::tool)?;
        let assistant = match (action, result.get("status").and_then(Value::as_str)) {
            ("apply", Some("confirmed")) => "The candidate rendered successfully and the user kept it; the returned activeRevision is confirmed.",
            ("apply", Some("rolled_back")) => "The candidate was not kept. BitFun restored the previous confirmed frontend; inspect reason for whether this was user choice, readiness failure, or timeout.",
            ("rollback", _) => "Frontend rollback completed.",
            ("prepare", _) => "Frontend draft prepared. Edit only draftPath, then apply the exact draftId.",
            _ => "Frontend workbench status returned.",
        };

        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: Some(assistant.to_string()),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn context(agent_type: &str) -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: Some(agent_type.to_string()),
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: Default::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: Default::default(),
        }
    }

    #[tokio::test]
    async fn execution_rejects_non_creative_agents_even_if_called_directly() {
        let error = FrontendWorkbenchTool::new()
            .call_impl(&json!({"action": "status"}), &context("agentic"))
            .await
            .expect_err("non-Creative call must fail");
        assert!(error.to_string().contains("Creative mode"));
    }

    #[tokio::test]
    async fn execution_rejects_remote_workspaces_even_in_creative_mode() {
        let mut remote = context("Creative");
        remote.workspace = Some(crate::agentic::WorkspaceBinding::new_remote(
            None,
            std::path::PathBuf::from("/srv/project"),
            "connection-1".to_string(),
            "Remote".to_string(),
            crate::service::remote_ssh::workspace_state::WorkspaceSessionIdentity {
                hostname: "remote.example".to_string(),
                logical_workspace_path: "/srv/project".to_string(),
                remote_connection_id: Some("connection-1".to_string()),
            },
        ));

        let error = FrontendWorkbenchTool::new()
            .call_impl(&json!({"action": "status"}), &remote)
            .await
            .expect_err("remote Creative calls must fail");
        assert!(error.to_string().contains("remote workspace"));
    }

    #[tokio::test]
    async fn execution_rejects_remote_control_turns_even_for_a_local_workspace() {
        let mut remote_control = context("Creative");
        remote_control.custom_data.insert(
            TOOL_CONTEXT_REMOTE_FILE_DELIVERY_KEY.to_string(),
            Value::Bool(true),
        );

        let error = FrontendWorkbenchTool::new()
            .call_impl(&json!({"action": "status"}), &remote_control)
            .await
            .expect_err("remote-control Creative calls must fail");
        assert!(error.to_string().contains("remote mobile or bot"));
    }

    #[test]
    fn apply_always_requires_fresh_approval() {
        let intents = FrontendWorkbenchTool::new()
            .permission_intents(
                &json!({"action": "apply", "draft_id": "draft-1"}),
                &context("Creative"),
            )
            .expect("permission intent");
        assert!(intents[0].save_resources.is_empty());
        assert_eq!(
            intents[0].display_metadata.get("requiresFreshApproval"),
            Some(&Value::Bool(true))
        );
    }
}
