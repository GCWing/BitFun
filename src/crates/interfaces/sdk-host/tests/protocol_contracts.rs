use bitfun_sdk_host::protocol::{
    ErrorCode, ErrorData, ErrorStage, HostCapabilities, InitializeParams, InitializeResult,
    JsonRpcErrorResponse, JsonRpcRequest, JsonRpcSuccessResponse, QueryEvent, QueryResultError,
    QueryResultParams, QueryTerminalStatus, RecoveryAction, RequestId, SessionLifetime, Stability,
    PROTOCOL_VERSION,
};

#[test]
fn initialize_contract_is_versioned_and_uses_familiar_capability_names() {
    let request: JsonRpcRequest = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientInfo": { "name": "fixture", "version": "0.1.0" },
            "capabilities": { "serverNotifications": true }
        }
    }))
    .unwrap();
    let params: InitializeParams = request.params_as().unwrap();

    assert_eq!(request.id, Some(RequestId::Number(1)));
    assert_eq!(params.protocol_version, PROTOCOL_VERSION);
    assert!(params.capabilities.server_notifications);

    let result = InitializeResult::current("0.2.13");
    assert_eq!(result.protocol_version, PROTOCOL_VERSION);
    assert_eq!(result.stability, Stability::NotDelivered);
    assert_eq!(
        result.capabilities,
        HostCapabilities {
            session_create: true,
            session_create_lifetime: SessionLifetime::Connection,
            query: true,
            query_cancel: true,
            session_close: true,
            event_stream: true,
            structured_output: false,
            usage: false,
            custom_tools: false,
            permission_callbacks: false,
            hooks: false,
            mcp_configuration: false,
            prestarted_transport: false,
        }
    );
}

#[test]
fn current_host_capabilities_are_a_deliberate_subset_of_the_headless_cli_target() {
    let capabilities = HostCapabilities::current();

    assert!(capabilities.session_create);
    assert!(capabilities.query);
    assert!(capabilities.query_cancel);
    assert!(capabilities.session_close);
    assert!(capabilities.event_stream);

    assert_eq!(
        capabilities.session_create_lifetime,
        SessionLifetime::Connection
    );
    assert!(!capabilities.structured_output);
    assert!(!capabilities.usage);
    assert!(!capabilities.custom_tools);
    assert!(!capabilities.permission_callbacks);
    assert!(!capabilities.hooks);
    assert!(!capabilities.mcp_configuration);
    assert!(!capabilities.prestarted_transport);
}

#[test]
fn query_events_and_terminal_errors_are_closed_protocol_values() {
    let event = serde_json::to_value(QueryEvent::AssistantTextDelta {
        text: "hello".to_string(),
    })
    .unwrap();
    assert_eq!(
        event,
        serde_json::json!({ "type": "assistant_text_delta", "text": "hello" })
    );

    let result = serde_json::to_value(QueryResultParams {
        query_id: "query-1".to_string(),
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        status: QueryTerminalStatus::Failed,
        error: Some(QueryResultError {
            message: "Permission approval is required".to_string(),
            data: ErrorData {
                code: ErrorCode::ActionRequired,
                stage: ErrorStage::Query,
                retryable: false,
                correlation_id: "query:query-1".to_string(),
                recovery: None,
            },
        }),
    })
    .unwrap();
    assert_eq!(result["error"]["data"]["code"], "action_required");
    assert_eq!(result["error"]["data"]["stage"], "query");
    assert_eq!(
        result["error"]["message"],
        "Permission approval is required"
    );
    assert_eq!(
        serde_json::to_value(ErrorCode::ProviderQuota).unwrap(),
        "provider_quota"
    );
    assert_eq!(
        serde_json::to_value(ErrorCode::ProviderBilling).unwrap(),
        "provider_billing"
    );
    assert_eq!(
        serde_json::to_value(ErrorCode::CleanupRequired).unwrap(),
        "cleanup_required"
    );
}

#[test]
fn success_and_error_envelopes_are_strict_json_rpc() {
    let success = JsonRpcSuccessResponse::new(
        RequestId::String("request-1".to_string()),
        serde_json::json!({ "accepted": true }),
    );
    assert_eq!(
        serde_json::to_value(success).unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "request-1",
            "result": { "accepted": true }
        })
    );

    let error = JsonRpcErrorResponse::new(
        RequestId::Number(2),
        -32003,
        "SDK Host is overloaded",
        ErrorData {
            code: ErrorCode::Overloaded,
            stage: ErrorStage::Query,
            retryable: true,
            correlation_id: "request:2".to_string(),
            recovery: Some(RecoveryAction::Retry),
        },
    );
    let value = serde_json::to_value(error).unwrap();
    assert_eq!(value["error"]["data"]["code"], "overloaded");
    assert_eq!(value["error"]["data"]["stage"], "query");
    assert_eq!(value["error"]["data"]["recovery"], "retry");
    assert_eq!(value["error"]["data"]["retryable"], true);
}

#[test]
fn request_ids_reject_null_fractional_and_structured_values() {
    for id in [
        serde_json::Value::Null,
        serde_json::json!(1.5),
        serde_json::json!({ "nested": true }),
        serde_json::json!([1]),
    ] {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {}
        });
        assert!(serde_json::from_value::<JsonRpcRequest>(request).is_err());
    }
}

#[test]
fn request_correlation_ids_preserve_json_rpc_id_type() {
    assert_eq!(RequestId::Number(1).correlation_id(), "request:number:1");
    assert_eq!(
        RequestId::String("1".to_string()).correlation_id(),
        "request:string:1"
    );
}
