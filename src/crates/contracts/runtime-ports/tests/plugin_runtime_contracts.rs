use bitfun_runtime_ports::{
    PluginDispatchEnvelope, PluginRuntimeAvailability, PluginRuntimeBinding,
    PluginRuntimeUnavailableReason, PortErrorKind,
};

fn envelope(id: &str) -> PluginDispatchEnvelope {
    PluginDispatchEnvelope {
        envelope_id: id.to_string(),
        event_name: "agent.turn.completed".to_string(),
        payload: serde_json::json!({ "turnId": "turn-1" }),
    }
}

#[tokio::test]
async fn disabled_plugin_runtime_binding_reports_not_available() {
    let binding = PluginRuntimeBinding::disabled(PluginRuntimeUnavailableReason::NotBuilt);

    assert_eq!(
        binding.availability(),
        PluginRuntimeAvailability::Disabled {
            reason: PluginRuntimeUnavailableReason::NotBuilt
        }
    );

    let error = binding
        .as_client()
        .dispatch(envelope("dispatch-1"))
        .await
        .expect_err("disabled binding must not accept plugin dispatches");

    assert_eq!(error.kind, PortErrorKind::NotAvailable);
    assert!(error.message.contains("plugin runtime is disabled"));
}

#[tokio::test]
async fn projection_only_plugin_runtime_rejects_dispatch_without_host() {
    let binding =
        PluginRuntimeBinding::projection_only(PluginRuntimeUnavailableReason::UnsupportedProfile);

    assert_eq!(
        binding.availability(),
        PluginRuntimeAvailability::ProjectionOnly {
            reason: PluginRuntimeUnavailableReason::UnsupportedProfile
        }
    );

    let error = binding
        .as_client()
        .dispatch(envelope("dispatch-2"))
        .await
        .expect_err("projection-only binding must not pretend to deliver plugin dispatches");

    assert_eq!(error.kind, PortErrorKind::NotAvailable);
    assert!(error.message.contains("projection-only"));
}
