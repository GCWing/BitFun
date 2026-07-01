#![cfg(feature = "mcp")]

use async_trait::async_trait;
use bitfun_services_integrations::mcp::auth::{
    MCPRemoteOAuthCredentialVault, MCPRemoteOAuthSessionSnapshot, MCPRemoteOAuthStatus,
};
use bitfun_services_integrations::mcp::config::ConfigLocation;
use bitfun_services_integrations::mcp::config::{
    config_to_cursor_format, format_mcp_json_config_value, get_mcp_remote_authorization_source,
    get_mcp_remote_authorization_value, has_mcp_remote_authorization, has_mcp_remote_oauth,
    has_mcp_remote_xaa, merge_mcp_server_config_sources, normalize_mcp_authorization_value,
    parse_cursor_format, remove_mcp_authorization_keys, validate_mcp_json_config, MCPConfigService,
    MCPConfigStore,
};
use bitfun_services_integrations::mcp::protocol::{
    create_initialize_request, create_mcp_client_info, create_ping_request,
    create_tools_call_request, create_tools_list_request, default_protocol_version,
    map_rmcp_initialize_result, map_rmcp_prompt, map_rmcp_prompt_message, map_rmcp_resource,
    map_rmcp_tool, map_rmcp_tool_result, MCPCapability, MCPError, MCPPrompt, MCPPromptArgument,
    MCPPromptContent, MCPPromptMessage, MCPPromptMessageContent, MCPPromptMessageContentBlock,
    MCPRequest, MCPResource, MCPResourceContent, MCPTool, MCPToolAnnotations, MCPToolResult,
    MCPToolResultContent,
};
use bitfun_services_integrations::mcp::server::{
    compute_mcp_backoff_delay, detect_mcp_list_changed_kind, is_mcp_auth_error_message,
    mcp_reconnect_runtime_decision, mcp_server_is_running, mcp_should_start_after_config_update,
    merge_mcp_remote_headers, MCPCatalogCache, MCPConnectionPool, MCPListChangedKind,
    MCPReconnectRuntimeDecision, MCPRuntimeErrorKind, MCPRuntimeResult, MCPServerConfig,
    MCPServerProcess, MCPServerRuntimeState, MCPServerStatus, MCPServerTransport, MCPServerType,
};
use bitfun_services_integrations::mcp::{
    build_mcp_tool_descriptor, build_mcp_tool_name, normalize_name_for_mcp,
    render_mcp_tool_result_for_assistant, MCPContextEnhancer, MCPContextEnhancerConfig,
    MCPDynamicToolProvider, MCPToolCatalogClient, McpDynamicToolDescriptor, McpToolInfo,
    PromptAdapter, ResourceAdapter, MCP_TOOL_DELIMITER, MCP_TOOL_PREFIX,
};
use rmcp::model::{AnnotateAble, Annotations, Content, Icon, Meta, RawResource, ResourceContents};
use rmcp::transport::auth::StoredCredentials;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

fn make_resource(name: &str, description: Option<&str>, uri: &str) -> MCPResource {
    MCPResource {
        uri: uri.to_string(),
        name: name.to_string(),
        title: None,
        description: description.map(str::to_string),
        mime_type: Some("text/plain".to_string()),
        icons: None,
        size: Some(12),
        annotations: None,
        metadata: None,
    }
}

#[derive(Default)]
struct InMemoryMCPConfigStore {
    values: tokio::sync::Mutex<HashMap<String, serde_json::Value>>,
}

#[async_trait]
impl MCPConfigStore for InMemoryMCPConfigStore {
    async fn get_config_value(&self, key: &str) -> MCPRuntimeResult<Option<serde_json::Value>> {
        Ok(self.values.lock().await.get(key).cloned())
    }

    async fn set_config_value(&self, key: &str, value: serde_json::Value) -> MCPRuntimeResult<()> {
        self.values.lock().await.insert(key.to_string(), value);
        Ok(())
    }
}

struct FailingMCPConfigStore;

#[async_trait]
impl MCPConfigStore for FailingMCPConfigStore {
    async fn get_config_value(&self, key: &str) -> MCPRuntimeResult<Option<serde_json::Value>> {
        Err(
            bitfun_services_integrations::mcp::MCPRuntimeError::configuration(format!(
                "backend unavailable for {key}"
            )),
        )
    }

    async fn set_config_value(&self, key: &str, _value: serde_json::Value) -> MCPRuntimeResult<()> {
        Err(
            bitfun_services_integrations::mcp::MCPRuntimeError::configuration(format!(
                "backend unavailable for {key}"
            )),
        )
    }
}

struct FakeMCPToolCatalogClient {
    tools: Vec<MCPTool>,
}

#[async_trait]
impl MCPToolCatalogClient for FakeMCPToolCatalogClient {
    async fn list_mcp_tools(&self) -> MCPRuntimeResult<Vec<MCPTool>> {
        Ok(self.tools.clone())
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
        serde_json::to_value(&info.capabilities.elicitation).unwrap(),
        serde_json::json!({})
    );
}

#[test]
fn mcp_rmcp_initialize_mapping_preserves_server_identity_and_capabilities() {
    let mut capabilities = rmcp::model::ServerCapabilities::default();
    capabilities.tools = Some(rmcp::model::ToolsCapability {
        list_changed: Some(true),
    });
    capabilities.resources = Some(rmcp::model::ResourcesCapability {
        subscribe: Some(true),
        list_changed: Some(false),
    });
    capabilities.prompts = Some(rmcp::model::PromptsCapability {
        list_changed: Some(true),
    });
    capabilities.logging = Some(rmcp::model::JsonObject::new());

    let server_info = rmcp::model::ServerInfo::new(capabilities)
        .with_protocol_version(rmcp::model::ProtocolVersion::LATEST)
        .with_server_info(
            rmcp::model::Implementation::new("docs-server", "2.0.0").with_title("Docs Server"),
        )
        .with_instructions("Fallback description");

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
    let mut tool = rmcp::model::Tool::new("search", "Find items", serde_json::Map::new());
    tool.title = Some("Search".to_string());
    tool.output_schema = Some(Arc::new(serde_json::Map::from_iter([(
        "type".to_string(),
        serde_json::json!("object"),
    )])));
    tool.annotations = Some(
        rmcp::model::ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .idempotent(true)
            .open_world(true),
    );
    tool.icons = Some(vec![Icon::new("https://example.com/tool.png")
        .with_mime_type("image/png")
        .with_sizes(vec!["32x32".to_string()])]);
    tool.meta = Some(tool_meta);
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
        icons: Some(vec![Icon::new("https://example.com/resource.png")
            .with_mime_type("image/png")
            .with_sizes(vec!["64x64".to_string()])]),
        meta: Some(resource_meta),
    }
    .annotate({
        let mut annotations = Annotations::default();
        annotations.audience = Some(vec![rmcp::model::Role::User]);
        annotations.priority = Some(0.9);
        annotations
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

    let prompt = rmcp::model::Prompt::new(
        "summarize",
        Some("Summarize content"),
        Some(vec![rmcp::model::PromptArgument::new("topic")
            .with_title("Topic")
            .with_description("Topic to summarize")
            .with_required(true)]),
    )
    .with_title("Summarize")
    .with_icons(vec![Icon::new("https://example.com/prompt.png")
        .with_mime_type("image/png")
        .with_sizes(vec!["16x16".to_string()])]);
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
    let mut result = rmcp::model::CallToolResult::success(vec![
        Content::text("done"),
        Content::resource_link(resource_link),
        Content::image("aGVsbG8=", "image/png"),
    ]);
    result.structured_content = Some(serde_json::json!({ "ok": true }));
    result.meta = Some(result_meta);

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
    let prompt_message =
        rmcp::model::PromptMessage::new_text(rmcp::model::PromptMessageRole::User, "hello");
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
    let prompt_message = rmcp::model::PromptMessage::new(
        rmcp::model::PromptMessageRole::Assistant,
        rmcp::model::PromptMessageContent::resource_link(resource_link),
    );
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
    let prompt_message = rmcp::model::PromptMessage::new(
        rmcp::model::PromptMessageRole::Assistant,
        rmcp::model::PromptMessageContent::Resource { resource: embedded },
    );
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
fn mcp_resource_and_prompt_adapters_preserve_context_rendering_contract() {
    let resource = MCPResource {
        title: Some("Design Notes".to_string()),
        metadata: Some(HashMap::from([(
            "source".to_string(),
            serde_json::json!("fixture"),
        )])),
        ..make_resource("notes", Some("project notes"), "file:///workspace/notes.md")
    };
    let content = MCPResourceContent {
        uri: resource.uri.clone(),
        content: Some("alpha beta".to_string()),
        blob: None,
        mime_type: Some("text/markdown".to_string()),
        annotations: None,
        meta: None,
    };

    assert_eq!(
        ResourceAdapter::to_context_block(&resource, Some(&content)),
        serde_json::json!({
            "type": "resource",
            "uri": "file:///workspace/notes.md",
            "name": "notes",
            "title": "Design Notes",
            "displayName": "Design Notes",
            "description": "project notes",
            "mimeType": "text/plain",
            "size": 12,
            "content": "alpha beta",
            "metadata": {
                "source": "fixture"
            }
        })
    );
    assert_eq!(
        ResourceAdapter::to_text(&content),
        "Resource: file:///workspace/notes.md\n\nalpha beta\n"
    );

    let ranked = ResourceAdapter::filter_and_rank(
        vec![
            make_resource("readme", Some("install guide"), "file:///README.md"),
            make_resource("report", Some("quarterly guide"), "file:///report.md"),
            make_resource("other", Some("misc"), "file:///other.md"),
        ],
        "guide",
        0.3,
        2,
    );
    assert_eq!(
        ranked
            .iter()
            .map(|(resource, _)| resource.name.as_str())
            .collect::<Vec<_>>(),
        vec!["readme", "report"]
    );

    let prompt = MCPPrompt {
        name: "review".to_string(),
        title: None,
        description: None,
        arguments: Some(vec![MCPPromptArgument {
            name: "target".to_string(),
            title: None,
            description: None,
            required: true,
        }]),
        icons: None,
    };
    assert!(!PromptAdapter::is_applicable(&prompt, &HashMap::new()));
    assert!(PromptAdapter::is_applicable(
        &prompt,
        &HashMap::from([("target".to_string(), "src/lib.rs".to_string())])
    ));

    let messages = PromptAdapter::substitute_arguments(
        vec![MCPPromptMessage {
            role: "user".to_string(),
            content: MCPPromptMessageContent::Plain("Review {{target}}".to_string()),
        }],
        &HashMap::from([("target".to_string(), "src/lib.rs".to_string())]),
    );
    let prompt_text = PromptAdapter::to_system_prompt(&MCPPromptContent {
        name: "review".to_string(),
        messages,
    });
    assert_eq!(prompt_text, "User: Review src/lib.rs");
}

#[tokio::test]
async fn mcp_context_enhancer_preserves_resource_selection_contract() {
    let enhancer = MCPContextEnhancer::new(MCPContextEnhancerConfig {
        min_relevance: 0.1,
        max_resources: 1,
        max_total_size: 1024,
        enable_caching: true,
    });

    let context = enhancer
        .enhance(
            "rust mcp",
            vec![
                (
                    make_resource("Rust MCP Guide", Some("runtime docs"), "file://guide.md"),
                    MCPResourceContent {
                        uri: "file://guide.md".to_string(),
                        content: Some("A useful MCP runtime guide".to_string()),
                        blob: None,
                        mime_type: Some("text/plain".to_string()),
                        annotations: None,
                        meta: None,
                    },
                ),
                (
                    make_resource("Unrelated", None, "file://image.png"),
                    MCPResourceContent {
                        uri: "file://image.png".to_string(),
                        content: None,
                        blob: Some("base64".to_string()),
                        mime_type: Some("image/png".to_string()),
                        annotations: None,
                        meta: None,
                    },
                ),
            ],
        )
        .await
        .unwrap();

    assert_eq!(context["type"], "mcp_context");
    assert_eq!(context["query"], "rust mcp");
    assert_eq!(context["resources"].as_array().unwrap().len(), 1);
    assert_eq!(context["resources"][0]["name"], "Rust MCP Guide");
    assert!(context["resources"][0]["relevance_score"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn mcp_catalog_cache_preserves_resource_prompt_lifecycle_contract() {
    let cache = MCPCatalogCache::new();
    let resource = make_resource("readme", Some("docs"), "file:///README.md");
    let prompt = MCPPrompt {
        name: "summarize".to_string(),
        title: Some("Summarize".to_string()),
        description: None,
        arguments: None,
        icons: None,
    };

    cache
        .replace_resources("server-a", vec![resource.clone()])
        .await;
    cache
        .replace_prompts("server-a", vec![prompt.clone()])
        .await;

    assert_eq!(cache.get_resources("server-a").await[0].name, "readme");
    assert_eq!(cache.get_prompts("server-a").await[0].name, "summarize");
    assert!(cache.get_resources("missing").await.is_empty());

    cache.remove_server("server-a").await;
    assert!(cache.get_resources("server-a").await.is_empty());
    assert!(cache.get_prompts("server-a").await.is_empty());

    cache.replace_resources("server-b", vec![resource]).await;
    cache.replace_prompts("server-b", vec![prompt]).await;
    cache.clear().await;
    assert!(cache.get_resources("server-b").await.is_empty());
    assert!(cache.get_prompts("server-b").await.is_empty());
}

#[tokio::test]
async fn mcp_catalog_cache_replacement_invalidates_stale_entries() {
    let cache = MCPCatalogCache::new();
    let old_resource = make_resource("old", Some("stale"), "file:///old.md");
    let new_resource = make_resource("new", Some("fresh"), "file:///new.md");
    let old_prompt = MCPPrompt {
        name: "old-prompt".to_string(),
        title: None,
        description: Some("stale".to_string()),
        arguments: None,
        icons: None,
    };
    let new_prompt = MCPPrompt {
        name: "new-prompt".to_string(),
        title: None,
        description: Some("fresh".to_string()),
        arguments: None,
        icons: None,
    };

    cache
        .replace_resources("server-a", vec![old_resource])
        .await;
    cache.replace_prompts("server-a", vec![old_prompt]).await;
    cache
        .replace_resources("server-a", vec![new_resource])
        .await;
    cache.replace_prompts("server-a", vec![new_prompt]).await;

    let resources = cache.get_resources("server-a").await;
    let prompts = cache.get_prompts("server-a").await;
    assert_eq!(
        resources
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        vec!["new"]
    );
    assert_eq!(
        prompts
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        vec!["new-prompt"]
    );

    cache.replace_resources("server-a", Vec::new()).await;
    cache.replace_prompts("server-a", Vec::new()).await;
    assert!(cache.get_resources("server-a").await.is_empty());
    assert!(cache.get_prompts("server-a").await.is_empty());
}

#[test]
fn mcp_runtime_notification_and_backoff_helpers_preserve_manager_contract() {
    assert_eq!(
        detect_mcp_list_changed_kind("notifications/tools/list_changed"),
        Some(MCPListChangedKind::Tools)
    );
    assert_eq!(
        detect_mcp_list_changed_kind("notifications/prompts/listChanged"),
        Some(MCPListChangedKind::Prompts)
    );
    assert_eq!(
        detect_mcp_list_changed_kind("resources/list_changed"),
        Some(MCPListChangedKind::Resources)
    );
    assert_eq!(detect_mcp_list_changed_kind("notifications/other"), None);

    assert_eq!(
        compute_mcp_backoff_delay(Duration::from_secs(2), Duration::from_secs(60), 1),
        Duration::from_secs(2)
    );
    assert_eq!(
        compute_mcp_backoff_delay(Duration::from_secs(2), Duration::from_secs(60), 5),
        Duration::from_secs(32)
    );
    assert_eq!(
        compute_mcp_backoff_delay(Duration::from_secs(2), Duration::from_secs(60), 10),
        Duration::from_secs(60)
    );
}

#[test]
fn mcp_dynamic_tool_descriptor_and_result_rendering_preserve_tool_contract() {
    let tool = MCPTool {
        name: "search".to_string(),
        title: Some("Search".to_string()),
        description: Some("Find docs".to_string()),
        input_schema: serde_json::json!({ "type": "object" }),
        output_schema: None,
        icons: None,
        annotations: Some(MCPToolAnnotations {
            title: Some("Search Docs".to_string()),
            read_only_hint: Some(true),
            destructive_hint: Some(false),
            idempotent_hint: Some(true),
            open_world_hint: Some(true),
        }),
        meta: None,
    };

    let descriptor = build_mcp_tool_descriptor("github", "GitHub", &tool);
    assert_eq!(
        descriptor,
        McpDynamicToolDescriptor {
            full_name: "mcp__github__search".to_string(),
            title: "Search Docs".to_string(),
            user_facing_name: "Search Docs (GitHub)".to_string(),
            description: "Tool 'Search Docs' from MCP server 'GitHub': Find docs [Hints: read-only, open-world]".to_string(),
            provider_id: "github".to_string(),
            provider_kind: "mcp".to_string(),
            tool_info: McpToolInfo {
                server_id: "github".to_string(),
                server_name: "GitHub".to_string(),
                tool_name: "search".to_string(),
            },
            read_only: true,
        }
    );

    let rendered = render_mcp_tool_result_for_assistant(
        "search",
        &MCPToolResult {
            content: Some(vec![
                MCPToolResultContent::Text {
                    text: "done".to_string(),
                },
                MCPToolResultContent::Image {
                    data: "base64".to_string(),
                    mime_type: "image/png".to_string(),
                },
                MCPToolResultContent::ResourceLink {
                    uri: "file:///tmp/output.json".to_string(),
                    name: Some("output".to_string()),
                    description: None,
                    mime_type: Some("application/json".to_string()),
                },
            ]),
            is_error: false,
            structured_content: Some(serde_json::json!({ "ignored": "content wins" })),
            meta: None,
        },
        12_000,
    );
    assert_eq!(
        rendered,
        "done\n[Image: image/png]\n[Resource: output (file:///tmp/output.json)]"
    );

    assert_eq!(
        render_mcp_tool_result_for_assistant(
            "search",
            &MCPToolResult {
                content: None,
                is_error: true,
                structured_content: None,
                meta: None,
            },
            12_000,
        ),
        "Error executing MCP tool 'search'"
    );
}

#[tokio::test]
async fn mcp_config_service_orchestration_preserves_load_save_delete_contract() {
    let store = Arc::new(InMemoryMCPConfigStore::default());
    store.values.lock().await.insert(
        "mcp_servers".to_string(),
        serde_json::json!({
            "mcpServers": {
                "remote-docs": {
                    "type": "remote",
                    "url": "https://example.com/mcp",
                    "headers": {
                        "X-Existing": "kept"
                    }
                }
            }
        }),
    );

    let service = MCPConfigService::new(store.clone());

    let loaded = service.load_all_configs().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "remote-docs");
    assert_eq!(loaded[0].location, ConfigLocation::User);

    let updated = service
        .set_remote_authorization("remote-docs", "plain-token")
        .await
        .unwrap();
    assert_eq!(
        updated.headers.get("Authorization").map(String::as_str),
        Some("Bearer plain-token")
    );

    let saved_value = store
        .values
        .lock()
        .await
        .get("mcp_servers")
        .cloned()
        .unwrap();
    assert_eq!(
        saved_value["mcpServers"]["remote-docs"]["headers"]["Authorization"],
        "Bearer plain-token"
    );
    assert_eq!(
        saved_value["mcpServers"]["remote-docs"]["headers"]["X-Existing"],
        "kept"
    );

    let cleared = service
        .clear_remote_authorization("remote-docs")
        .await
        .unwrap();
    assert!(!cleared.headers.contains_key("Authorization"));

    service.delete_server_config("remote-docs").await.unwrap();
    let deleted_value = store
        .values
        .lock()
        .await
        .get("mcp_servers")
        .cloned()
        .unwrap();
    assert!(deleted_value["mcpServers"]
        .as_object()
        .unwrap()
        .get("remote-docs")
        .is_none());
}

#[tokio::test]
async fn mcp_config_service_keeps_load_failures_as_empty_baseline() {
    let service = MCPConfigService::new(Arc::new(FailingMCPConfigStore));

    let configs = service
        .load_all_configs()
        .await
        .expect("load failures are treated as empty config sources");
    assert!(configs.is_empty());

    let missing = service
        .get_server_config("missing")
        .await
        .expect("get_server_config also sees empty config sources");
    assert!(missing.is_none());

    let save_error = service
        .save_server_config(&make_mcp_config(
            "remote-docs",
            ConfigLocation::User,
            MCPServerType::Remote,
            None,
            Some("https://example.com/mcp"),
        ))
        .await
        .expect_err("writes must still surface config backend failures");
    assert_eq!(save_error.kind(), MCPRuntimeErrorKind::Configuration);
}

#[tokio::test]
async fn mcp_dynamic_tool_provider_preserves_manifest_contract() {
    let provider = MCPDynamicToolProvider::new("github", "GitHub");
    let definitions = provider
        .load_tool_definitions(&FakeMCPToolCatalogClient {
            tools: vec![MCPTool {
                name: "search".to_string(),
                title: Some("Search".to_string()),
                description: Some("Search repositories".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    }
                }),
                output_schema: None,
                icons: None,
                annotations: Some(MCPToolAnnotations {
                    title: Some("Search".to_string()),
                    read_only_hint: Some(true),
                    destructive_hint: Some(false),
                    idempotent_hint: Some(true),
                    open_world_hint: Some(false),
                }),
                meta: None,
            }],
        })
        .await
        .unwrap();

    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].mcp_tool.name, "search");
    assert_eq!(definitions[0].descriptor.full_name, "mcp__github__search");
    assert_eq!(definitions[0].descriptor.provider_id, "github");
    assert_eq!(definitions[0].descriptor.tool_info.server_name, "GitHub");
    assert!(definitions[0].descriptor.read_only);
}

#[tokio::test]
async fn mcp_dynamic_tool_provider_preserves_manifest_order_and_metadata_snapshot() {
    let provider = MCPDynamicToolProvider::new("docs-prod", "Docs Production");
    let definitions = provider
        .load_tool_definitions(&FakeMCPToolCatalogClient {
            tools: vec![
                MCPTool {
                    name: "lookup".to_string(),
                    title: None,
                    description: Some("Lookup docs".to_string()),
                    input_schema: serde_json::json!({ "type": "object" }),
                    output_schema: None,
                    icons: None,
                    annotations: Some(MCPToolAnnotations {
                        title: Some("Lookup".to_string()),
                        read_only_hint: Some(true),
                        destructive_hint: None,
                        idempotent_hint: Some(true),
                        open_world_hint: Some(false),
                    }),
                    meta: None,
                },
                MCPTool {
                    name: "write-note".to_string(),
                    title: Some("Write Note".to_string()),
                    description: None,
                    input_schema: serde_json::json!({ "type": "object" }),
                    output_schema: None,
                    icons: None,
                    annotations: Some(MCPToolAnnotations {
                        title: None,
                        read_only_hint: Some(false),
                        destructive_hint: Some(true),
                        idempotent_hint: Some(false),
                        open_world_hint: None,
                    }),
                    meta: None,
                },
            ],
        })
        .await
        .unwrap();

    let snapshot = definitions
        .iter()
        .map(|definition| {
            (
                definition.descriptor.full_name.as_str(),
                definition.descriptor.title.as_str(),
                definition.descriptor.provider_id.as_str(),
                definition.descriptor.provider_kind.as_str(),
                definition.descriptor.tool_info.tool_name.as_str(),
                definition.descriptor.read_only,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        snapshot,
        vec![
            (
                "mcp__docs-prod__lookup",
                "Lookup",
                "docs-prod",
                "mcp",
                "lookup",
                true
            ),
            (
                "mcp__docs-prod__write-note",
                "Write Note",
                "docs-prod",
                "mcp",
                "write-note",
                false,
            ),
        ]
    );
}

#[tokio::test]
async fn mcp_server_process_owner_preserves_unsupported_remote_transport_contract() {
    let mut config = make_mcp_config(
        "remote-sse",
        ConfigLocation::User,
        MCPServerType::Remote,
        None,
        Some("https://example.com/mcp"),
    );
    config.transport = Some(MCPServerTransport::Sse);

    let mut process = MCPServerProcess::new(
        "remote-sse".to_string(),
        "Remote SSE".to_string(),
        MCPServerType::Remote,
    );
    assert_eq!(process.status().await, MCPServerStatus::Uninitialized);
    assert_eq!(process.server_type(), MCPServerType::Remote);

    let error = process
        .start_remote(std::env::temp_dir(), &config)
        .await
        .unwrap_err();
    assert_eq!(error.kind(), MCPRuntimeErrorKind::NotImplemented);
    assert!(error
        .to_string()
        .contains("Remote MCP transport 'sse' is not yet supported"));
    assert_eq!(process.status().await, MCPServerStatus::Uninitialized);

    let pool = MCPConnectionPool::new();
    assert!(pool.get_all_server_ids().await.is_empty());
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

#[tokio::test]
async fn mcp_runtime_state_owns_registry_runtime_config_and_reconnect_state() {
    let runtime = MCPServerRuntimeState::new();
    let mut config = make_mcp_config(
        "runtime-only",
        ConfigLocation::User,
        MCPServerType::Local,
        Some("node"),
        None,
    );
    config.auto_start = false;

    assert!(runtime.is_empty().await);

    runtime
        .insert_runtime_config(config.clone())
        .await
        .expect("insert runtime config");
    runtime
        .register(&config)
        .await
        .expect("register runtime process");

    assert!(runtime.contains("runtime-only").await);
    assert_eq!(runtime.get_all_server_ids().await, vec!["runtime-only"]);
    assert!(runtime.get_process("runtime-only").await.is_some());
    assert_eq!(
        runtime.get_all_statuses().await,
        vec![("runtime-only".to_string(), MCPServerStatus::Uninitialized)]
    );
    assert_eq!(
        runtime
            .get_runtime_config("runtime-only")
            .await
            .expect("runtime config")
            .command
            .as_deref(),
        Some("node")
    );

    runtime.clear_reconnect_state("runtime-only").await;
    runtime.remove_catalog("runtime-only").await;
    runtime
        .unregister("runtime-only")
        .await
        .expect("unregister");
    runtime.remove_runtime_config("runtime-only").await;

    assert!(runtime.is_empty().await);
    assert!(runtime.get_runtime_config("runtime-only").await.is_none());
}

#[test]
fn mcp_runtime_policy_preserves_status_transition_contract() {
    let mut config = make_mcp_config(
        "local",
        ConfigLocation::User,
        MCPServerType::Local,
        Some("node"),
        None,
    );

    assert!(mcp_server_is_running(MCPServerStatus::Connected));
    assert!(mcp_server_is_running(MCPServerStatus::Healthy));
    assert!(!mcp_server_is_running(MCPServerStatus::Starting));

    assert!(mcp_should_start_after_config_update(
        &config,
        MCPServerStatus::Failed
    ));
    assert!(mcp_should_start_after_config_update(
        &config,
        MCPServerStatus::NeedsAuth
    ));
    assert!(!mcp_should_start_after_config_update(
        &config,
        MCPServerStatus::Connected
    ));

    assert_eq!(
        mcp_reconnect_runtime_decision(&config, MCPServerStatus::Failed),
        MCPReconnectRuntimeDecision::Retry
    );
    assert_eq!(
        mcp_reconnect_runtime_decision(&config, MCPServerStatus::NeedsAuth),
        MCPReconnectRuntimeDecision::Clear
    );
    assert_eq!(
        mcp_reconnect_runtime_decision(&config, MCPServerStatus::Starting),
        MCPReconnectRuntimeDecision::Clear
    );
    assert_eq!(
        mcp_reconnect_runtime_decision(&config, MCPServerStatus::Stopped),
        MCPReconnectRuntimeDecision::Skip
    );
    assert_eq!(
        mcp_reconnect_runtime_decision(&config, MCPServerStatus::Uninitialized),
        MCPReconnectRuntimeDecision::Skip
    );

    config.auto_start = false;
    assert_eq!(
        mcp_reconnect_runtime_decision(&config, MCPServerStatus::Failed),
        MCPReconnectRuntimeDecision::Clear
    );
    config.auto_start = true;
    config.enabled = false;
    assert_eq!(
        mcp_reconnect_runtime_decision(&config, MCPServerStatus::Failed),
        MCPReconnectRuntimeDecision::Clear
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
    let credentials = StoredCredentials::new("client-123".to_string(), None, Vec::new(), None);

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
    assert!(vault
        .load("server-a")
        .await
        .expect("load after clear")
        .is_none());

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
