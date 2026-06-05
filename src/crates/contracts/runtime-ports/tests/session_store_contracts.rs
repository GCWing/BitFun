use std::path::PathBuf;

use bitfun_runtime_ports::{
    RuntimeServiceCapability, RuntimeServicePort, SessionStorageKind, SessionStoragePathRequest,
    SessionStoragePathResolution, SessionStorePort, SessionTurnLoadTiming,
    SessionViewRestoreTiming,
};

#[test]
fn session_storage_path_resolution_carries_local_and_remote_facts() {
    let local = SessionStoragePathResolution::new(
        PathBuf::from("/repo"),
        PathBuf::from("/repo"),
        SessionStorageKind::Local,
        None,
        None,
    );
    assert_eq!(local.storage_kind, SessionStorageKind::Local);
    assert!(!local.is_remote_storage());

    let remote = SessionStoragePathResolution::new(
        PathBuf::from("/repo"),
        PathBuf::from("/state/sessions/remote"),
        SessionStorageKind::Remote,
        Some("conn-1".to_string()),
        Some("devbox".to_string()),
    );
    assert_eq!(remote.storage_kind, SessionStorageKind::Remote);
    assert!(remote.is_remote_storage());

    let encoded = serde_json::to_string(&remote).expect("resolution should serialize");
    assert!(encoded.contains("effectiveStoragePath"));
    assert!(encoded.contains("remoteConnectionId"));
}

#[test]
fn session_restore_timing_serializes_camel_case_fields() {
    let timing = SessionViewRestoreTiming {
        resolve_storage_path_duration_ms: 1,
        visibility_metadata_duration_ms: 2,
        load_session_with_turns_duration_ms: 3,
        normalize_turn_ids_duration_ms: 4,
        total_duration_ms: 5,
        turn_load: SessionTurnLoadTiming {
            requested_tail_turn_count: Some(8),
            loaded_turn_count: 8,
            total_turn_count: 10,
            turn_file_count: 10,
            missing_turn_file_count: 0,
            fast_path: true,
            metadata_duration_ms: 1,
            state_duration_ms: 1,
            scan_duration_ms: 0,
            read_duration_ms: 2,
            max_turn_read_duration_ms: 1,
            build_session_duration_ms: 1,
            total_duration_ms: 5,
        },
    };

    let encoded = serde_json::to_value(&timing).expect("timing should serialize");
    assert_eq!(encoded["resolveStoragePathDurationMs"], 1);
    assert_eq!(encoded["turnLoad"]["requestedTailTurnCount"], 8);
    assert_eq!(encoded["turnLoad"]["fastPath"], true);
}

struct ContractSessionStorePort;

impl RuntimeServicePort for ContractSessionStorePort {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::SessionStore
    }
}

#[async_trait::async_trait]
impl SessionStorePort for ContractSessionStorePort {
    async fn resolve_session_storage_path(
        &self,
        request: SessionStoragePathRequest,
    ) -> bitfun_runtime_ports::PortResult<SessionStoragePathResolution> {
        Ok(SessionStoragePathResolution::new(
            request.workspace_path.clone(),
            request.workspace_path,
            SessionStorageKind::Local,
            request.remote_connection_id,
            request.remote_ssh_host,
        ))
    }
}

#[tokio::test]
async fn session_store_port_exposes_typed_storage_path_resolution() {
    let port = ContractSessionStorePort;
    assert_eq!(port.capability(), RuntimeServiceCapability::SessionStore);

    let resolution = port
        .resolve_session_storage_path(SessionStoragePathRequest {
            workspace_path: PathBuf::from("/workspace"),
            remote_connection_id: None,
            remote_ssh_host: None,
        })
        .await
        .expect("path should resolve");

    assert_eq!(
        resolution.effective_storage_path,
        PathBuf::from("/workspace")
    );
}
