//! Session metadata pagination and cursor shaping.

use super::types::{SessionMetadata, SessionStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadataPage {
    pub sessions: Vec<SessionMetadata>,
    pub total_top_level_count: usize,
    pub loaded_top_level_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionMetadataPageCursor {
    last_active_at: u64,
    session_id: String,
}

pub fn empty_session_metadata_page() -> SessionMetadataPage {
    SessionMetadataPage {
        sessions: Vec::new(),
        total_top_level_count: 0,
        loaded_top_level_count: 0,
        next_cursor: None,
        has_more: false,
    }
}

pub fn build_session_metadata_page(
    indexed_sessions: Vec<SessionMetadata>,
    cursor: Option<&str>,
    limit: usize,
) -> SessionMetadataPage {
    build_session_metadata_page_with_options(indexed_sessions, cursor, limit, false)
}

/// Paginated session metadata builder. With `include_hidden`, sessions hidden
/// from user lists (Subagent/Ephemeral) participate in pagination for full
/// conversation management.
pub fn build_session_metadata_page_with_options(
    indexed_sessions: Vec<SessionMetadata>,
    cursor: Option<&str>,
    limit: usize,
    include_hidden: bool,
) -> SessionMetadataPage {
    let visible_sessions = indexed_sessions
        .into_iter()
        .filter(|metadata| {
            (include_hidden || !metadata.should_hide_from_user_lists())
                && metadata.status != SessionStatus::Archived
        })
        .collect::<Vec<_>>();
    let visible_ids = visible_sessions
        .iter()
        .map(|metadata| metadata.session_id.clone())
        .collect::<HashSet<_>>();

    let mut top_level_sessions = Vec::new();
    let mut children_by_parent: HashMap<String, Vec<SessionMetadata>> = HashMap::new();
    let mut orphan_ids: HashSet<String> = HashSet::new();
    let mut orphan_kinds: HashMap<String, String> = HashMap::new();
    for metadata in visible_sessions {
        if let Some(parent_id) = session_parent_id(&metadata) {
            if visible_ids.contains(&parent_id) {
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(metadata);
                continue;
            }
            // R-AD-08: the parent is missing from the visible set. Promote to
            // a top-level row but carry the orphan marker so the frontend can
            // group it under the orphan section instead of presenting it as a
            // normal root (mirrors the SessionControl tree `orphaned` marker).
            orphan_ids.insert(metadata.session_id.clone());
            orphan_kinds.insert(metadata.session_id.clone(), "DanglingChild".to_string());
        }
        top_level_sessions.push(metadata);
    }

    // DetachedChild: sessions with a `session-{parent}` creator marker but no
    // relationship whose parent is also missing. Conservative — only marker
    // creators are treated as lineage facts (same rule as the GC classifier).
    for metadata in &top_level_sessions {
        if orphan_ids.contains(&metadata.session_id) {
            continue;
        }
        if metadata.relationship.is_some() {
            continue;
        }
        let Some(creator) = metadata.created_by.as_deref() else {
            continue;
        };
        let Some(parent_id) = creator.strip_prefix("session-") else {
            continue;
        };
        let parent_id = parent_id.trim();
        if parent_id.is_empty() || parent_id == metadata.session_id {
            continue;
        }
        if !visible_ids.contains(parent_id) {
            orphan_ids.insert(metadata.session_id.clone());
            orphan_kinds.insert(metadata.session_id.clone(), "DetachedChild".to_string());
        }
    }

    for metadata in top_level_sessions.iter_mut() {
        if orphan_ids.contains(&metadata.session_id) {
            metadata.orphaned = true;
            metadata.orphan_kind = orphan_kinds.get(&metadata.session_id).cloned();
        }
    }

    let total_top_level_count = top_level_sessions.len();
    let limit = limit.max(1);
    let offset = session_metadata_page_offset(cursor, &top_level_sessions);
    let offset = offset.min(total_top_level_count);
    let next_offset = offset.saturating_add(limit).min(total_top_level_count);
    let selected_top_level = top_level_sessions
        .iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let loaded_top_level_count = selected_top_level.len();
    let has_more = next_offset < total_top_level_count;
    let next_cursor = has_more
        .then(|| selected_top_level.last().map(session_metadata_page_cursor))
        .flatten();

    let mut sessions = Vec::new();
    for metadata in selected_top_level {
        let session_id = metadata.session_id.clone();
        sessions.push(metadata);

        if let Some(mut children) = children_by_parent.remove(&session_id) {
            children.sort_by_key(|metadata| std::cmp::Reverse(metadata.last_active_at));
            sessions.extend(children);
        }
    }

    SessionMetadataPage {
        sessions,
        total_top_level_count,
        loaded_top_level_count,
        next_cursor,
        has_more,
    }
}

fn session_parent_id(metadata: &SessionMetadata) -> Option<String> {
    if let Some(parent_id) = metadata
        .relationship
        .as_ref()
        .and_then(|relationship| relationship.parent_session_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(parent_id.to_string());
    }

    metadata
        .custom_metadata
        .as_ref()
        .and_then(|custom| {
            custom
                .get("parentSessionId")
                .or_else(|| custom.get("parent_session_id"))
        })
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn session_metadata_page_offset(
    cursor: Option<&str>,
    top_level_sessions: &[SessionMetadata],
) -> usize {
    let Some(cursor) = cursor else {
        return 0;
    };

    if let Ok(parsed) = serde_json::from_str::<SessionMetadataPageCursor>(cursor) {
        if let Some(index) = top_level_sessions.iter().position(|metadata| {
            metadata.session_id == parsed.session_id
                && metadata.last_active_at == parsed.last_active_at
        }) {
            return index + 1;
        }

        if let Some(index) = top_level_sessions
            .iter()
            .position(|metadata| metadata.session_id == parsed.session_id)
        {
            return index + 1;
        }
    }

    cursor.parse::<usize>().unwrap_or(0)
}

fn session_metadata_page_cursor(metadata: &SessionMetadata) -> String {
    serde_json::to_string(&SessionMetadataPageCursor {
        last_active_at: metadata.last_active_at,
        session_id: metadata.session_id.clone(),
    })
    .unwrap_or_else(|_| metadata.session_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::types::{SessionRelationship, SessionRelationshipKind};

    fn metadata(id: &str, created_by: Option<&str>, parent: Option<&str>) -> SessionMetadata {
        let mut m = SessionMetadata::new(
            id.to_string(),
            format!("Session {}", id),
            "agentic".to_string(),
            "model".to_string(),
        );
        m.created_by = created_by.map(str::to_string);
        m.relationship = parent.map(|pid| SessionRelationship {
            kind: Some(SessionRelationshipKind::Subagent),
            parent_session_id: Some(pid.to_string()),
            ..Default::default()
        });
        m
    }

    #[test]
    fn dangling_child_is_marked_orphaned() {
        let page = build_session_metadata_page(
            vec![metadata("child-1", None, Some("ghost-parent"))],
            None,
            10,
        );
        assert_eq!(page.total_top_level_count, 1);
        let session = &page.sessions[0];
        assert!(session.orphaned);
        assert_eq!(session.orphan_kind.as_deref(), Some("DanglingChild"));
    }

    #[test]
    fn child_with_live_parent_is_not_orphaned() {
        let page = build_session_metadata_page(
            vec![
                metadata("parent-1", None, None),
                metadata("child-1", None, Some("parent-1")),
            ],
            None,
            10,
        );
        assert_eq!(page.total_top_level_count, 1);
        assert_eq!(page.sessions.len(), 2);
        assert!(!page.sessions[0].orphaned);
        assert!(!page.sessions[1].orphaned);
    }

    #[test]
    fn detached_child_with_missing_creator_parent_is_marked_orphaned() {
        let page = build_session_metadata_page(
            vec![metadata("detached-1", Some("session-ghost"), None)],
            None,
            10,
        );
        let session = &page.sessions[0];
        assert!(session.orphaned);
        assert_eq!(session.orphan_kind.as_deref(), Some("DetachedChild"));
    }

    #[test]
    fn non_marker_creator_is_never_orphaned() {
        let page =
            build_session_metadata_page(vec![metadata("user-1", Some("alice"), None)], None, 10);
        assert!(!page.sessions[0].orphaned);
        assert_eq!(page.sessions[0].orphan_kind, None);
    }

    #[test]
    fn orphan_marker_does_not_break_pagination() {
        // 20 sessions (last is an orphan): a page of 5 must still page and the
        // orphan marker must survive across pages.
        let mut sessions = (0..19)
            .map(|i| metadata(&format!("root-{}", i), None, None))
            .collect::<Vec<_>>();
        sessions.push(metadata("orphan-1", None, Some("ghost")));
        let page = build_session_metadata_page(sessions, None, 20);
        assert_eq!(page.total_top_level_count, 20);
        let orphan = page
            .sessions
            .iter()
            .find(|m| m.session_id == "orphan-1")
            .expect("orphan present");
        assert!(orphan.orphaned);
        assert_eq!(orphan.orphan_kind.as_deref(), Some("DanglingChild"));
    }
}
