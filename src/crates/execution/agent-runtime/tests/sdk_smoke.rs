use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bitfun_agent_runtime::sdk::{
    AgentEventStream, AgentRunRequest, AgentRuntimeBuilder, RuntimeAgentRegistry,
    RuntimeAgentRegistryQuery, RuntimeHookErrorPolicy, RuntimeHookKind, RuntimeHookPlan,
    RuntimeHookRegistry, SessionSelector,
};
use bitfun_agent_tools::{ToolRegistry, ToolRegistryItem};
use bitfun_harness::{
    build_descriptor_harness_registry, HarnessCapability, HarnessProviderDescriptor,
    HarnessWorkflow,
};
use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSessionCreateResult, AgentSubmissionPort,
    AgentSubmissionRequest, AgentSubmissionResult, AgentSubmissionSource, PortResult,
    RuntimeEventEnvelope, RuntimeEventType,
};
use bitfun_runtime_services::test_support::FakeRuntimeServicesProvider;
use serde_json::{json, Value};

#[derive(Debug, Default)]
struct FakeSdkAgentProvider {
    created_sessions: Mutex<Vec<AgentSessionCreateRequest>>,
    submitted_turns: Mutex<Vec<AgentSubmissionRequest>>,
}

struct FakeSdkTool;

#[derive(Debug)]
struct FakeSdkAgentRegistry {
    agent_ids: Vec<String>,
    workspace_agent_ids: Vec<String>,
}

impl RuntimeAgentRegistry for FakeSdkAgentRegistry {
    fn agent_ids(&self, query: RuntimeAgentRegistryQuery<'_>) -> Vec<String> {
        if query.workspace_root.is_some() {
            self.workspace_agent_ids.clone()
        } else {
            self.agent_ids.clone()
        }
    }
}

#[async_trait]
impl ToolRegistryItem for FakeSdkTool {
    fn name(&self) -> &str {
        "sdk_echo"
    }

    async fn description(&self) -> Result<String, String> {
        Ok("Echo input for SDK smoke coverage".to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" }
            }
        })
    }
}

#[async_trait]
impl AgentSubmissionPort for FakeSdkAgentProvider {
    async fn create_session(
        &self,
        request: AgentSessionCreateRequest,
    ) -> PortResult<AgentSessionCreateResult> {
        self.created_sessions.lock().unwrap().push(request.clone());
        Ok(AgentSessionCreateResult {
            session_id: "sdk-session-1".to_string(),
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
            turn_id: request.turn_id.unwrap_or_else(|| "sdk-turn-1".to_string()),
            accepted: true,
        })
    }

    async fn resolve_session_agent_type(&self, _session_id: &str) -> PortResult<Option<String>> {
        Ok(Some("agentic".to_string()))
    }
}

#[tokio::test]
async fn sdk_facade_runs_with_fake_provider_and_local_event_stream() {
    let provider = Arc::new(FakeSdkAgentProvider::default());
    let events = AgentEventStream::new();
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(provider.clone())
        .with_event_stream(events.clone())
        .build()
        .expect("sdk runtime");

    let handle = runtime
        .run(
            AgentRunRequest::new(
                SessionSelector::create(
                    "SDK smoke",
                    "agentic",
                    Some("/workspace/project".to_string()),
                ),
                "hello from sdk",
            )
            .with_turn_id("sdk-turn-1")
            .with_source(AgentSubmissionSource::Cli),
        )
        .await
        .expect("sdk run");

    assert_eq!(handle.session_id, "sdk-session-1");
    assert_eq!(handle.turn_id, "sdk-turn-1");
    assert_eq!(handle.agent_type.as_deref(), Some("agentic"));
    assert!(handle.accepted);

    runtime
        .publish_event(RuntimeEventEnvelope {
            session_id: handle.session_id.clone(),
            turn_id: Some(handle.turn_id.clone()),
            source: Some(AgentSubmissionSource::Cli),
            event_type: RuntimeEventType::TurnStarted,
            payload: serde_json::json!({ "source": "sdk-smoke" }),
        })
        .await
        .expect("publish sdk event");

    assert_eq!(provider.created_sessions.lock().unwrap().len(), 1);
    assert_eq!(provider.submitted_turns.lock().unwrap().len(), 1);
    assert_eq!(
        handle.events.expect("event stream").snapshot(),
        events.snapshot()
    );
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn sdk_facade_accepts_fake_services_tools_harnesses_and_hooks_without_core() {
    let provider = Arc::new(FakeSdkAgentProvider::default());
    let services = FakeRuntimeServicesProvider::with_all_required()
        .build_services()
        .expect("fake required services");
    let mut tools = ToolRegistry::new();
    tools.register_tool(Arc::new(FakeSdkTool));
    let harnesses = build_descriptor_harness_registry([HarnessProviderDescriptor::legacy_facade(
        "sdk.fake_harness",
        HarnessWorkflow::Sdd,
        &[HarnessCapability::Plan],
        "external-sdk-harness",
    )])
    .expect("fake harness registry should build");
    let hooks = RuntimeHookRegistry::builder()
        .register(
            RuntimeHookPlan::new("sdk.post_call", RuntimeHookKind::SuccessfulToolPostCall)
                .with_timeout_millis(250)
                .with_error_policy(RuntimeHookErrorPolicy::RecordWarning),
        )
        .build()
        .expect("hook registry should build");

    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(provider)
        .with_services(services)
        .with_tool_registry(Arc::new(tools))
        .with_harness_registry(Arc::new(harnesses))
        .with_hook_registry(hooks)
        .with_agent_registry(Arc::new(FakeSdkAgentRegistry {
            agent_ids: vec!["agentic".to_string(), "Explore".to_string()],
            workspace_agent_ids: vec![
                "agentic".to_string(),
                "Explore".to_string(),
                "ProjectReviewer".to_string(),
            ],
        }))
        .build()
        .expect("sdk runtime");

    assert_eq!(runtime.registered_tool_names(), vec!["sdk_echo"]);
    assert_eq!(runtime.harness_provider_ids(), vec!["sdk.fake_harness"]);
    assert_eq!(runtime.hook_registry().hooks()[0].id(), "sdk.post_call");
    assert_eq!(
        runtime.registered_agent_ids(RuntimeAgentRegistryQuery::default()),
        vec!["agentic".to_string(), "Explore".to_string()]
    );
    assert_eq!(
        runtime.registered_agent_ids(RuntimeAgentRegistryQuery {
            workspace_root: Some(Path::new("/workspace/project")),
        }),
        vec![
            "agentic".to_string(),
            "Explore".to_string(),
            "ProjectReviewer".to_string()
        ]
    );
    assert!(runtime
        .services()
        .expect("services should be injected")
        .has_capability(bitfun_runtime_ports::RuntimeServiceCapability::SessionStore));
}
