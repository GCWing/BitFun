use bitfun_core::service::config::AIConfig;

/// Resolve the shared future-mode selector to the concrete enabled model shown
/// by CLI model pickers and status surfaces.
pub(crate) fn resolve_mode_model_id(ai_config: &AIConfig) -> Option<String> {
    let selector = ai_config.agent_model_defaults.mode.trim();
    match selector {
        "" | "auto" | "default" => ai_config.resolve_model_selection("primary"),
        selector => ai_config.resolve_model_selection(selector),
    }
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
}
