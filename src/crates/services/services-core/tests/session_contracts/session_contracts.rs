use bitfun_services_core::session::{DialogTurnKind, SessionKind, SessionMetadata};

#[test]
fn session_metadata_preserves_subagent_visibility_contract() {
    let mut metadata = SessionMetadata::new(
        "session-1".to_string(),
        "Subagent: inspect".to_string(),
        "Explore".to_string(),
        "model".to_string(),
    );
    metadata.session_kind = SessionKind::Subagent;

    assert!(metadata.is_subagent());
    assert!(metadata.should_hide_from_user_lists());
}

#[test]
fn session_metadata_hides_ephemeral_child_sessions_from_user_lists() {
    let mut metadata = SessionMetadata::new(
        "session-ephemeral".to_string(),
        "Side thread".to_string(),
        "agentic".to_string(),
        "model".to_string(),
    );
    metadata.session_kind = SessionKind::EphemeralChild;

    assert!(!metadata.is_subagent());
    assert!(metadata.is_internal_hidden());
    assert!(metadata.should_hide_from_user_lists());
}

#[test]
fn dialog_turn_kind_preserves_default_visibility_contract() {
    assert_eq!(DialogTurnKind::default(), DialogTurnKind::UserDialog);
    assert!(DialogTurnKind::UserDialog.is_model_visible());
    assert!(!DialogTurnKind::ManualCompaction.is_model_visible());
    assert!(!DialogTurnKind::LocalCommand.is_model_visible());
}
