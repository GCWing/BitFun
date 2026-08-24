//! Shared ACP permission mailbox view for Desktop UI and Remote Poll.
//!
//! Product hosts (Desktop) write entries when ACP asks for permission. Remote
//! poll sync reads the same map. Entries survive disconnect; ACP timeout still
//! clears them when the oneshot converges to Cancelled.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcpPermissionMailboxEntry {
    pub permission_id: String,
    pub session_id: String,
    pub tool_call: Value,
    pub options: Value,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Default)]
pub struct AcpPermissionMailbox {
    pending: Mutex<HashMap<String, AcpPermissionMailboxEntry>>,
}

impl AcpPermissionMailbox {
    pub fn insert(&self, entry: AcpPermissionMailboxEntry) {
        self.pending
            .lock()
            .expect("ACP permission mailbox")
            .insert(entry.permission_id.clone(), entry);
    }

    pub fn remove(&self, permission_id: &str) -> Option<AcpPermissionMailboxEntry> {
        self.pending
            .lock()
            .expect("ACP permission mailbox")
            .remove(permission_id)
    }

    pub fn get(&self, permission_id: &str) -> Option<AcpPermissionMailboxEntry> {
        self.pending
            .lock()
            .expect("ACP permission mailbox")
            .get(permission_id)
            .cloned()
    }

    pub fn list_for_session(&self, session_id: &str) -> Vec<AcpPermissionMailboxEntry> {
        self.pending
            .lock()
            .expect("ACP permission mailbox")
            .values()
            .filter(|entry| entry.session_id == session_id)
            .cloned()
            .collect()
    }

    pub fn clear_session(&self, session_id: &str) {
        self.pending
            .lock()
            .expect("ACP permission mailbox")
            .retain(|_, entry| entry.session_id != session_id);
    }

    pub fn contains(&self, permission_id: &str) -> bool {
        self.pending
            .lock()
            .expect("ACP permission mailbox")
            .contains_key(permission_id)
    }
}

static MAILBOX: OnceLock<Arc<AcpPermissionMailbox>> = OnceLock::new();

pub fn install_acp_permission_mailbox(
    mailbox: Arc<AcpPermissionMailbox>,
) -> Arc<AcpPermissionMailbox> {
    let _ = MAILBOX.set(mailbox.clone());
    mailbox
}

pub fn acp_permission_mailbox() -> Option<Arc<AcpPermissionMailbox>> {
    MAILBOX.get().cloned()
}

pub fn acp_permission_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn sync_acp_permission_mailbox_into_tracker(
    session_id: &str,
    tracker: &super::RemoteSessionStateTracker,
) {
    let Some(mailbox) = acp_permission_mailbox() else {
        return;
    };
    for entry in mailbox.list_for_session(session_id) {
        let tool_id = entry
            .tool_call
            .get("toolCallId")
            .or_else(|| entry.tool_call.get("tool_call_id"))
            .and_then(|value| value.as_str())
            .unwrap_or(entry.permission_id.as_str())
            .to_string();
        let tool_name = entry
            .tool_call
            .get("title")
            .or_else(|| entry.tool_call.get("kind"))
            .and_then(|value| value.as_str())
            .unwrap_or("acp_permission")
            .to_string();
        let tool_input = Some(serde_json::json!({
            "permissionId": entry.permission_id,
            "options": entry.options,
            "toolCall": entry.tool_call,
            "expiresAtMs": entry.expires_at_ms,
        }));
        let input_preview = tool_input
            .as_ref()
            .and_then(|input| serde_json::to_string(input).ok());
        tracker.sync_pending_permission(tool_id, tool_name, input_preview, tool_input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_session_removes_only_matching_entries() {
        let mailbox = AcpPermissionMailbox::default();
        mailbox.insert(AcpPermissionMailboxEntry {
            permission_id: "p1".to_string(),
            session_id: "s1".to_string(),
            tool_call: serde_json::json!({}),
            options: serde_json::json!([]),
            created_at_ms: 1,
            expires_at_ms: 2,
        });
        mailbox.insert(AcpPermissionMailboxEntry {
            permission_id: "p2".to_string(),
            session_id: "s2".to_string(),
            tool_call: serde_json::json!({}),
            options: serde_json::json!([]),
            created_at_ms: 1,
            expires_at_ms: 2,
        });
        mailbox.clear_session("s1");
        assert!(mailbox.list_for_session("s1").is_empty());
        assert_eq!(mailbox.list_for_session("s2").len(), 1);
    }
}
