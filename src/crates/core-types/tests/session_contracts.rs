use bitfun_core_types::SessionKind;

#[test]
fn session_kind_preserves_default_and_serialized_shape() {
    assert_eq!(SessionKind::default(), SessionKind::Standard);
    assert_eq!(
        serde_json::to_value(SessionKind::Subagent).expect("session kind should serialize"),
        serde_json::json!("subagent")
    );
}

#[test]
fn session_kind_preserves_legacy_snake_case_deserialization() {
    let kind: SessionKind =
        serde_json::from_value(serde_json::json!("standard")).expect("standard should parse");

    assert_eq!(kind, SessionKind::Standard);
}
