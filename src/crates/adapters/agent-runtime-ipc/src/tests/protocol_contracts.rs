use crate::{InitializeRequest, RuntimeIpcFrame, RuntimeIpcOperation, PROTOCOL_VERSION};

#[test]
fn protocol_rejects_unknown_fields_and_operations() {
    let unknown_field =
        r#"{"type":"request","request_id":1,"operation":{"operation":"health"},"metadata":{}}"#
            .to_string();
    assert!(serde_json::from_str::<RuntimeIpcFrame>(&unknown_field).is_err());

    let unknown_operation =
        r#"{"type":"request","request_id":1,"operation":{"operation":"list_sessions"}}"#;
    assert!(serde_json::from_str::<RuntimeIpcFrame>(unknown_operation).is_err());
}

#[test]
fn initialize_debug_redacts_the_bearer_token() {
    let request = InitializeRequest {
        protocol_version: PROTOCOL_VERSION,
        instance_identity: "a".repeat(64),
        token: "top-secret-token".to_string(),
        client_id: "foundation-test".to_string(),
        client_version: "0.1.0".to_string(),
    };

    let debug = format!("{request:?}");
    assert!(!debug.contains("top-secret-token"));
    assert!(debug.contains("[REDACTED]"));

    let frame = RuntimeIpcFrame::Request {
        request_id: 7,
        operation: RuntimeIpcOperation::Health,
    };
    let json = serde_json::to_string(&frame).expect("serialize Health frame");
    assert_eq!(
        json,
        r#"{"type":"request","request_id":7,"operation":{"operation":"health"}}"#
    );
}
