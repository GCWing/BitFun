use std::collections::HashSet;

use agent_client_protocol::schema::{
    CancelNotification, ContentChunk, PromptRequest, PromptResponse, RequestPermissionOutcome,
    SessionUpdate, StopReason,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Result};
use bitfun_agent_runtime::sdk::{
    AgentDialogTurnRequest, AgentSessionEventReceiver, AgentSubmissionSource,
    AgentToolConfirmationRequest, AgentToolRejectionRequest, AgentTurnCancellationRequest,
    DialogSubmissionPolicy, DialogSubmitOutcome,
};
use bitfun_events::AgenticEvent as CoreEvent;
use log::warn;
use serde_json::json;
use tokio::sync::broadcast;

use super::content::{parse_prompt_blocks, ParsedPrompt};
use super::events::{
    permission_request, send_update, tool_event_updates, PERMISSION_ALLOW_ONCE,
    PERMISSION_REJECT_ONCE,
};
use super::thinking::{InlineThinkRouter, InlineThinkSegment};
use super::{AcpSessionState, BitfunAcpRuntime};

impl BitfunAcpRuntime {
    pub(super) async fn run_prompt(&self, request: PromptRequest) -> Result<PromptResponse> {
        let session_id = request.session_id.to_string();
        let (acp_session, lifecycle_guard) = self.lock_active_session(&session_id).await?;
        let connection = self
            .connections
            .get(&session_id)
            .ok_or_else(|| Error::resource_not_found(Some(session_id.clone())))?
            .clone();

        let parsed_prompt = parse_prompt_blocks(&session_id, request.prompt);

        if parsed_prompt.user_message.trim().is_empty() && parsed_prompt.attachments.is_empty() {
            return Err(Error::invalid_params().data("empty prompt"));
        }

        let mut event_rx = self
            .agent_runtime
            .subscribe_session_events(&acp_session.bitfun_session_id)
            .map_err(|error| Self::session_runtime_error(&session_id, error))?;
        let outcome = self
            .agent_runtime
            .submit_dialog_turn(dialog_turn_request(&acp_session, parsed_prompt))
            .await
            .map_err(|error| Self::session_runtime_error(&session_id, error))?;
        let turn_id = match resolve_started_prompt_turn(outcome) {
            Ok(turn_id) => turn_id,
            Err(queued_turn_id) => {
                self.agent_runtime
                    .cancel_turn(turn_cancellation_request(
                        &acp_session.bitfun_session_id,
                        Some(&queued_turn_id),
                        "acp_busy_rejected",
                    ))
                    .await
                    .map_err(|error| Self::session_runtime_error(&session_id, error))?;
                return Err(Error::internal_error()
                    .data("Session state does not allow starting new dialog: Processing"));
            }
        };
        drop(lifecycle_guard);

        let stop_reason = wait_for_prompt_completion(
            self,
            &mut event_rx,
            &connection,
            &acp_session.acp_session_id,
            &acp_session.bitfun_session_id,
            &turn_id,
        )
        .await?;

        Ok(PromptResponse::new(stop_reason))
    }

    pub(super) async fn cancel_prompt(&self, notification: CancelNotification) -> Result<()> {
        let session_id = notification.session_id.to_string();
        let (acp_session, _lifecycle_guard) = self.lock_active_session(&session_id).await?;

        self.agent_runtime
            .cancel_turn(turn_cancellation_request(
                &acp_session.bitfun_session_id,
                None,
                "acp_client_cancelled",
            ))
            .await
            .map_err(|error| Self::session_runtime_error(&session_id, error))?;

        Ok(())
    }
}

fn dialog_turn_request(session: &AcpSessionState, prompt: ParsedPrompt) -> AgentDialogTurnRequest {
    AgentDialogTurnRequest {
        session_id: session.bitfun_session_id.clone(),
        message: prompt.user_message,
        original_message: prompt.original_user_message,
        turn_id: None,
        agent_type: session.mode_id.clone(),
        workspace_path: Some(session.cwd.clone()),
        remote_connection_id: None,
        remote_ssh_host: None,
        policy: DialogSubmissionPolicy::for_source(AgentSubmissionSource::Cli),
        reply_route: None,
        prepended_reminders: Vec::new(),
        attachments: prompt.attachments,
        metadata: acp_user_message_metadata(),
    }
}

fn resolve_started_prompt_turn(
    outcome: DialogSubmitOutcome,
) -> std::result::Result<String, String> {
    match outcome {
        DialogSubmitOutcome::Started { turn_id, .. } => Ok(turn_id),
        DialogSubmitOutcome::Queued { turn_id, .. } => Err(turn_id),
    }
}

pub(super) fn turn_cancellation_request(
    session_id: &str,
    turn_id: Option<&str>,
    reason: &str,
) -> AgentTurnCancellationRequest {
    AgentTurnCancellationRequest {
        session_id: session_id.to_string(),
        turn_id: turn_id.map(ToOwned::to_owned),
        source: Some(AgentSubmissionSource::Cli),
        requester_session_id: None,
        reason: Some(reason.to_string()),
        wait_timeout_ms: Some(5_000),
    }
}

fn acp_user_message_metadata() -> serde_json::Map<String, serde_json::Value> {
    json!({ "acp_transport": true })
        .as_object()
        .cloned()
        .expect("ACP metadata must be an object")
}

async fn wait_for_prompt_completion(
    runtime: &BitfunAcpRuntime,
    event_rx: &mut AgentSessionEventReceiver,
    connection: &ConnectionTo<Client>,
    acp_session_id: &str,
    bitfun_session_id: &str,
    turn_id: &str,
) -> Result<StopReason> {
    let mut seen_tool_calls = HashSet::new();
    let mut inline_think = InlineThinkRouter::new();

    loop {
        let event = match event_rx.recv().await {
            Ok(envelope) => envelope.event,
            Err(broadcast::error::RecvError::Lagged(count)) => {
                let message = format!(
                    "agent event stream lagged; cancelled turn after skipping {count} events"
                );
                cancel_turn_after_event_stream_failure(
                    runtime,
                    bitfun_session_id,
                    turn_id,
                    "acp_event_stream_lagged",
                )
                .await;
                return Err(Error::internal_error().data(message));
            }
            Err(broadcast::error::RecvError::Closed) => {
                cancel_turn_after_event_stream_failure(
                    runtime,
                    bitfun_session_id,
                    turn_id,
                    "acp_event_stream_closed",
                )
                .await;
                return Err(Error::internal_error().data("event stream closed"));
            }
        };

        if event.session_id() != Some(bitfun_session_id) {
            continue;
        }
        if !prompt_event_matches_turn(&event, turn_id) {
            continue;
        }

        match event {
            CoreEvent::TextChunk { text, .. } => {
                send_inline_think_segments(
                    connection,
                    acp_session_id,
                    inline_think.route_text(text),
                )?;
            }
            CoreEvent::ThinkingChunk { content, .. } => {
                send_update(
                    connection,
                    acp_session_id,
                    SessionUpdate::AgentThoughtChunk(ContentChunk::new(content.into())),
                )?;
            }
            CoreEvent::ToolEvent { tool_event, .. } => {
                for update in tool_event_updates(&tool_event, &mut seen_tool_calls) {
                    send_update(connection, acp_session_id, update)?;
                }

                if let bitfun_events::ToolEventData::ConfirmationNeeded {
                    identity, params, ..
                } = tool_event
                {
                    let (effective_tool_name, effective_params) =
                        bitfun_agent_tools::effective_tool_invocation(&identity.tool_name, &params);
                    handle_permission_request(
                        runtime,
                        connection,
                        acp_session_id,
                        &identity.tool_id,
                        effective_tool_name,
                        effective_params,
                    )
                    .await?;
                }
            }
            CoreEvent::DialogTurnCompleted { .. } => {
                send_inline_think_segments(connection, acp_session_id, inline_think.flush())?;
                return Ok(StopReason::EndTurn);
            }
            CoreEvent::DialogTurnCancelled { .. } => {
                send_inline_think_segments(connection, acp_session_id, inline_think.flush())?;
                return Ok(StopReason::Cancelled);
            }
            CoreEvent::DialogTurnFailed { error, .. } | CoreEvent::SystemError { error, .. } => {
                send_inline_think_segments(connection, acp_session_id, inline_think.flush())?;
                send_update(
                    connection,
                    acp_session_id,
                    SessionUpdate::AgentMessageChunk(ContentChunk::new(
                        format!("Error: {}", error).into(),
                    )),
                )?;
                return Err(Error::internal_error().data(serde_json::json!(error)));
            }
            _ => {}
        }
    }
}

async fn cancel_turn_after_event_stream_failure(
    runtime: &BitfunAcpRuntime,
    session_id: &str,
    turn_id: &str,
    reason: &str,
) {
    if let Err(error) = runtime
        .agent_runtime
        .cancel_turn(turn_cancellation_request(session_id, Some(turn_id), reason))
        .await
    {
        warn!(
            "Failed to cancel ACP turn after event stream failure: session_id={}, turn_id={}, error={}",
            session_id,
            turn_id,
            error.into_message()
        );
    }
}

fn prompt_event_matches_turn(event: &CoreEvent, expected_turn_id: &str) -> bool {
    match event {
        CoreEvent::DialogTurnStarted { turn_id, .. }
        | CoreEvent::DialogTurnCompleted { turn_id, .. }
        | CoreEvent::DialogTurnCancelled { turn_id, .. }
        | CoreEvent::DialogTurnFailed { turn_id, .. }
        | CoreEvent::TextChunk { turn_id, .. }
        | CoreEvent::ThinkingChunk { turn_id, .. }
        | CoreEvent::ToolEvent { turn_id, .. } => turn_id == expected_turn_id,
        CoreEvent::SystemError { .. } => true,
        _ => false,
    }
}

fn send_inline_think_segments(
    connection: &ConnectionTo<Client>,
    acp_session_id: &str,
    segments: Vec<InlineThinkSegment>,
) -> Result<()> {
    for segment in segments {
        let update = match segment {
            InlineThinkSegment::Text(text) => {
                SessionUpdate::AgentMessageChunk(ContentChunk::new(text.into()))
            }
            InlineThinkSegment::Thinking(content) => {
                SessionUpdate::AgentThoughtChunk(ContentChunk::new(content.into()))
            }
        };
        send_update(connection, acp_session_id, update)?;
    }

    Ok(())
}

async fn handle_permission_request(
    runtime: &BitfunAcpRuntime,
    connection: &ConnectionTo<Client>,
    acp_session_id: &str,
    tool_id: &str,
    tool_name: &str,
    params: &serde_json::Value,
) -> Result<()> {
    let request = permission_request(acp_session_id, tool_id, tool_name, params);
    let response = match connection.send_request(request).block_task().await {
        Ok(response) => response,
        Err(error) => {
            let reason = format!("ACP permission request failed: {}", error);
            let _ = runtime
                .agent_runtime
                .reject_tool(AgentToolRejectionRequest {
                    tool_id: tool_id.to_string(),
                    reason: reason.clone(),
                })
                .await;
            return Err(error);
        }
    };

    match response.outcome {
        RequestPermissionOutcome::Selected(selected)
            if selected.option_id.to_string() == PERMISSION_ALLOW_ONCE =>
        {
            runtime
                .agent_runtime
                .confirm_tool(AgentToolConfirmationRequest {
                    tool_id: tool_id.to_string(),
                    updated_input: None,
                })
                .await
                .map_err(BitfunAcpRuntime::runtime_error)?;
        }
        RequestPermissionOutcome::Selected(selected)
            if selected.option_id.to_string() == PERMISSION_REJECT_ONCE =>
        {
            runtime
                .agent_runtime
                .reject_tool(AgentToolRejectionRequest {
                    tool_id: tool_id.to_string(),
                    reason: "Rejected by ACP client".to_string(),
                })
                .await
                .map_err(BitfunAcpRuntime::runtime_error)?;
        }
        RequestPermissionOutcome::Cancelled => {
            runtime
                .agent_runtime
                .reject_tool(AgentToolRejectionRequest {
                    tool_id: tool_id.to_string(),
                    reason: "ACP permission request cancelled".to_string(),
                })
                .await
                .map_err(BitfunAcpRuntime::runtime_error)?;
        }
        RequestPermissionOutcome::Selected(selected) => {
            let reason = format!(
                "Unknown ACP permission option selected: {}",
                selected.option_id
            );
            runtime
                .agent_runtime
                .reject_tool(AgentToolRejectionRequest {
                    tool_id: tool_id.to_string(),
                    reason,
                })
                .await
                .map_err(BitfunAcpRuntime::runtime_error)?;
        }
        _ => {
            runtime
                .agent_runtime
                .reject_tool(AgentToolRejectionRequest {
                    tool_id: tool_id.to_string(),
                    reason: "Unsupported ACP permission outcome".to_string(),
                })
                .await
                .map_err(BitfunAcpRuntime::runtime_error)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use bitfun_agent_runtime::sdk::{
        AgentInputAttachment, AgentSubmissionSource, DialogSubmitOutcome,
    };
    use bitfun_events::AgenticEvent;

    use super::{
        dialog_turn_request, prompt_event_matches_turn, resolve_started_prompt_turn,
        turn_cancellation_request, AcpSessionState, ParsedPrompt,
    };

    fn session() -> AcpSessionState {
        AcpSessionState {
            acp_session_id: "acp-session".to_string(),
            bitfun_session_id: "bitfun-session".to_string(),
            cwd: "/workspace".to_string(),
            mode_id: "agentic".to_string(),
            model_id: "auto".to_string(),
            mcp_server_ids: Vec::new(),
            lifecycle: std::sync::Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    #[test]
    fn dialog_request_preserves_cli_confirmation_and_acp_metadata() {
        let request = dialog_turn_request(
            &session(),
            ParsedPrompt {
                user_message: "describe".to_string(),
                original_user_message: Some("describe".to_string()),
                attachments: vec![AgentInputAttachment::remote_image(
                    "image-1",
                    "clip.png",
                    "data:image/png;base64,QQ==",
                )],
            },
        );

        assert_eq!(request.session_id, "bitfun-session");
        assert_eq!(request.workspace_path.as_deref(), Some("/workspace"));
        assert_eq!(request.policy.trigger_source, AgentSubmissionSource::Cli);
        assert!(request.policy.requires_tool_confirmation());
        assert_eq!(request.metadata["acp_transport"], true);
        assert_eq!(request.attachments.len(), 1);
    }

    #[test]
    fn cancellation_request_keeps_bounded_wait_and_active_turn_semantics() {
        let request =
            turn_cancellation_request(&session().bitfun_session_id, None, "acp_client_cancelled");

        assert_eq!(request.session_id, "bitfun-session");
        assert_eq!(request.turn_id, None);
        assert_eq!(request.source, Some(AgentSubmissionSource::Cli));
        assert_eq!(request.wait_timeout_ms, Some(5_000));
    }

    #[test]
    fn queued_prompt_is_rejected_with_its_exact_turn_identity() {
        let queued_turn_id = resolve_started_prompt_turn(DialogSubmitOutcome::Queued {
            session_id: "bitfun-session".to_string(),
            turn_id: "turn-queued".to_string(),
        })
        .expect_err("queued prompt must not be treated as started");
        let cancellation =
            turn_cancellation_request("bitfun-session", Some(&queued_turn_id), "acp_busy_rejected");

        assert_eq!(cancellation.turn_id.as_deref(), Some("turn-queued"));
        assert_eq!(cancellation.reason.as_deref(), Some("acp_busy_rejected"));
    }

    #[test]
    fn prompt_events_are_scoped_to_the_submitted_turn() {
        let current = AgenticEvent::TextChunk {
            session_id: "bitfun-session".to_string(),
            turn_id: "turn-current".to_string(),
            round_id: "round".to_string(),
            attempt_id: None,
            attempt_index: None,
            text: "current".to_string(),
        };
        let other = AgenticEvent::DialogTurnCompleted {
            session_id: "bitfun-session".to_string(),
            turn_id: "turn-other".to_string(),
            total_rounds: 1,
            total_tools: 0,
            duration_ms: 1,
            partial_recovery_reason: None,
            success: Some(true),
            finish_reason: None,
            has_final_response: Some(true),
        };

        assert!(prompt_event_matches_turn(&current, "turn-current"));
        assert!(!prompt_event_matches_turn(&other, "turn-current"));
    }
}
