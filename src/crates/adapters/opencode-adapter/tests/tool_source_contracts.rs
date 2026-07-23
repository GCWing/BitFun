use bitfun_opencode_adapter::{OpenCodeToolProvider, OpenCodeToolProviderOptions};
use bitfun_product_domains::external_sources::{
    ExecutionDomainId, ExternalSourceContext, ExternalSourceScope, ExternalToolSourceProvider,
    ExternalToolStaticStatus,
};
use bitfun_runtime_ports::{
    ScriptToolExpectedExport, ScriptToolInvokeRequest, ScriptToolLoadRequest, ScriptToolRuntime,
    ScriptToolRuntimeAvailability,
};
use bitfun_services_integrations::script_tool::NodeScriptToolRuntime;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn context(workspace: PathBuf) -> ExternalSourceContext {
    ExternalSourceContext {
        workspace_root: Some(workspace),
        execution_domain_id: ExecutionDomainId::new("local-user").unwrap(),
    }
}

fn provider(user_config_dir: PathBuf) -> OpenCodeToolProvider {
    OpenCodeToolProvider::new(OpenCodeToolProviderOptions {
        user_config_dir,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: true,
    })
}

#[test]
fn discovers_global_and_project_js_tools_without_executing_them() {
    let temp = tempfile::tempdir().unwrap();
    let user = temp.path().join("user-opencode");
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(user.join("tools")).unwrap();
    fs::create_dir_all(workspace.join(".opencode/tool")).unwrap();
    fs::write(
        user.join("tools/weather.js"),
        r#"
import { tool } from "@opencode-ai/plugin"
export default tool({
  description: "Get weather",
  args: { location: tool.schema.string() },
  async execute(args) { return args.location },
})
"#,
    )
    .unwrap();
    let marker = temp.path().join("must-not-exist");
    fs::write(
        workspace.join(".opencode/tool/utility.js"),
        format!(
            r#"
import {{ tool }} from "@opencode-ai/plugin"
import {{ writeFileSync }} from "node:fs"
writeFileSync({marker:?}, "executed")
export const echo = tool({{
  description: "Echo text",
  args: {{ text: tool.schema.string() }},
  async execute(args) {{ return args.text }},
}})
"#
        ),
    )
    .unwrap();

    let snapshot = provider(user)
        .discover(&context(workspace))
        .expect("static discovery should succeed");

    let names = snapshot
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["utility_echo", "weather"]);
    assert!(matches!(
        snapshot.tools[0].static_status,
        ExternalToolStaticStatus::Unsupported { .. }
    ));
    assert!(matches!(
        snapshot.tools[1].static_status,
        ExternalToolStaticStatus::Ready
    ));
    assert!(!marker.exists(), "discovery must never import tool code");
}

#[test]
fn recognizes_typescript_but_keeps_it_explicitly_unavailable_in_pr2() {
    let temp = tempfile::tempdir().unwrap();
    let user = temp.path().join("user-opencode");
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(user.join("tool")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        user.join("tool/search.ts"),
        r#"export default { description: "Search", args: {}, execute() { return "ok" } }"#,
    )
    .unwrap();

    let snapshot = provider(user).discover(&context(workspace)).unwrap();
    assert_eq!(snapshot.tools.len(), 1);
    assert!(matches!(
        &snapshot.tools[0].static_status,
        ExternalToolStaticStatus::Unsupported { reason }
            if reason.contains("TypeScript")
    ));
}

#[test]
fn prepares_only_the_supported_single_file_javascript_subset_and_checks_revision() {
    let temp = tempfile::tempdir().unwrap();
    let user = temp.path().join("user-opencode");
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(user.join("tools")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    let path = user.join("tools/weather.js");
    fs::write(
        &path,
        r#"
import { tool } from "@opencode-ai/plugin"
export default tool({
  description: "Get weather",
  args: { location: tool.schema.string().describe("Location") },
  async execute(args) { return args.location },
})
"#,
    )
    .unwrap();
    let provider = provider(user);
    let context = context(workspace);
    let snapshot = provider.discover(&context).unwrap();
    let definition = snapshot.tools[0].clone();

    let prepared = provider
        .prepare_target(&context, &definition.id.target, &definition.content_version)
        .expect("supported source should be prepared after approval");
    assert!(!prepared.module_source.contains("@opencode-ai/plugin"));
    assert!(prepared.module_source.contains("const tool ="));
    assert_eq!(prepared.expected_tools[0].tool_name, "weather");

    fs::write(&path, "export default {").unwrap();
    let error = provider
        .prepare_target(&context, &definition.id.target, &definition.content_version)
        .unwrap_err();
    assert_eq!(error.code, "opencode.tool.stale_revision");
}

#[test]
fn dynamic_import_with_comments_is_recognized_as_unsupported() {
    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    let tools = global.join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    std::fs::write(
        tools.join("dynamic.js"),
        r#"export default { description: "dynamic", args: {}, async execute() { return import /* hidden */ ("node:fs"); } };"#,
    )
    .unwrap();
    let provider = OpenCodeToolProvider::new(OpenCodeToolProviderOptions {
        user_config_dir: global,
        legacy_user_config_dir: None,
        explicit_config_dir: None,
        project_config_enabled: false,
    });
    let snapshot = provider
        .discover(&context(temp.path().join("workspace")))
        .unwrap();

    assert!(matches!(
        snapshot.tools[0].static_status,
        ExternalToolStaticStatus::Unsupported { .. }
    ));
}

#[test]
fn explicit_config_directory_is_appended_to_the_default_global_directory() {
    let temp = tempfile::tempdir().unwrap();
    let default_global = temp.path().join("default-global");
    let explicit_global = temp.path().join("explicit-global");
    fs::create_dir_all(default_global.join("tools")).unwrap();
    fs::create_dir_all(explicit_global.join("tools")).unwrap();
    fs::write(
        default_global.join("tools/default.js"),
        r#"export default { description: "default", args: {}, execute() { return "default" } }"#,
    )
    .unwrap();
    fs::write(
        explicit_global.join("tools/explicit.js"),
        r#"export default { description: "explicit", args: {}, execute() { return "explicit" } }"#,
    )
    .unwrap();
    let provider = OpenCodeToolProvider::new(OpenCodeToolProviderOptions {
        user_config_dir: default_global,
        legacy_user_config_dir: None,
        explicit_config_dir: Some(explicit_global),
        project_config_enabled: false,
    });

    let snapshot = provider
        .discover(&context(temp.path().join("workspace")))
        .unwrap();
    assert_eq!(
        snapshot
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        vec!["default", "explicit"]
    );
    assert!(snapshot
        .sources
        .iter()
        .all(|source| source.scope == ExternalSourceScope::UserGlobal));
}

#[test]
fn non_portable_tool_names_are_reported_without_poisoning_other_tools() {
    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    fs::create_dir_all(global.join("tools")).unwrap();
    fs::write(
        global.join("tools/bad name.js"),
        r#"export default { description: "bad", args: {}, execute() { return "bad" } }"#,
    )
    .unwrap();
    fs::write(
        global.join("tools/safe.js"),
        r#"export default { description: "safe", args: {}, execute() { return "safe" } }"#,
    )
    .unwrap();

    let snapshot = provider(global)
        .discover(&context(temp.path().join("workspace")))
        .unwrap();
    assert_eq!(snapshot.tools.len(), 1);
    assert_eq!(snapshot.tools[0].name, "safe");
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.tool.name_unsupported"));
}

#[tokio::test]
async fn prepared_schema_chains_preserve_defaults_and_type_specific_bounds() {
    let runtime = NodeScriptToolRuntime::discover();
    if matches!(
        runtime.availability().await,
        ScriptToolRuntimeAvailability::Unavailable { .. }
    ) {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(global.join("tools")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        global.join("tools/schema.js"),
        r#"
import { tool } from "@opencode-ai/plugin"
export default tool({
  description: "Checks schema chains",
  args: {
    greeting: tool.schema.string().min(2).default("hello"),
    tags: tool.schema.array(tool.schema.string()).min(2).max(3),
  },
  execute(args) { return `${args.greeting}:${args.tags.join(",")}` },
})
"#,
    )
    .unwrap();
    let provider = provider(global);
    let context = context(workspace);
    let definition = provider.discover(&context).unwrap().tools.remove(0);
    let prepared = provider
        .prepare_target(&context, &definition.id.target, &definition.content_version)
        .unwrap();
    let target_id = prepared.target_id.stable_key();
    let revision = prepared.content_version.clone();
    let loaded = runtime
        .load(ScriptToolLoadRequest {
            target_id: target_id.clone(),
            revision: revision.clone(),
            module_source: prepared.module_source,
            module_url: prepared.module_url,
            working_directory: prepared.working_directory,
            expected_tools: prepared
                .expected_tools
                .into_iter()
                .map(|tool| ScriptToolExpectedExport {
                    export_name: tool.export_name,
                    tool_name: tool.tool_name,
                })
                .collect(),
        })
        .await
        .unwrap();
    assert_eq!(loaded.tools[0].input_schema["required"], json!(["tags"]));
    assert_eq!(
        loaded.tools[0].input_schema["properties"]["greeting"]["minLength"],
        json!(2)
    );
    assert_eq!(
        loaded.tools[0].input_schema["properties"]["tags"]["minItems"],
        json!(2)
    );

    let response = runtime
        .invoke(ScriptToolInvokeRequest {
            target_id,
            revision,
            export_name: "default".to_string(),
            operation_id: "schema-default".to_string(),
            arguments: json!({"tags": ["a", "b"]}),
            workspace_root: None,
            worktree_root: None,
            session_id: None,
        })
        .await
        .unwrap();
    assert_eq!(response.output, "hello:a,b");
}

#[tokio::test]
async fn prepared_optional_schema_can_be_omitted_at_runtime() {
    let runtime = NodeScriptToolRuntime::discover();
    if matches!(
        runtime.availability().await,
        ScriptToolRuntimeAvailability::Unavailable { .. }
    ) {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(global.join("tools")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        global.join("tools/optional.js"),
        r#"
import { tool } from "@opencode-ai/plugin"
export default tool({
  description: "Checks optional schema arguments",
  args: {
    required: tool.schema.string(),
    note: tool.schema.string().optional(),
  },
  execute(args) { return `${args.required}:${args.note ?? "missing"}` },
})
"#,
    )
    .unwrap();
    let provider = provider(global);
    let context = context(workspace);
    let definition = provider.discover(&context).unwrap().tools.remove(0);
    let prepared = provider
        .prepare_target(&context, &definition.id.target, &definition.content_version)
        .unwrap();
    let target_id = prepared.target_id.stable_key();
    let revision = prepared.content_version.clone();
    let loaded = runtime
        .load(ScriptToolLoadRequest {
            target_id: target_id.clone(),
            revision: revision.clone(),
            module_source: prepared.module_source,
            module_url: prepared.module_url,
            working_directory: prepared.working_directory,
            expected_tools: prepared
                .expected_tools
                .into_iter()
                .map(|tool| ScriptToolExpectedExport {
                    export_name: tool.export_name,
                    tool_name: tool.tool_name,
                })
                .collect(),
        })
        .await
        .unwrap();
    assert_eq!(
        loaded.tools[0].input_schema["required"],
        json!(["required"])
    );

    let response = runtime
        .invoke(ScriptToolInvokeRequest {
            target_id,
            revision,
            export_name: "default".to_string(),
            operation_id: "schema-optional".to_string(),
            arguments: json!({"required": "hello"}),
            workspace_root: None,
            worktree_root: None,
            session_id: None,
        })
        .await
        .expect("optional arguments without defaults may be omitted");
    assert_eq!(response.output, "hello:missing");
}

#[tokio::test]
async fn prepared_nested_schema_preserves_required_optional_and_defaults() {
    let runtime = NodeScriptToolRuntime::discover();
    if matches!(
        runtime.availability().await,
        ScriptToolRuntimeAvailability::Unavailable { .. }
    ) {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let global = temp.path().join("global");
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(global.join("tools")).unwrap();
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        global.join("tools/nested.js"),
        r#"
import { tool } from "@opencode-ai/plugin"
export default tool({
  description: "Checks nested schema arguments",
  args: {
    payload: tool.schema.object({
      required: tool.schema.string(),
      optional: tool.schema.string().optional(),
      defaulted: tool.schema.string().default("fallback"),
      items: tool.schema.array(tool.schema.object({
        name: tool.schema.string(),
        note: tool.schema.string().optional(),
      })),
    }),
  },
  execute(args) {
    return `${args.payload.required}:${args.payload.optional ?? "missing"}:${args.payload.defaulted}:${args.payload.items[0].name}`
  },
})
"#,
    )
    .unwrap();
    let provider = provider(global);
    let context = context(workspace);
    let definition = provider.discover(&context).unwrap().tools.remove(0);
    let prepared = provider
        .prepare_target(&context, &definition.id.target, &definition.content_version)
        .unwrap();
    let target_id = prepared.target_id.stable_key();
    let revision = prepared.content_version.clone();
    let loaded = runtime
        .load(ScriptToolLoadRequest {
            target_id: target_id.clone(),
            revision: revision.clone(),
            module_source: prepared.module_source,
            module_url: prepared.module_url,
            working_directory: prepared.working_directory,
            expected_tools: prepared
                .expected_tools
                .into_iter()
                .map(|tool| ScriptToolExpectedExport {
                    export_name: tool.export_name,
                    tool_name: tool.tool_name,
                })
                .collect(),
        })
        .await
        .unwrap();
    let schema = &loaded.tools[0].input_schema;
    assert_eq!(schema["required"], json!(["payload"]));
    assert_eq!(
        schema["properties"]["payload"]["required"],
        json!(["required", "items"])
    );
    assert_eq!(
        schema["properties"]["payload"]["properties"]["items"]["items"]["required"],
        json!(["name"])
    );
    assert!(!serde_json::to_string(schema).unwrap().contains("__"));

    let missing_required = runtime
        .invoke(ScriptToolInvokeRequest {
            target_id: target_id.clone(),
            revision: revision.clone(),
            export_name: "default".to_string(),
            operation_id: "nested-required".to_string(),
            arguments: json!({"payload": {"items": [{"name": "first"}]}}),
            workspace_root: None,
            worktree_root: None,
            session_id: None,
        })
        .await
        .expect_err("nested required fields must be validated");
    assert!(missing_required
        .message
        .contains("payload.required is required"));

    let response = runtime
        .invoke(ScriptToolInvokeRequest {
            target_id,
            revision,
            export_name: "default".to_string(),
            operation_id: "nested-default".to_string(),
            arguments: json!({"payload": {"required": "hello", "items": [{"name": "first"}]}}),
            workspace_root: None,
            worktree_root: None,
            session_id: None,
        })
        .await
        .unwrap();
    assert_eq!(response.output, "hello:missing:fallback:first");
}
