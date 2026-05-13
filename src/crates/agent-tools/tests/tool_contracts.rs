use bitfun_agent_tools::{
    DynamicMcpToolInfo, DynamicToolInfo, InputValidator, ToolImageAttachment, ToolPathBackend,
    ToolPathResolution, ToolRenderOptions, ToolResult, ToolRuntimeRestrictions, ValidationResult,
};
use bitfun_agent_tools::{DynamicToolDescriptor, DynamicToolProvider, PortResult, ToolDecorator};
use serde_json::json;
use std::path::PathBuf;

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
