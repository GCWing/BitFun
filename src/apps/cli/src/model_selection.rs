use bitfun_core::service::config::AIConfig;

fn resolve_model_selector(ai_config: &AIConfig, selector: &str) -> Option<String> {
    match selector.trim() {
        "" | "auto" | "default" => ai_config.resolve_model_selection("primary"),
        selector => ai_config.resolve_model_selection(selector),
    }
}

/// Resolve the shared future-mode selector to the concrete enabled model shown
/// by CLI model pickers and status surfaces.
pub(crate) fn resolve_mode_model_id(ai_config: &AIConfig) -> Option<String> {
    resolve_model_selector(ai_config, &ai_config.agent_model_defaults.mode)
}

/// Resolve the Runtime-owned Session selector to the concrete catalog model
/// used by CLI display surfaces. A missing selector is limited to the fresh
/// Session fallback; it does not become Session authority in the Client.
pub(crate) fn resolve_session_model_display_id(
    ai_config: &AIConfig,
    session_selector: Option<&str>,
) -> Option<String> {
    let selector = session_selector
        .map(str::trim)
        .filter(|selector| !selector.is_empty())
        .unwrap_or(ai_config.agent_model_defaults.mode.as_str());
    resolve_model_selector(ai_config, selector)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_selector(selector: &str) -> AIConfig {
        serde_json::from_value(serde_json::json!({
            "models": [
                {
                    "id": "primary-model",
                    "name": "Primary",
                    "provider": "openai",
                    "model_name": "primary-model",
                    "enabled": true
                },
                {
                    "id": "fast-model",
                    "name": "Fast",
                    "provider": "openai",
                    "model_name": "fast-model",
                    "enabled": true
                },
                {
                    "id": "explicit-model",
                    "name": "Explicit",
                    "provider": "openai",
                    "model_name": "explicit-model",
                    "enabled": true
                }
            ],
            "default_models": {
                "primary": "primary-model",
                "fast": "fast-model"
            },
            "agent_model_defaults": {
                "mode": selector
            }
        }))
        .expect("test AI config should deserialize")
    }

    #[test]
    fn resolves_symbolic_and_explicit_mode_defaults_for_cli_display() {
        assert_eq!(
            resolve_mode_model_id(&config_with_selector("auto")).as_deref(),
            Some("primary-model")
        );
        assert_eq!(
            resolve_mode_model_id(&config_with_selector("fast")).as_deref(),
            Some("fast-model")
        );
        assert_eq!(
            resolve_mode_model_id(&config_with_selector("explicit-model")).as_deref(),
            Some("explicit-model")
        );
    }

    #[test]
    fn resolves_runtime_session_selectors_to_the_effective_catalog_model() {
        let config = config_with_selector("fast");

        assert_eq!(
            resolve_session_model_display_id(&config, Some("auto")).as_deref(),
            Some("primary-model")
        );
        assert_eq!(
            resolve_session_model_display_id(&config, Some("primary")).as_deref(),
            Some("primary-model")
        );
        assert_eq!(
            resolve_session_model_display_id(&config, Some("fast")).as_deref(),
            Some("fast-model")
        );
        assert_eq!(
            resolve_session_model_display_id(&config, Some("explicit-model")).as_deref(),
            Some("explicit-model")
        );
    }

    #[test]
    fn missing_runtime_session_selector_uses_the_future_session_default_for_display_only() {
        let config = config_with_selector("fast");

        assert_eq!(
            resolve_session_model_display_id(&config, None).as_deref(),
            Some("fast-model")
        );
    }
}
