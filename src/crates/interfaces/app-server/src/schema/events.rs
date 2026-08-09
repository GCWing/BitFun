use agent_client_protocol::JsonRpcNotification;
use serde::{Deserialize, Serialize};

pub use bitfun_app_server_protocol::event::{
    AgentEventNotification as SessionEventNotification, ConfigEventNotification, ConfigUpdate,
    EventCursor, EventStream, EventStreamState, EventStreamStateNotification, ResyncDirective,
    SyncEventsRequest, SyncEventsResponse,
};

/// Browser-facing projected runtime or permission event.
#[derive(Debug, Clone, Serialize, Deserialize, JsonRpcNotification)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[notification(method = "agent/frontendEvent")]
pub struct FrontendEventNotification {
    /// Frontend event name, such as `agentic://session-created`.
    pub event: String,
    /// Projected payload in the frontend's expected shape.
    pub payload: serde_json::Value,
}

pub(crate) fn config_update_from_owner(
    value: bitfun_core::service::config::ConfigUpdateEvent,
) -> ConfigUpdate {
    use bitfun_core::service::config::ConfigUpdateEvent;

    match value {
        ConfigUpdateEvent::ModelConfigurationUpdated => ConfigUpdate::ModelConfigurationUpdated,
        ConfigUpdateEvent::AIModelUpdated {
            model_id,
            model_name,
        } => ConfigUpdate::AiModelUpdated {
            model_id,
            model_name,
        },
        ConfigUpdateEvent::DefaultAIModelUpdated {
            model_id,
            model_name,
        } => ConfigUpdate::DefaultAiModelUpdated {
            model_id,
            model_name,
        },
        ConfigUpdateEvent::AppearanceUpdated { appearance_id } => {
            ConfigUpdate::AppearanceUpdated { appearance_id }
        }
        ConfigUpdateEvent::EditorUpdated => ConfigUpdate::EditorUpdated,
        ConfigUpdateEvent::TerminalUpdated => ConfigUpdate::TerminalUpdated,
        ConfigUpdateEvent::WorkspaceUpdated => ConfigUpdate::WorkspaceUpdated,
        ConfigUpdateEvent::AppUpdated => ConfigUpdate::AppUpdated,
        ConfigUpdateEvent::ConfigReloaded => ConfigUpdate::ConfigReloaded,
        ConfigUpdateEvent::ReasoningCatalogUpdated => ConfigUpdate::ReasoningCatalogUpdated,
        ConfigUpdateEvent::DebugModeConfigUpdated {
            new_port,
            new_log_path,
        } => ConfigUpdate::DebugModeConfigUpdated {
            new_port,
            new_log_path,
        },
        ConfigUpdateEvent::LogLevelUpdated { new_level } => {
            ConfigUpdate::LogLevelUpdated { new_level }
        }
        ConfigUpdateEvent::LoggingSensitiveDiagnosticsUpdated {
            include_sensitive_diagnostics,
        } => ConfigUpdate::LoggingSensitiveDiagnosticsUpdated {
            include_sensitive_diagnostics,
        },
        ConfigUpdateEvent::ModelsReconciled {
            invalidated_model_ids,
            default_models_changed,
            func_agent_models_changed,
            agent_model_defaults_changed,
        } => ConfigUpdate::ModelsReconciled {
            invalidated_model_ids,
            default_models_changed,
            func_agent_models_changed,
            agent_model_defaults_changed,
        },
    }
}

#[cfg(test)]
mod tests {
    use bitfun_core::service::config::ConfigUpdateEvent;
    use serde_json::json;

    #[test]
    fn models_reconciled_preserves_all_owner_facts() {
        let update = super::config_update_from_owner(ConfigUpdateEvent::ModelsReconciled {
            invalidated_model_ids: vec!["model-1".to_string()],
            default_models_changed: true,
            func_agent_models_changed: false,
            agent_model_defaults_changed: true,
        });

        assert_eq!(
            serde_json::to_value(update).expect("config update should serialize"),
            json!({
                "kind": "modelsReconciled",
                "invalidatedModelIds": ["model-1"],
                "defaultModelsChanged": true,
                "funcAgentModelsChanged": false,
                "agentModelDefaultsChanged": true
            })
        );
    }

    #[test]
    fn reasoning_catalog_update_is_projected() {
        let update = super::config_update_from_owner(ConfigUpdateEvent::ReasoningCatalogUpdated);

        assert_eq!(
            serde_json::to_value(update).expect("config update should serialize"),
            json!({ "kind": "reasoningCatalogUpdated" })
        );
    }
}

#[cfg(all(test, feature = "ts"))]
mod ts_exports {
    use super::ConfigUpdate;
    use ts_rs::{Config, TS};

    #[test]
    fn export_upstream_config_update() {
        ConfigUpdate::export(&Config::from_env())
            .expect("ConfigUpdate TypeScript export should succeed");
    }
}
