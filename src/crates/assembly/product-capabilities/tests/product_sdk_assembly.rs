use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bitfun_agent_runtime::runtime::AgentRuntimeBuilder;
use bitfun_agent_runtime::sdk::{
    AgentEventStream, AgentRunRequest, AgentSessionCreateRequest, AgentSessionCreateResult,
    AgentSubmissionPort, AgentSubmissionRequest, AgentSubmissionResult, AgentSubmissionSource,
    PortResult, RuntimeServiceCapability, SessionSelector,
};
use bitfun_product_capabilities::{DeliveryProfile, ProductAssembler, ProductAssemblyInput};
use bitfun_runtime_services::test_support::FakeRuntimeServicesProvider;
use bitfun_runtime_services::{
    RuntimeServiceMarkerPort, RuntimeServices, RuntimeServicesBuilder, RuntimeServicesProvider,
};

#[derive(Debug, Default)]
struct ProductSdkAgentProvider {
    created_sessions: Mutex<Vec<AgentSessionCreateRequest>>,
    submitted_turns: Mutex<Vec<AgentSubmissionRequest>>,
}

#[async_trait]
impl AgentSubmissionPort for ProductSdkAgentProvider {
    async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        self.created_sessions.lock().unwrap().push(request.clone());
        Ok(AgentSessionCreateResult {
            session_id: "product-sdk-session".to_string(),
            session_name: request.session_name,
            agent_type: request.agent_type,
        })
    }

    async fn submit_message(
        &self,
        request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult> {
        self.submitted_turns.lock().unwrap().push(request.clone());
        Ok(AgentSubmissionResult {
            turn_id: request
                .turn_id
                .clone()
                .unwrap_or_else(|| "product-sdk-turn".to_string()),
            accepted: true,
        })
    }

    async fn resolve_session_agent_type(&self, _session_id: &str) -> PortResult<Option<String>> {
        Ok(Some("agentic".to_string()))
    }
}

fn product_full_compatible_services() -> RuntimeServices {
    FakeRuntimeServicesProvider::with_all_required()
        .register(RuntimeServicesBuilder::new())
        .with_optional_terminal(Some(FakeRuntimeServicesProvider::terminal_port()))
        .with_optional_git(Some(RuntimeServiceMarkerPort::git_port()))
        .with_optional_network(Some(RuntimeServiceMarkerPort::network_port()))
        .build()
        .expect("product-full compatible services should build")
}

#[tokio::test]
async fn sdk_delivery_profile_builds_shared_runtime_owner_ceiling_without_bitfun_core() {
    let parts = ProductAssembler::new()
        .assemble(ProductAssemblyInput::new(
            DeliveryProfile::Sdk,
            product_full_compatible_services(),
        ))
        .expect("SDK delivery profile should assemble with its shared runtime services");
    let cli_plan =
        bitfun_product_capabilities::product_assembly_plan_for_profile(DeliveryProfile::Cli);

    assert_eq!(parts.plan().profile(), DeliveryProfile::Sdk);
    assert_eq!(
        parts.plan().capability_set().ids(),
        cli_plan.capability_set().ids(),
        "SDK and Headless CLI currently select the same assembly-plan ceiling without sharing product identity"
    );
    assert!(parts.missing_service_requirements().is_empty());
    assert_eq!(
        parts.harness_registry().provider_ids(),
        vec!["core.deep_review", "core.deep_research", "core.miniapp"]
    );
    for capability in [
        RuntimeServiceCapability::Terminal,
        RuntimeServiceCapability::Git,
        RuntimeServiceCapability::Network,
    ] {
        assert!(parts.services().has_capability(capability));
    }

    let (services, harness_registry, plugin_runtime) = parts.into_runtime_parts();
    let provider = Arc::new(ProductSdkAgentProvider::default());
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(provider)
        .with_services(services)
        .with_harness_registry(Arc::new(harness_registry))
        .with_plugin_runtime(plugin_runtime)
        .build()
        .expect("SDK profile parts should build a runtime from the shared owner contracts");

    let handle = runtime
        .run(AgentRunRequest::new(
            SessionSelector::create("SDK profile smoke", "agentic", None),
            "hello from sdk profile",
        ))
        .await
        .expect("SDK delivery profile runtime should accept a run");

    assert_eq!(handle.session_id, "product-sdk-session");
    assert_eq!(handle.turn_id, "product-sdk-turn");
    assert!(handle.accepted);
    assert_eq!(
        runtime.harness_provider_ids(),
        vec!["core.deep_review", "core.deep_research", "core.miniapp"]
    );
}

#[tokio::test]
async fn product_runtime_parts_can_build_agent_runtime_sdk_without_core() {
    let parts = ProductAssembler::new()
        .assemble(ProductAssemblyInput::new(
            DeliveryProfile::Cli,
            product_full_compatible_services(),
        ))
        .expect("CLI product-full compatibility profile should assemble");

    assert_eq!(parts.plan().profile(), DeliveryProfile::Cli);
    assert!(parts.missing_service_requirements().is_empty());

    let (services, harness_registry, plugin_runtime) = parts.into_runtime_parts();
    let provider = Arc::new(ProductSdkAgentProvider::default());
    let events = AgentEventStream::new();
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(provider.clone())
        .with_services(services)
        .with_harness_registry(Arc::new(harness_registry))
        .with_plugin_runtime(plugin_runtime)
        .with_event_stream(events.clone())
        .build()
        .expect("product assembly parts should build an SDK runtime");

    let runtime_services = runtime
        .services()
        .expect("product assembly services should be attached to runtime");
    assert!(runtime_services.has_capability(RuntimeServiceCapability::Terminal));
    assert!(runtime_services.has_capability(RuntimeServiceCapability::Git));
    assert!(runtime_services.has_capability(RuntimeServiceCapability::Network));
    assert_eq!(
        runtime.harness_provider_ids(),
        vec!["core.deep_review", "core.deep_research", "core.miniapp"]
    );

    let handle = runtime
        .run(
            AgentRunRequest::new(
                SessionSelector::create(
                    "Product SDK smoke",
                    "agentic",
                    Some("/workspace/project".to_string()),
                ),
                "hello from product assembly",
            )
            .with_turn_id("product-sdk-turn")
            .with_source(AgentSubmissionSource::SdkHost),
        )
        .await
        .expect("product assembly runtime should accept an SDK run");

    assert_eq!(handle.session_id, "product-sdk-session");
    assert_eq!(handle.turn_id, "product-sdk-turn");
    assert_eq!(handle.agent_type.as_deref(), Some("agentic"));
    assert!(handle.accepted);
    assert_eq!(
        handle.events.expect("event stream").snapshot(),
        events.snapshot()
    );
    assert_eq!(provider.created_sessions.lock().unwrap().len(), 1);
    assert_eq!(provider.submitted_turns.lock().unwrap().len(), 1);
}
