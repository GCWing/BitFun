#![cfg(feature = "mcp")]

use bitfun_services_integrations::mcp::config::ConfigLocation;
use bitfun_services_integrations::mcp::config::{config_to_cursor_format, parse_cursor_format};
use bitfun_services_integrations::mcp::protocol::{
    create_initialize_request, create_ping_request, create_tools_call_request,
    create_tools_list_request, default_protocol_version, MCPCapability, MCPError,
    MCPPromptMessageContent, MCPPromptMessageContentBlock, MCPRequest,
};
use bitfun_services_integrations::mcp::server::{
    MCPServerConfig, MCPServerStatus, MCPServerTransport, MCPServerType,
};
use bitfun_services_integrations::mcp::{
    build_mcp_tool_name, normalize_name_for_mcp, McpToolInfo, MCP_TOOL_DELIMITER, MCP_TOOL_PREFIX,
};

#[test]
fn mcp_tool_name_contract_matches_existing_wire_format() {
    assert_eq!(MCP_TOOL_PREFIX, "mcp__");
    assert_eq!(MCP_TOOL_DELIMITER, "__");
    assert_eq!(
        normalize_name_for_mcp("Acme Search / Primary"),
        "Acme_Search___Primary"
    );
    assert_eq!(
        build_mcp_tool_name("Claude Code", "search repos"),
        "mcp__Claude_Code__search_repos"
    );
}

#[test]
fn mcp_tool_info_preserves_json_shape() {
    let info = McpToolInfo {
        server_id: "server-1".to_string(),
        server_name: "Docs".to_string(),
        tool_name: "search".to_string(),
    };

    assert_eq!(
        serde_json::to_value(info).unwrap(),
        serde_json::json!({
            "server_id": "server-1",
            "server_name": "Docs",
            "tool_name": "search"
        })
    );
}

#[test]
fn mcp_protocol_capability_contract_matches_existing_default() {
    assert_eq!(default_protocol_version(), "2025-11-25");
    assert_eq!(
        serde_json::to_value(MCPCapability::default()).unwrap(),
        serde_json::json!({
            "resources": {
                "subscribe": false,
                "listChanged": false
            },
            "prompts": {
                "listChanged": false
            },
            "tools": {
                "listChanged": false
            }
        })
    );
}

#[test]
fn mcp_protocol_jsonrpc_helpers_preserve_wire_shape() {
    let request = MCPRequest::new(
        serde_json::json!(7),
        "tools/list".to_string(),
        Some(serde_json::json!({ "cursor": "next" })),
    );

    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/list",
            "params": {
                "cursor": "next"
            }
        })
    );

    assert_eq!(
        serde_json::to_value(MCPError::method_not_found("tools/call")).unwrap(),
        serde_json::json!({
            "code": -32601,
            "message": "Method not found: tools/call"
        })
    );
}

#[test]
fn mcp_protocol_request_builders_preserve_wire_shape() {
    assert_eq!(
        serde_json::to_value(create_initialize_request(9, "BitFun", "0.2.6")).unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {
                    "resources": {
                        "subscribe": false,
                        "listChanged": false
                    },
                    "prompts": {
                        "listChanged": false
                    },
                    "tools": {
                        "listChanged": false
                    }
                },
                "clientInfo": {
                    "name": "BitFun",
                    "version": "0.2.6",
                    "description": "BitFun MCP Client",
                    "vendor": "BitFun"
                }
            }
        })
    );

    assert_eq!(
        serde_json::to_value(create_tools_list_request(10, None)).unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/list"
        })
    );

    assert_eq!(
        serde_json::to_value(create_tools_list_request(11, Some("cursor-1".to_string()))).unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/list",
            "params": {
                "cursor": "cursor-1"
            }
        })
    );

    assert_eq!(
        serde_json::to_value(create_tools_call_request(
            12,
            "search",
            Some(serde_json::json!({ "query": "rust" }))
        ))
        .unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {
                    "query": "rust"
                }
            }
        })
    );

    assert_eq!(
        serde_json::to_value(create_ping_request(13)).unwrap(),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "ping",
            "params": {}
        })
    );
}

#[test]
fn mcp_protocol_prompt_content_helpers_preserve_legacy_text_behavior() {
    let mut content = MCPPromptMessageContent::Plain("Review {{target}}".to_string());
    content.substitute_placeholders(&std::collections::HashMap::from([(
        "target".to_string(),
        "src/main.rs".to_string(),
    )]));

    assert_eq!(content.text_or_placeholder(), "Review src/main.rs");

    let image = MCPPromptMessageContent::Block(Box::new(MCPPromptMessageContentBlock::Image {
        data: "base64".to_string(),
        mime_type: "image/png".to_string(),
    }));
    assert_eq!(image.text_or_placeholder(), "[Image: image/png]");
}

#[test]
fn mcp_config_location_preserves_kebab_case_wire_contract() {
    assert_eq!(
        serde_json::to_value(ConfigLocation::BuiltIn).unwrap(),
        serde_json::json!("built-in")
    );
    assert_eq!(
        serde_json::from_value::<ConfigLocation>(serde_json::json!("user")).unwrap(),
        ConfigLocation::User
    );
    assert_eq!(
        serde_json::from_value::<ConfigLocation>(serde_json::json!("project")).unwrap(),
        ConfigLocation::Project
    );
}

#[test]
fn mcp_server_type_and_status_preserve_lowercase_wire_contract() {
    assert_eq!(
        serde_json::to_value(MCPServerType::Local).unwrap(),
        serde_json::json!("local")
    );
    assert_eq!(
        serde_json::from_value::<MCPServerType>(serde_json::json!("remote")).unwrap(),
        MCPServerType::Remote
    );
    assert_eq!(
        serde_json::to_value(MCPServerStatus::NeedsAuth).unwrap(),
        serde_json::json!("needsauth")
    );
    assert_eq!(
        serde_json::from_value::<MCPServerStatus>(serde_json::json!("reconnecting")).unwrap(),
        MCPServerStatus::Reconnecting
    );
}

#[test]
fn mcp_server_config_preserves_transport_defaults_and_validation_contract() {
    let local = MCPServerConfig {
        id: "local".to_string(),
        name: "Local".to_string(),
        server_type: MCPServerType::Local,
        transport: None,
        command: Some("npx".to_string()),
        args: vec!["server".to_string()],
        env: Default::default(),
        headers: Default::default(),
        url: None,
        auto_start: true,
        enabled: true,
        location: ConfigLocation::User,
        capabilities: Vec::new(),
        settings: Default::default(),
        oauth: None,
        xaa: None,
    };
    assert_eq!(local.resolved_transport(), MCPServerTransport::Stdio);
    local.validate().expect("local stdio config is valid");

    let mut remote = local.clone();
    remote.id = "remote".to_string();
    remote.name = "Remote".to_string();
    remote.server_type = MCPServerType::Remote;
    remote.command = None;
    remote.transport = None;
    assert_eq!(
        remote.validate().unwrap_err().to_string(),
        "Remote MCP server 'remote' must have a URL"
    );

    remote.url = Some("https://example.com/mcp".to_string());
    assert_eq!(
        remote.resolved_transport(),
        MCPServerTransport::StreamableHttp
    );
    remote
        .validate()
        .expect("remote streamable-http config is valid");
}

#[test]
fn mcp_cursor_format_helpers_preserve_cursor_compatibility_contract() {
    let remote = MCPServerConfig {
        id: "remote-sse".to_string(),
        name: "Remote SSE".to_string(),
        server_type: MCPServerType::Remote,
        transport: Some(MCPServerTransport::Sse),
        command: None,
        args: Vec::new(),
        env: Default::default(),
        headers: std::collections::HashMap::from([(
            "Authorization".to_string(),
            "Bearer token".to_string(),
        )]),
        url: Some("https://example.com/sse".to_string()),
        auto_start: false,
        enabled: true,
        location: ConfigLocation::User,
        capabilities: Vec::new(),
        settings: Default::default(),
        oauth: None,
        xaa: None,
    };

    assert_eq!(
        config_to_cursor_format(&remote),
        serde_json::json!({
            "type": "sse",
            "name": "Remote SSE",
            "enabled": true,
            "autoStart": false,
            "headers": {
                "Authorization": "Bearer token"
            },
            "url": "https://example.com/sse"
        })
    );

    let parsed = parse_cursor_format(&serde_json::json!({
        "mcpServers": {
            "remote-sse": {
                "type": "sse",
                "url": "https://example.com/sse"
            },
            "unsupported": {
                "type": "container",
                "command": "docker",
                "args": ["run", "--rm", "-i", "example/server"]
            }
        }
    }));

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, "remote-sse");
    assert_eq!(parsed[0].server_type, MCPServerType::Remote);
    assert_eq!(parsed[0].transport, Some(MCPServerTransport::Sse));
    assert_eq!(parsed[0].location, ConfigLocation::User);
}
