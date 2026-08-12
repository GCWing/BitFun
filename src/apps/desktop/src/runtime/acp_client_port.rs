//! Desktop-side implementation of the ACP client runtime port.
//!
//! Bridges `bitfun_runtime_ports::AcpClientPort` to the real
//! `AcpClientService` owned by the desktop host. Core tools never touch the
//! ACP crate; this file is the desktop injection point of the dedicated ACP
//! tool family (`acp_control` / `acp_message` / `acp_history`).
//!
//! Every method forwards to the external ACP client process through the
//! manager service (true bridge, never a local model consumption path).

use std::sync::Arc;

use async_trait::async_trait;
use bitfun_acp::client::AcpClientStreamEvent;
use bitfun_acp::AcpClientService;
use bitfun_core::agentic::coordination::ConversationCoordinator;
use bitfun_core::service::remote_ssh::workspace_state::get_effective_session_path;
use bitfun_events::AgenticEvent;
use bitfun_runtime_ports::{
    acp_backend_error, AcpClientBitfunMessageRequest, AcpClientCancelRequest, AcpClientCreateRequest,
    AcpClientCreateResult, AcpClientHistoryEntry, AcpClientHistoryRequest, AcpClientHistoryResult,
    AcpClientListResult, AcpClientMessageRequest, AcpClientMessageResult, AcpClientPort,
    AcpClientReleaseRequest, AcpClientStreamChunk, AcpClientStreamChunkSink, AcpClientSummary,
    PortErrorKind, PortResult, RuntimeServiceCapability, RuntimeServicePort,
};

/// Desktop implementation of [`AcpClientPort`] over the real ACP client service.
pub(crate) struct DesktopAcpClientPort {
    acp_client_service: Option<Arc<AcpClientService>>,
    coordinator: Option<Arc<ConversationCoordinator>>,
}

impl std::fmt::Debug for DesktopAcpClientPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopAcpClientPort")
            .field(
                "acp_client_service",
                &self
                    .acp_client_service
                    .as_ref()
                    .map(|_| "<AcpClientService>"),
            )
            .field(
                "coordinator",
                &self.coordinator.as_ref().map(|_| "<ConversationCoordinator>"),
            )
            .finish()
    }
}

impl DesktopAcpClientPort {
    pub(crate) fn new(
        acp_client_service: Option<Arc<AcpClientService>>,
        coordinator: Option<Arc<ConversationCoordinator>>,
    ) -> Self {
        Self {
            acp_client_service,
            coordinator,
        }
    }

    fn service(&self) -> PortResult<&Arc<AcpClientService>> {
        self.acp_client_service
            .as_ref()
            .ok_or_else(|| acp_backend_error("ACP client service not initialized"))
    }

    fn coordinator(&self) -> PortResult<&Arc<ConversationCoordinator>> {
        self.coordinator
            .as_ref()
            .ok_or_else(|| acp_backend_error("coordinator not initialized"))
    }

    async fn session_storage_path(
        &self,
        workspace_path: Option<&str>,
    ) -> PortResult<std::path::PathBuf> {
        let workspace_path = workspace_path
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                bitfun_runtime_ports::PortError::new(
                    PortErrorKind::InvalidRequest,
                    "workspace_path is required to resolve the ACP session storage path",
                )
            })?;
        Ok(get_effective_session_path(workspace_path, None, None).await)
    }

    /// Stream one prompt through the real ACP channel.
    ///
    /// Translates the ACP crate's `AcpClientStreamEvent` stream into the
    /// boundary `AcpClientStreamChunk` sequence pushed into `chunk_sink`.
    /// `Text` chunks are accumulated so the returned full response text stays
    /// equivalent to the non-streaming `prompt_agent` path; `Thought` chunks
    /// are forwarded as informational chunks but excluded from the response.
    async fn prompt_agent_streamed(
        &self,
        client_id: &str,
        message: String,
        workspace_path: Option<String>,
        bitfun_session_id: String,
        timeout_seconds: Option<u64>,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<String> {
        let service = self.service()?.clone();
        let mut response = String::new();
        service
            .prompt_agent_stream(
                client_id,
                message,
                workspace_path,
                None,
                bitfun_session_id.clone(),
                None,
                timeout_seconds,
                None,
                None,
                |event| {
                    match event {
                        AcpClientStreamEvent::AgentText(text) => {
                            response.push_str(&text);
                            let _ = chunk_sink.send(AcpClientStreamChunk::Text { text });
                        }
                        AcpClientStreamEvent::AgentThought(text) => {
                            let _ = chunk_sink.send(AcpClientStreamChunk::Thought { text });
                        }
                        AcpClientStreamEvent::Completed => {
                            let _ = chunk_sink.send(AcpClientStreamChunk::Completed);
                        }
                        AcpClientStreamEvent::Cancelled => {
                            let _ = chunk_sink.send(AcpClientStreamChunk::Cancelled);
                        }
                        _ => {}
                    }
                    Ok(())
                },
            )
            .await
            .map_err(|error| acp_backend_error(format!("ACP agent failed: {error}")))?;
        Ok(response)
    }
}

impl RuntimeServicePort for DesktopAcpClientPort {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::AcpClient
    }
}

#[async_trait]
impl AcpClientPort for DesktopAcpClientPort {
    async fn create_session(
        &self,
        request: AcpClientCreateRequest,
    ) -> PortResult<AcpClientCreateResult> {
        let service = self.service()?.clone();
        let session_storage_path = self
            .session_storage_path(Some(&request.workspace_path))
            .await?;

        // Mirrors the FlowChat path (`create_acp_flow_session`): create the
        // persisted record first, then start the external client process and
        // roll the record back when the process start fails so no orphan
        // record is left behind.
        let response = service
            .create_flow_session_record(
                &session_storage_path,
                &request.workspace_path,
                &request.client_id,
                request.session_name,
            )
            .await
            .map_err(|error| acp_backend_error(format!("failed to create ACP session: {error}")))?;

        if let Err(error) = service
            .start_client_for_session(
                &request.client_id,
                &response.session_id,
                Some(&request.workspace_path),
                request.remote_connection_id.as_deref(),
            )
            .await
        {
            if let Err(cleanup_error) = service
                .delete_flow_session_record(&session_storage_path, &response.session_id)
                .await
            {
                log::warn!(
                    "Failed to delete ACP session record after client start failure: session_id={}, error={}",
                    response.session_id,
                    cleanup_error
                );
            }
            return Err(acp_backend_error(format!(
                "failed to start ACP client for session: {error}"
            )));
        }

        // Broadcast `agentic://session-created` so the frontend can register
        // the external ACP session (payload shape mirrors the FlowChat
        // `create_acp_flow_session` emit in acp_client_api.rs). Best-effort:
        // a missing coordinator only drops the UI event, never the session.
        if let Some(coordinator) = self.coordinator.as_ref() {
            coordinator
                .emit_event(AgenticEvent::SessionCreated {
                    session_id: response.session_id.clone(),
                    session_name: response.session_name.clone(),
                    agent_type: response.agent_type.clone(),
                    workspace_path: Some(request.workspace_path.clone()),
                    project_workspace_path: None,
                    execution_target: None,
                    workspace_id: None,
                    remote_connection_id: request.remote_connection_id.clone(),
                    remote_ssh_host: None,
                    parent_session_id: None,
                    subagent_type: None,
                })
                .await;
        }

        Ok(AcpClientCreateResult {
            session_id: response.session_id,
            session_name: response.session_name,
            agent_type: response.agent_type,
        })
    }

    async fn list_clients(&self) -> PortResult<AcpClientListResult> {
        let service = self.service()?.clone();
        let infos = service
            .list_clients()
            .await
            .map_err(|error| acp_backend_error(format!("failed to list ACP clients: {error}")))?;
        Ok(AcpClientListResult {
            clients: infos
                .into_iter()
                .map(|info| AcpClientSummary {
                    client_id: info.id,
                    name: info.name,
                    status: format!("{:?}", info.status),
                    session_count: info.session_count,
                    readonly: info.readonly,
                })
                .collect(),
        })
    }

    async fn release_session(&self, request: AcpClientReleaseRequest) -> PortResult<()> {
        let service = self.service()?.clone();
        // Idempotent: releasing a session that has no live external process is
        // a no-op success, matching the session lifecycle bridge semantics. A
        // `false` return still means "nothing live to release", which is worth
        // surfacing so callers can tell an expected no-op from a lost binding.
        if !service.release_bitfun_session(&request.session_id).await {
            log::warn!(
                "ACP release_bitfun_session reported no live session: session_id={}",
                request.session_id
            );
        }
        Ok(())
    }

    async fn cancel_session(&self, request: AcpClientCancelRequest) -> PortResult<()> {
        let service = self.service()?.clone();
        // d3-P2-4：cancel 必须带确认语义。`cancel_bitfun_session` 返回
        // `Ok(false)` 表示没有找到可取消的活动外部 turn——此前被上层吞掉，
        // UI 会显示已取消而外部进程仍在运行。这里把 false 显式化为
        // NotFound，调用方（acp_control cancel / Task cancel）能区分
        // 「已确认取消」与「无活动 turn 可取消」。
        let cancelled = service
            .cancel_bitfun_session(&request.session_id)
            .await
            .map_err(|error| acp_backend_error(format!("failed to cancel ACP session: {error}")))?;
        if !cancelled {
            return Err(bitfun_runtime_ports::PortError::new(
                bitfun_runtime_ports::PortErrorKind::NotFound,
                format!(
                    "ACP session '{}' has no active external turn to cancel; the cancel notification was not delivered",
                    request.session_id
                ),
            ));
        }
        Ok(())
    }

    async fn send_message(
        &self,
        request: AcpClientMessageRequest,
    ) -> PortResult<AcpClientMessageResult> {
        let service = self.service()?.clone();
        let client_id = client_id_from_session_id(&request.session_id).ok_or_else(|| {
            bitfun_runtime_ports::PortError::new(
                PortErrorKind::InvalidRequest,
                format!(
                    "session_id '{}' is not an ACP flow session id (expected acp_<client_id>_<uuid>)",
                    request.session_id
                ),
            )
        })?;
        let response = service
            .prompt_agent(
                &client_id,
                request.message,
                request.workspace_path,
                None,
                request.session_id.clone(),
                None,
                request.timeout_seconds,
            )
            .await
            .map_err(|error| acp_backend_error(format!("ACP agent failed: {error}")))?;
        Ok(AcpClientMessageResult {
            session_id: request.session_id,
            response,
        })
    }

    async fn send_message_stream(
        &self,
        request: AcpClientMessageRequest,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult> {
        let client_id = client_id_from_session_id(&request.session_id).ok_or_else(|| {
            bitfun_runtime_ports::PortError::new(
                PortErrorKind::InvalidRequest,
                format!(
                    "session_id '{}' is not an ACP flow session id (expected acp_<client_id>_<uuid>)",
                    request.session_id
                ),
            )
        })?;
        let response = self
            .prompt_agent_streamed(
                &client_id,
                request.message,
                request.workspace_path,
                request.session_id.clone(),
                request.timeout_seconds,
                chunk_sink,
            )
            .await?;
        Ok(AcpClientMessageResult {
            session_id: request.session_id,
            response,
        })
    }

    async fn send_message_to_bitfun_session(
        &self,
        request: AcpClientBitfunMessageRequest,
    ) -> PortResult<AcpClientMessageResult> {
        let service = self.service()?.clone();
        // Same forwarding shape as AcpAgentTool::call_impl (the
        // `acp__<client>__prompt` bridge tool): the external process is
        // addressed by the internal BitFun session id, so the conversation
        // state is shared with the delegated-turn path.
        // 参考 bitfun-acp interfaces/acp/src/client/tool.rs:157-168 —
        // AcpAgentTool::call_impl → service.prompt_agent，Rust 翻译实现
        let response = service
            .prompt_agent(
                &request.client_id,
                request.message,
                request.workspace_path,
                None,
                request.bitfun_session_id.clone(),
                None,
                request.timeout_seconds,
            )
            .await
            .map_err(|error| acp_backend_error(format!("ACP agent failed: {error}")))?;
        Ok(AcpClientMessageResult {
            session_id: request.bitfun_session_id,
            response,
        })
    }

    async fn send_message_to_bitfun_session_stream(
        &self,
        request: AcpClientBitfunMessageRequest,
        chunk_sink: AcpClientStreamChunkSink,
    ) -> PortResult<AcpClientMessageResult> {
        let response = self
            .prompt_agent_streamed(
                &request.client_id,
                request.message,
                request.workspace_path,
                request.bitfun_session_id.clone(),
                request.timeout_seconds,
                chunk_sink,
            )
            .await?;
        Ok(AcpClientMessageResult {
            session_id: request.bitfun_session_id,
            response,
        })
    }

    async fn delete_session_record(
        &self,
        session_id: String,
        workspace_path: Option<String>,
    ) -> PortResult<()> {
        let service = self.service()?.clone();
        // Resolve the storage path up front: a missing workspace would
        // otherwise release the process without removing the persisted record,
        // silently leaving an orphan record that keeps the recycled session in
        // listings. Reject with InvalidRequest instead of half-cleaning.
        let Some(workspace_path) = workspace_path.as_deref() else {
            return Err(bitfun_runtime_ports::PortError::new(
                PortErrorKind::InvalidRequest,
                "workspace_path is required to delete the ACP session record; refusing to release-only (would leave an orphan record)",
            ));
        };
        let session_storage_path = self.session_storage_path(Some(workspace_path)).await?;
        // Release the external process if one is bound to the session
        // (idempotent), then remove the persisted flow-session record so the
        // recycled session stops appearing in listings.
        if !service.release_bitfun_session(&session_id).await {
            log::warn!(
                "ACP release_bitfun_session reported no live session during delete_session_record: session_id={}",
                session_id
            );
        }
        service
            .delete_flow_session_record(&session_storage_path, &session_id)
            .await
            .map_err(|error| {
                acp_backend_error(format!("failed to delete ACP session record: {error}"))
            })?;
        Ok(())
    }

    async fn read_history(
        &self,
        request: AcpClientHistoryRequest,
    ) -> PortResult<AcpClientHistoryResult> {
        let coordinator = self.coordinator()?.clone();
        let session_storage_path = self.session_storage_path(request.workspace_path.as_deref()).await?;
        let turns = coordinator
            .load_visible_persisted_session_turns(&session_storage_path, &request.session_id)
            .await
            .map_err(|error| acp_backend_error(format!("failed to read session turns: {error}")))?;

        // d3-P2-7：acp_history 无读取上限会把长会话全量转录进 ToolResult
        // data JSON（父上下文/工具结果膨胀），且 truncated 恒 false 误导调用方。
        // 补每条消息的上限——超过时按「保留最新消息」截断（最新 turn 是模型
        // 最需要续接的上下文），truncated 置 true 如实上报。
        const MAX_HISTORY_ENTRIES: usize = 100;

        let mut entries = Vec::with_capacity(turns.len().min(MAX_HISTORY_ENTRIES).saturating_mul(2));
        for turn in turns.iter().rev().take(MAX_HISTORY_ENTRIES).rev() {
            entries.push(AcpClientHistoryEntry {
                role: "user".to_string(),
                content: turn.user_message.content.clone(),
                timestamp_ms: Some(turn.user_message.timestamp),
            });
            let assistant_text = turn
                .model_rounds
                .iter()
                .flat_map(|round| round.text_items.iter())
                .map(|item| item.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if !assistant_text.trim().is_empty() {
                entries.push(AcpClientHistoryEntry {
                    role: "assistant".to_string(),
                    content: assistant_text,
                    timestamp_ms: Some(turn.timestamp),
                });
            }
        }
        let truncated = turns.len() > MAX_HISTORY_ENTRIES;

        Ok(AcpClientHistoryResult {
            session_id: request.session_id,
            entries,
            truncated,
        })
    }
}

/// Parse the ACP client id out of a flow session id.
///
/// Flow session ids have the shape `acp_<client_id>_<uuid>`; the client id is
/// everything between the `acp_` prefix and the final uuid segment. The trailing
/// segment must be a canonical uuid (length 36, dashed, hex) — matching the
/// strict `SessionMessage` detection — so an internal session id that merely
/// starts with `acp_` is never mistaken for a flow session, and an empty client
/// id (`acp__<uuid>`) is rejected. Single authoritative implementation lives in
/// `bitfun_runtime_ports` (d3-P2-2) so all layers share the same判定.
fn client_id_from_session_id(session_id: &str) -> Option<String> {
    bitfun_runtime_ports::acp_flow_client_id_from_session_id(session_id)
}

#[cfg(test)]
mod tests {
    use super::client_id_from_session_id;

    #[test]
    fn client_id_parses_from_flow_session_id() {
        assert_eq!(
            client_id_from_session_id("acp_codex_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b").as_deref(),
            Some("codex")
        );
    }

    #[test]
    fn client_id_parses_client_ids_containing_underscores() {
        assert_eq!(
            client_id_from_session_id("acp_claude_code_7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b")
                .as_deref(),
            Some("claude_code")
        );
    }

    #[test]
    fn client_id_rejects_non_acp_session_ids() {
        assert!(client_id_from_session_id("session-123").is_none());
        assert!(client_id_from_session_id("acp_codex").is_none());
        assert!(client_id_from_session_id("").is_none());
    }

    #[test]
    fn client_id_rejects_non_uuid_trailing_segment() {
        // 与 SessionMessage 严格版一致：尾段必须是规范 uuid，非 uuid 一律拒绝
        assert!(client_id_from_session_id("acp_codex_s1").is_none());
        assert!(client_id_from_session_id("acp_codex_7f0e1a2b-3c4d-4e5f-8a9b").is_none());
        // acp__<uuid> 解析出空 client_id，拒绝
        assert!(
            client_id_from_session_id("acp__7f0e1a2b-3c4d-4e5f-8a9b-0c1d2e3f4a5b").is_none()
        );
    }
}
