#![cfg(feature = "workspace-runtime")]

use bitfun_runtime_ports::{RuntimeEventEnvelope, RuntimeEventType, RuntimeServiceCapability};
use bitfun_services_core::local_runtime_ports::LocalRuntimePorts;

#[tokio::test]
async fn local_runtime_ports_bind_one_canonical_workspace_and_runtime_facts() {
    let workspace = tempfile::tempdir().expect("workspace");
    let ports = LocalRuntimePorts::new(workspace.path(), 8).expect("local runtime ports");

    assert_eq!(
        ports.workspace_root(),
        dunce::canonicalize(workspace.path()).unwrap()
    );
    assert_eq!(
        ports.filesystem().capability(),
        RuntimeServiceCapability::FileSystem
    );
    assert_eq!(
        ports.workspace().capability(),
        RuntimeServiceCapability::Workspace
    );
    assert!(ports.clock().now_unix_millis() > 0);
    ports
        .events()
        .publish_runtime_event(RuntimeEventEnvelope {
            session_id: "local-runtime-test".to_string(),
            turn_id: None,
            source: None,
            event_type: RuntimeEventType::SessionStateChanged,
            payload: serde_json::json!({ "status": "ready" }),
        })
        .await
        .expect("publish local runtime event");
}

#[test]
fn local_runtime_ports_reject_a_missing_workspace() {
    let temp = tempfile::tempdir().expect("tempdir");
    let error = LocalRuntimePorts::new(temp.path().join("missing"), 8)
        .expect_err("missing workspace must fail");

    assert!(error.to_string().contains("workspace"), "{error}");
}
