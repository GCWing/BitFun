use bitfun_core_types::SessionKind;
use bitfun_services_core::session::{
    build_session_metadata_page, SessionMetadata, SessionRelationship, SessionRelationshipKind,
    SessionStatus,
};

fn metadata(session_id: &str, last_active_at: u64) -> SessionMetadata {
    let mut metadata = SessionMetadata::new(
        session_id.to_string(),
        session_id.to_string(),
        "agent".to_string(),
        "model".to_string(),
    );
    metadata.last_active_at = last_active_at;
    metadata
}

#[test]
fn session_page_keeps_visible_top_level_order_and_attaches_children() {
    let mut parent_older = metadata("parent-older", 1_000);
    let parent_latest = metadata("parent-latest", 2_000);
    let mut child = metadata("child-latest", 3_000);
    child.relationship = Some(SessionRelationship {
        kind: Some(SessionRelationshipKind::Btw),
        parent_session_id: Some("parent-latest".to_string()),
        ..Default::default()
    });
    let mut archived = metadata("archived", 4_000);
    archived.status = SessionStatus::Archived;
    parent_older.session_kind = SessionKind::Standard;

    let page =
        build_session_metadata_page(vec![archived, child, parent_latest, parent_older], None, 1);
    let session_ids = page
        .sessions
        .iter()
        .map(|metadata| metadata.session_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(page.total_top_level_count, 2);
    assert_eq!(page.loaded_top_level_count, 1);
    assert!(page.has_more);
    assert_eq!(session_ids, vec!["parent-latest", "child-latest"]);
}

#[test]
fn session_page_cursor_loads_next_top_level_window() {
    let page = build_session_metadata_page(
        vec![
            metadata("parent-3", 3_000),
            metadata("parent-2", 2_000),
            metadata("parent-1", 1_000),
        ],
        None,
        1,
    );

    let second_page = build_session_metadata_page(
        vec![
            metadata("parent-3", 3_000),
            metadata("parent-2", 2_000),
            metadata("parent-1", 1_000),
        ],
        page.next_cursor.as_deref(),
        1,
    );

    assert_eq!(second_page.loaded_top_level_count, 1);
    assert_eq!(second_page.sessions[0].session_id, "parent-2");
}
