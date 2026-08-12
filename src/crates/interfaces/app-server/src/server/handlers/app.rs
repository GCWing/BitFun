use agent_client_protocol::{Builder, Error, HandleDispatchFrom};
use bitfun_app_server_protocol::app::{
    CapabilityAvailability, CapabilityDescriptor, HealthRequest, HealthResponse, HealthStatus,
    InitializeRequest, InitializeResponse, ServerInfo, TransportLimits,
};
use bitfun_app_server_protocol::error::{AppServerErrorData, AppServerErrorKind};
use bitfun_app_server_protocol::event::{SyncEventsRequest, SyncEventsResponse};
use bitfun_app_server_protocol::{MIN_PROTOCOL_VERSION, PROTOCOL_VERSION};

use crate::management::EXTERNAL_SOURCES_CAPABILITY;
use crate::role::{AppClient, AppServer};

const MAX_FRAME_BYTES: u64 = 16 * 1024 * 1024;
const EVENT_BUFFER_CAPACITY: u32 = 1024;

pub(in crate::server) fn builder(
    runtime: std::sync::Arc<crate::agent::BitfunAppRuntime>,
    event_state: std::sync::Arc<crate::server::ConnectionEventState>,
    management: Option<std::sync::Arc<crate::management::AppManagementService>>,
) -> Builder<AppServer, impl HandleDispatchFrom<AppClient>> {
    let capabilities = registered_capabilities(management.as_deref());
    let external_source_snapshot_available = capabilities.iter().any(|capability| {
        capability.id == EXTERNAL_SOURCES_CAPABILITY
            && matches!(capability.availability, CapabilityAvailability::Available)
    });
    AppServer
        .builder()
        .name("app lifecycle handlers")
        .on_receive_request(
            async move |request: InitializeRequest, responder, _cx| {
                if request.protocol_version < MIN_PROTOCOL_VERSION
                    || request.protocol_version > PROTOCOL_VERSION
                {
                    return responder.respond_with_result(Err(Error::invalid_params().data(
                        serde_json::to_value(AppServerErrorData {
                            kind: AppServerErrorKind::InvalidRequest,
                            retryable: false,
                            outcome_unknown: false,
                            capability: Some("app.initialize".to_string()),
                            request_id: None,
                        })
                        .unwrap_or(serde_json::Value::Null),
                    )));
                }
                responder.respond_with_result(Ok(InitializeResponse::new(
                    ServerInfo {
                        name: "bitfun-app-server".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    capabilities.clone(),
                    TransportLimits {
                        max_frame_bytes: MAX_FRAME_BYTES,
                        event_buffer_capacity: EVENT_BUFFER_CAPACITY,
                    },
                )))
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |_: HealthRequest, responder, _cx| {
                responder.respond(HealthResponse {
                    status: HealthStatus::Ready,
                    protocol_version: PROTOCOL_VERSION,
                })
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: SyncEventsRequest, responder, _cx| {
                let pending_permissions = runtime
                    .runtime()
                    .pending_permission_requests()
                    .unwrap_or_default();
                responder.respond(SyncEventsResponse {
                    cursors: request
                        .streams
                        .into_iter()
                        .map(|stream| event_state.cursor(stream))
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

fn registered_capabilities(
    management: Option<&crate::management::AppManagementService>,
) -> Vec<CapabilityDescriptor> {
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
    .map(|(id, methods)| CapabilityDescriptor {
        id: id.to_string(),
        availability: CapabilityAvailability::Available,
        methods: methods.into_iter().map(str::to_string).collect(),
    })
    .collect::<Vec<_>>();
    capabilities.extend(
        management
            .map(|service| service.capabilities())
            .unwrap_or_else(|| {
                crate::management::AppManagementCapabilities::unavailable(
                    "The Host did not provide management owners",
                )
            })
            .descriptors(),
    );
    capabilities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_host_management_service_declares_capabilities_unavailable() {
        let capabilities = registered_capabilities(None);
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
