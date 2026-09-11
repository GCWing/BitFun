//! Reusable display slots for default-titled sessions within a workspace.
//! The metadata store holds the cross-process index lock while choosing and
//! persisting a slot. Session metadata is the only source of occupied numbers.

use super::types::{SessionMetadata, SessionStatus};
use std::collections::HashSet;

pub(super) const METADATA_KEY: &str = "workspaceSessionNumber";
const MAX_NUMBER: u64 = 9_007_199_254_740_991;

pub(super) fn workspace_key(metadata: &SessionMetadata) -> String {
    let raw_path = metadata
        .project_workspace_path
        .as_deref()
        .or(metadata.workspace_path.as_deref())
        .unwrap_or("")
        .trim();
    let path = raw_path.replace('\\', "/");
    let path = path.trim_end_matches('/');
    // Remote paths are case-sensitive POSIX paths even on a Windows controller.
    let local_windows_path = path.as_bytes().get(1) == Some(&b':') || raw_path.starts_with("\\\\");
    if local_windows_path {
        path.to_lowercase()
    } else {
        path.to_owned()
    }
}

pub(super) fn number(metadata: &SessionMetadata) -> Option<u64> {
    metadata
        .custom_metadata
        .as_ref()?
        .get(METADATA_KEY)?
        .as_u64()
        .filter(|number| *number > 0 && *number <= MAX_NUMBER)
}

/// Only an explicit default descriptor occupies a slot. Literal and legacy
/// titles are never inferred from their text or assigned a display number.
pub(super) fn occupies_slot(metadata: &SessionMetadata) -> bool {
    let Some(custom) = metadata.custom_metadata.as_ref() else {
        return false;
    };
    metadata.status != SessionStatus::Archived
        && metadata.turn_count == 0
        && !metadata.should_hide_from_user_lists()
        && !metadata.relationship.as_ref().is_some_and(|relationship| {
            relationship.kind.is_some() || relationship.parent_session_id.is_some()
        })
        && custom
            .get("parentSessionId")
            .and_then(|value| value.as_str())
            .is_none()
        && custom.get("titleSource").and_then(|value| value.as_str()) == Some("i18n")
        && custom.get("titleKey").and_then(|value| value.as_str()) == Some("flow-chat:session.new")
        && custom
            .get("titleParams")
            .and_then(|params| params.get("defaultTitleText"))
            .and_then(|value| value.as_str())
            == Some(metadata.session_name.as_str())
}

pub(super) fn next_available_number(
    target: &SessionMetadata,
    sessions: &[SessionMetadata],
) -> Option<u64> {
    let workspace = workspace_key(target);
    let occupied: HashSet<_> = sessions
        .iter()
        .filter(|metadata| {
            metadata.session_id != target.session_id
                && occupies_slot(metadata)
                && workspace_key(metadata) == workspace
        })
        .filter_map(number)
        .collect();
    (1..=MAX_NUMBER).find(|number| !occupied.contains(number))
}
