//! MiniApp agent bridge domain rules.
//!
//! This module owns provider-neutral run ownership, rate-limit, registry, and
//! turn-text extraction rules. Product hosts still own filesystem creation and
//! agent scheduler/coordinator calls.

use crate::miniapp::rate_limit::{MiniAppRateLimitState, MiniAppRateLimitSubject};
use crate::miniapp::types::AgentPermissions;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const MINIAPP_AGENT_SURFACE: &str = "miniapp_agent";
pub const MINIAPP_AGENT_SESSION_NAME: &str = "MiniApp Agent Run";
pub const MINIAPP_AGENT_KIND: &str = "Cowork";
pub const AGENT_ACCESS_DISABLED_MESSAGE: &str = "Agent access is not enabled for this MiniApp";
pub const UNKNOWN_AGENT_SESSION_MESSAGE: &str = "Unknown MiniApp agent session";
pub const UNKNOWN_AGENT_RUN_MESSAGE: &str = "Unknown MiniApp agent run";
pub const WORKSPACE_MISMATCH_MESSAGE: &str =
    "MiniApp agent session workspace does not match this run";
pub const APP_DATA_WORKSPACE_INVALID_MESSAGE: &str =
    "appDataWorkspace must be a clean relative path";
pub const WORKSPACE_REQUIRED_MESSAGE: &str = "workspacePath is required for MiniApp agent runs";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniAppAgentRunRecord {
    pub app_id: String,
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug)]
pub struct MiniAppAgentRunRegistry {
    records: Mutex<HashMap<String, MiniAppAgentRunRecord>>,
    max_records: usize,
}

impl Default for MiniAppAgentRunRegistry {
    fn default() -> Self {
        Self::new(256)
    }
}

impl MiniAppAgentRunRegistry {
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            max_records,
        }
    }

    pub fn register(&self, record: MiniAppAgentRunRecord) {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if records.len() >= self.max_records {
            if let Some(key) = records.keys().next().cloned() {
                records.remove(&key);
            }
        }
        records.insert(record.turn_id.clone(), record);
    }

    pub fn lookup(
        &self,
        app_id: &str,
        session_id: &str,
        turn_id: &str,
    ) -> Option<MiniAppAgentRunRecord> {
        let records = self
            .records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        records
            .get(turn_id)
            .filter(|record| record.app_id == app_id && record.session_id == session_id)
            .cloned()
    }

    pub fn take_for_app(&self, app_id: &str) -> Vec<MiniAppAgentRunRecord> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let turn_ids: Vec<String> = records
            .iter()
            .filter(|(_, record)| record.app_id == app_id)
            .map(|(turn_id, _)| turn_id.clone())
            .collect();
        turn_ids
            .into_iter()
            .filter_map(|turn_id| records.remove(&turn_id))
            .collect()
    }

    pub fn remove(&self, turn_id: &str) {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        records.remove(turn_id);
    }
}

#[derive(Debug, Default)]
pub struct MiniAppAgentRateLimiter {
    state: Mutex<MiniAppRateLimitState>,
}

impl MiniAppAgentRateLimiter {
    pub fn check(
        &self,
        app_id: &str,
        rate_limit_per_minute: u32,
        now_ms: u64,
    ) -> Result<(), String> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .check(
                app_id,
                rate_limit_per_minute,
                now_ms,
                MiniAppRateLimitSubject::Agent,
            )
    }
}

pub fn require_enabled_agent_permissions(
    agent_permissions: Option<&AgentPermissions>,
) -> Result<AgentPermissions, String> {
    let agent_permissions = agent_permissions
        .cloned()
        .ok_or(AGENT_ACCESS_DISABLED_MESSAGE)?;
    if !agent_permissions.enabled {
        return Err(AGENT_ACCESS_DISABLED_MESSAGE.to_string());
    }
    Ok(agent_permissions)
}

pub fn is_clean_relative_subdir(subdir: &str) -> bool {
    let relative = Path::new(subdir);
    !relative.as_os_str().is_empty()
        && relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

pub fn app_data_workspace_path(app_data_dir: &Path, subdir: &str) -> Result<PathBuf, String> {
    if !is_clean_relative_subdir(subdir) {
        return Err(APP_DATA_WORKSPACE_INVALID_MESSAGE.to_string());
    }
    Ok(app_data_dir.join(Path::new(subdir)))
}

pub fn resolve_agent_workspace_path(
    explicit_workspace_path: Option<&str>,
    app_data_workspace: Option<&str>,
    app_data_dir: &Path,
) -> Result<PathBuf, String> {
    if let Some(subdir) = app_data_workspace
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return app_data_workspace_path(app_data_dir, subdir);
    }
    explicit_workspace_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| WORKSPACE_REQUIRED_MESSAGE.to_string())
}

pub fn default_agent_run_id(app_id: &str, sequence: u64) -> String {
    format!("miniapp-agent-{}-{}", app_id, sequence)
}

pub fn agent_owner(app_id: &str, run_id: &str) -> String {
    format!("miniapp-agent:{}:{}", app_id, run_id)
}

pub fn agent_owner_prefix(app_id: &str) -> String {
    format!("miniapp-agent:{}:", app_id)
}

pub fn session_name_or_default(session_name: Option<&str>) -> String {
    session_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(MINIAPP_AGENT_SESSION_NAME)
        .to_string()
}

pub fn requested_session_id(session_id: Option<&str>) -> Option<String> {
    session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn validate_reused_session(
    created_by: Option<&str>,
    workspace_path: Option<&str>,
    app_id: &str,
    expected_workspace_path: &str,
) -> Result<(), String> {
    let owner_prefix = agent_owner_prefix(app_id);
    if !created_by.is_some_and(|created_by| created_by.starts_with(&owner_prefix)) {
        return Err(UNKNOWN_AGENT_SESSION_MESSAGE.to_string());
    }
    if workspace_path != Some(expected_workspace_path) {
        return Err(WORKSPACE_MISMATCH_MESSAGE.to_string());
    }
    Ok(())
}

pub fn agent_run_metadata(app_id: &str, run_id: &str) -> serde_json::Value {
    json!({
        "surface": MINIAPP_AGENT_SURFACE,
        "appId": app_id,
        "runId": run_id,
        "acp_transport": true,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniAppAgentTurnMessageRole {
    Assistant,
    Tool,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppAgentTurnMessage {
    pub turn_id: Option<String>,
    pub role: MiniAppAgentTurnMessageRole,
    pub is_tool_result: bool,
    pub text: String,
}

pub fn extract_agent_turn_text(messages: &[MiniAppAgentTurnMessage], turn_id: &str) -> String {
    let turn_messages: Vec<&MiniAppAgentTurnMessage> = messages
        .iter()
        .filter(|message| message.turn_id.as_deref() == Some(turn_id))
        .collect();
    let answer_start = turn_messages
        .iter()
        .rposition(|message| {
            message.role == MiniAppAgentTurnMessageRole::Tool || message.is_tool_result
        })
        .map_or(0, |index| index + 1);
    turn_messages[answer_start..]
        .iter()
        .filter(|message| message.role == MiniAppAgentTurnMessageRole::Assistant)
        .filter_map(|message| {
            if message.text.trim().is_empty() {
                None
            } else {
                Some(message.text.as_str())
            }
        })
        .collect::<Vec<_>>()
        .concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_workspace_rejects_paths_outside_app_storage() {
        assert!(is_clean_relative_subdir("decks/deck-123"));
        assert!(is_clean_relative_subdir("decks"));
        assert!(!is_clean_relative_subdir(""));
        assert!(!is_clean_relative_subdir("/etc"));
        assert!(!is_clean_relative_subdir("../outside"));
        assert!(!is_clean_relative_subdir("decks/../../outside"));
        assert!(!is_clean_relative_subdir("./decks"));

        assert_eq!(
            app_data_workspace_path(Path::new("/appdata"), "decks/deck-123").unwrap(),
            PathBuf::from("/appdata").join("decks").join("deck-123")
        );
    }

    #[test]
    fn run_registry_preserves_owner_lookup_and_bounded_cleanup() {
        let registry = MiniAppAgentRunRegistry::new(1);
        registry.register(MiniAppAgentRunRecord {
            app_id: "app-1".to_string(),
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
        });
        assert!(registry.lookup("app-1", "session-1", "turn-1").is_some());
        assert!(registry.lookup("other", "session-1", "turn-1").is_none());

        registry.register(MiniAppAgentRunRecord {
            app_id: "app-1".to_string(),
            session_id: "session-2".to_string(),
            turn_id: "turn-2".to_string(),
        });
        assert_eq!(registry.take_for_app("app-1").len(), 1);
    }

    #[test]
    fn reused_session_validation_preserves_owner_and_workspace_checks() {
        validate_reused_session(
            Some("miniapp-agent:builtin-ppt-live:run-1"),
            Some("/workspace"),
            "builtin-ppt-live",
            "/workspace",
        )
        .unwrap();
        assert_eq!(
            validate_reused_session(
                Some("other-agent:builtin-ppt-live:run-1"),
                Some("/workspace"),
                "builtin-ppt-live",
                "/workspace",
            )
            .unwrap_err(),
            UNKNOWN_AGENT_SESSION_MESSAGE
        );
        assert_eq!(
            validate_reused_session(
                Some("miniapp-agent:builtin-ppt-live:run-1"),
                Some("/other"),
                "builtin-ppt-live",
                "/workspace",
            )
            .unwrap_err(),
            WORKSPACE_MISMATCH_MESSAGE
        );
    }

    #[test]
    fn turn_text_starts_after_last_tool_result_for_requested_turn() {
        let messages = vec![
            MiniAppAgentTurnMessage {
                turn_id: Some("turn-1".to_string()),
                role: MiniAppAgentTurnMessageRole::Assistant,
                is_tool_result: false,
                text: "old".to_string(),
            },
            MiniAppAgentTurnMessage {
                turn_id: Some("turn-1".to_string()),
                role: MiniAppAgentTurnMessageRole::Tool,
                is_tool_result: true,
                text: String::new(),
            },
            MiniAppAgentTurnMessage {
                turn_id: Some("turn-1".to_string()),
                role: MiniAppAgentTurnMessageRole::Assistant,
                is_tool_result: false,
                text: "new".to_string(),
            },
            MiniAppAgentTurnMessage {
                turn_id: Some("turn-2".to_string()),
                role: MiniAppAgentTurnMessageRole::Assistant,
                is_tool_result: false,
                text: "ignored".to_string(),
            },
        ];

        assert_eq!(extract_agent_turn_text(&messages, "turn-1"), "new");
    }
}
