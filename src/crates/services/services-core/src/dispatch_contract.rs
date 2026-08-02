//! Single source of truth for the detached-dispatch wire contract.
//!
//! The target CLI advertises these capabilities, the controller requires
//! them, and the Web UI pins its own copy against this file in
//! `dispatch.contract.test.ts`. A protocol evolution is one edit here plus
//! that cross-language test — never a hunt across advertise/require lists.

use serde::{Deserialize, Serialize};

pub const DISPATCH_PROTOCOL_VERSION: u32 = 4;

/// Capabilities every v4 target advertises unconditionally.
pub const DISPATCH_BASE_TARGET_CAPABILITIES: &[&str] = &[
    "persistent_jobs",
    "cursor_events",
    "workspace_serialization",
    "approval_auto",
    "approval_reject_and_report",
    "approval_remote",
    "frontend_event_projection",
    "append_message",
    "event_log_completeness",
    // Git-worktree delivery. A target without these cannot be provisioned at
    // all — there is no snapshot fallback left — so controllers fail
    // preflight rather than degrade.
    "workspace_git_worktree",
    "workspace_git_bundle_upload",
    "workspace_git_sync",
    // A target may share the same package version while predating the
    // dispatch entrypoint's early CLI-profile selection. Such a binary can
    // accept a job but every detached worker then fails before execution.
    // Advertise the behavioral fix explicitly so controllers fail closed.
    "dispatch_worker_cli_profile",
    // v4: follow-up turns may override model and approval policy.
    "per_turn_options",
    // v4: read-only persisted-state queries (usage report) and compact turns
    // delivered through the continue mailbox.
    "session_query",
    // v4: inline image attachments on submit and follow-up turns.
    "inline_attachments",
];

/// Advertised only where detached workers can run (Linux/macOS), and
/// required by every controller: a target that cannot detach cannot dispatch.
pub const DISPATCH_DETACHED_WORKER_CAPABILITY: &str = "detached_worker";

/// Everything a controller refuses to submit without: the unconditional set
/// plus the platform-conditional detached worker.
pub fn dispatch_required_target_capabilities() -> impl Iterator<Item = &'static str> {
    DISPATCH_BASE_TARGET_CAPABILITIES
        .iter()
        .copied()
        .chain(std::iter::once(DISPATCH_DETACHED_WORKER_CAPABILITY))
}

/// One inline image attachment on a submit/continue turn.
///
/// v4 carries images as data URLs inside the request: SSH stages the request
/// as a file over SFTP so size is a policy choice, while the account-device
/// envelope keeps a much smaller controller-enforced budget. Staged chunked
/// transfer for larger payloads is a follow-up capability.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DispatchAttachment {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub mime_type: String,
    pub data_url: String,
}

pub const MAX_DISPATCH_ATTACHMENTS: usize = 8;
pub const MAX_DISPATCH_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DISPATCH_ATTACHMENTS_TOTAL_BYTES: usize = 16 * 1024 * 1024;
/// Inline budget for the account-device envelope. SSH stages the request as
/// an SFTP file, so only relay-carried requests need this much smaller cap.
pub const MAX_DEVICE_DISPATCH_ATTACHMENTS_TOTAL_BYTES: usize = 192 * 1024;

/// Shared structural validation, used verbatim by the controller (fail fast
/// before a transport round trip) and the target (authoritative).
pub fn validate_dispatch_attachments(attachments: &[DispatchAttachment]) -> Result<(), String> {
    if attachments.len() > MAX_DISPATCH_ATTACHMENTS {
        return Err(format!(
            "dispatch accepts at most {MAX_DISPATCH_ATTACHMENTS} attachments per turn"
        ));
    }
    let mut total = 0usize;
    for attachment in attachments {
        if attachment.id.trim().is_empty() || attachment.id.len() > 128 {
            return Err("dispatch attachment id must contain 1-128 bytes".to_string());
        }
        if !attachment.data_url.starts_with("data:image/") {
            return Err("dispatch attachments must be image data URLs".to_string());
        }
        if attachment.data_url.len() > MAX_DISPATCH_ATTACHMENT_BYTES {
            return Err("a dispatch attachment exceeds the 8 MiB limit".to_string());
        }
        total = total.saturating_add(attachment.data_url.len());
    }
    if total > MAX_DISPATCH_ATTACHMENTS_TOTAL_BYTES {
        return Err("dispatch attachments exceed the 16 MiB total limit".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_capabilities_are_base_plus_detached_worker() {
        let required: Vec<&str> = dispatch_required_target_capabilities().collect();
        assert_eq!(required.len(), DISPATCH_BASE_TARGET_CAPABILITIES.len() + 1);
        assert!(required.contains(&DISPATCH_DETACHED_WORKER_CAPABILITY));
        for capability in DISPATCH_BASE_TARGET_CAPABILITIES {
            assert!(required.contains(capability));
        }
    }
}
