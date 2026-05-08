use std::collections::HashSet;

use agent_client_protocol::schema::{
    CancelNotification, ContentChunk, PromptRequest, PromptResponse, RequestPermissionOutcome,
    SessionUpdate, StopReason,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Result};
use bitfun_core::agentic::coordination::{DialogSubmissionPolicy, DialogTriggerSource};
use bitfun_core::agentic::events::EventEnvelope;
use bitfun_events::AgenticEvent as CoreEvent;
use log::warn;
use tokio::sync::broadcast;

use super::content::parse_prompt_blocks;
use super::events::{
    permission_request, send_update, tool_event_updates, PERMISSION_ALLOW_ONCE,
    PERMISSION_REJECT_ONCE,
};
use super::thinking::{InlineThinkRouter, InlineThinkSegment};
use super::BitfunAcpRuntime;

impl BitfunAcpRuntime {
    pub(super) async fn run_prompt(&self, request: PromptRequest) -> Result<PromptResponse> {
        let session_id = request.session_id.to_string();
        let acp_session = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| Error::resource_not_found(Some(session_id.clone())))?;
        let acp_session = acp_session.clone();
        let connection = self
            .connections
            .get(&session_id)
            .ok_or_else(|| Error::resource_not_found(Some(session_id.clone())))?
            .clone();

        let parsed_prompt = parse_prompt_blocks(&session_id, request.prompt);

        if parsed_prompt.user_message.trim().is_empty() && parsed_prompt.image_contexts.is_empty() {
            return Err(Error::invalid_params().data("empty prompt"));
        }

        let mut event_rx = self.agentic_system.event_queue.subscribe();
        if parsed_prompt.image_contexts.is_empty() {
            self.agentic_system
                .coordinator
                .start_dialog_turn(
                    acp_session.bitfun_session_id.clone(),
                    parsed_prompt.user_message,
                    parsed_prompt.original_user_message,
                    None,
                    acp_session.mode_id.clone(),
                    Some(acp_session.cwd.clone()),
                    DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                )
                .await
                .map_err(Self::internal_error)?;
        } else {
            self.agentic_system
                .coordinator
                .start_dialog_turn_with_image_contexts(
                    acp_session.bitfun_session_id.clone(),
                    parsed_prompt.user_message,
                    parsed_prompt.original_user_message,
                    parsed_prompt.image_contexts,
                    None,
                    acp_session.mode_id.clone(),
                    Some(acp_session.cwd.clone()),
                    DialogSubmissionPolicy::for_source(DialogTriggerSource::Cli),
                )
                .await
                .map_err(Self::internal_error)?;
        }

        let stop_reason = wait_for_prompt_completion(
            self,
            &mut event_rx,
            &connection,
            &acp_session.acp_session_id,
            &acp_session.bitfun_session_id,
        )
        .await?;

        Ok(PromptResponse::new(stop_reason))
    }

    pub(super) async fn cancel_prompt(&self, notification: CancelNotification) -> Result<()> {
        let session_id = notification.session_id.to_string();
        let acp_session = self
            .sessions
            .get(&session_id)
            .ok_or_else(|| Error::resource_not_found(Some(session_id.clone())))?;
        let acp_session = acp_session.clone();

        self.agentic_system
            .coordinator
            .cancel_active_turn_for_session(
                &acp_session.bitfun_session_id,
                std::time::Duration::from_secs(5),
            )
            .await
            .map_err(Self::internal_error)?;

        Ok(())
    }
}

async fn wait_for_prompt_completion(
    runtime: &BitfunAcpRuntime,
    event_rx: &mut broadcast::Receiver<EventEnvelope>,
    connection: &ConnectionTo<Client>,
    acp_session_id: &str,
    bitfun_session_id: &str,
) -> Result<StopReason> {
    let mut seen_tool_calls = HashSet::new();
    let mut inline_think = InlineThinkRouter::new();

    loop {
        let event = match event_rx.recv().await {
            Ok(envelope) => envelope.event,
            Err(broadcast::error::RecvError::Lagged(count)) => {
                warn!("ACP event receiver lagged: skipped {} events", count);
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                return Err(Error::internal_error().data("event stream closed"));
            }
        };

        if event.session_id() != Some(bitfun_session_id) {
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
                    tool_id,
                    tool_name,
                    params,
                } = tool_event
                {
                    handle_permission_request(
                        runtime,
                        connection,
                        acp_session_id,
                        &tool_id,
                        &tool_name,
                        &params,
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
                .agentic_system
                .coordinator
                .reject_tool(tool_id, reason.clone())
                .await;
            return Err(error);
        }
    };

    match response.outcome {
        RequestPermissionOutcome::Selected(selected)
            if selected.option_id.to_string() == PERMISSION_ALLOW_ONCE =>
        {
            runtime
                .agentic_system
                .coordinator
                .confirm_tool(tool_id, None)
                .await
                .map_err(BitfunAcpRuntime::internal_error)?;
        }
        RequestPermissionOutcome::Selected(selected)
            if selected.option_id.to_string() == PERMISSION_REJECT_ONCE =>
        {
            runtime
                .agentic_system
                .coordinator
                .reject_tool(tool_id, "Rejected by ACP client".to_string())
                .await
                .map_err(BitfunAcpRuntime::internal_error)?;
        }
        RequestPermissionOutcome::Cancelled => {
            runtime
                .agentic_system
                .coordinator
                .reject_tool(tool_id, "ACP permission request cancelled".to_string())
                .await
                .map_err(BitfunAcpRuntime::internal_error)?;
        }
        RequestPermissionOutcome::Selected(selected) => {
            let reason = format!(
                "Unknown ACP permission option selected: {}",
                selected.option_id
            );
            runtime
                .agentic_system
                .coordinator
                .reject_tool(tool_id, reason)
                .await
                .map_err(BitfunAcpRuntime::internal_error)?;
        }
        _ => {
            runtime
                .agentic_system
                .coordinator
                .reject_tool(tool_id, "Unsupported ACP permission outcome".to_string())
                .await
                .map_err(BitfunAcpRuntime::internal_error)?;
        }
    }

    Ok(())
}
