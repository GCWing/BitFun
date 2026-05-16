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
fn dialog_turn_kind_preserves_default_visibility_contract() {
    assert_eq!(DialogTurnKind::default(), DialogTurnKind::UserDialog);
    assert!(DialogTurnKind::UserDialog.is_model_visible());
    assert!(!DialogTurnKind::ManualCompaction.is_model_visible());
    assert!(!DialogTurnKind::LocalCommand.is_model_visible());
}
