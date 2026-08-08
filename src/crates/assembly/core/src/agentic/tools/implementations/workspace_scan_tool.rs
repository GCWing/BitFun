use crate::agentic::tools::framework::{
    Tool, ToolExposure, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::service::workspace::{
    get_global_workspace_service, WorkspaceInfo, WorkspaceStatus, WorkspaceSummary,
};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

/// WorkspaceScan tool - scan existing workspaces by scope without modifying them.
pub struct WorkspaceScanTool;

impl Default for WorkspaceScanTool {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceScanTool {
    pub fn new() -> Self {
        Self
    }
}

/// Resolved scan scope.
#[derive(Debug, Clone, PartialEq)]
enum WorkspaceScanScope {
    Opened,
    Recent,
    All,
    ByStatus(WorkspaceStatus),
}

/// Parses the user-facing `scope` string into a concrete scan scope.
///
/// Scope matching is case-insensitive (LEGION-13): "OPENED", "Recent", and
/// "BY_STATUS:ARCHIVED" all resolve like their lowercase forms.
fn parse_scope(scope: &str) -> Result<WorkspaceScanScope, String> {
    let trimmed = scope.trim();
    let lowered = trimmed.to_ascii_lowercase();
    match lowered.as_str() {
        "" | "opened" => Ok(WorkspaceScanScope::Opened),
        "recent" => Ok(WorkspaceScanScope::Recent),
        "all" => Ok(WorkspaceScanScope::All),
        _ => match lowered.strip_prefix("by_status:") {
            Some(status) => parse_status(status).map(WorkspaceScanScope::ByStatus),
            None => Err(format!(
                "Unsupported scope '{}'. Expected one of: opened, recent, all, by_status:<status>",
                trimmed
            )),
        },
    }
}

/// Parses a workspace status string (case-insensitive).
fn parse_status(status: &str) -> Result<WorkspaceStatus, String> {
    match status.trim().to_ascii_lowercase().as_str() {
        "active" => Ok(WorkspaceStatus::Active),
        "inactive" => Ok(WorkspaceStatus::Inactive),
        "loading" => Ok(WorkspaceStatus::Loading),
        "error" => Ok(WorkspaceStatus::Error),
        "archived" => Ok(WorkspaceStatus::Archived),
        other => Err(format!(
            "Unsupported workspace status '{}'. Expected one of: active, inactive, loading, error, archived",
            other
        )),
    }
}

/// Compact entry shape shared by every scope.
fn workspace_info_to_entry(info: &WorkspaceInfo) -> Value {
    json!({
        "id": info.id,
        "name": info.name,
        "rootPath": info.root_path.to_string_lossy(),
        "status": format!("{:?}", info.status),
        "openedAt": info.opened_at.to_rfc3339(),
        "lastAccessed": info.last_accessed.to_rfc3339(),
        "workspaceType": format!("{:?}", info.workspace_type),
    })
}

/// Compact entry shape for summaries (the summary type has no `openedAt` field).
fn workspace_summary_to_entry(summary: &WorkspaceSummary) -> Value {
    json!({
        "id": summary.id,
        "name": summary.name,
        "rootPath": summary.root_path.to_string_lossy(),
        "status": format!("{:?}", summary.status),
        "openedAt": Value::Null,
        "lastAccessed": summary.last_accessed.to_rfc3339(),
        "workspaceType": format!("{:?}", summary.workspace_type),
    })
}

#[derive(Debug, Clone, Deserialize)]
struct WorkspaceScanInput {
    #[serde(default)]
    scope: Option<String>,
}

#[async_trait]
impl Tool for WorkspaceScanTool {
    fn name(&self) -> &str {
        "WorkspaceScan"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Use this tool when you need to scan and query existing workspaces in the current environment.

This tool is read-only and never modifies workspace state. It lists workspaces known to the workspace service, which is the prerequisite for cross-workspace orchestration: inspect what is opened, recently accessed, or tracked, then direct follow-up work at the right workspace.

`scope` parameter (defaults to "opened"):
- "opened": currently opened workspaces
- "recent": recently accessed workspaces
- "all": every tracked workspace (including inactive ones)
- "by_status:<status>": every tracked workspace filtered by status; status is one of active, inactive, loading, error, archived

Each returned entry has the shape {id, name, rootPath, status, openedAt, lastAccessed, workspaceType}. For scopes backed by workspace summaries ("all", "by_status:<status>") `openedAt` is null because the summary record does not carry it.

Examples:
1. List currently opened workspaces: leave `scope` empty
2. List recently accessed workspaces: scope="recent"
3. List every tracked workspace: scope="all"
4. List archived workspaces: scope="by_status:archived""#
                .to_string(),
        )
    }

    fn short_description(&self) -> String {
        "Scan and query existing workspaces (opened, recent, all, or by status). Read-only."
            .to_string()
    }

    fn default_exposure(&self) -> ToolExposure {
        // Mirrors the plan tool family calibration: commander/Claw staples
        // stay Direct so no GetToolSpec unlock round-trip is needed.
        ToolExposure::Direct
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "description": "Scan scope. One of: opened, recent, all, by_status:<status>. Defaults to opened."
                }
            },
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: WorkspaceScanInput = match serde_json::from_value(input.clone()) {
            Ok(value) => value,
            Err(err) => {
                return ValidationResult {
                    result: false,
                    message: Some(format!("Invalid input: {}", err)),
                    error_code: Some(400),
                    meta: None,
                };
            }
        };

        if let Some(scope) = parsed.scope.as_deref() {
            if let Err(message) = parse_scope(scope) {
                return ValidationResult {
                    result: false,
                    message: Some(message),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let scope = input
            .get("scope")
            .and_then(|value| value.as_str())
            .unwrap_or("opened");
        format!("Scan workspaces with scope '{}'", scope)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: WorkspaceScanInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;

        let scope = params.scope.as_deref().unwrap_or("opened");
        let resolved = parse_scope(scope)
            .map_err(|message| BitFunError::tool(format!("Invalid scope: {}", message)))?;

        let service = get_global_workspace_service().ok_or_else(|| {
            BitFunError::service("Global workspace service is unavailable for WorkspaceScan")
        })?;

        let entries = match resolved {
            WorkspaceScanScope::Opened => {
                let workspaces = service.get_opened_workspaces().await;
                workspaces
                    .iter()
                    .map(workspace_info_to_entry)
                    .collect::<Vec<_>>()
            }
            WorkspaceScanScope::Recent => {
                let workspaces = service.get_recent_workspaces().await;
                workspaces
                    .iter()
                    .map(workspace_info_to_entry)
                    .collect::<Vec<_>>()
            }
            WorkspaceScanScope::All => {
                let workspaces = service.list_workspaces().await;
                workspaces
                    .iter()
                    .map(workspace_summary_to_entry)
                    .collect::<Vec<_>>()
            }
            WorkspaceScanScope::ByStatus(status) => {
                let workspaces = service.list_workspaces_by_status(status).await;
                workspaces
                    .iter()
                    .map(workspace_summary_to_entry)
                    .collect::<Vec<_>>()
            }
        };

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "scope": scope,
                "count": entries.len(),
                "workspaces": entries,
            }),
            result_for_assistant: Some(format!(
                "Scanned {} workspace(s) with scope '{}'. Use the returned entries to direct follow-up work.",
                entries.len(),
                scope
            )),
            image_attachments: None,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scope_accepts_default_and_known_scopes() {
        assert_eq!(parse_scope(""), Ok(WorkspaceScanScope::Opened));
        assert_eq!(parse_scope("opened"), Ok(WorkspaceScanScope::Opened));
        assert_eq!(parse_scope("recent"), Ok(WorkspaceScanScope::Recent));
        assert_eq!(parse_scope("all"), Ok(WorkspaceScanScope::All));
        assert_eq!(
            parse_scope("by_status:active"),
            Ok(WorkspaceScanScope::ByStatus(WorkspaceStatus::Active))
        );
        assert_eq!(
            parse_scope("by_status:Archived"),
            Ok(WorkspaceScanScope::ByStatus(WorkspaceStatus::Archived))
        );
    }

    #[test]
    fn parse_scope_is_case_insensitive() {
        // LEGION-13: scope keywords and the by_status prefix match
        // case-insensitively, like parse_status already did.
        assert_eq!(parse_scope("OPENED"), Ok(WorkspaceScanScope::Opened));
        assert_eq!(parse_scope("Recent"), Ok(WorkspaceScanScope::Recent));
        assert_eq!(parse_scope("ALL"), Ok(WorkspaceScanScope::All));
        assert_eq!(
            parse_scope("BY_STATUS:Active"),
            Ok(WorkspaceScanScope::ByStatus(WorkspaceStatus::Active))
        );
        assert_eq!(
            parse_scope("By_Status:error"),
            Ok(WorkspaceScanScope::ByStatus(WorkspaceStatus::Error))
        );
    }

    #[test]
    fn parse_scope_rejects_unknown_scopes() {
        assert!(parse_scope("unknown").is_err());
        assert!(parse_scope("by_status:").is_err());
        assert!(parse_scope("by_status:unknown_status").is_err());
    }

    #[tokio::test]
    async fn validate_accepts_omitted_scope() {
        let tool = WorkspaceScanTool::new();

        let validation = tool.validate_input(&json!({}), None).await;

        assert!(validation.result, "{:?}", validation.message);
    }

    #[tokio::test]
    async fn validate_rejects_unknown_scope() {
        let tool = WorkspaceScanTool::new();

        let validation = tool
            .validate_input(&json!({ "scope": "unknown" }), None)
            .await;

        assert!(!validation.result);
        assert_eq!(validation.error_code, Some(400));
    }
}
