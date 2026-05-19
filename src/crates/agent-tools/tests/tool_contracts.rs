use bitfun_agent_tools::{
    DynamicMcpToolInfo, DynamicToolInfo, GET_TOOL_SPEC_TOOL_NAME, InputValidator, ToolContextFacts,
    ToolExposure, ToolImageAttachment, ToolManifestDefinition, ToolManifestPolicyTool,
    ToolPathBackend, ToolPathResolution, ToolRenderOptions, ToolResult, ToolRuntimeRestrictions,
    ToolWorkspaceKind, ValidationResult, build_collapsed_tool_stub_definition,
    build_get_tool_spec_assistant_detail, build_get_tool_spec_collapsed_tool_entry,
    build_get_tool_spec_description, build_get_tool_spec_duplicate_load_hint,
    get_tool_spec_input_schema, resolve_tool_manifest_policy, sort_tool_manifest_definitions,
    validate_get_tool_spec_input,
};
use bitfun_agent_tools::{
    DynamicToolDescriptor, DynamicToolProvider, PortResult, PortableToolContextProvider,
    StaticToolProvider, StaticToolProviderGroup, ToolDecorator, ToolRegistry, ToolRegistryItem,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn validation_result_default_preserves_success_contract() {
    assert!(ValidationResult::default().result);
    assert_eq!(ValidationResult::default().message, None);
}

#[test]
fn input_validator_preserves_required_field_error() {
    let result = InputValidator::new(&json!({}))
        .validate_required("path")
        .finish();

    assert!(!result.result);
    assert_eq!(result.message.as_deref(), Some("path is required"));
    assert_eq!(result.error_code, Some(400));
}

#[test]
fn tool_result_ok_keeps_result_shape() {
    let result = ToolResult::ok(json!({"ok": true}), Some("done".to_string()));
    let value = serde_json::to_value(result).expect("serialize tool result");

    assert_eq!(value["type"], "result");
    assert_eq!(value["data"]["ok"], true);
    assert_eq!(value["result_for_assistant"], "done");
}

#[test]
fn tool_image_attachment_keeps_wire_shape_without_ai_adapter_dependency() {
    let attachment = ToolImageAttachment {
        mime_type: "image/png".to_string(),
        data_base64: "aW1hZ2U=".to_string(),
    };
    let result = ToolResult::ok_with_images(
        json!({"ok": true}),
        Some("captured screenshot".to_string()),
        vec![attachment],
    );

    let value = serde_json::to_value(&result).expect("serialize image tool result");
    assert_eq!(value["type"], "result");
    assert_eq!(value["image_attachments"][0]["mime_type"], "image/png");
    assert_eq!(value["image_attachments"][0]["data_base64"], "aW1hZ2U=");

    let round_trip: ToolResult = serde_json::from_value(value).expect("deserialize tool result");
    match round_trip {
        ToolResult::Result {
            image_attachments: Some(images),
            ..
        } => {
            assert_eq!(images.len(), 1);
            assert_eq!(images[0].mime_type, "image/png");
            assert_eq!(images[0].data_base64, "aW1hZ2U=");
        }
        other => panic!("expected image result, got {other:?}"),
    }
}

#[test]
fn dynamic_tool_info_keeps_provider_and_mcp_metadata_without_core_dependency() {
    let info = DynamicToolInfo {
        provider_id: "github-server-id".to_string(),
        provider_kind: Some("mcp".to_string()),
        mcp: Some(DynamicMcpToolInfo {
            server_id: "github-server-id".to_string(),
            server_name: "GitHub".to_string(),
            tool_name: "search_repos".to_string(),
        }),
    };

    let value = serde_json::to_value(&info).expect("serialize dynamic info");

    assert_eq!(value["providerId"], "github-server-id");
    assert_eq!(value["providerKind"], "mcp");
    assert_eq!(value["mcp"]["serverId"], "github-server-id");
    assert_eq!(value["mcp"]["serverName"], "GitHub");
    assert_eq!(value["mcp"]["toolName"], "search_repos");

    let round_trip: DynamicToolInfo =
        serde_json::from_value(value).expect("deserialize dynamic info");
    assert_eq!(round_trip.provider_id, "github-server-id");
    assert_eq!(round_trip.provider_kind.as_deref(), Some("mcp"));
    assert_eq!(
        round_trip.mcp.as_ref().map(|mcp| mcp.tool_name.as_str()),
        Some("search_repos")
    );
}

#[test]
fn tool_render_options_stays_a_lightweight_contract() {
    let options = ToolRenderOptions { verbose: true };

    assert!(options.verbose);
}

#[test]
fn runtime_restrictions_keep_allow_deny_semantics_without_core_dependency() {
    let restrictions = ToolRuntimeRestrictions {
        allowed_tool_names: ["Read", "Write"].into_iter().map(str::to_string).collect(),
        denied_tool_names: ["Write"].into_iter().map(str::to_string).collect(),
        path_policy: Default::default(),
    };

    assert!(restrictions.is_tool_allowed("Read"));
    assert!(!restrictions.is_tool_allowed("Write"));
    assert!(!restrictions.is_tool_allowed("Bash"));

    let denied = restrictions
        .ensure_tool_allowed("Write")
        .expect_err("deny list must override allow list");
    assert_eq!(
        denied.to_string(),
        "Tool 'Write' is denied by runtime restrictions"
    );

    let not_allowed = restrictions
        .ensure_tool_allowed("Bash")
        .expect_err("non-empty allow list must reject missing tools");
    assert_eq!(
        not_allowed.to_string(),
        "Tool 'Bash' is not allowed by runtime restrictions"
    );
}

#[test]
fn tool_context_facts_keep_portable_wire_shape_without_runtime_handles() {
    let facts = ToolContextFacts {
        tool_call_id: Some("call-1".to_string()),
        agent_type: Some("Agentic".to_string()),
        session_id: Some("session-1".to_string()),
        dialog_turn_id: Some("turn-1".to_string()),
        workspace_kind: Some(ToolWorkspaceKind::Remote),
        workspace_root: Some("/remote/workspace".to_string()),
        runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
    };

    let value = serde_json::to_value(&facts).expect("serialize context facts");

    assert_eq!(value["toolCallId"], "call-1");
    assert_eq!(value["agentType"], "Agentic");
    assert_eq!(value["sessionId"], "session-1");
    assert_eq!(value["dialogTurnId"], "turn-1");
    assert_eq!(value["workspaceKind"], "remote");
    assert_eq!(value["workspaceRoot"], "/remote/workspace");
    assert!(value.get("unlockedCollapsedTools").is_none());
    assert!(value.get("computer_use_host").is_none());
    assert!(value.get("workspace_services").is_none());
    assert!(value.get("cancellation_token").is_none());

    let round_trip: ToolContextFacts =
        serde_json::from_value(value).expect("deserialize context facts");
    assert_eq!(round_trip.workspace_kind, Some(ToolWorkspaceKind::Remote));
}

#[test]
fn portable_tool_context_provider_exposes_facts_only() {
    struct FactsOnlyProvider {
        facts: ToolContextFacts,
    }

    impl PortableToolContextProvider for FactsOnlyProvider {
        fn tool_context_facts(&self) -> ToolContextFacts {
            self.facts.clone()
        }
    }

    let provider = FactsOnlyProvider {
        facts: ToolContextFacts {
            tool_call_id: Some("call-2".to_string()),
            agent_type: Some("Agentic".to_string()),
            session_id: Some("session-2".to_string()),
            dialog_turn_id: None,
            workspace_kind: Some(ToolWorkspaceKind::Local),
            workspace_root: Some("/repo/project".to_string()),
            runtime_tool_restrictions: ToolRuntimeRestrictions::default(),
        },
    };

    let value =
        serde_json::to_value(provider.tool_context_facts()).expect("serialize context facts");

    assert_eq!(value["toolCallId"], "call-2");
    assert_eq!(value["workspaceKind"], "local");
    assert!(value.get("workspace_services").is_none());
    assert!(value.get("unlockedCollapsedTools").is_none());
}

#[test]
fn runtime_restrictions_keep_current_snake_case_wire_shape() {
    let value = json!({
        "allowed_tool_names": ["Read"],
        "denied_tool_names": ["Write"],
        "path_policy": {
            "write_roots": ["src"],
            "edit_roots": ["docs"],
            "delete_roots": ["target/generated"]
        }
    });

    let restrictions: ToolRuntimeRestrictions =
        serde_json::from_value(value.clone()).expect("deserialize restrictions");
    assert!(restrictions.is_tool_allowed("Read"));
    assert!(!restrictions.is_tool_allowed("Write"));
    assert_eq!(restrictions.path_policy.write_roots, vec!["src"]);
    assert_eq!(restrictions.path_policy.edit_roots, vec!["docs"]);
    assert_eq!(
        restrictions.path_policy.delete_roots,
        vec!["target/generated"]
    );

    let round_trip = serde_json::to_value(&restrictions).expect("serialize restrictions");
    assert_eq!(round_trip, value);
}

#[test]
fn path_resolution_contract_keeps_backend_and_runtime_helpers() {
    let remote = ToolPathResolution {
        requested_path: "src/lib.rs".to_string(),
        logical_path: "/workspace/src/lib.rs".to_string(),
        resolved_path: "/workspace/src/lib.rs".to_string(),
        backend: ToolPathBackend::RemoteWorkspace,
        runtime_scope: None,
        runtime_root: None,
    };
    assert!(remote.uses_remote_workspace_backend());
    assert!(!remote.is_runtime_artifact());

    let runtime_root = PathBuf::from("/runtime/workspace");
    let runtime = ToolPathResolution {
        requested_path: "bitfun://runtime/workspace-1/logs/tool.txt".to_string(),
        logical_path: "bitfun://runtime/workspace-1/logs/tool.txt".to_string(),
        resolved_path: runtime_root
            .join("logs")
            .join("tool.txt")
            .display()
            .to_string(),
        backend: ToolPathBackend::Local,
        runtime_scope: Some("workspace-1".to_string()),
        runtime_root: Some(runtime_root.clone()),
    };

    assert!(!runtime.uses_remote_workspace_backend());
    assert!(runtime.is_runtime_artifact());
    assert_eq!(
        runtime.logical_child_path(&runtime_root.join("logs").join("tool.txt")),
        Some("bitfun://runtime/workspace-1/logs/tool.txt".to_string())
    );
    assert_eq!(
        runtime.logical_child_path(&PathBuf::from("/outside/tool.txt")),
        None
    );
}

#[test]
fn dynamic_tool_provider_contract_is_available_from_agent_tools_boundary() {
    fn assert_provider_contract<T: DynamicToolProvider>() {}
    fn assert_decorator_contract<T: ToolDecorator<String>>() {}

    struct MarkerProvider;
    #[async_trait::async_trait]
    impl DynamicToolProvider for MarkerProvider {
        async fn list_dynamic_tools(&self) -> PortResult<Vec<DynamicToolDescriptor>> {
            Ok(Vec::new())
        }
    }

    struct MarkerDecorator;
    impl ToolDecorator<String> for MarkerDecorator {
        fn decorate(&self, tool: String) -> String {
            tool
        }
    }

    assert_provider_contract::<MarkerProvider>();
    assert_decorator_contract::<MarkerDecorator>();
}

#[test]
fn tool_exposure_contract_keeps_lightweight_wire_shape() {
    let collapsed = ToolExposure::Collapsed;
    let value = serde_json::to_value(collapsed).expect("serialize exposure");

    assert_eq!(value, json!("Collapsed"));
    assert_eq!(
        serde_json::from_value::<ToolExposure>(value).expect("deserialize exposure"),
        ToolExposure::Collapsed
    );
}

#[test]
fn tool_manifest_definition_keeps_lightweight_wire_shape() {
    let definition = ToolManifestDefinition::new(
        "Read",
        "Read a file",
        json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string" }
            },
            "required": ["file_path"]
        }),
    );

    let value = serde_json::to_value(&definition).expect("serialize definition");

    assert_eq!(value["name"], json!("Read"));
    assert_eq!(value["description"], json!("Read a file"));
    assert_eq!(value["parameters"]["required"], json!(["file_path"]));
    assert_eq!(
        serde_json::from_value::<ToolManifestDefinition>(value).expect("deserialize definition"),
        definition
    );
}

#[test]
fn tool_manifest_policy_keeps_get_tool_spec_insertion_and_registry_order() {
    let tools = vec![
        ToolManifestPolicyTool {
            name: "Read".to_string(),
            default_exposure: ToolExposure::Expanded,
            available: true,
        },
        ToolManifestPolicyTool {
            name: "WebSearch".to_string(),
            default_exposure: ToolExposure::Collapsed,
            available: true,
        },
        ToolManifestPolicyTool {
            name: "WebFetch".to_string(),
            default_exposure: ToolExposure::Collapsed,
            available: true,
        },
        ToolManifestPolicyTool {
            name: GET_TOOL_SPEC_TOOL_NAME.to_string(),
            default_exposure: ToolExposure::Expanded,
            available: true,
        },
        ToolManifestPolicyTool {
            name: "HiddenUnavailable".to_string(),
            default_exposure: ToolExposure::Expanded,
            available: false,
        },
    ];
    let allowed_tools = vec![
        "WebFetch".to_string(),
        "Read".to_string(),
        "WebSearch".to_string(),
        "HiddenUnavailable".to_string(),
    ];
    let overrides = Default::default();

    let policy =
        resolve_tool_manifest_policy(&tools, &allowed_tools, &overrides, GET_TOOL_SPEC_TOOL_NAME);

    assert_eq!(
        policy.allowed_tool_names,
        vec![
            "WebFetch",
            "Read",
            "WebSearch",
            "HiddenUnavailable",
            GET_TOOL_SPEC_TOOL_NAME,
        ]
    );
    assert_eq!(
        policy.expanded_tool_names,
        vec!["Read", GET_TOOL_SPEC_TOOL_NAME]
    );
    assert_eq!(policy.collapsed_tool_names, vec!["WebSearch", "WebFetch"]);
}

#[test]
fn tool_manifest_policy_preserves_explicit_get_tool_spec_duplicate_runtime_contract() {
    let tools = vec![
        ToolManifestPolicyTool {
            name: GET_TOOL_SPEC_TOOL_NAME.to_string(),
            default_exposure: ToolExposure::Expanded,
            available: true,
        },
        ToolManifestPolicyTool {
            name: "WebFetch".to_string(),
            default_exposure: ToolExposure::Collapsed,
            available: true,
        },
    ];
    let allowed_tools = vec![GET_TOOL_SPEC_TOOL_NAME.to_string(), "WebFetch".to_string()];
    let overrides = Default::default();

    let policy =
        resolve_tool_manifest_policy(&tools, &allowed_tools, &overrides, GET_TOOL_SPEC_TOOL_NAME);

    assert_eq!(
        policy.allowed_tool_names,
        vec![GET_TOOL_SPEC_TOOL_NAME, "WebFetch"]
    );
    assert_eq!(
        policy.expanded_tool_names,
        vec![GET_TOOL_SPEC_TOOL_NAME, GET_TOOL_SPEC_TOOL_NAME],
        "core currently appends the runtime GetToolSpec entry whenever collapsed tools exist"
    );
    assert_eq!(policy.collapsed_tool_names, vec!["WebFetch"]);
}

#[test]
fn collapsed_tool_stub_definition_preserves_prompt_visible_guardrail() {
    let stub = build_collapsed_tool_stub_definition(
        "WebFetch",
        "Fetch a URL and return readable content.",
    );

    assert_eq!(stub.name, "WebFetch");
    assert!(stub.description.contains("Fetch a URL"));
    assert!(
        stub.description
            .contains("First call `GetToolSpec` with {\"tool_name\":\"WebFetch\"}")
    );
    assert_eq!(
        stub.parameters,
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Do not supply WebFetch arguments here while the tool is collapsed. Use GetToolSpec with {\"tool_name\":\"WebFetch\"} first."
                }
            }
        })
    );
}

#[test]
fn tool_manifest_sorting_preserves_prompt_visible_order() {
    let mut definitions = vec![
        ToolManifestDefinition::new("ControlHub", "control", json!({ "type": "object" })),
        ToolManifestDefinition::new("Read", "read", json!({ "type": "object" })),
        ToolManifestDefinition::new("ExternalTool", "external", json!({ "type": "object" })),
        ToolManifestDefinition::new("GetToolSpec", "spec", json!({ "type": "object" })),
        ToolManifestDefinition::new("Task", "task", json!({ "type": "object" })),
    ];

    sort_tool_manifest_definitions(&mut definitions);

    assert_eq!(
        definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Task", "Read", "GetToolSpec", "ControlHub", "ExternalTool"]
    );
}

#[test]
fn get_tool_spec_contract_preserves_input_schema_and_validation() {
    let schema = get_tool_spec_input_schema();

    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["required"], json!(["tool_name"]));
    assert_eq!(schema["properties"]["tool_name"]["type"], "string");
    assert!(
        schema["properties"]["tool_name"]["description"]
            .as_str()
            .unwrap_or_default()
            .contains("canonical casing")
    );

    let missing = validate_get_tool_spec_input(&json!({}));
    assert!(!missing.result);
    assert_eq!(
        missing.message.as_deref(),
        Some("tool_name is required and cannot be empty")
    );
    assert_eq!(missing.error_code, Some(400));

    let empty = validate_get_tool_spec_input(&json!({ "tool_name": "" }));
    assert!(!empty.result);
    assert_eq!(
        empty.message.as_deref(),
        Some("tool_name is required and cannot be empty")
    );
    assert_eq!(empty.error_code, Some(400));

    assert!(validate_get_tool_spec_input(&json!({ "tool_name": "Git" })).result);
}

#[test]
fn get_tool_spec_contract_preserves_collapsed_prompt_description() {
    let collapsed_tools_list = [
        build_get_tool_spec_collapsed_tool_entry("Git", "Inspect the repository."),
        build_get_tool_spec_collapsed_tool_entry("WebFetch", "Fetch readable web content."),
    ]
    .join("\n");

    let description = build_get_tool_spec_description(&collapsed_tools_list);

    assert!(description.contains("<collapsed_tools>\n- Git: Inspect the repository."));
    assert!(description.contains("- WebFetch: Fetch readable web content."));
    assert!(description.contains("Do not call GetToolSpec again"));
    assert!(description.contains("call `GetToolSpec` with `{\"tool_name\":\"Git\"}`"));
}

#[test]
fn get_tool_spec_contract_escapes_assistant_detail_for_xml_sections() {
    let detail = build_get_tool_spec_assistant_detail(
        "Use <danger> & keep output valid.",
        &json!({
            "type": "object",
            "properties": {
                "query": {
                    "description": "Match <tag> & symbols"
                }
            }
        }),
    );

    assert!(detail.contains("<description>\nUse &lt;danger&gt; &amp; keep output valid."));
    assert!(detail.contains("\"description\":\"Match &lt;tag&gt; &amp; symbols\""));
    assert!(!detail.contains("Use <danger> & keep output valid."));
}

#[test]
fn get_tool_spec_contract_preserves_duplicate_load_hint() {
    assert_eq!(
        build_get_tool_spec_duplicate_load_hint("WebFetch"),
        "Tool 'WebFetch' is already loaded in the current conversation. Do not call GetToolSpec again for it. Use 'WebFetch' directly."
    );
}

#[derive(Clone)]
struct RegistryMarkerTool {
    name: String,
    provider_id: Option<String>,
}

#[async_trait::async_trait]
impl ToolRegistryItem for RegistryMarkerTool {
    fn name(&self) -> &str {
        &self.name
    }

    async fn description(&self) -> Result<String, String> {
        Ok("marker tool".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({ "type": "object" })
    }

    async fn input_schema_for_model(&self) -> serde_json::Value {
        self.input_schema()
    }

    fn dynamic_tool_info(&self) -> Option<DynamicToolInfo> {
        self.provider_id
            .as_ref()
            .map(|provider_id| DynamicToolInfo {
                provider_id: provider_id.clone(),
                provider_kind: None,
                mcp: None,
            })
    }
}

fn registry_marker_tool(name: &str, provider_id: Option<&str>) -> Arc<RegistryMarkerTool> {
    Arc::new(RegistryMarkerTool {
        name: name.to_string(),
        provider_id: provider_id.map(str::to_string),
    })
}

struct RegistryMarkerProvider {
    provider_id: &'static str,
    tools: Vec<Arc<RegistryMarkerTool>>,
}

impl StaticToolProvider<RegistryMarkerTool> for RegistryMarkerProvider {
    fn provider_id(&self) -> &'static str {
        self.provider_id
    }

    fn tools(&self) -> Vec<Arc<RegistryMarkerTool>> {
        self.tools.clone()
    }
}

#[test]
fn static_tool_provider_group_preserves_provider_id_and_tool_order() {
    let provider = StaticToolProviderGroup::new(
        "core-basic",
        vec![
            registry_marker_tool("Read", None),
            registry_marker_tool("Write", None),
        ],
    );

    assert_eq!(provider.provider_id(), "core-basic");
    assert_eq!(
        provider
            .tools()
            .iter()
            .map(|tool| tool.name())
            .collect::<Vec<_>>(),
        vec!["Read", "Write"]
    );
}

struct RegistryMarkerDecorator;

impl ToolDecorator<Arc<RegistryMarkerTool>> for RegistryMarkerDecorator {
    fn decorate(&self, tool: Arc<RegistryMarkerTool>) -> Arc<RegistryMarkerTool> {
        Arc::new(RegistryMarkerTool {
            name: format!("decorated_{}", tool.name),
            provider_id: tool.provider_id.clone(),
        })
    }
}

#[test]
fn generic_tool_registry_installs_static_provider_in_order() {
    let mut registry = ToolRegistry::new();
    let provider = RegistryMarkerProvider {
        provider_id: "core-basic",
        tools: vec![
            registry_marker_tool("Read", None),
            registry_marker_tool("Write", None),
        ],
    };

    registry.install_static_provider(&provider);

    assert_eq!(provider.provider_id(), "core-basic");
    assert_eq!(
        registry.get_tool_names(),
        vec!["Read".to_string(), "Write".to_string()]
    );
}

#[test]
fn generic_tool_registry_applies_decorator_to_static_provider_tools() {
    let mut registry = ToolRegistry::with_tool_decorator(Arc::new(RegistryMarkerDecorator));
    let provider = RegistryMarkerProvider {
        provider_id: "decorated-provider",
        tools: vec![registry_marker_tool("Read", None)],
    };

    registry.install_static_provider(&provider);

    assert_eq!(
        registry.get_tool_names(),
        vec!["decorated_Read".to_string()]
    );
}

#[tokio::test]
async fn generic_tool_registry_preserves_dynamic_descriptor_contract() {
    let mut registry = ToolRegistry::new();
    registry.register_tool(registry_marker_tool("external_search", Some("provider-a")));
    registry.register_tool(registry_marker_tool("local_docs", Some("provider-b")));
    registry.register_tool(registry_marker_tool("static_tool", None));

    assert_eq!(
        registry.get_tool_names(),
        vec!["external_search", "local_docs", "static_tool"]
    );
    assert_eq!(
        registry
            .get_dynamic_tool_info("external_search")
            .expect("dynamic metadata")
            .provider_id,
        "provider-a"
    );

    let descriptors = registry
        .list_dynamic_tools()
        .await
        .expect("list dynamic tools");
    assert_eq!(
        descriptors
            .iter()
            .map(|descriptor| (descriptor.name.as_str(), descriptor.provider_id.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("external_search", Some("provider-a")),
            ("local_docs", Some("provider-b")),
        ]
    );
    assert_eq!(descriptors[0].description, "marker tool");
    assert_eq!(descriptors[0].input_schema, json!({ "type": "object" }));
}

#[tokio::test]
async fn generic_tool_registry_clears_stale_dynamic_metadata_on_overwrite() {
    let mut registry = ToolRegistry::new();
    registry.register_tool(registry_marker_tool("external_search", Some("provider-a")));

    registry.register_tool(registry_marker_tool("external_search", None));

    assert!(registry.get_dynamic_tool_info("external_search").is_none());
    let descriptors = registry
        .list_dynamic_tools()
        .await
        .expect("list dynamic tools");
    assert!(descriptors.is_empty());
}
