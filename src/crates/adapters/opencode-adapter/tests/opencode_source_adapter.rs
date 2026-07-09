use bitfun_opencode_adapter::load_opencode_workspace_adapter;
use bitfun_plugin_runtime_host::PluginRuntimeHost;
use bitfun_runtime_ports::{
    PluginCapabilityRef, PluginDataClassification, PluginDispatchEnvelope, PluginOwnerKind,
    PluginOwnerRef, PluginPayloadRedaction, PluginPayloadRef, PluginRuntimeAvailability,
    PluginRuntimeClient, PluginRuntimeEpochs, PluginRuntimeReadRequest,
    PluginRuntimeUnavailableReason, PluginSourceKind, PluginStatusKind, PluginTrustLevel,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

const CONFIG: &str = include_str!("fixtures/opencode-example/opencode.json");
const LOCAL_PLUGIN_SOURCE: &str =
    include_str!("fixtures/opencode-example/.opencode/plugins/workspace-tools.ts");

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("bitfun-opencode-{name}-{}-{nanos}", process::id()));
        fs::create_dir_all(&root).expect("create temp project");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write_opencode_fixture(&self) {
        fs::write(self.root.join("opencode.json"), CONFIG).expect("write opencode config");
        let plugin_dir = self.root.join(".opencode").join("plugins");
        fs::create_dir_all(&plugin_dir).expect("create opencode plugin directory");
        fs::write(plugin_dir.join("workspace-tools.ts"), LOCAL_PLUGIN_SOURCE)
            .expect("write local opencode plugin");
    }

    fn write_opencode_config(&self) {
        fs::write(self.root.join("opencode.json"), CONFIG).expect("write opencode config");
        fs::create_dir_all(self.root.join(".opencode").join("plugins"))
            .expect("create opencode plugin directory");
    }

    fn write_plugin(&self, name: &str, source: &str) {
        fs::write(
            self.root.join(".opencode").join("plugins").join(name),
            source,
        )
        .expect("write opencode plugin");
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn discovers_opencode_sources_through_plugin_runtime_host() {
    let project = TempProject::new("read-model");
    project.write_opencode_fixture();

    let adapter = load_opencode_workspace_adapter(project.path(), 1_720_000_001)
        .expect("load OpenCode-compatible workspace adapter");
    let host = PluginRuntimeHost::new(adapter);

    let response = host
        .read_plugins(read_request(Vec::new()))
        .await
        .expect("read plugins through host");

    let source_ids = response
        .sources
        .iter()
        .map(|source| source.plugin_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        source_ids,
        [
            "opencode.local.workspace_tools",
            "opencode.npm.opencode_wakatime",
            "opencode.npm.my_org_custom_plugin"
        ]
    );
    assert!(response.sources.iter().all(|source| {
        source.source_kind == PluginSourceKind::OpenCodeCompatible
            && source.trust_level == PluginTrustLevel::Unknown
            && source.content_hash.starts_with("sha256:")
    }));

    let local_status = response
        .plugin_statuses
        .iter()
        .find(|status| status.source.plugin_id == "opencode.local.workspace_tools")
        .expect("local plugin status");
    assert_eq!(local_status.status, PluginStatusKind::TrustRequired);
    assert_eq!(
        local_status.availability,
        PluginRuntimeAvailability::ProjectionOnly {
            reason: PluginRuntimeUnavailableReason::DisabledByPolicy
        }
    );

    assert!(response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.trust_required"));
    assert!(response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.npm_plugin_projection_only"));
    assert!(response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.hook_projection_only"));
}

#[tokio::test]
async fn dispatch_for_discovered_opencode_source_remains_projection_only() {
    let project = TempProject::new("dispatch-model");
    project.write_opencode_fixture();

    let adapter = load_opencode_workspace_adapter(project.path(), 1_720_000_001)
        .expect("load OpenCode-compatible workspace adapter");
    let host = PluginRuntimeHost::new(adapter);
    let read_response = host
        .read_plugins(read_request(vec![
            "opencode.local.workspace_tools".to_string()
        ]))
        .await
        .expect("read local plugin");
    let source = read_response
        .sources
        .into_iter()
        .find(|source| source.plugin_id == "opencode.local.workspace_tools")
        .expect("local plugin source");

    let response = host
        .dispatch(dispatch_envelope(source))
        .await
        .expect("dispatch projection-only source");

    assert_eq!(response.adapter_id, "opencode-compatible");
    assert_eq!(
        response.plugin_id.as_deref(),
        Some("opencode.local.workspace_tools")
    );
    assert!(response.effects.is_empty());
    assert_eq!(
        response.plugin_statuses[0].status,
        PluginStatusKind::TrustRequired
    );
    assert!(response
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "opencode.trust_required"));
}

#[tokio::test]
async fn invalid_local_plugin_reports_diagnostic_without_hiding_valid_sources() {
    let project = TempProject::new("invalid-local");
    project.write_opencode_fixture();
    project.write_plugin(
        "broken-plugin.ts",
        "export const BrokenPlugin = async () => ({ name: 'broken' })",
    );

    let adapter = load_opencode_workspace_adapter(project.path(), 1_720_000_001)
        .expect("load OpenCode-compatible workspace adapter");
    let host = PluginRuntimeHost::new(adapter);
    let response = host
        .read_plugins(read_request(Vec::new()))
        .await
        .expect("read plugins through host");

    let source_ids = response
        .sources
        .iter()
        .map(|source| source.plugin_id.as_str())
        .collect::<Vec<_>>();
    assert!(source_ids.contains(&"opencode.local.workspace_tools"));
    assert!(source_ids.contains(&"opencode.npm.opencode_wakatime"));
    assert!(source_ids.contains(&"opencode.local.broken_plugin"));

    let invalid_status = response
        .plugin_statuses
        .iter()
        .find(|status| status.source.plugin_id == "opencode.local.broken_plugin")
        .expect("invalid plugin status");
    assert_eq!(invalid_status.status, PluginStatusKind::InvalidConfig);
    assert!(invalid_status
        .config_validation
        .as_ref()
        .expect("config validation")
        .issues
        .iter()
        .any(|issue| issue.code == "opencode.local_plugin_invalid"));
    assert!(response.diagnostics.iter().any(|diagnostic| {
        diagnostic.source.plugin_id == "opencode.local.broken_plugin"
            && diagnostic.code == "opencode.local_plugin_invalid"
    }));
}

#[tokio::test]
async fn event_only_local_plugin_projects_unsupported_hook_diagnostic() {
    let project = TempProject::new("event-only");
    project.write_opencode_config();
    project.write_plugin(
        "event-plugin.ts",
        r#"
export const EventPlugin = async () => ({
  event: async ({ event }) => {
    if (event.type === "session.idle") {
      console.log(event.sessionID)
    }
  },
})
"#,
    );

    let adapter = load_opencode_workspace_adapter(project.path(), 1_720_000_001)
        .expect("load OpenCode-compatible workspace adapter");
    let host = PluginRuntimeHost::new(adapter);
    let response = host
        .read_plugins(read_request(Vec::new()))
        .await
        .expect("read plugins through host");

    let status = response
        .plugin_statuses
        .iter()
        .find(|status| status.source.plugin_id == "opencode.local.event_plugin")
        .expect("event plugin status");
    assert_eq!(status.status, PluginStatusKind::TrustRequired);
    assert!(response.diagnostics.iter().any(|diagnostic| {
        diagnostic.source.plugin_id == "opencode.local.event_plugin"
            && diagnostic.code == "opencode.hook_projection_only"
            && diagnostic.message.contains("event")
    }));
}

#[tokio::test]
async fn oversized_local_plugin_reports_diagnostic_without_reading_source() {
    let project = TempProject::new("large-local");
    project.write_opencode_config();
    let oversized_source = "x".repeat(1_048_577);
    project.write_plugin("large-plugin.ts", &oversized_source);

    let adapter = load_opencode_workspace_adapter(project.path(), 1_720_000_001)
        .expect("load OpenCode-compatible workspace adapter");
    let host = PluginRuntimeHost::new(adapter);
    let response = host
        .read_plugins(read_request(Vec::new()))
        .await
        .expect("read plugins through host");

    let status = response
        .plugin_statuses
        .iter()
        .find(|status| status.source.plugin_id == "opencode.local.large_plugin")
        .expect("large plugin status");
    assert_eq!(status.status, PluginStatusKind::InvalidConfig);
    assert!(response.diagnostics.iter().any(|diagnostic| {
        diagnostic.source.plugin_id == "opencode.local.large_plugin"
            && diagnostic.code == "opencode.local_plugin_too_large"
    }));
    assert!(response
        .sources
        .iter()
        .any(|source| source.plugin_id == "opencode.npm.opencode_wakatime"));
}

fn read_request(plugin_ids: Vec<String>) -> PluginRuntimeReadRequest {
    PluginRuntimeReadRequest {
        request_id: "read-1".to_string(),
        project_domain_id: "project-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        plugin_ids,
        include_config_validation: true,
        epochs: epochs(),
    }
}

fn dispatch_envelope(source: bitfun_runtime_ports::PluginSourceRef) -> PluginDispatchEnvelope {
    PluginDispatchEnvelope {
        envelope_version: 1,
        event_id: "event-tool".to_string(),
        event_type: "agent.turn.completed".to_string(),
        event_version: "2026-07-07".to_string(),
        project_domain_id: "project-1".to_string(),
        workspace_id: "workspace-1".to_string(),
        extension_point_id: "tool".to_string(),
        source,
        declared_capability: PluginCapabilityRef {
            capability_id: "opencode.custom_tool".to_string(),
            owner: PluginOwnerRef {
                kind: PluginOwnerKind::ExtensionContract,
                id: "opencode.custom-tools".to_string(),
            },
        },
        correlation_id: "corr-1".to_string(),
        causation_id: None,
        idempotency_key: "event-tool:tool".to_string(),
        deadline_ms: 30_000,
        epochs: epochs(),
        payload_ref: Some(PluginPayloadRef {
            payload_id: "payload-1".to_string(),
            schema_version: "agent.turn.completed.v1".to_string(),
            data_classification: PluginDataClassification::Workspace,
            redaction: PluginPayloadRedaction::Partial,
            uri: Some("bitfun://payloads/payload-1".to_string()),
        }),
    }
}

fn epochs() -> PluginRuntimeEpochs {
    PluginRuntimeEpochs {
        project_epoch: 7,
        trust_epoch: 3,
        policy_epoch: 5,
        tool_registry_epoch: Some(11),
    }
}
