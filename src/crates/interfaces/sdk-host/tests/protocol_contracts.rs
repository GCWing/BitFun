use openbitfun_sdk_host::protocol::{
    ErrorCode, ErrorData, ErrorStage, HostCapabilities, InitializeParams, InitializeResult,
    JsonRpcErrorResponse, JsonRpcRequest, JsonRpcSuccessResponse, OutcomeCertainty,
    PermissionDecision, PermissionRespondParams, PermissionSource, PermissionSourceKind,
    QueryEvent, QueryOutput, QueryResultError, QueryResultParams, QueryStartParams,
    QueryTerminalStatus, QueryUsage, RecoveryAction, RequestId, SessionLifetime,
    SessionResumeParams, Stability, TemporaryModelConfig, TemporaryModelProvider, ToolEventStatus,
    PROTOCOL_VERSION,
};

#[test]
fn initialize_contract_is_versioned_and_binds_one_temporary_model() {
    let request: JsonRpcRequest = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": 6,
            "clientInfo": { "name": "fixture", "version": "0.1.0" },
            "capabilities": {
                "serverNotifications": true,
                "permissionResponses": true
            },
            "model": {
                "provider": "openai",
                "model": "fixture-model",
                "apiKey": "fixture-secret",
                "baseUrl": "http://127.0.0.1:43123/v1"
            }
        }
    }))
    .unwrap();
    let params: InitializeParams = request.params_as().unwrap();

    assert_eq!(request.id, Some(RequestId::Number(1)));
    assert_eq!(PROTOCOL_VERSION, 6);
    assert_eq!(params.protocol_version, PROTOCOL_VERSION);
    assert!(params.capabilities.server_notifications);
    assert!(params.capabilities.permission_responses);
    assert_eq!(params.model.provider, TemporaryModelProvider::Openai);
    assert_eq!(params.model.model, "fixture-model");
    assert_eq!(params.model.api_key, "fixture-secret");
    assert_eq!(
        params.model.base_url.as_deref(),
        Some("http://127.0.0.1:43123/v1")
    );

    for provider in ["openai", "responses", "anthropic", "gemini"] {
        let model: TemporaryModelConfig = serde_json::from_value(serde_json::json!({
            "provider": provider,
            "model": "fixture-model",
            "apiKey": "fixture-secret"
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(model.provider).unwrap(),
            serde_json::json!(provider)
        );
    }
    assert!(
        serde_json::from_value::<TemporaryModelConfig>(serde_json::json!({
            "provider": "unknown",
            "model": "fixture-model",
            "apiKey": "fixture-secret"
        }))
        .is_err()
    );

    let result = InitializeResult::current("0.2.13", "sdk:openai:fixture");
    assert_eq!(result.protocol_version, PROTOCOL_VERSION);
    assert_eq!(result.stability, Stability::NotDelivered);
    assert_eq!(
        result.capabilities,
        HostCapabilities {
            session_create: true,
            session_create_lifetime: SessionLifetime::Durable,
            session_resume: true,
            query: true,
            query_cancel: true,
            session_close: true,
            event_stream: true,
            tool_events: true,
            image_input: true,
            structured_output: true,
            usage: true,
            custom_tools: false,
            permission_responses: true,
            hooks: false,
            mcp_configuration: false,
            prestarted_transport: false,
        }
    );
    let result_json = serde_json::to_string(&result).unwrap();
    assert!(result_json.contains("\"modelId\":\"sdk:openai:fixture\""));
    assert!(!result_json.contains("fixture-secret"));
}

#[test]
fn current_host_capabilities_are_a_deliberate_subset_of_the_headless_cli_target() {
    let capabilities = HostCapabilities::current();

    assert!(capabilities.session_create);
    assert!(capabilities.query);
    assert!(capabilities.query_cancel);
    assert!(capabilities.session_close);
    assert!(capabilities.event_stream);
    assert!(capabilities.tool_events);
    assert!(capabilities.image_input);

    assert_eq!(
        capabilities.session_create_lifetime,
        SessionLifetime::Durable
    );
    assert!(capabilities.structured_output);
    assert!(capabilities.usage);
    assert!(!capabilities.custom_tools);
    assert!(capabilities.permission_responses);
    assert!(!capabilities.hooks);
    assert!(!capabilities.mcp_configuration);
    assert!(!capabilities.prestarted_transport);
}

#[test]
fn query_input_carries_only_text_and_local_image_paths() {
    let params: QueryStartParams = serde_json::from_value(serde_json::json!({
        "prompt": "describe these images",
        "images": ["screenshots/one.png", "D:/captures/two.jpg"]
    }))
    .unwrap();

    assert_eq!(params.prompt, "describe these images");
    assert_eq!(
        params.images,
        vec!["screenshots/one.png", "D:/captures/two.jpg"]
    );
    assert!(
        serde_json::from_value::<QueryStartParams>(serde_json::json!({
            "prompt": "unsupported payload",
            "imageUrl": "https://example.com/image.png"
        }))
        .is_err()
    );
}

#[test]
fn query_input_accepts_one_turn_scoped_output_schema() {
    let params: QueryStartParams = serde_json::from_value(serde_json::json!({
        "prompt": "summarize the repository",
        "outputSchema": {
            "type": "object",
            "properties": {
                "summary": { "type": "string" }
            },
            "required": ["summary"],
            "additionalProperties": false
        }
    }))
    .unwrap();

    assert_eq!(
        params.output_schema,
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string" }
            },
            "required": ["summary"],
            "additionalProperties": false
        }))
    );
}

#[test]
fn durable_sessions_have_one_minimal_resume_contract() {
    let params: SessionResumeParams = serde_json::from_value(serde_json::json!({
        "sessionId": "session-1"
    }))
    .unwrap();

    assert_eq!(params.session_id, "session-1");
    assert_eq!(
        serde_json::to_value(SessionLifetime::Durable).unwrap(),
        serde_json::json!("durable")
    );
    assert!(HostCapabilities::current().session_resume);
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

    let tool_event = serde_json::to_value(QueryEvent::ToolEvent {
        tool_call_id: "tool-1".to_string(),
        tool_name: "Read".to_string(),
        status: ToolEventStatus::Started,
        progress: None,
        duration_ms: None,
    })
    .unwrap();
    assert_eq!(
        tool_event,
        serde_json::json!({
            "type": "tool_event",
            "toolCallId": "tool-1",
            "toolName": "Read",
            "status": "started"
        })
    );
    assert!(tool_event.get("params").is_none());
    assert!(tool_event.get("result").is_none());

    let permission_event = serde_json::to_value(QueryEvent::PermissionRequest {
        request_id: "permission-1".to_string(),
        action: "edit".to_string(),
        resources: vec!["src/lib.rs".to_string()],
        source: PermissionSource {
            kind: PermissionSourceKind::ToolCall,
            identity: "edit".to_string(),
        },
        tool_call_id: Some("tool-1".to_string()),
        response_timeout_ms: 120_000,
    })
    .unwrap();
    assert_eq!(permission_event["type"], "permission_request");
    assert_eq!(permission_event["requestId"], "permission-1");
    assert_eq!(permission_event["source"]["kind"], "tool_call");
    assert_eq!(permission_event["responseTimeoutMs"], 120_000);

    let respond: PermissionRespondParams = serde_json::from_value(serde_json::json!({
        "queryId": "query-1",
        "sessionId": "session-1",
        "turnId": "turn-1",
        "operationId": "operation-1",
        "requestId": "permission-1",
        "decision": "allow_once"
    }))
    .unwrap();
    assert_eq!(respond.decision, PermissionDecision::AllowOnce);
    assert!(respond.feedback.is_none());

    let result = serde_json::to_value(QueryResultParams {
        query_id: "query-1".to_string(),
        session_id: "session-1".to_string(),
        turn_id: "turn-1".to_string(),
        operation_id: "operation-1".to_string(),
        status: QueryTerminalStatus::Failed,
        output: QueryOutput {
            text: "partial response".to_string(),
            structured: None,
        },
        usage: Some(QueryUsage {
            input_tokens: 100,
            output_tokens: Some(25),
            total_tokens: 125,
            cached_tokens: Some(40),
        }),
        error: Some(QueryResultError {
            message: "Permission approval is required".to_string(),
            data: ErrorData {
                code: ErrorCode::ActionRequired,
                stage: ErrorStage::Query,
                retryable: false,
                correlation_id: "query:query-1".to_string(),
                operation_id: Some("operation-1".to_string()),
                causation_id: None,
                outcome_certainty: OutcomeCertainty::Committed,
                recovery: None,
            },
        }),
    })
    .unwrap();
    assert_eq!(result["error"]["data"]["code"], "action_required");
    assert_eq!(result["error"]["data"]["stage"], "query");
    assert_eq!(result["operationId"], "operation-1");
    assert_eq!(result["output"]["text"], "partial response");
    assert_eq!(result["usage"]["inputTokens"], 100);
    assert_eq!(result["usage"]["cachedTokens"], 40);
    assert_eq!(result["error"]["data"]["outcomeCertainty"], "committed");
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
            operation_id: Some("operation-2".to_string()),
            causation_id: None,
            outcome_certainty: OutcomeCertainty::NotStarted,
            recovery: Some(RecoveryAction::Retry),
        },
    );
    let value = serde_json::to_value(error).unwrap();
    assert_eq!(value["error"]["data"]["code"], "overloaded");
    assert_eq!(value["error"]["data"]["stage"], "query");
    assert_eq!(value["error"]["data"]["recovery"], "retry");
    assert_eq!(value["error"]["data"]["retryable"], true);
    assert_eq!(value["error"]["data"]["outcomeCertainty"], "not_started");
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

#[test]
fn json_rpc_request_debug_redacts_temporary_model_secret() {
    let request: JsonRpcRequest = serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": 3,
            "clientInfo": { "name": "fixture", "version": "0.1.0" },
            "capabilities": {
                "serverNotifications": true,
                "permissionResponses": true
            },
            "model": {
                "provider": "openai",
                "model": "fixture-model",
                "apiKey": "openbitfun-sdk-debug-secret-31d4"
            }
        }
    }))
    .unwrap();

    let debug = format!("{request:?}");
    assert!(debug.contains("initialize"));
    assert!(!debug.contains("openbitfun-sdk-debug-secret-31d4"));
}
