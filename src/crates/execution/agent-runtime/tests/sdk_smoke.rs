use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bitfun_agent_runtime::sdk::{
    AgentEventStream, AgentRunRequest, AgentRuntimeBuilder, SessionSelector,
};
use bitfun_runtime_ports::{
    AgentSessionCreateRequest, AgentSessionCreateResult, AgentSubmissionPort,
    AgentSubmissionRequest, AgentSubmissionResult, AgentSubmissionSource, PortResult,
    RuntimeEventEnvelope, RuntimeEventType,
};

#[derive(Debug, Default)]
struct FakeSdkAgentProvider {
    created_sessions: Mutex<Vec<AgentSessionCreateRequest>>,
    submitted_turns: Mutex<Vec<AgentSubmissionRequest>>,
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
