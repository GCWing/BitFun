use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bitfun_agent_runtime::sdk::{
    AgentInteractionResponsePort, AgentRuntimeBuilder, AgentSubmissionPort, AgentSubmissionRequest,
    AgentSubmissionResult, AgentToolConfirmationRequest, AgentToolRejectionRequest,
    AgentUserAnswersRequest, PortError, PortResult, RuntimeError,
};
use bitfun_runtime_ports::PortErrorKind;
use serde_json::json;

#[derive(Debug, Default)]
struct FakeSubmissionPort;

#[async_trait]
impl AgentSubmissionPort for FakeSubmissionPort {
    async fn create_session(
        &self,
        _request: bitfun_agent_runtime::sdk::AgentSessionCreateRequest,
    ) -> PortResult<bitfun_agent_runtime::sdk::AgentSessionCreateResult> {
        unreachable!("interaction response contracts do not create sessions")
    }

    async fn submit_message(
        &self,
        _request: AgentSubmissionRequest,
    ) -> PortResult<AgentSubmissionResult> {
        unreachable!("interaction response contracts do not submit messages")
    }

    async fn resolve_session_agent_type(&self, _session_id: &str) -> PortResult<Option<String>> {
        Ok(None)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum RecordedResponse {
    Confirm(AgentToolConfirmationRequest),
    Reject(AgentToolRejectionRequest),
    Answers(AgentUserAnswersRequest),
}

#[derive(Debug, Default)]
struct RecordingInteractionResponsePort {
    responses: Mutex<Vec<RecordedResponse>>,
}

#[async_trait]
impl AgentInteractionResponsePort for RecordingInteractionResponsePort {
    async fn confirm_tool(&self, request: AgentToolConfirmationRequest) -> PortResult<()> {
        self.responses
            .lock()
            .unwrap()
            .push(RecordedResponse::Confirm(request));
        Ok(())
    }

    async fn reject_tool(&self, request: AgentToolRejectionRequest) -> PortResult<()> {
        self.responses
            .lock()
            .unwrap()
            .push(RecordedResponse::Reject(request));
        Ok(())
    }

    async fn submit_user_answers(&self, request: AgentUserAnswersRequest) -> PortResult<()> {
        self.responses
            .lock()
            .unwrap()
            .push(RecordedResponse::Answers(request));
        Ok(())
    }
}

#[tokio::test]
async fn sdk_forwards_typed_interaction_responses_without_losing_payloads() {
    let responses = Arc::new(RecordingInteractionResponsePort::default());
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(Arc::new(FakeSubmissionPort))
        .with_interaction_response_port(responses.clone())
        .build()
        .expect("runtime with interaction response port");

    let confirmation = AgentToolConfirmationRequest {
        tool_id: "tool-1".to_string(),
        updated_input: Some(json!({ "path": "updated.txt" })),
    };
    let rejection = AgentToolRejectionRequest {
        tool_id: "tool-2".to_string(),
        reason: "Use the read-only path".to_string(),
    };
    let answers = AgentUserAnswersRequest {
        tool_id: "tool-3".to_string(),
        answers: json!({ "choice": "continue", "notes": ["keep history"] }),
    };

    runtime
        .confirm_tool(confirmation.clone())
        .await
        .expect("confirm tool");
    runtime
        .reject_tool(rejection.clone())
        .await
        .expect("reject tool");
    runtime
        .submit_user_answers(answers.clone())
        .await
        .expect("submit user answers");

    assert_eq!(
        *responses.responses.lock().unwrap(),
        vec![
            RecordedResponse::Confirm(confirmation),
            RecordedResponse::Reject(rejection),
            RecordedResponse::Answers(answers),
        ]
    );
}

#[tokio::test]
async fn sdk_reports_a_missing_interaction_response_port() {
    let runtime = AgentRuntimeBuilder::new()
        .with_submission_port(Arc::new(FakeSubmissionPort))
        .build()
        .expect("runtime without optional interaction response port");

    let error = runtime
        .confirm_tool(AgentToolConfirmationRequest {
            tool_id: "tool-1".to_string(),
            updated_input: None,
        })
        .await
        .expect_err("missing port must be explicit");

    assert_eq!(error, RuntimeError::MissingInteractionResponsePort);
}

#[test]
fn interaction_response_requests_keep_camel_case_wire_fields() {
    assert_eq!(
        serde_json::to_value(AgentToolConfirmationRequest {
            tool_id: "tool-1".to_string(),
            updated_input: Some(json!({ "command": "safe" })),
        })
        .expect("serialize confirmation request"),
        json!({
            "toolId": "tool-1",
            "updatedInput": { "command": "safe" },
        })
    );
    assert_eq!(
        serde_json::to_value(AgentToolRejectionRequest {
            tool_id: "tool-2".to_string(),
            reason: "User rejected".to_string(),
        })
        .expect("serialize rejection request"),
        json!({
            "toolId": "tool-2",
            "reason": "User rejected",
        })
    );
    assert_eq!(
        serde_json::to_value(AgentUserAnswersRequest {
            tool_id: "tool-3".to_string(),
            answers: json!({ "choice": "continue" }),
        })
        .expect("serialize user answers request"),
        json!({
            "toolId": "tool-3",
            "answers": { "choice": "continue" },
        })
    );
}

#[test]
fn runtime_error_message_keeps_provider_text_without_port_kind_prefix() {
    let message = RuntimeError::Port(PortError::new(
        PortErrorKind::Backend,
        "Tool error: question channel closed",
    ))
    .into_message();

    assert_eq!(message, "Tool error: question channel closed");
}
