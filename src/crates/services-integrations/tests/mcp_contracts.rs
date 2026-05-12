#![cfg(feature = "mcp")]

use bitfun_services_integrations::mcp::auth::{
    MCPRemoteOAuthCredentialVault, MCPRemoteOAuthSessionSnapshot, MCPRemoteOAuthStatus,
};
use bitfun_services_integrations::mcp::config::ConfigLocation;
use bitfun_services_integrations::mcp::config::{
    config_to_cursor_format, format_mcp_json_config_value, get_mcp_remote_authorization_source,
    get_mcp_remote_authorization_value, has_mcp_remote_authorization, has_mcp_remote_oauth,
    has_mcp_remote_xaa, merge_mcp_server_config_sources, normalize_mcp_authorization_value,
    parse_cursor_format, remove_mcp_authorization_keys, validate_mcp_json_config,
};
use bitfun_services_integrations::mcp::protocol::{
    MCPCapability, MCPError, MCPPromptMessageContent, MCPPromptMessageContentBlock, MCPRequest,
    MCPToolResultContent, create_initialize_request, create_mcp_client_info, create_ping_request,
    create_tools_call_request, create_tools_list_request, default_protocol_version,
    map_rmcp_initialize_result, map_rmcp_prompt, map_rmcp_prompt_message, map_rmcp_resource,
    map_rmcp_tool, map_rmcp_tool_result,
};
use bitfun_services_integrations::mcp::server::{
    MCPServerConfig, MCPServerStatus, MCPServerTransport, MCPServerType, is_mcp_auth_error_message,
    merge_mcp_remote_headers,
};
use bitfun_services_integrations::mcp::{
    MCP_TOOL_DELIMITER, MCP_TOOL_PREFIX, McpToolInfo, build_mcp_tool_name, normalize_name_for_mcp,
};
use rmcp::model::{AnnotateAble, Annotations, Content, Icon, Meta, RawResource, ResourceContents};
use rmcp::transport::auth::StoredCredentials;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn make_mcp_config(
    id: &str,
    location: ConfigLocation,
    server_type: MCPServerType,
    command: Option<&str>,
    url: Option<&str>,
) -> MCPServerConfig {
    MCPServerConfig {
        id: id.to_string(),
        name: id.to_string(),
        server_type,
        transport: None,
        command: command.map(str::to_string),
        args: Vec::new(),
        env: HashMap::new(),
        headers: HashMap::new(),
        url: url.map(str::to_string),
        auto_start: true,
        enabled: true,
        location,
        capabilities: Vec::new(),
        settings: Default::default(),
        oauth: None,
        xaa: None,
    }
}

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
fn mcp_remote_client_info_declares_supported_client_capabilities() {
    let info = create_mcp_client_info("BitFun", "1.0.0");

    assert_eq!(info.client_info.name, "BitFun");
    assert_eq!(info.client_info.version, "1.0.0");
    assert!(info.capabilities.roots.is_some());
    assert!(info.capabilities.sampling.is_some());
    assert!(info.capabilities.elicitation.is_some());
    assert_eq!(
        info.capabilities
            .elicitation
            .as_ref()
            .and_then(|cap| cap.schema_validation),
        Some(true)
    );
}

#[test]
fn mcp_rmcp_initialize_mapping_preserves_server_identity_and_capabilities() {
    let server_info = rmcp::model::ServerInfo {
        protocol_version: rmcp::model::ProtocolVersion::LATEST,
        capabilities: rmcp::model::ServerCapabilities {
            tools: Some(rmcp::model::ToolsCapability {
                list_changed: Some(true),
            }),
            resources: Some(rmcp::model::ResourcesCapability {
                subscribe: Some(true),
                list_changed: Some(false),
            }),
            prompts: Some(rmcp::model::PromptsCapability {
                list_changed: Some(true),
            }),
            logging: Some(rmcp::model::JsonObject::new()),
            ..Default::default()
        },
        server_info: rmcp::model::Implementation {
            name: "docs-server".to_string(),
            title: Some("Docs Server".to_string()),
            version: "2.0.0".to_string(),
            icons: None,
            website_url: None,
        },
        instructions: Some("Fallback description".to_string()),
    };

    let mapped = map_rmcp_initialize_result(&server_info);

    assert_eq!(
        mapped.protocol_version,
        rmcp::model::ProtocolVersion::LATEST.to_string()
    );
    assert_eq!(mapped.server_info.name, "docs-server");
    assert_eq!(mapped.server_info.version, "2.0.0");
    assert_eq!(
        mapped.server_info.description.as_deref(),
        Some("Docs Server")
    );
    assert_eq!(
        mapped
            .capabilities
            .tools
            .as_ref()
            .map(|cap| cap.list_changed),
        Some(true)
    );
    assert_eq!(
        mapped
            .capabilities
            .resources
            .as_ref()
            .map(|cap| (cap.subscribe, cap.list_changed)),
        Some((true, false))
    );
    assert!(mapped.capabilities.logging.is_some());
}

#[test]
fn mcp_rmcp_mapping_preserves_remote_tool_resource_and_prompt_metadata() {
    let mut tool_meta = Meta::default();
    tool_meta.insert(
        "ui".to_string(),
        serde_json::json!({ "resourceUri": "ui://widget" }),
    );
    let tool = rmcp::model::Tool {
        name: "search".into(),
        title: Some("Search".to_string()),
        description: Some("Find items".into()),
        input_schema: Arc::new(serde_json::Map::new()),
        output_schema: Some(Arc::new(serde_json::Map::from_iter([(
            "type".to_string(),
            serde_json::json!("object"),
        )]))),
        annotations: Some(
            rmcp::model::ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .idempotent(true)
                .open_world(true),
        ),
        icons: Some(vec![Icon {
            src: "https://example.com/tool.png".to_string(),
            mime_type: Some("image/png".to_string()),
            sizes: Some(vec!["32x32".to_string()]),
        }]),
        meta: Some(tool_meta),
    };
    let mapped_tool = map_rmcp_tool(tool);
    assert_eq!(mapped_tool.title.as_deref(), Some("Search"));
    assert_eq!(
        mapped_tool.output_schema,
        Some(serde_json::json!({ "type": "object" }))
    );
    assert_eq!(
        mapped_tool
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.read_only_hint),
        Some(true)
    );
    assert_eq!(
        mapped_tool
            .meta
            .as_ref()
            .and_then(|meta| meta.ui.as_ref())
            .and_then(|ui| ui.resource_uri.as_deref()),
        Some("ui://widget")
    );

    let mut resource_meta = Meta::default();
    resource_meta.insert("source".to_string(), serde_json::json!("catalog"));
    let resource = RawResource {
        uri: "file:///tmp/report.md".to_string(),
        name: "report".to_string(),
        title: Some("Quarterly Report".to_string()),
        description: Some("Report".to_string()),
        mime_type: Some("text/markdown".to_string()),
        size: Some(42),
        icons: Some(vec![Icon {
            src: "https://example.com/resource.png".to_string(),
            mime_type: Some("image/png".to_string()),
            sizes: Some(vec!["64x64".to_string()]),
        }]),
        meta: Some(resource_meta),
    }
    .annotate(Annotations {
        audience: Some(vec![rmcp::model::Role::User]),
        priority: Some(0.9),
        last_modified: None,
    });
    let mapped_resource = map_rmcp_resource(resource);
    assert_eq!(mapped_resource.title.as_deref(), Some("Quarterly Report"));
    assert_eq!(mapped_resource.size, Some(42));
    assert_eq!(
        mapped_resource
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.audience.as_ref())
            .cloned(),
        Some(vec!["user".to_string()])
    );
    assert_eq!(
        mapped_resource
            .metadata
            .as_ref()
            .and_then(|meta| meta.get("source")),
        Some(&serde_json::json!("catalog"))
    );

    let prompt = rmcp::model::Prompt {
        name: "summarize".to_string(),
        title: Some("Summarize".to_string()),
        description: Some("Summarize content".to_string()),
        arguments: Some(vec![rmcp::model::PromptArgument {
            name: "topic".to_string(),
            title: Some("Topic".to_string()),
            description: Some("Topic to summarize".to_string()),
            required: Some(true),
        }]),
        icons: Some(vec![Icon {
            src: "https://example.com/prompt.png".to_string(),
            mime_type: Some("image/png".to_string()),
            sizes: Some(vec!["16x16".to_string()]),
        }]),
        meta: None,
    };
    let mapped_prompt = map_rmcp_prompt(prompt);
    assert_eq!(mapped_prompt.title.as_deref(), Some("Summarize"));
    assert_eq!(
        mapped_prompt
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.first())
            .and_then(|argument| argument.title.as_deref()),
        Some("Topic")
    );
    assert!(mapped_prompt.icons.is_some());
}

#[test]
fn mcp_rmcp_mapping_preserves_structured_results_and_resource_links() {
    let resource_link = RawResource {
        uri: "file:///tmp/output.json".to_string(),
        name: "output".to_string(),
        title: Some("Output".to_string()),
        description: Some("Generated output".to_string()),
        mime_type: Some("application/json".to_string()),
        size: Some(7),
        icons: None,
        meta: None,
    };
    let mut result_meta = Meta::default();
    result_meta.insert("traceId".to_string(), serde_json::json!("abc123"));
    let result = rmcp::model::CallToolResult {
        content: vec![
            Content::text("done"),
            Content::resource_link(resource_link),
            Content::image("aGVsbG8=", "image/png"),
        ],
        structured_content: Some(serde_json::json!({ "ok": true })),
        is_error: Some(false),
        meta: Some(result_meta),
    };

    let mapped = map_rmcp_tool_result(result);

    assert_eq!(
        mapped.structured_content,
        Some(serde_json::json!({ "ok": true }))
    );
    assert_eq!(
        mapped.meta,
        Some(serde_json::json!({ "traceId": "abc123" }))
    );
    assert!(matches!(
        mapped.content.as_ref().and_then(|content| content.get(1)),
        Some(MCPToolResultContent::ResourceLink { uri, .. }) if uri == "file:///tmp/output.json"
    ));
    assert!(matches!(
        mapped.content.as_ref().and_then(|content| content.get(2)),
        Some(MCPToolResultContent::Image { mime_type, .. }) if mime_type == "image/png"
    ));
}

#[test]
fn mcp_rmcp_mapping_preserves_prompt_message_blocks() {
    let prompt_message = rmcp::model::PromptMessage {
        role: rmcp::model::PromptMessageRole::User,
        content: rmcp::model::PromptMessageContent::Text {
            text: "hello".to_string(),
        },
    };
    let mapped = map_rmcp_prompt_message(prompt_message);
    assert!(matches!(
        mapped.content,
        MCPPromptMessageContent::Block(ref block)
            if matches!(block.as_ref(), MCPPromptMessageContentBlock::Text { text } if text == "hello")
    ));

    let resource_link = RawResource {
        uri: "file:///tmp/input.md".to_string(),
        name: "input".to_string(),
        title: None,
        description: Some("input".to_string()),
        mime_type: Some("text/markdown".to_string()),
        size: None,
        icons: None,
        meta: None,
    }
    .no_annotation();
    let prompt_message = rmcp::model::PromptMessage {
        role: rmcp::model::PromptMessageRole::Assistant,
        content: rmcp::model::PromptMessageContent::ResourceLink {
            link: resource_link,
        },
    };
    let mapped = map_rmcp_prompt_message(prompt_message);
    assert!(matches!(
        mapped.content,
        MCPPromptMessageContent::Block(ref block)
            if matches!(
                block.as_ref(),
                MCPPromptMessageContentBlock::ResourceLink { uri, .. }
                    if uri == "file:///tmp/input.md"
            )
    ));

    let embedded = rmcp::model::RawEmbeddedResource {
        meta: Some(Meta::default()),
        resource: ResourceContents::TextResourceContents {
            uri: "file:///tmp/embedded.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: "embedded".to_string(),
            meta: None,
        },
    }
    .no_annotation();
    let prompt_message = rmcp::model::PromptMessage {
        role: rmcp::model::PromptMessageRole::Assistant,
        content: rmcp::model::PromptMessageContent::Resource { resource: embedded },
    };
    let mapped = map_rmcp_prompt_message(prompt_message);
    assert!(matches!(
        mapped.content,
        MCPPromptMessageContent::Block(ref block)
            if matches!(
                block.as_ref(),
                MCPPromptMessageContentBlock::Resource { resource }
                    if resource.uri == "file:///tmp/embedded.txt"
            )
    ));
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
fn mcp_json_config_helpers_preserve_load_format_and_save_validation_contract() {
    let legacy_array = serde_json::json!([
        {
            "id": "local",
            "name": "Local",
            "type": "local",
            "command": "npx"
        }
    ]);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &format_mcp_json_config_value(Some(&legacy_array)).unwrap()
        )
        .unwrap(),
        serde_json::json!({
            "mcpServers": {
                "local": {
                    "id": "local",
                    "name": "Local",
                    "type": "local",
                    "command": "npx"
                }
            }
        })
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&format_mcp_json_config_value(None).unwrap())
            .unwrap(),
        serde_json::json!({ "mcpServers": {} })
    );

    validate_mcp_json_config(&serde_json::json!({
        "mcpServers": {
            "remote": {
                "type": "sse",
                "url": "https://example.com/sse",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }))
    .expect("valid remote SSE config");

    assert_eq!(
        validate_mcp_json_config(&serde_json::json!({}))
            .unwrap_err()
            .to_string(),
        "Config missing 'mcpServers' field"
    );
    assert_eq!(
        validate_mcp_json_config(&serde_json::json!({
            "mcpServers": {
                "bad": {
                    "type": "container",
                    "command": "docker"
                }
            }
        }))
        .unwrap_err()
        .to_string(),
        "Server 'bad' has unsupported 'type' value: 'container'"
    );
    assert_eq!(
        validate_mcp_json_config(&serde_json::json!({
            "mcpServers": {
                "bad": {
                    "source": "remote",
                    "command": "npx"
                }
            }
        }))
        .unwrap_err()
        .to_string(),
        "Server 'bad' source='remote' conflicts with command-based configuration"
    );
}

#[test]
fn mcp_config_merge_helpers_preserve_precedence_and_dedup_contract() {
    let merged = merge_mcp_server_config_sources([
        vec![make_mcp_config(
            "github-user",
            ConfigLocation::User,
            MCPServerType::Remote,
            None,
            Some("https://example.com/mcp"),
        )],
        vec![
            make_mcp_config(
                "github-user",
                ConfigLocation::Project,
                MCPServerType::Remote,
                None,
                Some("https://project.example.com/mcp"),
            ),
            make_mcp_config(
                "github-project",
                ConfigLocation::Project,
                MCPServerType::Remote,
                None,
                Some("https://example.com/mcp"),
            ),
        ],
    ]);

    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].id, "github-user");
    assert_eq!(merged[0].location, ConfigLocation::Project);
    assert_eq!(
        merged[0].url.as_deref(),
        Some("https://project.example.com/mcp")
    );
    assert_eq!(merged[1].id, "github-project");
    assert_eq!(merged[1].location, ConfigLocation::Project);

    let deduped = merge_mcp_server_config_sources([
        vec![make_mcp_config(
            "github-user",
            ConfigLocation::User,
            MCPServerType::Remote,
            None,
            Some("https://example.com/mcp"),
        )],
        vec![make_mcp_config(
            "github-project",
            ConfigLocation::Project,
            MCPServerType::Remote,
            None,
            Some("https://example.com/mcp"),
        )],
    ]);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].id, "github-project");
    assert_eq!(deduped[0].location, ConfigLocation::Project);
}

#[test]
fn mcp_config_authorization_helpers_preserve_header_precedence_and_normalization() {
    let mut config = make_mcp_config(
        "remote-auth",
        ConfigLocation::User,
        MCPServerType::Remote,
        None,
        Some("https://example.com/mcp"),
    );
    config
        .env
        .insert("Authorization".to_string(), "legacy-token".to_string());
    config.headers.insert(
        "Authorization".to_string(),
        "Bearer header-token".to_string(),
    );

    assert_eq!(
        get_mcp_remote_authorization_value(&config).as_deref(),
        Some("Bearer header-token")
    );
    assert_eq!(
        get_mcp_remote_authorization_source(&config),
        Some("headers")
    );
    assert!(has_mcp_remote_authorization(&config));
    assert!(!has_mcp_remote_oauth(&config));
    assert!(!has_mcp_remote_xaa(&config));
    assert_eq!(
        normalize_mcp_authorization_value("plain-token").as_deref(),
        Some("Bearer plain-token")
    );
    assert_eq!(
        normalize_mcp_authorization_value("Bearer existing").as_deref(),
        Some("Bearer existing")
    );
    assert_eq!(normalize_mcp_authorization_value("   "), None);

    remove_mcp_authorization_keys(&mut config.headers);
    remove_mcp_authorization_keys(&mut config.env);
    assert_eq!(get_mcp_remote_authorization_value(&config), None);
    assert_eq!(get_mcp_remote_authorization_source(&config), None);
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
fn mcp_runtime_auth_error_classifier_preserves_process_status_contract() {
    assert!(is_mcp_auth_error_message(
        "Handshake failed: Unauthorized (401)"
    ));
    assert!(is_mcp_auth_error_message(
        "Ping failed: OAuth token refresh failed: no refresh token available"
    ));
    assert!(is_mcp_auth_error_message(
        "remote server returned status code: 403"
    ));
    assert!(!is_mcp_auth_error_message(
        "Handshake failed: connection reset"
    ));
}

#[test]
fn mcp_runtime_remote_header_merge_preserves_legacy_env_authorization_fallback() {
    let mut env = HashMap::new();
    env.insert("Authorization".to_string(), "legacy-token".to_string());
    env.insert("X-Env".to_string(), "env-only".to_string());

    let headers = HashMap::new();
    let merged = merge_mcp_remote_headers(&headers, &env);
    assert_eq!(
        merged.get("Authorization").map(String::as_str),
        Some("legacy-token")
    );
    assert!(!merged.contains_key("X-Env"));

    let mut explicit_headers = HashMap::new();
    explicit_headers.insert(
        "authorization".to_string(),
        "Bearer header-token".to_string(),
    );
    let merged = merge_mcp_remote_headers(&explicit_headers, &env);
    assert_eq!(
        merged.get("authorization").map(String::as_str),
        Some("Bearer header-token")
    );
    assert!(!merged.contains_key("Authorization"));

    let mut empty_header = HashMap::new();
    empty_header.insert("AUTHORIZATION".to_string(), String::new());
    let merged = merge_mcp_remote_headers(&empty_header, &env);
    assert_eq!(merged.get("AUTHORIZATION").map(String::as_str), Some(""));
    assert!(!merged.contains_key("Authorization"));
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
fn mcp_oauth_session_snapshot_preserves_camel_case_status_contract() {
    let snapshot = MCPRemoteOAuthSessionSnapshot::new(
        "remote-server",
        MCPRemoteOAuthStatus::AwaitingBrowser,
        Some("https://auth.example.com/start".to_string()),
        Some("http://127.0.0.1:49152/oauth/callback".to_string()),
        None,
    );

    assert_eq!(
        serde_json::to_value(&snapshot).unwrap(),
        serde_json::json!({
            "serverId": "remote-server",
            "status": "awaitingBrowser",
            "authorizationUrl": "https://auth.example.com/start",
            "redirectUri": "http://127.0.0.1:49152/oauth/callback"
        })
    );
}

#[tokio::test]
async fn mcp_oauth_credential_vault_uses_injected_data_dir_and_roundtrips_credentials() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let data_dir = std::env::temp_dir().join(format!(
        "bitfun-mcp-oauth-vault-contract-{}-{}",
        std::process::id(),
        unique
    ));

    let vault = MCPRemoteOAuthCredentialVault::new(data_dir.clone());
    let credentials = StoredCredentials {
        client_id: "client-123".to_string(),
        token_response: None,
    };

    vault
        .store("server-a", &credentials)
        .await
        .expect("store credentials");

    assert!(data_dir.join(".mcp_oauth_vault.key").exists());
    assert!(data_dir.join("mcp_oauth_vault.json").exists());

    let loaded = vault
        .load("server-a")
        .await
        .expect("load credentials")
        .expect("stored credentials");
    assert_eq!(loaded.client_id, "client-123");
    assert!(loaded.token_response.is_none());

    vault.clear("server-a").await.expect("clear credentials");
    assert!(
        vault
            .load("server-a")
            .await
            .expect("load after clear")
            .is_none()
    );

    let _ = std::fs::remove_dir_all(data_dir);
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
