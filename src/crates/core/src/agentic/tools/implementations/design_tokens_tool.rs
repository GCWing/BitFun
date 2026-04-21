use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext, ValidationResult};
use crate::agentic::tools::user_input_manager::get_user_input_manager;
use crate::infrastructure::get_path_manager_arc;
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use chrono::Utc;
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignTokenProposal {
    pub id: String,
    pub name: String,
    pub mood: String,
    pub colors: Value,
    pub typography: Value,
    pub radius: Value,
    pub shadow: Value,
    pub motion: Value,
    pub component_samples: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<Value>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignTokensDocument {
    pub version: u32,
    pub proposals: Vec<DesignTokenProposal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

pub struct DesignTokensTool;

impl Default for DesignTokensTool {
    fn default() -> Self {
        Self::new()
    }
}

impl DesignTokensTool {
    pub fn new() -> Self {
        Self
    }

    fn workspace_root(context: &ToolUseContext) -> BitFunResult<PathBuf> {
        context
            .workspace_root()
            .map(Path::to_path_buf)
            .ok_or_else(|| BitFunError::tool("DesignTokens requires an active workspace binding"))
    }

    fn root_tokens_file(context: &ToolUseContext) -> BitFunResult<PathBuf> {
        let workspace_root = Self::workspace_root(context)?;
        Ok(get_path_manager_arc().workspace_design_tokens_file(&workspace_root))
    }

    fn artifact_tokens_file(context: &ToolUseContext, artifact_id: &str) -> BitFunResult<PathBuf> {
        let workspace_root = Self::workspace_root(context)?;
        Ok(get_path_manager_arc()
            .workspace_design_artifact_dir(&workspace_root, artifact_id)
            .join("tokens.json"))
    }

    async fn ensure_parent(path: &Path) -> BitFunResult<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    BitFunError::tool(format!(
                        "DesignTokens: failed to create {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
        }
        Ok(())
    }

    async fn load_or_default(path: &Path) -> BitFunResult<DesignTokensDocument> {
        if !path.exists() {
            return Ok(DesignTokensDocument {
                version: 1,
                proposals: Vec::new(),
                committed_id: None,
                committed_at: None,
                scope: None,
            });
        }
        let raw = fs::read_to_string(path).await.map_err(|e| {
            BitFunError::tool(format!(
                "DesignTokens: failed to read {}: {}",
                path.display(),
                e
            ))
        })?;
        serde_json::from_str(&raw).map_err(|e| {
            BitFunError::tool(format!(
                "DesignTokens: invalid json at {}: {}",
                path.display(),
                e
            ))
        })
    }

    async fn save(path: &Path, document: &DesignTokensDocument) -> BitFunResult<()> {
        Self::ensure_parent(path).await?;
        let raw = serde_json::to_string_pretty(document)
            .map_err(|e| BitFunError::tool(format!("DesignTokens: serialize failed: {}", e)))?;
        fs::write(path, raw).await.map_err(|e| {
            BitFunError::tool(format!(
                "DesignTokens: failed to write {}: {}",
                path.display(),
                e
            ))
        })
    }

    /// Parse the user's selection from the generic answers payload.
    /// Accepts either `{ proposal_id: "..." }`, `{ "0": "proposal_id" }` (AskUser flat form),
    /// or a bare string.
    fn extract_proposal_id(answers: &Value) -> Option<String> {
        if let Some(s) = answers.as_str() {
            return Some(s.to_string());
        }
        if let Some(obj) = answers.as_object() {
            for key in ["proposal_id", "proposalId", "id", "0"] {
                if let Some(v) = obj.get(key) {
                    if let Some(s) = v.as_str() {
                        if !s.is_empty() {
                            return Some(s.to_string());
                        }
                    }
                }
            }
            if let Some(nested) = obj.get("answers") {
                return Self::extract_proposal_id(nested);
            }
        }
        None
    }

    fn make_event(data: Value, event: &str) -> ToolResult {
        let output = json!({
            "success": true,
            "artifact_event": event,
            "tokens_event": event,
            "data": data,
        });
        let result_for_assistant = Self::format_result_for_assistant(&output);
        ToolResult::ok(output, Some(result_for_assistant))
    }

    fn format_result_for_assistant(output: &Value) -> String {
        let event = output
            .get("tokens_event")
            .and_then(|v| v.as_str())
            .unwrap_or("ok");
        let data = output.get("data").unwrap_or(output);
        let selection = data
            .get("selection_status")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tokens = data.get("tokens");
        let committed = data
            .get("committed_id")
            .and_then(|v| v.as_str())
            .or_else(|| {
                tokens
                    .and_then(|v| v.get("committed_id"))
                    .and_then(|v| v.as_str())
            });
        let picked_proposal = committed.and_then(|pid| {
            tokens
                .and_then(|t| t.get("proposals"))
                .and_then(|arr| arr.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(pid))
                })
        });
        let picked_name = picked_proposal
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let picked_mood = picked_proposal
            .and_then(|p| p.get("mood"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let path = data.get("path").and_then(|v| v.as_str()).unwrap_or("");

        match (event, selection, committed) {
            (_, "timeout", _) => "The user did not pick a design direction. Ask them which one they prefer before continuing.".to_string(),
            (_, "cancelled", _) => "The user dismissed the design-direction choice. Ask them how they would like to proceed before continuing.".to_string(),
            (_, "invalid", _) => "The user's design-direction selection could not be resolved to a proposal. Confirm with the user and try again.".to_string(),
            ("tokens-committed", _, Some(pid)) => {
                let detail = if !picked_name.is_empty() {
                    format!(
                        " named \"{}\"{}",
                        picked_name,
                        if picked_mood.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", picked_mood)
                        }
                    )
                } else {
                    String::new()
                };
                format!(
                    "The user selected and committed design direction `{}`{}. This is now the active design token system{}; read the exact token values from the workspace file before creating or updating the artifact.",
                    pid,
                    detail,
                    if path.is_empty() {
                        String::new()
                    } else {
                        format!(" at {}", path)
                    }
                )
            }
            ("tokens-preview" | "tokens-listed", _, Some(pid)) => format!(
                "Current committed design direction is `{}`. Build against the exact token values stored in the workspace file system.",
                pid
            ),
            ("tokens-preview" | "tokens-listed", _, None) => {
                "No design direction is committed yet in the workspace file system.".to_string()
            }
            _ => format!("Design tokens {}.", event),
        }
    }
}

#[async_trait]
impl Tool for DesignTokensTool {
    fn name(&self) -> &str {
        "DesignTokens"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok("Propose, commit, preview, and list structured design token systems before creating a Design artifact. Use `propose` first to record 2-3 aesthetically distinct directions. The committed design token system persisted in the workspace file system is the single source of truth — always use `get` / `preview` to read it before building or patching an artifact, and build against those exact token values instead of relying on prior chat memory.".to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "additionalProperties": false,
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["propose", "await_selection", "commit", "update", "preview", "get", "list"],
                    "description": "propose — write proposals and (when running under a UI with tool_call_id) block up to 600s for the user to pick. await_selection — block on an already-written proposals doc; use this for resumed UIs. commit — explicit pick by proposal_id. preview/get — read. list — enumerate."
                },
                "artifact_id": { "type": "string" },
                "proposals": {
                    "type": "array",
                    "minItems": 2,
                    "maxItems": 3,
                    "description": "2-3 aesthetically DISTINCT directions (different hue families, typography voice, component personality) — NOT light/dark variants of the same palette. Each proposal is one background-agnostic system that will be previewed on both light and dark surfaces. Each MUST include colors, typography (family + size scale + weight + line-height), radius, shadow, spacing (a 4/8-based scale object), motion, and component_samples describing concrete UI primitives (button variants, input, switch/toggle, card, chip).",
                    "items": {
                        "type": "object",
                        "required": ["id", "name", "mood", "colors", "typography", "radius", "shadow", "motion", "component_samples"],
                        "additionalProperties": true
                    }
                },
                "proposal_id": { "type": "string", "description": "Proposal id to commit." },
                "proposal": {
                    "type": "object",
                    "description": "Full edited proposal object used by update. The id must match an existing proposal."
                }
            }
        })
    }

    fn user_facing_name(&self) -> String {
        "Design Tokens".to_string()
    }

    async fn validate_input(
        &self,
        input: &Value,
        _context: Option<&ToolUseContext>,
    ) -> ValidationResult {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("");
        if action.is_empty() {
            return ValidationResult {
                result: false,
                message: Some("action is required".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        if action == "propose" {
            let Some(proposals) = input.get("proposals").and_then(|v| v.as_array()) else {
                return ValidationResult {
                    result: false,
                    message: Some("DesignTokens.propose requires a proposals array".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            };
            if proposals.len() < 2 || proposals.len() > 3 {
                return ValidationResult {
                    result: false,
                    message: Some("DesignTokens.propose expects 2-3 aesthetically distinct directions (do not submit light/dark variants as separate proposals — each proposal is one system previewed on both light and dark surfaces)".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }
        if action == "commit"
            && input
                .get("proposal_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
        {
            return ValidationResult {
                result: false,
                message: Some("DesignTokens.commit requires proposal_id".to_string()),
                error_code: Some(400),
                meta: None,
            };
        }
        if action == "update" {
            let Some(proposal) = input.get("proposal").and_then(|v| v.as_object()) else {
                return ValidationResult {
                    result: false,
                    message: Some("DesignTokens.update requires proposal".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            };
            if proposal
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
            {
                return ValidationResult {
                    result: false,
                    message: Some("DesignTokens.update requires proposal.id".to_string()),
                    error_code: Some(400),
                    meta: None,
                };
            }
        }
        ValidationResult::default()
    }

    fn render_tool_use_message(
        &self,
        input: &Value,
        _options: &crate::agentic::tools::framework::ToolRenderOptions,
    ) -> String {
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("?");
        format!("DesignTokens.{}", action)
    }

    fn render_result_for_assistant(&self, output: &Value) -> String {
        let event = output
            .get("tokens_event")
            .and_then(|v| v.as_str())
            .unwrap_or("ok");
        let data = output.get("data").unwrap_or(output);
        let selection = data
            .get("selection_status")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tokens = data.get("tokens");
        let committed = tokens
            .and_then(|v| v.get("committed_id"))
            .and_then(|v| v.as_str());
        let picked_proposal = committed.and_then(|pid| {
            tokens
                .and_then(|t| t.get("proposals"))
                .and_then(|arr| arr.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(pid))
                })
        });
        let picked_name = picked_proposal
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let picked_mood = picked_proposal
            .and_then(|p| p.get("mood"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match (event, selection, committed) {
            (_, "timeout", _) => "The user didn't pick a design direction. Ask them which one they prefer before continuing.".to_string(),
            (_, "cancelled", _) => "The user dismissed the design-direction choice. Ask them how they would like to proceed before continuing.".to_string(),
            (_, "invalid", _) => "The user's selection could not be resolved to a proposal. Confirm with the user and try again.".to_string(),
            ("tokens-committed", _, Some(pid)) => {
                let detail = if !picked_name.is_empty() {
                    format!(" — \"{}\"{}", picked_name, if picked_mood.is_empty() { String::new() } else { format!(" ({})", picked_mood) })
                } else {
                    String::new()
                };
                format!(
                    "The user selected design direction `{}`{}. It is now the committed design token system in the workspace — read it from the file system and build against its exact token values.",
                    pid, detail
                )
            }
            ("tokens-preview" | "tokens-listed", _, Some(pid)) => format!(
                "Current committed design direction is `{}`. Build against the exact token values stored in the workspace file system.",
                pid
            ),
            ("tokens-preview" | "tokens-listed", _, None) => "No design direction is committed yet in the workspace file system.".to_string(),
            _ => format!("Design tokens {}.", event),
        }
    }

    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BitFunError::tool("DesignTokens: action is required"))?;

        let artifact_id = input.get("artifact_id").and_then(|v| v.as_str());
        let target_path = if let Some(artifact_id) = artifact_id {
            Self::artifact_tokens_file(context, artifact_id)?
        } else {
            Self::root_tokens_file(context)?
        };

        match action {
            "propose" | "await_selection" => {
                let mut doc = Self::load_or_default(&target_path).await?;
                if action == "propose" {
                    let proposals = input
                        .get("proposals")
                        .and_then(|v| v.as_array())
                        .ok_or_else(|| {
                            BitFunError::tool("DesignTokens.propose requires proposals")
                        })?;
                    doc.proposals = proposals
                        .iter()
                        .cloned()
                        .map(|proposal| {
                            let mut p: DesignTokenProposal = serde_json::from_value(proposal)
                                .map_err(|e| {
                                    BitFunError::tool(format!(
                                        "DesignTokens.propose invalid proposal: {}",
                                        e
                                    ))
                                })?;
                            if p.created_at.trim().is_empty() {
                                p.created_at = Utc::now().to_rfc3339();
                            }
                            Ok::<_, BitFunError>(p)
                        })
                        .collect::<BitFunResult<Vec<_>>>()?;
                    doc.scope = Some(if artifact_id.is_some() {
                        "artifact".to_string()
                    } else {
                        "workspace".to_string()
                    });
                    // A fresh proposal round requires a fresh decision.
                    doc.committed_id = None;
                    doc.committed_at = None;
                    Self::save(&target_path, &doc).await?;
                } else if doc.proposals.is_empty() {
                    return Err(BitFunError::tool(
                        "DesignTokens.await_selection: no proposals on disk — call propose first",
                    ));
                }

                // When running headless (no tool_call_id, e.g. pipeline batch),
                // `propose` just writes and returns. `await_selection` is the
                // explicit "now block for UI pick" action; it also short-circuits
                // if a commit already exists (so UIs can safely replay it).
                let Some(tool_id) = context.tool_call_id.clone() else {
                    if action == "propose" {
                        warn!(
                            "DesignTokens.propose has no tool_call_id; returning without blocking"
                        );
                    }
                    return Ok(vec![Self::make_event(
                        json!({
                            "tokens": doc,
                            "path": target_path,
                            "awaiting_selection": action == "await_selection",
                            "selection_status": doc.committed_id.as_ref().map(|_| "committed"),
                        }),
                        if doc.committed_id.is_some() {
                            "tokens-committed"
                        } else {
                            "tokens-proposed"
                        },
                    )]);
                };

                if action == "await_selection" && doc.committed_id.is_some() {
                    return Ok(vec![Self::make_event(
                        json!({
                            "tokens": doc,
                            "path": target_path,
                            "committed_id": doc.committed_id,
                            "selection_status": "committed"
                        }),
                        "tokens-committed",
                    )]);
                }

                let (tx, rx) = tokio::sync::oneshot::channel();
                let manager = get_user_input_manager();
                manager.register_channel(tool_id.clone(), tx);
                debug!(
                    "DesignTokens.{} waiting for user selection, tool_id: {}",
                    action, tool_id
                );

                let timeout_duration = Duration::from_secs(600);
                match timeout(timeout_duration, rx).await {
                    Ok(Ok(response)) => {
                        let proposal_id = Self::extract_proposal_id(&response.answers);
                        if let Some(pid) = proposal_id {
                            if let Some(p) =
                                doc.proposals.iter().find(|proposal| proposal.id == pid)
                            {
                                doc.committed_id = Some(pid.clone());
                                doc.committed_at = Some(Utc::now().to_rfc3339());
                                Self::save(&target_path, &doc).await?;
                                debug!(
                                    "DesignTokens.{} committed via card — {}（{}）",
                                    action, p.name, p.mood
                                );
                                return Ok(vec![Self::make_event(
                                    json!({
                                        "tokens": doc,
                                        "path": target_path,
                                        "committed_id": pid,
                                        "selection_status": "committed"
                                    }),
                                    "tokens-committed",
                                )]);
                            } else {
                                warn!(
                                    "DesignTokens.{}: submitted proposal_id '{}' not in proposals",
                                    action, pid
                                );
                            }
                        }
                        Ok(vec![Self::make_event(
                            json!({
                                "tokens": doc,
                                "path": target_path,
                                "selection_status": "invalid"
                            }),
                            "tokens-proposed",
                        )])
                    }
                    Ok(Err(_)) => {
                        warn!(
                            "DesignTokens.{} channel closed (cancelled), tool_id: {}",
                            action, tool_id
                        );
                        Ok(vec![Self::make_event(
                            json!({
                                "tokens": doc,
                                "path": target_path,
                                "selection_status": "cancelled"
                            }),
                            "tokens-proposed",
                        )])
                    }
                    Err(_) => {
                        warn!(
                            "DesignTokens.{} timed out after 600s, tool_id: {}",
                            action, tool_id
                        );
                        manager.cancel(&tool_id);
                        Ok(vec![Self::make_event(
                            json!({
                                "tokens": doc,
                                "path": target_path,
                                "selection_status": "timeout"
                            }),
                            "tokens-proposed",
                        )])
                    }
                }
            }
            "commit" => {
                let proposal_id = input
                    .get("proposal_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| BitFunError::tool("DesignTokens.commit requires proposal_id"))?;
                let mut doc = Self::load_or_default(&target_path).await?;
                if !doc.proposals.iter().any(|p| p.id == proposal_id) {
                    return Err(BitFunError::tool(format!(
                        "DesignTokens.commit: proposal_id '{}' not found",
                        proposal_id
                    )));
                }
                doc.committed_id = Some(proposal_id.to_string());
                doc.committed_at = Some(Utc::now().to_rfc3339());
                Self::save(&target_path, &doc).await?;
                if let Some(p) = doc.proposals.iter().find(|p| p.id == proposal_id) {
                    debug!("DesignTokens.commit — {}（{}）", p.name, p.mood);
                }
                Ok(vec![Self::make_event(
                    json!({ "tokens": doc, "path": target_path }),
                    "tokens-committed",
                )])
            }
            "update" => {
                let mut edited: DesignTokenProposal = serde_json::from_value(
                    input
                        .get("proposal")
                        .cloned()
                        .ok_or_else(|| BitFunError::tool("DesignTokens.update requires proposal"))?,
                )
                .map_err(|e| {
                    BitFunError::tool(format!("DesignTokens.update invalid proposal: {}", e))
                })?;
                let mut doc = Self::load_or_default(&target_path).await?;
                let Some(existing_index) = doc
                    .proposals
                    .iter()
                    .position(|proposal| proposal.id == edited.id)
                else {
                    return Err(BitFunError::tool(format!(
                        "DesignTokens.update: proposal_id '{}' not found",
                        edited.id
                    )));
                };

                if edited.created_at.trim().is_empty() {
                    edited.created_at = doc.proposals[existing_index].created_at.clone();
                }
                doc.proposals[existing_index] = edited.clone();
                if doc.committed_id.as_deref() == Some(edited.id.as_str()) {
                    doc.committed_at = Some(Utc::now().to_rfc3339());
                }
                Self::save(&target_path, &doc).await?;
                debug!("DesignTokens.update saved edited proposal: {}", edited.id);
                Ok(vec![Self::make_event(
                    json!({ "tokens": doc, "path": target_path, "proposal_id": edited.id }),
                    "tokens-updated",
                )])
            }
            "preview" | "get" => {
                let doc = Self::load_or_default(&target_path).await?;
                Ok(vec![Self::make_event(
                    json!({ "tokens": doc, "path": target_path }),
                    "tokens-preview",
                )])
            }
            "list" => {
                let workspace_root = Self::workspace_root(context)?;
                let design_root = get_path_manager_arc().workspace_design_root(&workspace_root);
                let mut paths = vec![Self::root_tokens_file(context)?];
                if design_root.exists() {
                    let mut entries = fs::read_dir(&design_root).await.map_err(|e| {
                        BitFunError::tool(format!(
                            "DesignTokens.list: failed to read {}: {}",
                            design_root.display(),
                            e
                        ))
                    })?;
                    while let Some(entry) = entries.next_entry().await.map_err(|e| {
                        BitFunError::tool(format!("DesignTokens.list: read_dir failed: {}", e))
                    })? {
                        let path = entry.path().join("tokens.json");
                        if path.exists() {
                            paths.push(path);
                        }
                    }
                }
                let mut docs = Vec::new();
                for path in paths {
                    if let Ok(doc) = Self::load_or_default(&path).await {
                        docs.push(json!({ "path": path, "tokens": doc }));
                    }
                }
                Ok(vec![Self::make_event(
                    json!({ "items": docs }),
                    "tokens-listed",
                )])
            }
            _ => Err(BitFunError::tool(format!(
                "DesignTokens: unknown action '{}'",
                action
            ))),
        }
    }
}
