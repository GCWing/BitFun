//! OpenCode-compatible source fixture projection.
//!
//! The fixture covers real OpenCode input shapes: `opencode.json` npm plugin
//! entries and project-local `.opencode/plugins/*.ts` source files. It does not
//! execute JavaScript or implement a Plugin Runtime Host.

use bitfun_runtime_ports::{
    PermissionPromptDenyState, PermissionPromptDescriptor, PermissionPromptEffectKind,
    PluginArtifactRef, PluginAuditRef, PluginCapabilityRef, PluginConfigValidationState,
    PluginConfigValidationStatus, PluginDataClassification, PluginDiagnostic,
    PluginDiagnosticDetail, PluginDiagnosticSeverity, PluginDispatchEnvelope,
    PluginEffectCandidate, PluginEffectCandidatePayload, PluginManifestRef, PluginOwnerKind,
    PluginOwnerRef, PluginPermissionGate, PluginResponseEnvelope, PluginRiskLevel,
    PluginRollbackMode, PluginRollbackPolicy, PluginRuntimeAvailability, PluginRuntimeReadRequest,
    PluginRuntimeReadResponse, PluginRuntimeUnavailableReason, PluginSourceKind, PluginSourceRef,
    PluginStatusKind, PluginStatusSnapshot, PluginTargetRef, PluginTrustLevel, PortError,
    PortErrorKind, PortResult,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const OPENCODE_ADAPTER_ID: &str = "opencode-compatible";
const OPENCODE_CONFIG_SCHEMA: &str = "https://opencode.ai/config.json";
const OPENCODE_LOCAL_PLUGIN_SCHEMA_VERSION: &str = "opencode.plugin.module.ts";
const PLUGIN_EFFECT_SCHEMA_VERSION: &str = "plugin.effect.v1";
const CUSTOM_TOOL_CONTRACT_ID: &str = "opencode.custom-tool.v1";
const CUSTOM_TOOL_EXTENSION_POINT: &str = "tool";

const UNSUPPORTED_HOOK_EVENTS: &[&str] = &[
    "command.executed",
    "permission.asked",
    "permission.replied",
    "session.compacted",
    "shell.env",
    "tool.execute.after",
    "tool.execute.before",
    "tui.toast.show",
];

#[derive(Debug, thiserror::Error)]
enum OpenCodeAdapterError {
    #[error("invalid OpenCode config JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid OpenCode config field {field}: {message}")]
    InvalidConfig {
        field: &'static str,
        message: String,
    },
    #[error("invalid OpenCode plugin source field {field}: {message}")]
    InvalidPluginSource {
        field: &'static str,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenCodeAdapterSource {
    config_uri: String,
    local_plugin_path: String,
    trust_level: PluginTrustLevel,
    observed_at_ms: u64,
}

impl OpenCodeAdapterSource {
    fn project_local(
        config_uri: impl Into<String>,
        local_plugin_path: impl Into<String>,
        trust_level: PluginTrustLevel,
        observed_at_ms: u64,
    ) -> Self {
        Self {
            config_uri: config_uri.into(),
            local_plugin_path: local_plugin_path.into(),
            trust_level,
            observed_at_ms,
        }
    }
}

#[derive(Debug, Clone)]
struct OpenCodeSourceProjection {
    config: OpenCodeConfig,
    local_plugin: OpenCodeLocalPlugin,
    source: PluginSourceRef,
    observed_at_ms: u64,
}

impl OpenCodeSourceProjection {
    fn from_opencode_sources(
        config_json: &str,
        local_plugin_source: &str,
        source: OpenCodeAdapterSource,
    ) -> Result<Self, OpenCodeAdapterError> {
        let config_doc: OpenCodeConfigDoc = serde_json::from_str(config_json)?;
        let mut config = OpenCodeConfig::try_from_doc(config_doc)?;
        config.config_uri = source.config_uri.clone();
        let local_plugin =
            OpenCodeLocalPlugin::from_source(&source.local_plugin_path, local_plugin_source)?;
        let source_ref = PluginSourceRef {
            plugin_id: local_plugin.plugin_id.clone(),
            source_kind: PluginSourceKind::OpenCodeCompatible,
            source: format!("file://{}", source.local_plugin_path),
            version: None,
            content_hash: sha256_content_hash(local_plugin_source),
            trust_level: source.trust_level,
            manifest: Some(PluginManifestRef {
                manifest_id: format!(
                    "{}:{}",
                    local_plugin.plugin_id, OPENCODE_LOCAL_PLUGIN_SCHEMA_VERSION
                ),
                schema_version: OPENCODE_LOCAL_PLUGIN_SCHEMA_VERSION.to_string(),
                path: Some(source.local_plugin_path.clone()),
            }),
        };

        Ok(Self {
            config,
            local_plugin,
            source: source_ref,
            observed_at_ms: source.observed_at_ms,
        })
    }

    fn source_ref(&self) -> &PluginSourceRef {
        &self.source
    }

    fn project_read_model(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        if !request.plugin_ids.is_empty()
            && !request
                .plugin_ids
                .iter()
                .any(|plugin_id| plugin_id == &self.source.plugin_id)
        {
            return Ok(PluginRuntimeReadResponse {
                request_id: request.request_id,
                project_domain_id: request.project_domain_id,
                sources: Vec::new(),
                plugin_statuses: Vec::new(),
                diagnostics: Vec::new(),
                observed_epochs: request.epochs,
            });
        }

        let diagnostics = self.read_diagnostics();
        let diagnostic_ids = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.diagnostic_id.clone())
            .collect();
        let (availability, status) = self.trust_status();

        Ok(PluginRuntimeReadResponse {
            request_id: request.request_id,
            project_domain_id: request.project_domain_id,
            sources: vec![self.source.clone()],
            plugin_statuses: vec![self.status_snapshot(
                availability,
                request.include_config_validation,
                status,
                diagnostic_ids,
            )],
            diagnostics,
            observed_epochs: request.epochs,
        })
    }

    fn project_dispatch_response(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        if envelope.source.plugin_id != self.source.plugin_id {
            return Err(PortError::new(
                PortErrorKind::NotFound,
                format!(
                    "OpenCode source {} is not loaded by this fixture",
                    envelope.source.plugin_id
                ),
            ));
        }

        if self.source.trust_level != PluginTrustLevel::Trusted {
            return Ok(self.response(
                &envelope,
                Vec::new(),
                vec![self.trust_diagnostic()],
                self.trust_status().1,
            ));
        }

        let effects = if envelope.extension_point_id == CUSTOM_TOOL_EXTENSION_POINT {
            self.local_plugin
                .custom_tools
                .iter()
                .map(|tool| self.provider_candidate_effect(&envelope, tool))
                .collect()
        } else {
            vec![self.unsupported_extension_effect(&envelope)]
        };

        Ok(self.response(
            &envelope,
            effects,
            Vec::new(),
            PluginStatusKind::ProjectionOnly,
        ))
    }

    fn read_diagnostics(&self) -> Vec<PluginDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.source.trust_level != PluginTrustLevel::Trusted {
            diagnostics.push(self.trust_diagnostic());
        }
        diagnostics.extend(
            self.config
                .npm_plugins
                .iter()
                .map(|package| self.npm_package_diagnostic(package)),
        );
        diagnostics.extend(
            self.local_plugin
                .unsupported_hooks
                .iter()
                .map(|hook| self.unsupported_hook_diagnostic(hook)),
        );
        diagnostics
    }

    fn trust_status(&self) -> (PluginRuntimeAvailability, PluginStatusKind) {
        match self.source.trust_level {
            PluginTrustLevel::Trusted => (
                PluginRuntimeAvailability::projection_only(
                    PluginRuntimeUnavailableReason::HostUnavailable,
                ),
                PluginStatusKind::ProjectionOnly,
            ),
            PluginTrustLevel::Unknown => (
                PluginRuntimeAvailability::projection_only(
                    PluginRuntimeUnavailableReason::DisabledByPolicy,
                ),
                PluginStatusKind::TrustRequired,
            ),
            PluginTrustLevel::Denied | PluginTrustLevel::Revoked => (
                PluginRuntimeAvailability::disabled(
                    PluginRuntimeUnavailableReason::DisabledByPolicy,
                ),
                PluginStatusKind::Disabled,
            ),
            _ => (
                PluginRuntimeAvailability::projection_only(
                    PluginRuntimeUnavailableReason::DisabledByPolicy,
                ),
                PluginStatusKind::TrustRequired,
            ),
        }
    }

    fn status_snapshot(
        &self,
        availability: PluginRuntimeAvailability,
        include_config_validation: bool,
        status: PluginStatusKind,
        diagnostic_ids: Vec<String>,
    ) -> PluginStatusSnapshot {
        PluginStatusSnapshot {
            source: self.source.clone(),
            status,
            availability,
            config_validation: include_config_validation.then(|| PluginConfigValidationState {
                status: PluginConfigValidationStatus::Valid,
                issues: Vec::new(),
            }),
            quarantine: None,
            diagnostic_ids,
            updated_at_ms: self.observed_at_ms,
        }
    }

    fn response(
        &self,
        envelope: &PluginDispatchEnvelope,
        effects: Vec<PluginEffectCandidate>,
        diagnostics: Vec<PluginDiagnostic>,
        status: PluginStatusKind,
    ) -> PluginResponseEnvelope {
        let diagnostic_ids = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.diagnostic_id.clone())
            .collect();

        PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            plugin_id: Some(self.source.plugin_id.clone()),
            completed_at_ms: self.observed_at_ms,
            effects,
            diagnostics,
            quarantine: None,
            plugin_statuses: vec![self.status_snapshot(
                self.trust_status().0,
                false,
                status,
                diagnostic_ids,
            )],
            observed_epochs: envelope.epochs,
        }
    }

    fn provider_candidate_effect(
        &self,
        envelope: &PluginDispatchEnvelope,
        tool: &OpenCodeCustomTool,
    ) -> PluginEffectCandidate {
        let audit = audit_ref(envelope);
        let target = tool.target_ref(&self.source.plugin_id);
        let permission = PluginPermissionGate::PermissionRequired {
            prompt: self.permission_prompt(envelope, tool, target.clone(), audit.clone()),
        };

        PluginEffectCandidate {
            effect_id: format!(
                "{}:{}:{}",
                envelope.event_id, self.source.plugin_id, tool.id
            ),
            schema_version: PLUGIN_EFFECT_SCHEMA_VERSION.to_string(),
            declared_capability: tool.capability_ref(),
            target_ref: target,
            data_classification: PluginDataClassification::Workspace,
            risk_level: PluginRiskLevel::Medium,
            permission,
            source_ref: self.source.clone(),
            payload: PluginEffectCandidatePayload::ProviderCandidate {
                provider_id: tool.provider_id(&self.source.plugin_id),
                tool_contract_id: tool.tool_contract_id.clone(),
            },
        }
    }

    fn permission_prompt(
        &self,
        envelope: &PluginDispatchEnvelope,
        tool: &OpenCodeCustomTool,
        target: PluginTargetRef,
        audit: PluginAuditRef,
    ) -> PermissionPromptDescriptor {
        PermissionPromptDescriptor {
            descriptor_version: 1,
            prompt_id: format!(
                "prompt:{}:{}:{}",
                self.source.plugin_id, tool.id, envelope.event_id
            ),
            plugin: self.source.clone(),
            requested_capability: tool.capability_ref(),
            requested_effect: PermissionPromptEffectKind::ProviderCandidate,
            target,
            risk_level: PluginRiskLevel::Medium,
            owner: PluginOwnerRef {
                kind: PluginOwnerKind::ProductFeature,
                id: "tools".to_string(),
            },
            rollback: PluginRollbackPolicy {
                mode: PluginRollbackMode::DisablePlugin,
                reason_ref: Some(format!("audit:{}", envelope.event_id)),
            },
            deny_state: PermissionPromptDenyState::CandidateDiscarded,
            audit,
        }
    }

    fn unsupported_extension_effect(
        &self,
        envelope: &PluginDispatchEnvelope,
    ) -> PluginEffectCandidate {
        PluginEffectCandidate {
            effect_id: format!(
                "{}:{}:unsupported",
                envelope.event_id, self.source.plugin_id
            ),
            schema_version: PLUGIN_EFFECT_SCHEMA_VERSION.to_string(),
            declared_capability: envelope.declared_capability.clone(),
            target_ref: PluginTargetRef {
                target_kind: "opencode_hook".to_string(),
                target_id: envelope.extension_point_id.clone(),
                display_name: envelope.extension_point_id.clone(),
                artifact: None,
            },
            data_classification: PluginDataClassification::Workspace,
            risk_level: PluginRiskLevel::Low,
            permission: PluginPermissionGate::PolicyDenied {
                deny_state: PermissionPromptDenyState::PolicyDenied,
                audit: audit_ref(envelope),
            },
            source_ref: self.source.clone(),
            payload: PluginEffectCandidatePayload::Unsupported {
                capability: envelope.extension_point_id.clone(),
                reason: "OpenCode hook execution requires a reviewed Plugin Runtime Host"
                    .to_string(),
            },
        }
    }

    fn npm_package_diagnostic(&self, package: &str) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:npm:{package}", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Info,
            source: self.source.clone(),
            code: "opencode.npm_plugin_projection_only".to_string(),
            message: format!(
                "OpenCode npm plugin is present in opencode.json but is not installed or executed by this fixture: {package}"
            ),
            detail: PluginDiagnosticDetail::Manifest {
                manifest: PluginManifestRef {
                    manifest_id: "opencode.config".to_string(),
                    schema_version: OPENCODE_CONFIG_SCHEMA.to_string(),
                    path: Some(self.config.config_uri.clone()),
                },
            },
            audit: PluginAuditRef {
                correlation_id: format!("config:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: false,
            recovery_actions: Vec::new(),
        }
    }

    fn unsupported_hook_diagnostic(&self, hook: &str) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:hook:{hook}", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Warning,
            source: self.source.clone(),
            code: "opencode.hook_projection_only".to_string(),
            message: format!(
                "OpenCode hook is discovered but cannot run before Plugin Runtime Host integration: {hook}"
            ),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: PluginAuditRef {
                correlation_id: format!("plugin-source:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: false,
            recovery_actions: Vec::new(),
        }
    }

    fn trust_diagnostic(&self) -> PluginDiagnostic {
        let retryable = self.source.trust_level == PluginTrustLevel::Unknown;
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:trust", self.source.plugin_id),
            severity: if retryable {
                PluginDiagnosticSeverity::Warning
            } else {
                PluginDiagnosticSeverity::Error
            },
            source: self.source.clone(),
            code: "opencode.trust_required".to_string(),
            message: "OpenCode plugin source is not trusted for projection".to_string(),
            detail: PluginDiagnosticDetail::Trust {
                trust_level: self.source.trust_level,
            },
            audit: PluginAuditRef {
                correlation_id: format!("trust:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable,
            recovery_actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenCodeConfig {
    config_uri: String,
    npm_plugins: Vec<String>,
}

impl OpenCodeConfig {
    fn try_from_doc(doc: OpenCodeConfigDoc) -> Result<Self, OpenCodeAdapterError> {
        if doc.schema.as_deref() != Some(OPENCODE_CONFIG_SCHEMA) {
            return Err(OpenCodeAdapterError::InvalidConfig {
                field: "$schema",
                message: format!("expected {OPENCODE_CONFIG_SCHEMA}"),
            });
        }

        let mut npm_plugins = Vec::new();
        for package in doc.plugin {
            let package = package.trim().to_string();
            if package.is_empty() {
                return Err(OpenCodeAdapterError::InvalidConfig {
                    field: "plugin",
                    message: "package names must not be empty".to_string(),
                });
            }
            npm_plugins.push(package);
        }

        Ok(Self {
            config_uri: "opencode.json".to_string(),
            npm_plugins,
        })
    }
}

#[derive(Debug, Deserialize)]
struct OpenCodeConfigDoc {
    #[serde(rename = "$schema")]
    schema: Option<String>,
    #[serde(default)]
    plugin: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenCodeLocalPlugin {
    plugin_id: String,
    export_name: String,
    custom_tools: Vec<OpenCodeCustomTool>,
    unsupported_hooks: Vec<String>,
}

impl OpenCodeLocalPlugin {
    fn from_source(path: &str, source: &str) -> Result<Self, OpenCodeAdapterError> {
        let export_name =
            exported_plugin_name(source).ok_or(OpenCodeAdapterError::InvalidPluginSource {
                field: "plugin.export",
                message: "expected an exported OpenCode plugin function".to_string(),
            })?;
        let custom_tools = discover_custom_tools(source);
        let unsupported_hooks = discover_unsupported_hooks(source);
        if custom_tools.is_empty() && unsupported_hooks.is_empty() {
            return Err(OpenCodeAdapterError::InvalidPluginSource {
                field: "plugin.contributions",
                message: "expected a custom tool or hook contribution".to_string(),
            });
        }

        Ok(Self {
            plugin_id: format!("opencode.local.{}", path_stem(path)),
            export_name,
            custom_tools,
            unsupported_hooks,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenCodeCustomTool {
    id: String,
    tool_contract_id: String,
}

impl OpenCodeCustomTool {
    fn provider_id(&self, plugin_id: &str) -> String {
        format!("{plugin_id}.{}", self.id)
    }

    fn target_ref(&self, plugin_id: &str) -> PluginTargetRef {
        let target_id = self.provider_id(plugin_id);
        PluginTargetRef {
            target_kind: "opencode_custom_tool".to_string(),
            target_id: target_id.clone(),
            display_name: self.id.clone(),
            artifact: Some(PluginArtifactRef {
                artifact_id: format!("{plugin_id}:{}:source", self.id),
                artifact_kind: "opencode_plugin_source".to_string(),
                display_name: self.id.clone(),
                uri: Some(format!("bitfun://plugins/{plugin_id}/tools/{target_id}")),
            }),
        }
    }

    fn capability_ref(&self) -> PluginCapabilityRef {
        PluginCapabilityRef {
            capability_id: "opencode.custom_tool".to_string(),
            owner: PluginOwnerRef {
                kind: PluginOwnerKind::ExtensionContract,
                id: "opencode.custom-tools".to_string(),
            },
        }
    }
}

fn exported_plugin_name(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("export const ")?;
        let name = rest
            .split(|ch: char| ch == ':' || ch == '=' || ch.is_whitespace())
            .next()?;
        is_identifier(name).then(|| name.to_string())
    })
}

fn discover_custom_tools(source: &str) -> Vec<OpenCodeCustomTool> {
    source
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.trim().split_once(':')?;
            rest.trim_start()
                .starts_with("tool({")
                .then(|| name.trim())
                .filter(|candidate| is_identifier(candidate))
                .map(|id| OpenCodeCustomTool {
                    id: id.to_string(),
                    tool_contract_id: CUSTOM_TOOL_CONTRACT_ID.to_string(),
                })
        })
        .collect()
}

fn discover_unsupported_hooks(source: &str) -> Vec<String> {
    UNSUPPORTED_HOOK_EVENTS
        .iter()
        .filter(|event| {
            source.contains(&format!("\"{event}\"")) || source.contains(&format!("'{event}'"))
        })
        .map(|event| (*event).to_string())
        .collect()
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn path_stem(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .and_then(|file| file.split('.').next())
        .unwrap_or("plugin")
        .replace('-', "_")
}

fn sha256_content_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn audit_ref(envelope: &PluginDispatchEnvelope) -> PluginAuditRef {
    PluginAuditRef {
        correlation_id: envelope.correlation_id.clone(),
        event_id: Some(envelope.event_id.clone()),
    }
}

#[cfg(test)]
mod opencode_fixture_contracts {
    use super::*;
    use bitfun_runtime_ports::{
        PermissionPromptDenyState, PermissionPromptEffectKind, PluginPayloadRedaction,
        PluginPayloadRef, PluginRuntimeEpochs,
    };

    const CONFIG: &str = include_str!("../tests/fixtures/opencode-example/opencode.json");
    const LOCAL_PLUGIN_PATH: &str = ".opencode/plugins/workspace-tools.ts";
    const LOCAL_PLUGIN_SOURCE: &str =
        include_str!("../tests/fixtures/opencode-example/.opencode/plugins/workspace-tools.ts");

    fn adapter(trust_level: PluginTrustLevel) -> OpenCodeSourceProjection {
        OpenCodeSourceProjection::from_opencode_sources(
            CONFIG,
            LOCAL_PLUGIN_SOURCE,
            OpenCodeAdapterSource::project_local(
                "file:///project/opencode.json",
                LOCAL_PLUGIN_PATH,
                trust_level,
                1_720_000_001,
            ),
        )
        .expect("OpenCode fixture sources should parse")
    }

    fn epochs() -> PluginRuntimeEpochs {
        PluginRuntimeEpochs {
            project_epoch: 7,
            trust_epoch: 3,
            policy_epoch: 5,
            tool_registry_epoch: Some(11),
        }
    }

    fn capability_ref() -> PluginCapabilityRef {
        PluginCapabilityRef {
            capability_id: "opencode.custom_tool".to_string(),
            owner: PluginOwnerRef {
                kind: PluginOwnerKind::ExtensionContract,
                id: "opencode.custom-tools".to_string(),
            },
        }
    }

    fn envelope(
        adapter: &OpenCodeSourceProjection,
        extension_point_id: &str,
    ) -> PluginDispatchEnvelope {
        PluginDispatchEnvelope {
            envelope_version: 1,
            event_id: format!("event-{extension_point_id}"),
            event_type: "agent.turn.completed".to_string(),
            event_version: "2026-07-07".to_string(),
            project_domain_id: "project-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            extension_point_id: extension_point_id.to_string(),
            source: adapter.source_ref().clone(),
            declared_capability: capability_ref(),
            correlation_id: "corr-1".to_string(),
            causation_id: None,
            idempotency_key: format!("event-{extension_point_id}:{extension_point_id}"),
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

    #[test]
    fn projects_real_opencode_config_and_local_plugin_source() {
        let adapter = adapter(PluginTrustLevel::Trusted);

        assert_eq!(
            adapter.config.npm_plugins,
            ["opencode-wakatime", "@my-org/custom-plugin"]
        );
        assert_eq!(adapter.local_plugin.export_name, "WorkspaceToolsPlugin");
        assert_eq!(adapter.local_plugin.custom_tools[0].id, "workspaceSummary");
        assert_eq!(
            adapter.local_plugin.unsupported_hooks,
            ["tool.execute.before"]
        );

        let response = adapter
            .project_read_model(PluginRuntimeReadRequest {
                request_id: "read-1".to_string(),
                project_domain_id: "project-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                plugin_ids: vec!["opencode.local.workspace_tools".to_string()],
                include_config_validation: true,
                epochs: epochs(),
            })
            .expect("project read model");

        assert_eq!(response.sources.len(), 1);
        assert_eq!(
            response.sources[0].plugin_id,
            "opencode.local.workspace_tools"
        );
        assert_eq!(
            response.sources[0].source_kind,
            PluginSourceKind::OpenCodeCompatible
        );
        assert!(response.sources[0].content_hash.starts_with("sha256:"));
        assert_eq!(
            response.plugin_statuses[0].status,
            PluginStatusKind::ProjectionOnly
        );
        assert_eq!(
            response.plugin_statuses[0].availability,
            PluginRuntimeAvailability::ProjectionOnly {
                reason: PluginRuntimeUnavailableReason::HostUnavailable
            }
        );
        assert!(response.plugin_statuses[0]
            .config_validation
            .as_ref()
            .expect("config validation")
            .issues
            .is_empty());
        assert_eq!(response.diagnostics.len(), 3);
        assert!(response
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "opencode.npm_plugin_projection_only"));
        assert!(response
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "opencode.hook_projection_only"));
    }

    #[test]
    fn projects_custom_tool_candidate_with_permission_prompt() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let response = adapter
            .project_dispatch_response(envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT))
            .expect("project dispatch response");

        assert_eq!(response.adapter_id, OPENCODE_ADAPTER_ID);
        assert_eq!(
            response.plugin_id.as_deref(),
            Some("opencode.local.workspace_tools")
        );
        assert_eq!(response.effects.len(), 1);
        assert!(response.diagnostics.is_empty());

        let effect = &response.effects[0];
        assert_eq!(
            effect.declared_capability.capability_id,
            "opencode.custom_tool"
        );
        assert_eq!(
            effect.target_ref.target_id,
            "opencode.local.workspace_tools.workspaceSummary"
        );
        assert_eq!(
            effect.source_ref.plugin_id,
            "opencode.local.workspace_tools"
        );
        assert!(effect.source_ref.content_hash.starts_with("sha256:"));
        assert_eq!(
            effect.data_classification,
            PluginDataClassification::Workspace
        );
        assert_eq!(response.observed_epochs.tool_registry_epoch, Some(11));

        match &effect.payload {
            PluginEffectCandidatePayload::ProviderCandidate {
                provider_id,
                tool_contract_id,
            } => {
                assert_eq!(
                    provider_id,
                    "opencode.local.workspace_tools.workspaceSummary"
                );
                assert_eq!(tool_contract_id, "opencode.custom-tool.v1");
            }
            other => panic!("expected provider candidate, got {other:?}"),
        }

        match &effect.permission {
            PluginPermissionGate::PermissionRequired { prompt } => {
                assert_eq!(prompt.plugin.plugin_id, "opencode.local.workspace_tools");
                assert_eq!(
                    prompt.requested_effect,
                    PermissionPromptEffectKind::ProviderCandidate
                );
                assert_eq!(
                    prompt.target.target_id,
                    "opencode.local.workspace_tools.workspaceSummary"
                );
                assert_eq!(prompt.owner.kind, PluginOwnerKind::ProductFeature);
                assert_eq!(
                    prompt.deny_state,
                    PermissionPromptDenyState::CandidateDiscarded
                );
                assert_eq!(prompt.audit.event_id.as_deref(), Some("event-tool"));
            }
            other => panic!("expected permission prompt, got {other:?}"),
        }
    }

    #[test]
    fn untrusted_source_stays_readable_but_projects_no_effects() {
        let adapter = adapter(PluginTrustLevel::Unknown);

        let read = adapter
            .project_read_model(PluginRuntimeReadRequest {
                request_id: "read-trust".to_string(),
                project_domain_id: "project-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                plugin_ids: vec!["opencode.local.workspace_tools".to_string()],
                include_config_validation: true,
                epochs: epochs(),
            })
            .expect("project read model");

        assert_eq!(
            read.plugin_statuses[0].status,
            PluginStatusKind::TrustRequired
        );
        assert!(read
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "opencode.trust_required"));

        let response = adapter
            .project_dispatch_response(envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT))
            .expect("project dispatch response");

        assert!(response.effects.is_empty());
        assert_eq!(
            response.plugin_statuses[0].status,
            PluginStatusKind::TrustRequired
        );
        assert_eq!(response.diagnostics[0].code, "opencode.trust_required");
    }

    #[test]
    fn unsupported_opencode_hook_projects_typed_unsupported_candidate() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let response = adapter
            .project_dispatch_response(envelope(&adapter, "tool.execute.before"))
            .expect("project dispatch response");

        assert_eq!(response.effects.len(), 1);
        match &response.effects[0].payload {
            PluginEffectCandidatePayload::Unsupported { capability, reason } => {
                assert_eq!(capability, "tool.execute.before");
                assert!(reason.contains("Plugin Runtime Host"));
            }
            other => panic!("expected unsupported effect, got {other:?}"),
        }
        assert!(matches!(
            response.effects[0].permission,
            PluginPermissionGate::PolicyDenied { .. }
        ));
    }

    #[test]
    fn invalid_opencode_config_fails_before_projection() {
        let error = OpenCodeSourceProjection::from_opencode_sources(
            r#"{"$schema":"https://example.invalid/config.json","plugin":[]}"#,
            LOCAL_PLUGIN_SOURCE,
            OpenCodeAdapterSource::project_local(
                "file:///project/opencode.json",
                LOCAL_PLUGIN_PATH,
                PluginTrustLevel::Trusted,
                1,
            ),
        )
        .expect_err("schema mismatch should fail");

        assert!(matches!(
            error,
            OpenCodeAdapterError::InvalidConfig {
                field: "$schema",
                ..
            }
        ));
    }

    #[test]
    fn plugin_source_without_module_export_fails_before_projection() {
        let error = OpenCodeSourceProjection::from_opencode_sources(
            CONFIG,
            "const WorkspaceToolsPlugin = async () => ({ tool: {} })",
            OpenCodeAdapterSource::project_local(
                "file:///project/opencode.json",
                LOCAL_PLUGIN_PATH,
                PluginTrustLevel::Trusted,
                1,
            ),
        )
        .expect_err("non-exported plugin source should fail");

        assert!(matches!(
            error,
            OpenCodeAdapterError::InvalidPluginSource {
                field: "plugin.export",
                ..
            }
        ));
    }
}
