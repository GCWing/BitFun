use async_trait::async_trait;
use bitfun_agent_runtime::runtime::AgentRuntimeBuilder;
use bitfun_product_capabilities::{
    product_assembly_plan_for_profile, DeliveryProfile, ProductAssembler, ProductAssemblyError,
    ProductAssemblyInput,
};
use bitfun_runtime_ports::{
    PluginDispatchEnvelope, PluginResponseEnvelope, PluginRuntimeAvailability,
    PluginRuntimeBinding, PluginRuntimeClient, PluginRuntimeUnavailableReason, PortResult,
};
use bitfun_runtime_services::test_support::FakeRuntimeServicesProvider;
use bitfun_runtime_services::{
    RuntimeServiceMarkerPort, RuntimeServicesBuilder, RuntimeServicesProvider,
};
use std::sync::{Arc, Mutex};

struct AvailablePluginRuntimeClient;

#[async_trait]
impl PluginRuntimeClient for AvailablePluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::Available
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id,
            project_domain_id: envelope.project_domain_id,
            workspace_id: envelope.workspace_id,
            adapter_id: "shape-test-host".to_string(),
            plugin_id: Some(envelope.source.plugin_id),
            completed_at_ms: 0,
            effects: Vec::new(),
            diagnostics: Vec::new(),
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs,
        })
    }
}

#[derive(Debug, Default)]
struct ProductShapeSubmissionPort {
    submitted_turns: Mutex<Vec<bitfun_runtime_ports::AgentSubmissionRequest>>,
}

#[async_trait]
impl bitfun_runtime_ports::AgentSubmissionPort for ProductShapeSubmissionPort {
    async fn create_session(
        &self,
        request: bitfun_runtime_ports::AgentSessionCreateRequest,
    ) -> PortResult<bitfun_runtime_ports::AgentSessionCreateResult> {
        Ok(bitfun_runtime_ports::AgentSessionCreateResult {
            session_id: "product-shape-session".to_string(),
            session_name: request.session_name,
            agent_type: request.agent_type,
        })
    }

    async fn submit_message(
        &self,
        request: bitfun_runtime_ports::AgentSubmissionRequest,
    ) -> PortResult<bitfun_runtime_ports::AgentSubmissionResult> {
        self.submitted_turns.lock().unwrap().push(request.clone());
        Ok(bitfun_runtime_ports::AgentSubmissionResult {
            turn_id: request
                .turn_id
                .clone()
                .unwrap_or_else(|| "product-shape-turn".to_string()),
            accepted: true,
        })
    }

    async fn resolve_session_agent_type(&self, _session_id: &str) -> PortResult<Option<String>> {
        Ok(Some("agentic".to_string()))
    }
}

fn product_full_services() -> bitfun_runtime_services::RuntimeServices {
    FakeRuntimeServicesProvider::with_all_required()
        .register(RuntimeServicesBuilder::new())
        .with_optional_terminal(Some(FakeRuntimeServicesProvider::terminal_port()))
        .with_optional_git(Some(RuntimeServiceMarkerPort::git_port()))
        .with_optional_network(Some(RuntimeServiceMarkerPort::network_port()))
        .build()
        .expect("product-full services should build")
}

fn baseline_services() -> bitfun_runtime_services::RuntimeServices {
    FakeRuntimeServicesProvider::with_all_required()
        .build_services()
        .expect("baseline services should build")
}

#[test]
fn p0_plugin_host_is_executable_only_for_product_full_desktop_and_cli() {
    for profile in [
        DeliveryProfile::ProductFull,
        DeliveryProfile::Desktop,
        DeliveryProfile::Cli,
    ] {
        let parts = ProductAssembler::new()
            .assemble(
                ProductAssemblyInput::new(profile, product_full_services()).with_plugin_runtime(
                    PluginRuntimeBinding::client(Arc::new(AvailablePluginRuntimeClient)),
                ),
            )
            .expect("P0 host-capable profiles should accept an executable host binding");

        assert_eq!(parts.plan().profile(), profile);
        assert_eq!(
            parts.plugin_runtime().availability(),
            PluginRuntimeAvailability::Available
        );
        assert_eq!(
            parts.plan().extension_capabilities().plugin_runtime(),
            PluginRuntimeAvailability::Available
        );
    }
}

#[test]
fn p0_plugin_host_binding_builds_agent_runtime_parts() {
    let parts = ProductAssembler::new()
        .assemble(
            ProductAssemblyInput::new(DeliveryProfile::Cli, product_full_services())
                .with_plugin_runtime(PluginRuntimeBinding::client(Arc::new(
                    AvailablePluginRuntimeClient,
                ))),
        )
        .expect("CLI should accept explicit executable host binding");

    let (services, harness_registry, plugin_runtime) = parts.into_runtime_parts();
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(Arc::new(ProductShapeSubmissionPort::default()))
        .with_services(services)
        .with_harness_registry(Arc::new(harness_registry))
        .with_plugin_runtime(plugin_runtime)
        .build()
        .expect("P0 host-capable assembly parts should build an agent runtime");

    assert_eq!(
        runtime.plugin_runtime().availability(),
        PluginRuntimeAvailability::Available
    );
}

#[test]
fn non_p0_surfaces_cannot_inherit_executable_plugin_host() {
    for profile in [
        DeliveryProfile::Server,
        DeliveryProfile::Remote,
        DeliveryProfile::Acp,
        DeliveryProfile::Web,
        DeliveryProfile::MobileWeb,
        DeliveryProfile::Sdk,
    ] {
        let services = if matches!(profile, DeliveryProfile::Acp | DeliveryProfile::Sdk) {
            product_full_services()
        } else {
            baseline_services()
        };
        let error = ProductAssembler::new()
            .assemble(
                ProductAssemblyInput::new(profile, services).with_plugin_runtime(
                    PluginRuntimeBinding::client(Arc::new(AvailablePluginRuntimeClient)),
                ),
            )
            .expect_err("non-P0 surfaces must not silently inherit executable plugin host");

        assert_eq!(
            error,
            ProductAssemblyError::UnsupportedPluginRuntime {
                profile,
                availability: PluginRuntimeAvailability::Available
            }
        );
    }
}

#[test]
fn default_product_shapes_expose_only_disabled_plugin_availability() {
    for profile in DeliveryProfile::all_current_product_profiles() {
        let availability = product_assembly_plan_for_profile(*profile)
            .extension_capabilities()
            .plugin_runtime();

        assert!(
            !availability.is_executable(),
            "{profile} must not imply executable plugin support without a host binding"
        );
        let expected_reason = if matches!(
            profile,
            DeliveryProfile::ProductFull | DeliveryProfile::Desktop | DeliveryProfile::Cli
        ) {
            PluginRuntimeUnavailableReason::NotBuilt
        } else {
            PluginRuntimeUnavailableReason::UnsupportedProfile
        };
        assert_eq!(
            availability,
            PluginRuntimeAvailability::Disabled {
                reason: expected_reason
            }
        );
    }
}

#[test]
fn default_assembled_product_shapes_keep_profile_specific_plugin_availability() {
    for profile in DeliveryProfile::all_current_product_profiles() {
        let services = if matches!(
            profile,
            DeliveryProfile::ProductFull
                | DeliveryProfile::Desktop
                | DeliveryProfile::Cli
                | DeliveryProfile::Acp
                | DeliveryProfile::Sdk
        ) {
            product_full_services()
        } else {
            baseline_services()
        };
        let parts = ProductAssembler::new()
            .assemble(ProductAssemblyInput::new(*profile, services))
            .expect("profile should assemble with matching baseline services");
        let expected_reason = if matches!(
            profile,
            DeliveryProfile::ProductFull | DeliveryProfile::Desktop | DeliveryProfile::Cli
        ) {
            PluginRuntimeUnavailableReason::NotBuilt
        } else {
            PluginRuntimeUnavailableReason::UnsupportedProfile
        };

        assert_eq!(
            parts.plugin_runtime().availability(),
            PluginRuntimeAvailability::Disabled {
                reason: expected_reason
            },
            "{profile} assembled parts must keep profile-specific default plugin availability"
        );
        assert_eq!(
            parts.plan().extension_capabilities().plugin_runtime(),
            PluginRuntimeAvailability::Disabled {
                reason: expected_reason
            },
            "{profile} assembly plan must keep profile-specific default plugin availability"
        );
    }
}
