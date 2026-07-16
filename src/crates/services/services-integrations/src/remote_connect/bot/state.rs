use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::menu::{MenuItem, MenuItemStyle, MenuView};

const PENDING_TTL_SECS: i64 = 5 * 60;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum BotDisplayMode {
    #[serde(rename = "pro")]
    Pro,
    #[serde(rename = "assistant")]
    #[default]
    Assistant,
}

/// Workspace selection retained by the bot, including SSH identity when remote.
///
/// Persisted bot state historically stored `current_workspace` as a bare path
/// string. Deserialization still accepts that form and upgrades it in memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BotWorkspaceRef {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_connection_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_ssh_host: Option<String>,
}

impl BotWorkspaceRef {
    pub fn local(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            remote_connection_id: None,
            remote_ssh_host: None,
        }
    }

    pub fn with_identity(
        path: impl Into<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
    ) -> Self {
        Self {
            path: path.into(),
            remote_connection_id: remote_connection_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            remote_ssh_host: remote_ssh_host
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

fn deserialize_optional_workspace_ref<'de, D>(
    deserializer: D,
) -> Result<Option<BotWorkspaceRef>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Null,
        Path(String),
        Full(BotWorkspaceRef),
    }

    match Option::<Raw>::deserialize(deserializer)? {
        None | Some(Raw::Null) => Ok(None),
        Some(Raw::Path(path)) => {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(BotWorkspaceRef::local(trimmed)))
            }
        }
        Some(Raw::Full(workspace)) => {
            if workspace.path.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(workspace))
            }
        }
    }
}

/// One selectable workspace row in the bot `/switch` picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotWorkspaceChoice {
    pub path: String,
    pub name: String,
    pub remote_connection_id: Option<String>,
    pub remote_ssh_host: Option<String>,
}

impl BotWorkspaceChoice {
    pub fn new(
        path: impl Into<String>,
        name: impl Into<String>,
        remote_connection_id: Option<String>,
        remote_ssh_host: Option<String>,
    ) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
            remote_connection_id,
            remote_ssh_host,
        }
    }

    pub fn to_workspace_ref(&self) -> BotWorkspaceRef {
        BotWorkspaceRef::with_identity(
            self.path.clone(),
            self.remote_connection_id.clone(),
            self.remote_ssh_host.clone(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotChatState {
    pub chat_id: String,
    pub paired: bool,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_workspace_ref",
        skip_serializing_if = "Option::is_none"
    )]
    pub current_workspace: Option<BotWorkspaceRef>,
    pub current_assistant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_assistant_name: Option<String>,
    pub current_session_id: Option<String>,
    #[serde(default)]
    pub display_mode: BotDisplayMode,
    #[serde(skip)]
    pub pending_action: Option<PendingAction>,
    #[serde(skip)]
    pub pending_expires_at: i64,
    #[serde(skip)]
    pub pending_invalid_count: u8,
    #[serde(skip, default)]
    pub last_menu_commands: Vec<String>,
    /// Relay base URL the bot should hit for multi-device control. Set by the
    /// desktop when the bot is paired so the bot can reach the same relay.
    #[serde(skip, default)]
    pub relay_url: Option<String>,
    /// Delegated account token + master key for multi-device control. Set by
    /// the desktop layer when the paired desktop has account login. These are
    /// in-memory only (`serde(skip)`) — the master key is never persisted,
    /// mirroring the `AccountSession.master_key` in-memory-only contract.
    #[serde(skip, default)]
    pub delegated_token: Option<String>,
    #[serde(skip, default)]
    pub delegated_master_key: Option<Vec<u8>>,
    /// When Some, the bot operates on a remote device via HTTP RPC instead
    /// of the local desktop. Set by `/devices` → pick a device.
    /// Cleared by `/devices` → pick "local" or selecting an offline device.
    #[serde(skip, default)]
    pub active_remote_device: Option<RemoteDeviceTarget>,
}

/// A remote device the bot has switched to. All subsequent bot commands
/// (create_session, send_message, list_sessions, etc.) are routed to this
/// device via the relay HTTP RPC API instead of executing locally.
#[derive(Debug, Clone)]
pub struct RemoteDeviceTarget {
    pub device_id: String,
    pub device_name: String,
}

impl BotChatState {
    pub fn new(chat_id: String) -> Self {
        Self {
            chat_id,
            paired: false,
            current_workspace: None,
            current_assistant: None,
            current_assistant_name: None,
            current_session_id: None,
            display_mode: BotDisplayMode::Assistant,
            pending_action: None,
            pending_expires_at: 0,
            pending_invalid_count: 0,
            last_menu_commands: Vec::new(),
            relay_url: None,
            delegated_token: None,
            delegated_master_key: None,
            active_remote_device: None,
        }
    }

    pub fn active_workspace_path(&self) -> Option<String> {
        self.current_workspace
            .as_ref()
            .map(|workspace| workspace.path.clone())
            .or_else(|| self.current_assistant.clone())
    }

    pub fn current_workspace_path(&self) -> Option<&str> {
        self.current_workspace.as_ref().map(BotWorkspaceRef::path)
    }

    pub fn current_workspace_identity(&self) -> (Option<String>, Option<String>) {
        self.current_workspace
            .as_ref()
            .map(|workspace| {
                (
                    workspace.remote_connection_id.clone(),
                    workspace.remote_ssh_host.clone(),
                )
            })
            .unwrap_or((None, None))
    }

    pub fn set_pending(&mut self, action: PendingAction) {
        self.pending_action = Some(action);
        self.pending_expires_at = now_secs() + PENDING_TTL_SECS;
        self.pending_invalid_count = 0;
    }

    pub fn clear_pending(&mut self) {
        self.pending_action = None;
        self.pending_expires_at = 0;
        self.pending_invalid_count = 0;
    }

    pub fn pending_expired(&self) -> bool {
        self.pending_action.is_some() && now_secs() > self.pending_expires_at
    }

    /// Install the desktop's delegated account identity (token + master key)
    /// so the bot can call the relay's device-control endpoints directly.
    /// The master key is kept in memory only; callers must never persist it.
    pub fn set_delegated_identity(&mut self, token: String, master_key: Vec<u8>) {
        self.delegated_token = Some(token);
        self.delegated_master_key = Some(master_key);
    }

    /// Whether a delegated account identity is available for multi-device
    /// control. Both the token and the master key must be present.
    pub fn has_delegated_identity(&self) -> bool {
        self.delegated_token.is_some() && self.delegated_master_key.is_some()
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    SelectWorkspace {
        options: Vec<BotWorkspaceChoice>,
    },
    SelectAssistant {
        options: Vec<(String, String)>,
    },
    SelectSession {
        options: Vec<(String, String)>,
        page: usize,
        has_more: bool,
    },
    SelectModel {
        options: Vec<(String, String)>,
    },
    AskUserQuestion {
        tool_id: String,
        questions: Vec<BotQuestion>,
        current_index: usize,
        answers: Vec<Value>,
        awaiting_custom_text: bool,
        pending_answer: Option<Value>,
    },
    ConfirmModeSwitch {
        target_mode: BotDisplayMode,
        target_cmd: String,
    },
    /// User is picking a device from the /devices list to switch context to.
    /// `options` is `(device_id, device_name)`; index 0 = "local" (clear).
    SelectDevice {
        options: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotQuestionOption {
    pub label: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotQuestion {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub options: Vec<BotQuestionOption>,
    #[serde(rename = "multiSelect", default)]
    pub multi_select: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotActionStyle {
    Primary,
    Default,
}

#[derive(Debug, Clone)]
pub struct BotAction {
    pub label: String,
    pub command: String,
    pub style: BotActionStyle,
}

impl BotAction {
    pub fn primary(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            style: BotActionStyle::Primary,
        }
    }

    pub fn secondary(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            style: BotActionStyle::Default,
        }
    }
}

impl From<MenuItem> for BotAction {
    fn from(item: MenuItem) -> Self {
        let style = match item.style {
            MenuItemStyle::Primary => BotActionStyle::Primary,
            _ => BotActionStyle::Default,
        };
        BotAction {
            label: item.label,
            command: item.command,
            style,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BotInteractiveRequest {
    pub reply: String,
    pub actions: Vec<BotAction>,
    pub menu: MenuView,
    pub pending_action: PendingAction,
}

pub type BotInteractionHandler =
    Arc<dyn Fn(BotInteractiveRequest) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub type BotMessageSender =
    Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_workspace_path_prefers_workspace_then_assistant() {
        let mut state = BotChatState::new("chat".into());
        assert_eq!(state.active_workspace_path(), None);

        state.current_assistant = Some("/assistant".into());
        assert_eq!(state.active_workspace_path().as_deref(), Some("/assistant"));

        state.current_workspace = Some(BotWorkspaceRef::local("/workspace"));
        assert_eq!(state.active_workspace_path().as_deref(), Some("/workspace"));
    }

    #[test]
    fn pending_state_sets_expires_and_clear_resets_transient_fields() {
        let mut state = BotChatState::new("chat".into());
        state.set_pending(PendingAction::SelectWorkspace { options: vec![] });
        assert!(state.pending_action.is_some());
        assert!(state.pending_expires_at > 0);
        assert_eq!(state.pending_invalid_count, 0);

        state.pending_invalid_count = 2;
        state.clear_pending();

        assert!(state.pending_action.is_none());
        assert_eq!(state.pending_expires_at, 0);
        assert_eq!(state.pending_invalid_count, 0);
    }

    #[test]
    fn current_workspace_deserializes_legacy_path_string() {
        let state: BotChatState = serde_json::from_value(serde_json::json!({
            "chat_id": "c1",
            "paired": true,
            "current_workspace": "/root/repos",
            "current_assistant": null,
            "current_session_id": null,
            "display_mode": "pro"
        }))
        .expect("legacy path string must deserialize");

        assert_eq!(
            state.current_workspace,
            Some(BotWorkspaceRef::local("/root/repos"))
        );
    }

    #[test]
    fn current_workspace_deserializes_identity_object() {
        let state: BotChatState = serde_json::from_value(serde_json::json!({
            "chat_id": "c1",
            "paired": true,
            "current_workspace": {
                "path": "/root/repos",
                "remote_connection_id": "conn-a",
                "remote_ssh_host": "host-a"
            },
            "current_assistant": null,
            "current_session_id": null,
            "display_mode": "pro"
        }))
        .expect("identity object must deserialize");

        assert_eq!(
            state.current_workspace,
            Some(BotWorkspaceRef::with_identity(
                "/root/repos",
                Some("conn-a".into()),
                Some("host-a".into()),
            ))
        );
        assert_eq!(
            state.current_workspace_identity(),
            (Some("conn-a".into()), Some("host-a".into()))
        );
    }
}
