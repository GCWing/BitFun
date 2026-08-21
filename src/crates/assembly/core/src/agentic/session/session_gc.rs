//! Session GC: orphan detection and transient sweep candidate reporting.
//!
//! Conservative by design: this module only *reports* cleanup candidates.
//! Automatic deletion is deliberately not performed, so a scan can never
//! destroy a session that a concurrent owner still holds a reference to.
//! Callers decide whether to act on a report.

use std::collections::HashSet;

use bitfun_services_core::session::SessionMetadata;

/// Why a session is considered an orphan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanKind {
    /// The session declares a parent that no longer exists in the scanned set.
    DanglingChild,
    /// The session carries a `session-{parent}` creator marker but declares no
    /// relationship, and that parent no longer exists.
    DetachedChild,
}

/// One reported orphan candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanedSessionRecord {
    pub session_id: String,
    pub kind: OrphanKind,
    pub reason: String,
}

/// Result of a report-only GC scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionGcReport {
    pub scanned_metadata_count: usize,
    pub orphaned: Vec<OrphanedSessionRecord>,
}

/// A transient session that finished executing and whose parent (if any) is
/// no longer loaded, so no reuse reference can remain (report-only).
///
/// Parent identity follows the same `session-{parent}` creator marker used by
/// `SessionManager::transient_descendants_postorder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransientSweepCandidate {
    pub session_id: String,
    pub parent_session_id: Option<String>,
}

/// Creator marker prefix used when a coordinator spawns a subagent session.
/// Mirrors the `session-{parent_session_id}` marker in
/// `SessionManager::transient_descendants_postorder`.
const SUBAGENT_CREATOR_PREFIX: &str = "session-";

/// Classify session metadata and report orphan candidates.
///
/// Conservative rules:
/// - `relationship.parent_session_id = Some(parent)` with `parent` absent
///   from the scanned set is a dangling child (its parent was deleted without
///   a cascading delete).
/// - A `created_by` of the form `session-{parent}` with no relationship and an
///   absent `parent` is a detached child. All other `created_by` values
///   (user-supplied names, `memory-phase2`, `None`, ...) are treated as
///   legitimate top-level creators and are never flagged.
/// - Children whose parent is present, and top-level sessions, are never
///   flagged.
pub fn classify_orphaned_metadata(metadata: &[SessionMetadata]) -> SessionGcReport {
    let known_ids: HashSet<&str> = metadata
        .iter()
        .map(|entry| entry.session_id.as_str())
        .collect();
    let mut orphaned = Vec::new();

    for entry in metadata {
        let session_id = entry.session_id.as_str();

        if let Some(parent_session_id) = entry
            .relationship
            .as_ref()
            .and_then(|relationship| relationship.parent_session_id.as_deref())
        {
            if !known_ids.contains(parent_session_id) {
                orphaned.push(OrphanedSessionRecord {
                    session_id: session_id.to_string(),
                    kind: OrphanKind::DanglingChild,
                    reason: format!(
                        "parent session {} is missing from metadata",
                        parent_session_id
                    ),
                });
            }
            continue;
        }

        if let Some(created_by) = entry.created_by.as_deref() {
            if let Some(parent_session_id) = created_by.strip_prefix(SUBAGENT_CREATOR_PREFIX) {
                if !known_ids.contains(parent_session_id) {
                    orphaned.push(OrphanedSessionRecord {
                        session_id: session_id.to_string(),
                        kind: OrphanKind::DetachedChild,
                        reason: format!(
                            "creator marker references missing parent {}",
                            parent_session_id
                        ),
                    });
                }
            }
        }
    }

    SessionGcReport {
        scanned_metadata_count: metadata.len(),
        orphaned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_core_types::{SessionContinuationPolicy, SessionKind};
    use bitfun_services_core::session::{SessionMemoryMode, SessionRelationship, SessionStatus};

    fn metadata(session_id: &str) -> SessionMetadata {
        SessionMetadata {
            session_id: session_id.to_string(),
            session_name: format!("test-{}", session_id),
            agent_type: "agentic".to_string(),
            last_user_dialog_agent_type: None,
            last_submitted_agent_type: None,
            created_by: None,
            session_kind: SessionKind::Standard,
            memory_mode: SessionMemoryMode::Enabled,
            model_name: "primary".to_string(),
            created_at: 1,
            last_active_at: 1,
            last_finished_at: None,
            turn_count: 0,
            message_count: 0,
            tool_call_count: 0,
            status: SessionStatus::Active,
            terminal_session_id: None,
            snapshot_session_id: None,
            tags: Vec::new(),
            custom_metadata: None,
            current_context_usage: None,
            relationship: None,
            todos: None,
            review_action_state: None,
            deep_review_run_manifest: None,
            review_target_evidence: None,
            deep_review_cache: None,
            workspace_path: None,
            project_workspace_path: None,
            execution_target: None,
            workspace_hostname: None,
            unread_completion: None,
            needs_user_attention: None,
            display_state: None,
            runtime_state: None,
            is_daemon: false,
            orphaned: false,
            orphan_kind: None,
        }
    }

    fn metadata_with_parent(session_id: &str, parent_session_id: &str) -> SessionMetadata {
        let mut entry = metadata(session_id);
        entry.created_by = Some(format!("session-{}", parent_session_id));
        entry.relationship = Some(SessionRelationship {
            parent_session_id: Some(parent_session_id.to_string()),
            continuation_policy: Some(SessionContinuationPolicy::FreshOnly),
            ..Default::default()
        });
        entry
    }

    #[test]
    fn top_level_sessions_are_never_flagged() {
        let entries = vec![metadata("root-a"), metadata("root-b")];
        let report = classify_orphaned_metadata(&entries);
        assert_eq!(report.scanned_metadata_count, 2);
        assert!(report.orphaned.is_empty());
    }

    #[test]
    fn child_with_live_parent_is_not_flagged() {
        let entries = vec![
            metadata("parent-1"),
            metadata_with_parent("child-1", "parent-1"),
        ];
        let report = classify_orphaned_metadata(&entries);
        assert!(report.orphaned.is_empty());
    }

    #[test]
    fn dangling_child_is_flagged_when_parent_metadata_is_missing() {
        let entries = vec![metadata_with_parent("child-1", "ghost-parent")];
        let report = classify_orphaned_metadata(&entries);
        assert_eq!(report.orphaned.len(), 1);
        let record = &report.orphaned[0];
        assert_eq!(record.session_id, "child-1");
        assert_eq!(record.kind, OrphanKind::DanglingChild);
        assert!(record.reason.contains("ghost-parent"));
    }

    #[test]
    fn detached_child_with_missing_creator_parent_is_flagged() {
        let mut entry = metadata("detached-1");
        entry.created_by = Some("session-ghost-creator".to_string());
        entry.relationship = None;
        let report = classify_orphaned_metadata(&[entry]);
        assert_eq!(report.orphaned.len(), 1);
        assert_eq!(report.orphaned[0].kind, OrphanKind::DetachedChild);
    }

    #[test]
    fn non_subagent_creator_markers_are_not_flagged() {
        let mut entry = metadata("memory-1");
        entry.created_by = Some("memory-phase2".to_string());
        let mut user_entry = metadata("user-1");
        user_entry.created_by = Some("alice".to_string());
        let report = classify_orphaned_metadata(&[entry, user_entry]);
        assert!(report.orphaned.is_empty());
    }

    #[test]
    fn detached_child_with_live_creator_parent_is_not_flagged() {
        let mut entry = metadata("child-2");
        entry.created_by = Some("session-parent-2".to_string());
        entry.relationship = None;
        let report = classify_orphaned_metadata(&[metadata("parent-2"), entry]);
        assert!(report.orphaned.is_empty());
    }
}
