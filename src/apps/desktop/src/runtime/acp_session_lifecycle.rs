//! Desktop-side ACP session lifecycle bridge.
//!
//! `SessionControl` creates `acp__<client>` sessions as plain internal
//! sessions (the external ACP process is never started by the tool itself).
//! This subscriber bridges the core coordinator's agentic lifecycle events
//! back to the ACP client service so the external process lifecycle follows
//! the internal session lifecycle:
//!
//! - `SessionCreated` with an `acp__*` agent type starts the external client
//!   process for that session (idempotent; a running connection is reused).
//! - `SessionDeleted` releases the ACP session, so no external process or
//!   remote session outlives the internal session.
//! - `DialogTurnCancelled` cancels the matching ACP dialog turn when the
//!   internal turn is cancelled (for example through SessionControl cancel).
//!
//! The bridge only touches the ACP client service from the desktop layer;
//! core keeps no dependency on the ACP service.

use std::sync::Arc;

use async_trait::async_trait;
use bitfun_agent_runtime::event_bus::EventSubscriberResult;
use bitfun_agent_runtime::event_router::EventSubscriber;
use bitfun_core::agentic::persistence::PersistenceManager;
use bitfun_core::infrastructure::PathManager;
use bitfun_events::AgenticEvent;

/// Routes agentic session lifecycle events to the ACP client service.
pub(crate) struct AcpSessionLifecycleSubscriber {
    acp_client_service: Option<Arc<bitfun_acp::AcpClientService>>,
}

impl AcpSessionLifecycleSubscriber {
    pub(crate) fn new(acp_client_service: Option<Arc<bitfun_acp::AcpClientService>>) -> Self {
        let subscriber = Self { acp_client_service };
        subscriber.spawn_startup_orphan_scan();
        subscriber
    }

    /// Kick off the one-shot startup orphan scan when a tokio runtime is
    /// available (desktop startup). Best-effort: without a runtime or an ACP
    /// service the scan is skipped and never fatal.
    fn spawn_startup_orphan_scan(&self) {
        let Some(service) = self.acp_client_service.clone() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            let reconciled = Self::scan_and_recover_orphan_connections(&service).await;
            log::info!(
                "ACP startup orphan scan finished: reconciled_flow_sessions={}",
                reconciled
            );
        });
    }

    /// Reconcile persisted ACP flow session records against the manager's
    /// in-memory connections on startup.
    ///
    /// After a desktop restart no external ACP connection is live, but
    /// persisted flow-session records (`provider=acp` in custom metadata)
    /// survive in the local workspace session directories. This scan walks
    /// `~/.bitfun/projects/*/sessions` and releases any stale in-memory
    /// session binding for every ACP flow record (idempotent no-op when none
    /// exists), so a resumed session never inherits a stale connection. Local
    /// workspaces only; remote session mirrors are reconciled by the remote
    /// host on connect.
    async fn scan_and_recover_orphan_connections(
        service: &Arc<bitfun_acp::AcpClientService>,
    ) -> usize {
        let path_manager = match PathManager::new() {
            Ok(path_manager) => path_manager,
            Err(error) => {
                log::warn!(
                    "ACP orphan scan: failed to initialize PathManager: {}",
                    error
                );
                return 0;
            }
        };
        let persistence = match PersistenceManager::new(Arc::new(path_manager)) {
            Ok(persistence) => persistence,
            Err(error) => {
                log::warn!(
                    "ACP orphan scan: failed to initialize PersistenceManager: {}",
                    error
                );
                return 0;
            }
        };
        let projects_root = persistence.path_manager().projects_root();
        let mut reconciled = 0;
        let Ok(entries) = std::fs::read_dir(&projects_root) else {
            return 0;
        };
        for entry in entries.flatten() {
            let sessions_dir = entry.path().join("sessions");
            if !sessions_dir.is_dir() {
                continue;
            }
            let metadata_list = match persistence
                .list_session_metadata_including_internal(&sessions_dir)
                .await
            {
                Ok(list) => list,
                Err(error) => {
                    log::warn!(
                        "ACP orphan scan: failed to list sessions under '{}': {}",
                        sessions_dir.display(),
                        error
                    );
                    continue;
                }
            };
            for metadata in metadata_list {
                // 仅处理 ACP 流会话记录（custom_metadata.provider == "acp"，
                // 与 interfaces/acp session_persistence.rs 的写入口径一致）。
                let is_acp_flow = metadata
                    .custom_metadata
                    .as_ref()
                    .and_then(|custom| custom.get("provider"))
                    .and_then(serde_json::Value::as_str)
                    == Some("acp");
                if !is_acp_flow {
                    continue;
                }
                // Release any stale in-memory binding for this flow session.
                // After a restart there is none, so this is an idempotent
                // reconciliation, not a record deletion.
                if service.release_bitfun_session(&metadata.session_id).await {
                    log::info!(
                        "ACP orphan scan: reclaimed stale connection for flow session: session_id={}",
                        metadata.session_id
                    );
                }
                reconciled += 1;
            }
        }
        reconciled
    }
}

#[async_trait]
impl EventSubscriber for AcpSessionLifecycleSubscriber {
    async fn on_event(&self, event: &AgenticEvent) -> EventSubscriberResult {
        match event {
            // Start the external ACP client process when an `acp__<client>`
            // session is created (SessionControl create path). A missing or
            // empty client id (`acp__`) is rejected up front. Failure is an
            // error-level log keyed by client_id: the internal session stays
            // usable for the forwarding tool, and the process can still be
            // started lazily by the first delegated turn.
            AgenticEvent::SessionCreated {
                session_id,
                agent_type,
                workspace_path,
                remote_connection_id,
                ..
            } => {
                let Some(client_id) = agent_type
                    .strip_prefix("acp__")
                    .filter(|client_id| !client_id.trim().is_empty())
                else {
                    return Ok(());
                };
                let Some(service) = self.acp_client_service.as_ref() else {
                    return Ok(());
                };
                if let Err(error) = service
                    .start_client_for_session(
                        client_id,
                        session_id,
                        workspace_path.as_deref(),
                        remote_connection_id.as_deref(),
                    )
                    .await
                {
                    log::error!(
                        "Failed to start ACP client for session: session_id={}, client_id={}, error={}",
                        session_id,
                        client_id,
                        error
                    );
                }
            }
            // SessionControl delete and the frontend delete both flow through
            // coordinator.delete_session_tree, which emits SessionDeleted.
            // Releasing here is idempotent and complements the frontend
            // delete path's host-effects release.
            AgenticEvent::SessionDeleted { session_id } => {
                if let Some(service) = self.acp_client_service.as_ref() {
                    if !service.release_bitfun_session(session_id).await {
                        log::warn!(
                            "ACP release_bitfun_session reported no live session on session deletion: session_id={}",
                            session_id
                        );
                    }
                }
            }
            // SessionControl cancel flows through runtime.cancel_turn; the
            // coordinator emits DialogTurnCancelled (duplicates are harmless).
            // d3-P2-5：与 SessionCreated 分支对称，仅处理 ACP 流会话形状
            // （`acp_<client_id>_<uuid>`），防止内部会话 id 被误路由到
            // 外部 ACP cancel（内部会话形状 `session-...` 与 flow id 不同，
            // 但守卫必须显式，杜绝未来 id 规则变更时波及无关外部 turn）。
            AgenticEvent::DialogTurnCancelled { session_id, .. } => {
                if bitfun_runtime_ports::acp_flow_client_id_from_session_id(session_id).is_none() {
                    return Ok(());
                }
                if let Some(service) = self.acp_client_service.as_ref() {
                    match service.cancel_bitfun_session(session_id).await {
                        Ok(false) => {
                            // d3-P2-4：无活动外部 turn 可取消——内部会话被取消
                            // 但外部进程可能仍在运行。显式告警，不静默吞掉。
                            log::warn!(
                                "ACP cancel_bitfun_session reported no active external turn on dialog turn cancellation: session_id={}",
                                session_id
                            );
                        }
                        Ok(true) => {}
                        Err(error) => {
                            log::warn!(
                                "Failed to cancel ACP session after dialog turn cancellation: session_id={}, error={}",
                                session_id,
                                error
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
