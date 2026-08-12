use bitfun_ai_adapters::{
    classify_model_selector, resolve_cache_model_selector, resolve_required_model_selector,
    ModelSelectorError, ModelSelectorKind,
};

fn resolve_selection(selector: &str) -> Option<String> {
    match selector {
        "primary" => Some("model-primary".to_string()),
        "fast" => Some("model-fast".to_string()),
        _ => None,
    }
}

fn resolve_reference(model_ref: &str) -> Option<String> {
    match model_ref {
        "Primary Chat" | "claude-sonnet-4.5" => Some("model-primary".to_string()),
        _ => None,
    }
}

#[test]
fn classifies_auto_default_and_empty_as_primary() {
    assert_eq!(classify_model_selector("auto"), ModelSelectorKind::Primary);
    assert_eq!(
        classify_model_selector(" default "),
        ModelSelectorKind::Primary
    );
    assert_eq!(classify_model_selector(""), ModelSelectorKind::Primary);
    assert_eq!(
        classify_model_selector("model-primary"),
        ModelSelectorKind::Explicit("model-primary".to_string())
    );
}

#[test]
fn required_selector_resolves_defaults_and_references() {
    assert_eq!(
        resolve_required_model_selector("primary", resolve_selection, resolve_reference)
            .expect("primary should resolve"),
        "model-primary"
    );
    assert_eq!(
        resolve_required_model_selector("fast", resolve_selection, resolve_reference)
            .expect("fast should resolve"),
        "model-fast"
    );
    assert_eq!(
        resolve_required_model_selector("Primary Chat", resolve_selection, resolve_reference)
            .expect("named model should resolve"),
        "model-primary"
    );
    assert_eq!(
        resolve_required_model_selector("literal-model-id", resolve_selection, resolve_reference)
            .expect("literal selector should pass through"),
        "literal-model-id"
    );
}

#[test]
fn required_selector_preserves_current_missing_default_errors() {
    let missing = |_selector: &str| None;

    assert_eq!(
        resolve_required_model_selector("primary", missing, resolve_reference),
        Err(ModelSelectorError::PrimaryUnavailable)
    );
    assert_eq!(
        resolve_required_model_selector("fast", missing, resolve_reference),
        Err(ModelSelectorError::FastUnavailable)
    );
    assert_eq!(
        ModelSelectorError::FastUnavailable.to_string(),
        "Fast model not configured or invalid, and primary model not configured or invalid"
    );
}

#[test]
fn cache_selector_keeps_legacy_default_passthrough_when_unresolved() {
    let missing = |_selector: &str| None;

    assert_eq!(
        resolve_cache_model_selector("primary", missing, resolve_reference),
        "primary"
    );
    assert_eq!(
        resolve_cache_model_selector("fast", missing, resolve_reference),
        "fast"
    );
    assert_eq!(
        resolve_cache_model_selector("claude-sonnet-4.5", resolve_selection, resolve_reference),
        "model-primary"
    );
}
