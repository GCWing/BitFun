use super::util::normalize_path;
use crate::agentic::persistence::PersistenceManager;
use crate::agentic::tools::framework::{
    Tool, ToolRenderOptions, ToolResult, ToolUseContext, ValidationResult,
};
use crate::infrastructure::PathManager;
use crate::service::session::SessionTranscriptExportOptions;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

/// SessionHistory tool - export a grep-friendly transcript file for a session.
pub struct SessionHistoryTool;

impl SessionHistoryTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_session_id(session_id: &str) -> Result<(), String> {
        if session_id.is_empty() {
            return Err("session_id cannot be empty".to_string());
        }
        if session_id == "." || session_id == ".." {
            return Err("session_id cannot be '.' or '..'".to_string());
        }
        if session_id.contains('/') || session_id.contains('\\') {
            return Err("session_id cannot contain path separators".to_string());
        }
        if !session_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(
                "session_id can only contain ASCII letters, numbers, '-' and '_'".to_string(),
            );
        }
        Ok(())
    }

    fn resolve_workspace(&self, workspace: &str) -> BitFunResult<String> {
        let workspace = workspace.trim();
        if workspace.is_empty() {
            return Err(BitFunError::tool(
                "workspace is required and cannot be empty".to_string(),
            ));
        }

        let path = Path::new(workspace);
        if !path.is_absolute() {
            return Err(BitFunError::tool(
                "workspace must be an absolute path".to_string(),
            ));
        }

        let resolved = normalize_path(workspace);
        let path = Path::new(&resolved);
        if !path.exists() {
            return Err(BitFunError::tool(format!(
                "Workspace does not exist: {}",
                resolved
            )));
        }
        if !path.is_dir() {
            return Err(BitFunError::tool(format!(
                "Workspace is not a directory: {}",
                resolved
            )));
        }

        Ok(resolved)
    }

    fn resolve_session_id(&self, session_id: &str) -> BitFunResult<String> {
        let session_id = session_id.trim().to_string();
        Self::validate_session_id(&session_id).map_err(BitFunError::tool)?;
        Ok(session_id)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SessionHistoryInput {
    workspace: String,
    session_id: String,
    #[serde(default)]
    tools: Option<bool>,
    #[serde(default)]
    tool_inputs: Option<bool>,
    #[serde(default)]
    thinking: Option<bool>,
    #[serde(default)]
    turns: Option<Vec<String>>,
}

#[async_trait]
impl Tool for SessionHistoryTool {
    fn name(&self) -> &str {
        "SessionHistory"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(
            r#"Use this tool when you need the history of an agent session.

This tool does not return full details directly. Instead, it exports a transcript file. The result includes the transcript file path together with index location hints.

The transcript file starts with a compact index. Each index entry includes the turn number, a short preview, and line ranges you can use for targeted reads.

Recommended workflow:
1. Call this tool.
2. Read only the index line range from the returned transcript path first.
3. Inspect the on-file index header to find the turn you want.
4. Read only the matching `range`.

Typical usage:
- To review session history across a workspace, first use `SessionControl` to list the sessions in that workspace, then call this tool for the sessions you want to inspect.
- To inspect the latest state of a specific session, call this tool with `turns=["-1:"]` to export only the last turn.

Minimal transcript example:
<example>
## Index
- turn=0 range=4-7 preview="Fix failing login test"

## Turn 0
[user]
Fix failing login test
[/user]
</example>
In the example above, read lines `1-2` first, then jump directly to `range=4-7`.

`turns` parameter:
- Optional list of turn selectors.
- Supports selectors such as `":1"`, `"-20:"`, `":1" + "-5:"`, and `"10:30"`.
- When omitted, exports all turns.

Examples:
1. Export the full transcript: leave `turns` empty
2. Export the first turn only: `turns=[":1"]`
3. Export the last 3 turns: `turns=["-3:"]`
4. Export the first turn and the last 3 turns: `turns=[":1", "-3:"]`
5. Export a middle range: `turns=["2:5"]`"#
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "workspace": {
                    "type": "string",
                    "description": "Required absolute workspace path."
                },
                "session_id": {
                    "type": "string",
                    "description": "Required session ID to export."
                },
                "tools": {
                    "type": "boolean",
                    "description": "Whether to include tool sections. Defaults to false."
                },
                "tool_inputs": {
                    "type": "boolean",
                    "description": "Whether to include tool input parameters in tool sections. Defaults to false. Only applies when tools is true."
                },
                "thinking": {
                    "type": "boolean",
                    "description": "Whether to include thinking sections. Defaults to false."
                },
                "turns": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional list of turn selectors. Supports index and start:end forms such as \":1\", \"-20:\", \"10:30\", or \"15\"."
                }
            },
            "required": ["workspace", "session_id"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        true
    }

    fn needs_permissions(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let parsed: SessionHistoryInput = match serde_json::from_value(input.clone()) {
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

        if parsed.workspace.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("workspace is required and cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if !Path::new(parsed.workspace.trim()).is_absolute() {
            return ValidationResult {
                result: false,
                message: Some("workspace must be an absolute path".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if parsed.session_id.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some("session_id cannot be empty".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if let Err(message) = Self::validate_session_id(parsed.session_id.trim()) {
            return ValidationResult {
                result: false,
                message: Some(message),
                error_code: Some(400),
                meta: None,
            };
        }

        if parsed.turns.as_ref().is_some_and(|selectors| selectors.is_empty()) {
            return ValidationResult {
                result: false,
                message: Some("turns cannot be an empty array".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        if parsed
            .turns
            .as_ref()
            .is_some_and(|selectors| selectors.iter().any(|selector| selector.trim().is_empty()))
        {
            return ValidationResult {
                result: false,
                message: Some("turns cannot contain empty selectors".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }

        ValidationResult::default()
    }

    fn render_tool_use_message(&self, input: &Value, _options: &ToolRenderOptions) -> String {
        let session_id = input
            .get("session_id")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown session");
        format!("Export transcript for {}", session_id)
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let params: SessionHistoryInput = serde_json::from_value(input.clone())
            .map_err(|e| BitFunError::tool(format!("Invalid input: {}", e)))?;

        let workspace = self.resolve_workspace(&params.workspace)?;
        let session_id = self.resolve_session_id(&params.session_id)?;
        let manager = PersistenceManager::new(Arc::new(PathManager::new()?))?;
        let transcript = manager
            .export_session_transcript(
                Path::new(&workspace),
                &session_id,
                &SessionTranscriptExportOptions {
                    tools: params.tools.unwrap_or(false),
                    tool_inputs: params.tool_inputs.unwrap_or(false),
                    thinking: params.thinking.unwrap_or(false),
                    turns: params.turns,
                },
            )
            .await?;

        Ok(vec![ToolResult::Result {
            data: json!({
                "success": true,
                "workspace": workspace,
                "transcript": transcript,
            }),
            result_for_assistant: Some(format!(
                "Transcript exported to '{}'. The index is on lines {}-{}. Read that range first, then use Grep or Read on that path for targeted navigation.",
                transcript.transcript_path,
                transcript.index_range.start_line,
                transcript.index_range.end_line
            )),
        }])
    }
}
