//! PagePublish tool — create/update a BitFun Page from inline files or a directory,
//! then optionally deploy to production (default: deploy).

use std::collections::HashMap;

use crate::agentic::tools::account_login_capability::account_login_available;
use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::agentic::tools::page_publish_host::{invoke_page_publish, PagePublishHostRequest};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct PagePublishTool;

impl PagePublishTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PagePublishTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PagePublishTool {
    fn name(&self) -> &str {
        "PagePublish"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Publish a BitFun Page to the account relay: upload content, freeze an immutable version, and deploy to production by default.

Requires a logged-in BitFun account. This tool is only available after account login. There is no separate Page management scene — create and ship pages from the conversation with this tool.

When you produce self-contained publishable web content (landing page, docs site, or a Page with server/worker.js) and the user is logged in, proactively ask whether they want it published to BitFun Page (suggest a slug and visibility). If they already said publish/deploy/上线, proceed with permission confirmation.

IMPORTANT — content source:
- Prefer `files` (inline path→UTF-8 content). For agent-authored pages, pass HTML/JS directly in `files` and call PagePublish. Do NOT Write/Edit page files into the user workspace just to publish, and do NOT create folders like bitfun-page/ unless the user explicitly asked to keep a local copy.
- Use `directory` only when the user already has (or explicitly wants) page sources on disk in the workspace.

Input:
- slug (required): page path id (lowercase letters, digits, hyphens)
- visibility: private | relay | public (default public)
- title?, note?
- deploy: boolean (default true). false = save version only; still returns absolute preview_url
- Exactly one of:
  - files: object map of relative path → UTF-8 file content (default/preferred). Must include index.html and/or server/worker.js
  - directory: existing local workspace path (only when user wants on-disk sources)

Returns version_id, absolute `url` / `preview_url` (plus relative paths), deployed_version_id when deployed.

When telling the user the link: paste the full absolute URL and put a trailing space after it (before any punctuation or newline), so chat linkifiers do not swallow the next character.

Use PageDeploy only to switch an already-saved version_id (rollback / promote a prior version)."#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Publish a BitFun Page (upload, save version, deploy).".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["slug"],
            "properties": {
                "slug": {
                    "type": "string",
                    "description": "Page slug (lowercase letters, digits, hyphens)."
                },
                "visibility": {
                    "type": "string",
                    "enum": ["private", "relay", "public"],
                    "description": "Page visibility. Defaults to public."
                },
                "title": {
                    "type": "string",
                    "description": "Optional page title (defaults to slug)."
                },
                "note": {
                    "type": "string",
                    "description": "Optional version note."
                },
                "deploy": {
                    "type": "boolean",
                    "description": "Deploy to production after saving. Defaults to true."
                },
                "files": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Preferred. Inline path→UTF-8 content map (e.g. index.html). Do not write these into the workspace first. Must include index.html and/or server/worker.js. Mutually exclusive with directory."
                },
                "directory": {
                    "type": "string",
                    "description": "Only when page sources already exist (or user asked to keep them) on disk. Mutually exclusive with files."
                }
            }
        })
    }

    fn is_readonly(&self) -> bool {
        false
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
                "PagePublish requires a logged-in BitFun account".to_string(),
            ));
        }

        let slug = input
            .get("slug")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| BitFunError::tool("slug is required".to_string()))?
            .to_string();

        let visibility = input
            .get("visibility")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("public")
            .to_string();

        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let note = input
            .get("note")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let deploy = input
            .get("deploy")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let directory = input
            .get("directory")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let files = parse_files_map(input.get("files"))?;

        match (&directory, &files) {
            (Some(_), Some(_)) => {
                return Err(BitFunError::tool(
                    "provide either directory or files, not both".to_string(),
                ));
            }
            (None, None) => {
                return Err(BitFunError::tool(
                    "either directory or files is required".to_string(),
                ));
            }
            _ => {}
        }

        let result = invoke_page_publish(PagePublishHostRequest {
            slug: slug.clone(),
            visibility,
            title,
            note,
            deploy,
            directory,
            files,
        })
        .await
        .map_err(BitFunError::tool)?;

        let version_id = result
            .get("version_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let url = result
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| result.get("url_path").and_then(|v| v.as_str()))
            .unwrap_or("");
        let preview = result
            .get("preview_url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| result.get("preview_url_path").and_then(|v| v.as_str()))
            .unwrap_or("");
        let deployed = result
            .get("deployed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Trailing space after URL keeps chat linkifiers from eating the next char.
        let assistant = if deployed {
            if url.is_empty() {
                format!("Published BitFun Page '{slug}' version '{version_id}' to production.")
            } else {
                format!(
                    "Published BitFun Page '{slug}' version '{version_id}'. Production URL: {url} \n\
                     Share this full absolute URL with the user, and keep a trailing space after the URL."
                )
            }
        } else if preview.is_empty() {
            format!("Saved BitFun Page '{slug}' version '{version_id}' (not deployed).")
        } else {
            format!(
                "Saved BitFun Page '{slug}' version '{version_id}' (not deployed). Preview URL: {preview} \n\
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

fn parse_files_map(value: Option<&Value>) -> BitFunResult<Option<HashMap<String, String>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let obj = value.as_object().ok_or_else(|| {
        BitFunError::tool("files must be an object of path→content strings".to_string())
    })?;
    if obj.is_empty() {
        return Err(BitFunError::tool("files must not be empty".to_string()));
    }
    let mut map = HashMap::with_capacity(obj.len());
    for (path, content) in obj {
        let Some(text) = content.as_str() else {
            return Err(BitFunError::tool(format!(
                "files['{path}'] must be a UTF-8 string"
            )));
        };
        map.insert(path.clone(), text.to_string());
    }
    Ok(Some(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agentic::tools::account_login_capability::set_account_login_available;
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
        let tool = PagePublishTool::new();

        set_account_login_available(false);
        assert!(!tool.is_available_in_context(None).await);

        set_account_login_available(true);
        assert!(tool.is_available_in_context(None).await);

        set_account_login_available(false);
        let err = tool
            .call_impl(
                &json!({
                    "slug": "demo",
                    "files": { "index.html": "<html></html>" }
                }),
                &empty_context(),
            )
            .await
            .expect_err("should reject without login");
        assert!(err.to_string().contains("logged-in"));
    }

    #[tokio::test]
    async fn rejects_missing_source_when_logged_in() {
        let _guard = LOGIN_GATE.lock().unwrap();
        let tool = PagePublishTool::new();
        set_account_login_available(true);
        let err = tool
            .call_impl(&json!({ "slug": "demo" }), &empty_context())
            .await
            .expect_err("should require files or directory");
        assert!(err.to_string().contains("directory or files"));
        set_account_login_available(false);
    }
}
