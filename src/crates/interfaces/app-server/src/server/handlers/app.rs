use agent_client_protocol::{Builder, ConnectionTo, Dispatch, Error, HandleDispatchFrom, Handled};
use bitfun_app_server_protocol::app::{
    CapabilityAvailability, CapabilityDescriptor, HealthRequest, HealthResponse, HealthStatus,
    InitializeRequest, InitializeResponse, ServerInfo, TransportLimits,
};
use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};
use bitfun_app_server_protocol::event::{SyncEventsRequest, SyncEventsResponse};
use bitfun_app_server_protocol::{MIN_PROTOCOL_VERSION, PROTOCOL_VERSION};

use crate::management::EXTERNAL_SOURCES_CAPABILITY;
use crate::role::{AppClient, AppServer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::server) enum ProtocolNegotiation {
    Pending,
    Negotiating,
    Accepted,
    Rejected,
}

#[derive(Clone)]
pub(in crate::server) struct ConnectionProtocolState {
    decision: tokio::sync::watch::Sender<ProtocolNegotiation>,
    event_subscriptions: std::sync::Arc<
        std::sync::Mutex<Option<crate::server::event_forwarder::EventSubscriptions>>,
    >,
}

impl ConnectionProtocolState {
    pub(in crate::server) fn new() -> Self {
        let (decision, _) = tokio::sync::watch::channel(ProtocolNegotiation::Pending);
        Self {
            decision,
            event_subscriptions: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn current(&self) -> ProtocolNegotiation {
        *self.decision.borrow()
    }

    fn accept(&self) -> bool {
        self.decision.send_if_modified(|decision| {
            if *decision != ProtocolNegotiation::Negotiating {
                return false;
            }
            *decision = ProtocolNegotiation::Accepted;
            true
        })
    }

    fn begin_negotiation(
        &self,
        subscriptions: crate::server::event_forwarder::EventSubscriptions,
    ) -> bool {
        let mut subscriptions = Some(subscriptions);
        self.decision.send_if_modified(|decision| {
            if *decision != ProtocolNegotiation::Pending {
                return false;
            }
            *self
                .event_subscriptions
                .lock()
                .expect("App Server event subscription state poisoned") = subscriptions.take();
            *decision = ProtocolNegotiation::Negotiating;
            true
        })
    }

    fn abort_negotiation(&self) {
        self.decision.send_if_modified(|decision| {
            if *decision != ProtocolNegotiation::Negotiating {
                return false;
            }
            self.event_subscriptions
                .lock()
                .expect("App Server event subscription state poisoned")
                .take();
            *decision = ProtocolNegotiation::Rejected;
            true
        });
    }

    pub(in crate::server) fn take_event_subscriptions(
        &self,
    ) -> Option<crate::server::event_forwarder::EventSubscriptions> {
        self.event_subscriptions
            .lock()
            .expect("App Server event subscription state poisoned")
            .take()
    }

    fn reject(&self) {
        self.decision.send_if_modified(|decision| {
            if *decision != ProtocolNegotiation::Pending {
                return false;
            }
            *decision = ProtocolNegotiation::Rejected;
            true
        });
    }

    pub(in crate::server) async fn wait_for_decision(&self) -> ProtocolNegotiation {
        let mut decision = self.decision.subscribe();
        loop {
            let current = *decision.borrow_and_update();
            match current {
                ProtocolNegotiation::Accepted | ProtocolNegotiation::Rejected => return current,
                ProtocolNegotiation::Pending | ProtocolNegotiation::Negotiating => {}
            }
            if decision.changed().await.is_err() {
                return ProtocolNegotiation::Rejected;
            }
        }
    }
}

pub(in crate::server) struct NegotiationGate {
    protocol_state: ConnectionProtocolState,
}

impl NegotiationGate {
    pub(in crate::server) fn new(protocol_state: ConnectionProtocolState) -> Self {
        Self { protocol_state }
    }
}

impl HandleDispatchFrom<AppClient> for NegotiationGate {
    async fn handle_dispatch_from(
        &mut self,
        message: Dispatch,
        _cx: ConnectionTo<AppClient>,
    ) -> Result<Handled<Dispatch>, Error> {
        match (self.protocol_state.current(), message) {
            (_, message @ Dispatch::Response(..)) | (ProtocolNegotiation::Accepted, message) => {
                Ok(Handled::No {
                    message,
                    retry: false,
                })
            }
            (_, Dispatch::Request(_, responder)) => {
                responder.respond_with_error(protocol_negotiation_error())?;
                Ok(Handled::Yes)
            }
            (_, Dispatch::Notification(_)) => Ok(Handled::Yes),
        }
    }

    fn describe_chain(&self) -> impl std::fmt::Debug {
        "AppServerProtocolNegotiationGate"
    }
}

pub(in crate::server) fn lifecycle_builder(
    protocol_state: ConnectionProtocolState,
    runtime: std::sync::Arc<crate::agent::BitfunAppRuntime>,
    transport_limits: TransportLimits,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    let initialize_state = protocol_state.clone();
    let health_state = protocol_state;
    let capabilities = registered_capabilities(permission_owner_available(&runtime));
    AppServer
        .builder()
        .name("app lifecycle handlers")
        .on_receive_request(
            async move |request: InitializeRequest, responder, _cx| {
                if initialize_state.current() != ProtocolNegotiation::Pending {
                    return responder.respond_with_result(Err(protocol_negotiation_error()));
                }
                if request.protocol_version < MIN_PROTOCOL_VERSION
                    || request.protocol_version > PROTOCOL_VERSION
                {
                    let result = responder.respond_with_result(Err(protocol_negotiation_error()));
                    if result.is_ok() {
                        initialize_state.reject();
                    }
                    return result;
                }
                let subscriptions =
                    crate::server::event_forwarder::EventSubscriptions::subscribe(&runtime);
                if !initialize_state.begin_negotiation(subscriptions) {
                    return responder.respond_with_result(Err(protocol_negotiation_error()));
                }
                let result = responder.respond_with_result(Ok(InitializeResponse::new(
                    ServerInfo {
                        name: "bitfun-app-server".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    capabilities.clone(),
                    transport_limits.clone(),
                )));
                if result.is_ok() {
                    debug_assert!(
                        initialize_state.accept(),
                        "successful App Server initialize must finalize Negotiating as Accepted"
                    );
                } else {
                    initialize_state.abort_negotiation();
                }
                result
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_: HealthRequest, responder, _cx| {
                if health_state.current() == ProtocolNegotiation::Rejected {
                    return responder.respond_with_result(Err(protocol_negotiation_error()));
                }
                responder.respond(HealthResponse {
                    status: HealthStatus::Ready,
                    protocol_version: PROTOCOL_VERSION,
                })
            },
            agent_client_protocol::on_receive_request!(),
        )
}

pub(in crate::server) fn event_sync_builder(
    runtime: std::sync::Arc<crate::agent::BitfunAppRuntime>,
    event_state: std::sync::Arc<crate::server::ConnectionEventState>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    let capabilities = registered_capabilities(permission_owner_available(&runtime));
    let external_source_snapshot_available = capabilities.iter().any(|capability| {
        capability.id == EXTERNAL_SOURCES_CAPABILITY
            && matches!(capability.availability, CapabilityAvailability::Available)
    });
    AppServer
        .builder()
        .name("app event synchronization handlers")
        .on_receive_request(
            async move |request: SyncEventsRequest, responder, _cx| {
                let (pending_permissions, permission_cursor) = event_state
                    .capture_permission_snapshot(|| {
                        runtime.runtime().pending_permission_requests()
                    });
                let pending_permissions = crate::agent::runtime_call(pending_permissions)?;
                responder.respond(SyncEventsResponse {
                    cursors: request
                        .streams
                        .into_iter()
                        .map(|stream| {
                            if stream == bitfun_app_server_protocol::event::EventStream::Permission
                            {
                                permission_cursor.clone()
                            } else {
                                event_state.cursor(stream)
                            }
                        })
                        .collect(),
                    pending_permissions,
                    agent_snapshot_available: false,
                    config_snapshot_available: false,
                    external_source_snapshot_available,
                })
            },
            agent_client_protocol::on_receive_request!(),
        )
}

fn protocol_negotiation_error() -> Error {
    Error::invalid_params().data(
        serde_json::to_value(AppServerErrorData {
            kind: AppServerErrorKind::InvalidRequest,
            retryable: false,
            outcome_unknown: false,
            capability: Some("app.initialize".to_string()),
            request_id: None,
        })
        .unwrap_or(serde_json::Value::Null),
    )
}

fn permission_owner_available(runtime: &crate::agent::BitfunAppRuntime) -> bool {
    runtime.runtime().pending_permission_requests().is_ok()
}

fn registered_capabilities(permission_owner_available: bool) -> Vec<CapabilityDescriptor> {
    let mut capabilities = [
        (
            "agent",
            vec![
                "agent/createSession",
                "agent/listSessions",
                "agent/deleteSession",
                "agent/submitTurn",
                "agent/submitDialogTurn",
                "agent/steerTurn",
                "agent/runUserShellCommand",
                "agent/submitUserAnswers",
                "agent/cancelTurn",
                "agent/run",
                "agent/event",
            ],
        ),
        (
            "session",
            vec![
                "session/sync",
                "session/readTranscript",
                "session/resolveWorkspace",
                "session/rename",
                "session/setArchived",
                "session/updateModel",
                "session/updateMode",
                "session/fork",
                "session/forkAtTurn",
                "session/forkBeforeTurn",
                "session/restore",
                "session/compact",
                "session/undo",
                "session/redo",
                "session/reloadContext",
                "session/usage",
                "session/waitForSettlement",
                "session/lineage",
                "session/inspectLineage",
                "session/cancelLineage",
            ],
        ),
        (
            "permission",
            vec![
                "agent/permissionEvent",
                "agent/respondPermission",
                "agent/respondPermissionBatch",
                "agent/listPendingPermissionRequests",
                "agent/listProjectPermissionGrants",
                "agent/removeProjectPermissionGrant",
                "agent/clearProjectPermissionGrants",
                "agent/listProjectPermissionAudit",
            ],
        ),
        (
            "workspace",
            vec![
                "workspace/diff",
                "workspace/searchReferences",
                "workspace/messageReferences",
            ],
        ),
        (
            "git",
            vec!["git/isRepository", "git/getStatus", "git/getBranches"],
        ),
        (
            "config",
            vec![
                "config/event",
                "config/getAgentProfileConfigs",
                "config/getAgentProfileConfig",
                "config/getModelConfigs",
                "config/getTuiModelCatalog",
                "model/projectReasoningCatalog",
                "config/getConfig",
                "config/getConfigs",
                "config/setConfig",
                "config/saveCloudSpeechConfig",
                "config/validateConfig",
                "config/setAgentProfileConfig",
                "config/resetAgentProfileConfig",
            ],
        ),
        (
            "i18n",
            vec![
                "i18n/getCurrentLanguage",
                "i18n/setLanguage",
                "i18n/getConfig",
                "i18n/setConfig",
                "i18n/getSupportedLanguages",
            ],
        ),
        ("eventSync", vec!["app/syncEvents", "app/eventStreamState"]),
    ]
    .into_iter()
    .map(|(id, methods)| {
        let availability = if id == "permission" && !permission_owner_available {
            CapabilityAvailability::Unavailable {
                reason: "The Runtime did not provide a Permission request manager".to_string(),
            }
        } else {
            CapabilityAvailability::Available
        };
        CapabilityDescriptor {
            id: id.to_string(),
            availability,
            methods: methods.into_iter().map(str::to_string).collect(),
        }
    })
    .collect::<Vec<_>>();
    capabilities.extend(
        crate::management::AppManagementCapabilities::unavailable(
            "The Host did not provide management owners",
        )
        .descriptors(),
    );
    capabilities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_without_management_owners_declares_capabilities_unavailable() {
        let capabilities = registered_capabilities(true);
        for id in [
            "tui.modes",
            "tui.models",
            "tui.skills",
            "tui.subagents",
            "tui.mcp",
            EXTERNAL_SOURCES_CAPABILITY,
            crate::management::ACCOUNT_CAPABILITY,
            crate::management::SETTINGS_SYNC_CAPABILITY,
            crate::management::WORKTREES_CAPABILITY,
        ] {
            let capability = capabilities
                .iter()
                .find(|capability| capability.id == id)
                .expect("management capability should be declared");
            assert!(matches!(
                capability.availability,
                CapabilityAvailability::Unavailable { .. }
            ));
        }
    }
}
