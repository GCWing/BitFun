//! JSON-RPC 2.0 request helper contracts for MCP.

use super::types::*;
use log::warn;
use serde_json::{json, Value};

fn serialize_params(method: &str, params: impl serde::Serialize) -> Option<Value> {
    match serde_json::to_value(params) {
        Ok(value) => Some(value),
        Err(err) => {
            warn!(
                "Failed to serialize MCP request params: method={}, error={}",
                method, err
            );
            None
        }
    }
}

/// Creates an `initialize` request.
pub fn create_initialize_request(
    id: u64,
    client_name: impl Into<String>,
    client_version: impl Into<String>,
) -> MCPRequest {
    let params = InitializeParams {
        protocol_version: super::types::default_protocol_version(),
        capabilities: MCPCapability::default(),
        client_info: MCPServerInfo {
            name: client_name.into(),
            version: client_version.into(),
            description: Some("BitFun MCP Client".to_string()),
            vendor: Some("BitFun".to_string()),
        },
    };

    MCPRequest::new(
        Value::Number(id.into()),
        "initialize".to_string(),
        serialize_params("initialize", params),
    )
}

/// Creates a `resources/list` request.
pub fn create_resources_list_request(id: u64, cursor: Option<String>) -> MCPRequest {
    let params = if cursor.is_some() {
        let params = ResourcesListParams { cursor };
        serialize_params("resources/list", params)
    } else {
        None
    };
    MCPRequest::new(
        Value::Number(id.into()),
        "resources/list".to_string(),
        params,
    )
}

/// Creates a `resources/read` request.
pub fn create_resources_read_request(id: u64, uri: impl Into<String>) -> MCPRequest {
    let params = ResourcesReadParams { uri: uri.into() };
    MCPRequest::new(
        Value::Number(id.into()),
        "resources/read".to_string(),
        serialize_params("resources/read", params),
    )
}

/// Creates a `prompts/list` request.
pub fn create_prompts_list_request(id: u64, cursor: Option<String>) -> MCPRequest {
    let params = if cursor.is_some() {
        let params = PromptsListParams { cursor };
        serialize_params("prompts/list", params)
    } else {
        None
    };
    MCPRequest::new(Value::Number(id.into()), "prompts/list".to_string(), params)
}

/// Creates a `prompts/get` request.
pub fn create_prompts_get_request(
    id: u64,
    name: impl Into<String>,
    arguments: Option<std::collections::HashMap<String, String>>,
) -> MCPRequest {
    let params = PromptsGetParams {
        name: name.into(),
        arguments,
    };
    MCPRequest::new(
        Value::Number(id.into()),
        "prompts/get".to_string(),
        serialize_params("prompts/get", params),
    )
}

/// Creates a `tools/list` request.
pub fn create_tools_list_request(id: u64, cursor: Option<String>) -> MCPRequest {
    let params = if cursor.is_some() {
        let params = ToolsListParams { cursor };
        serialize_params("tools/list", params)
    } else {
        None
    };
    MCPRequest::new(Value::Number(id.into()), "tools/list".to_string(), params)
}

/// Creates a `tools/call` request.
pub fn create_tools_call_request(
    id: u64,
    name: impl Into<String>,
    arguments: Option<Value>,
) -> MCPRequest {
    let params = ToolsCallParams {
        name: name.into(),
        arguments,
    };
    MCPRequest::new(
        Value::Number(id.into()),
        "tools/call".to_string(),
        serialize_params("tools/call", params),
    )
}

/// Creates a `ping` request.
pub fn create_ping_request(id: u64) -> MCPRequest {
    MCPRequest::new(
        Value::Number(id.into()),
        "ping".to_string(),
        Some(json!({})),
    )
}
