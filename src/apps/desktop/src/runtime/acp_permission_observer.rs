//! Desktop observer that mirrors ACP permission requests into the shared mailbox
//! and keeps the existing Web UI event name for local tool-card UX.

use std::sync::Arc;
use std::time::Duration;

use bitfun_acp::client::AcpPermissionObserver;
use bitfun_core::infrastructure::events::{emit_global_event, BackendEvent};
use bitfun_services_integrations::remote_connect::{
    acp_permission_now_ms, AcpPermissionMailbox, AcpPermissionMailboxEntry,
};
use log::warn;
use serde_json::json;

pub(crate) struct DesktopAcpPermissionObserver {
    mailbox: Arc<AcpPermissionMailbox>,
}

impl DesktopAcpPermissionObserver {
    pub(crate) fn new(mailbox: Arc<AcpPermissionMailbox>) -> Self {
        Self { mailbox }
    }
}

impl AcpPermissionObserver for DesktopAcpPermissionObserver {
    fn on_permission_requested(
        &self,
        permission_id: &str,
        session_id: &str,
        tool_call: &serde_json::Value,
        options: &serde_json::Value,
        timeout: Duration,
    ) {
        let created_at_ms = acp_permission_now_ms();
        let expires_at_ms = created_at_ms.saturating_add(timeout.as_millis() as u64);
        self.mailbox.insert(AcpPermissionMailboxEntry {
            permission_id: permission_id.to_string(),
            session_id: session_id.to_string(),
            tool_call: tool_call.clone(),
            options: options.clone(),
            created_at_ms,
            expires_at_ms,
        });

        let payload = json!({
            "permissionId": permission_id,
            "sessionId": session_id,
            "toolCall": tool_call,
            "options": options,
        });
        // Local Web UI still listens on this event name; the mailbox is the
        // shared source for Remote Poll and remote respond.
        tokio::spawn(async move {
            if let Err(error) = emit_global_event(BackendEvent::Custom {
                event_name: "backend-event-acppermissionrequest".to_string(),
                payload,
            })
            .await
            {
                warn!("Failed to emit ACP permission request to local UI: {error}");
            }
        });
    }

    fn on_permission_resolved(&self, permission_id: &str) {
        let _ = self.mailbox.remove(permission_id);
    }
}

#[cfg(test)]
mod tests {
    use bitfun_acp::client::{is_acp_permission_id, ACP_PERMISSION_ID_PREFIX};

    #[test]
    fn permission_prefix_constant_matches_minted_ids() {
        assert!(is_acp_permission_id(&format!(
            "{ACP_PERMISSION_ID_PREFIX}{}",
            uuid::Uuid::new_v4()
        )));
        assert!(!is_acp_permission_id("native-tool-1"));
    }
}
