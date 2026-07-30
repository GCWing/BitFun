use crate::operation::RuntimeIpcSessionRequirement;
use crate::{
    serialize_frame_with_limit, InitializeRequest, RuntimeIpcError, RuntimeIpcErrorCode,
    RuntimeIpcFrame, RuntimeIpcOperation, RuntimeSessionRenameRequest, RuntimeUserAnswersRequest,
    MAX_REQUEST_FRAME_BYTES, PROTOCOL_VERSION,
};

use bitfun_product_domains::tool_permissions::PermissionReply;
use bitfun_runtime_ports::{
    AgentContextReloadRequest, AgentContextReloadTarget, AgentDialogTurnRequest,
    AgentSessionModeUpdateRequest, AgentSessionModelUpdateRequest, AgentSubmissionSource,
    DialogSubmissionPolicy,
};
use serde_json::{json, Map};

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
}

#[test]
fn protocol_round_trips_reviewed_permission_and_user_input_operations() {
    let operations = vec![
        RuntimeIpcOperation::PendingPermissions {
            session_id: "session-1".to_string(),
        },
        RuntimeIpcOperation::RespondPermission {
            session_id: "session-1".to_string(),
            request_id: "permission-1".to_string(),
            reply: PermissionReply::Once,
        },
        RuntimeIpcOperation::SubmitUserAnswers {
            request: RuntimeUserAnswersRequest {
                session_id: "session-1".to_string(),
                tool_id: "question-1".to_string(),
                answers: json!({"choice": "yes"}),
            },
        },
    ];

    for operation in operations {
        let encoded = serde_json::to_value(&operation).expect("serialize operation");
        let decoded: RuntimeIpcOperation =
            serde_json::from_value(encoded).expect("deserialize operation");
        assert_eq!(decoded, operation);
    }
}

#[test]
fn protocol_round_trips_the_reviewed_session_mode_operation() {
    let operation = RuntimeIpcOperation::UpdateSessionMode {
        request: AgentSessionModeUpdateRequest {
            session_id: "session-1".to_string(),
            mode_id: "ask".to_string(),
        },
    };

    let encoded = serde_json::to_value(&operation).expect("serialize mode update");
    assert_eq!(encoded["operation"], "update_session_mode");
    assert_eq!(encoded["request"]["sessionId"], "session-1");
    assert_eq!(encoded["request"]["modeId"], "ask");
    let decoded: RuntimeIpcOperation =
        serde_json::from_value(encoded).expect("deserialize mode update");

    assert_eq!(decoded, operation);
    assert_eq!(decoded.session_id(), Some("session-1"));
    assert_eq!(
        decoded.rules().session_requirement,
        RuntimeIpcSessionRequirement::CurrentController
    );
}

#[test]
fn protocol_round_trips_the_reviewed_session_model_operation() {
    let operation = RuntimeIpcOperation::UpdateSessionModel {
        request: AgentSessionModelUpdateRequest {
            session_id: "session-1".to_string(),
            model_id: "provider/model".to_string(),
        },
    };

    let encoded = serde_json::to_value(&operation).expect("serialize model update");
    assert_eq!(encoded["operation"], "update_session_model");
    assert_eq!(encoded["request"]["sessionId"], "session-1");
    assert_eq!(encoded["request"]["modelId"], "provider/model");
    let decoded: RuntimeIpcOperation =
        serde_json::from_value(encoded).expect("deserialize model update");

    assert_eq!(decoded, operation);
    assert_eq!(decoded.session_id(), Some("session-1"));
    assert_eq!(
        decoded.rules().session_requirement,
        RuntimeIpcSessionRequirement::CurrentController
    );
}

#[test]
fn protocol_round_trips_the_current_session_rename_operation() {
    assert_eq!(PROTOCOL_VERSION, 7);

    let operation = RuntimeIpcOperation::RenameSession {
        request: RuntimeSessionRenameRequest {
            session_id: "session-1".to_string(),
            session_name: "Auth refactor".to_string(),
        },
    };

    let encoded = serde_json::to_value(&operation).expect("serialize session rename");
    assert_eq!(
        encoded,
        json!({
            "operation": "rename_session",
            "request": {
                "sessionId": "session-1",
                "sessionName": "Auth refactor"
            }
        })
    );
    let decoded: RuntimeIpcOperation =
        serde_json::from_value(encoded).expect("deserialize session rename");

    assert_eq!(decoded, operation);
    assert_eq!(decoded.session_id(), Some("session-1"));
    assert_eq!(
        decoded.rules().session_requirement,
        RuntimeIpcSessionRequirement::CurrentController
    );
}

#[test]
fn protocol_round_trips_session_delete_and_not_found() {
    let operation = RuntimeIpcOperation::DeleteSession {
        session_id: "session-2".to_string(),
    };
    let encoded = serde_json::to_value(&operation).expect("serialize session delete");
    assert_eq!(
        encoded,
        json!({
            "operation": "delete_session",
            "sessionId": "session-2"
        })
    );
    let decoded: RuntimeIpcOperation =
        serde_json::from_value(encoded).expect("deserialize session delete");
    assert_eq!(decoded, operation);
    assert_eq!(decoded.session_id(), Some("session-2"));

    let frame = RuntimeIpcFrame::Error {
        request_id: Some(7),
        error: RuntimeIpcError {
            code: RuntimeIpcErrorCode::NotFound,
            message: "session not found".to_string(),
        },
    };
    let encoded = serde_json::to_value(&frame).expect("serialize not-found error");
    assert_eq!(encoded["error"]["code"], "not_found");
    let decoded: RuntimeIpcFrame =
        serde_json::from_value(encoded).expect("deserialize not-found error");
    assert_eq!(decoded, frame);
}

#[test]
fn protocol_round_trips_context_reload_as_a_controller_operation() {
    let operation = RuntimeIpcOperation::ReloadSessionContext {
        request: AgentContextReloadRequest {
            session_id: "session-1".to_string(),
            target: AgentContextReloadTarget::Instructions,
        },
    };

    let encoded = serde_json::to_value(&operation).expect("serialize context reload");
    assert_eq!(encoded["operation"], "reload_session_context");
    assert_eq!(encoded["request"]["sessionId"], "session-1");
    assert_eq!(encoded["request"]["target"], "instructions");
    let decoded: RuntimeIpcOperation =
        serde_json::from_value(encoded).expect("deserialize context reload");

    assert_eq!(decoded, operation);
    assert_eq!(decoded.session_id(), Some("session-1"));
    let rules = decoded.rules();
    assert_eq!(
        rules.session_requirement,
        RuntimeIpcSessionRequirement::CurrentController
    );
    assert!(!rules.requires_idle);
    assert!(!rules.serializes_session_selection);
    assert!(rules.side_effecting);
}

#[test]
fn session_mode_operation_rejects_unknown_envelope_fields() {
    let unknown_field = json!({
        "operation": "update_session_mode",
        "request": {
            "sessionId": "session-1",
            "modeId": "ask"
        },
        "metadata": {}
    });

    assert!(serde_json::from_value::<RuntimeIpcOperation>(unknown_field).is_err());
}

#[test]
fn submit_turn_accepts_the_existing_64_kib_tui_paste_contract() {
    let frame = RuntimeIpcFrame::Request {
        request_id: 1,
        operation: RuntimeIpcOperation::SubmitTurn {
            request: AgentDialogTurnRequest {
                session_id: "session-1".to_string(),
                message: "x".repeat(64 * 1024),
                original_message: None,
                turn_id: Some("turn-1".to_string()),
                agent_type: "agentic".to_string(),
                workspace_path: Some("D:/workspace/project".to_string()),
                remote_connection_id: None,
                remote_ssh_host: None,
                policy: DialogSubmissionPolicy::for_source(AgentSubmissionSource::Cli),
                reply_route: None,
                prepended_reminders: Vec::new(),
                attachments: Vec::new(),
                metadata: Map::new(),
            },
        },
    };

    serialize_frame_with_limit(&frame, MAX_REQUEST_FRAME_BYTES)
        .expect("64 KiB TUI input plus its typed envelope must fit the request frame");
}
