//! PageDeploy tool — deploy a saved BitFun Page version to production.

use crate::agentic::tools::account_login_capability::account_login_available;
use crate::agentic::tools::framework::{PermissionIntent, Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::page_deploy_host::invoke_page_deploy;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct PageDeployTool;

impl PageDeployTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PageDeployTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PageDeployTool {
    fn name(&self) -> &str {
        "PageDeploy"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Switch the production pointer of an existing BitFun Page to a previously saved version_id (rollback or promote a prior version).

Requires a logged-in BitFun account. This tool is only available after account login. To create or update page content and publish, use PagePublish instead — do not ask the user for a version_id they do not have, and do not mention a Page management scene.

Input: slug (page path id), version_id (immutable saved version from a prior PagePublish). Returns absolute `url` plus url_path / deployed_version_id.

When telling the user the link: paste the full absolute URL and put a trailing space after it (before any punctuation or newline).

Preview a version at /p/{username}/{slug}/@v/{version_id}."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Deploy a saved BitFun Page version to production.".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["slug", "version_id"],
            "properties": {
                "slug": {
                    "type": "string",
                    "description": "Page slug (lowercase letters, digits, hyphens)."
                },
                "version_id": {
                    "type": "string",
                    "description": "Immutable version id to deploy (from a prior Save version)."
                }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<PermissionIntent>> {
        Ok(Vec::new())
    }

    async fn is_available_in_context(&self, _context: Option<&ToolUseContext>) -> bool {
        account_login_available()
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        if !account_login_available() {
            return Err(BitFunError::tool(
                "PageDeploy requires a logged-in BitFun account".to_string(),
            ));
        }

        let slug = input
            .get("slug")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BitFunError::tool("slug is required".to_string()))?
            .to_string();
        let version_id = input
            .get("version_id")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BitFunError::tool("version_id is required".to_string()))?
            .to_string();

        let result = invoke_page_deploy(slug.clone(), version_id.clone())
            .await
            .map_err(BitFunError::tool)?;

        let url = result
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| result.get("url_path").and_then(|v| v.as_str()))
            .unwrap_or("");
        // Trailing space after URL keeps chat linkifiers from eating the next char.
        let assistant = if url.is_empty() {
            format!("Deployed BitFun Page '{slug}' version '{version_id}' to production.")
        } else {
            format!(
                "Deployed BitFun Page '{slug}' version '{version_id}'. Production URL: {url} \n\
                 Share this full absolute URL with the user, and keep a trailing space after the URL."
            )
        };

        Ok(vec![ToolResult::Result {
            data: result,
            result_for_assistant: Some(assistant),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::account_login_capability::set_account_login_available;
    use std::collections::HashMap;
    use std::sync::Mutex;

    static LOGIN_GATE: Mutex<()> = Mutex::new(());

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            loaded_deferred_tool_specs: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn login_gate_controls_availability_and_execution() {
        let _guard = LOGIN_GATE.lock().unwrap();
        let tool = PageDeployTool::new();

        set_account_login_available(false);
        assert!(!tool.is_available_in_context(None).await);

        set_account_login_available(true);
        assert!(tool.is_available_in_context(None).await);

        set_account_login_available(false);
        let err = tool
            .call_impl(
                &json!({ "slug": "demo", "version_id": "v1" }),
                &empty_context(),
            )
            .await
            .expect_err("should reject without login");
        assert!(err.to_string().contains("logged-in"));
    }
}
