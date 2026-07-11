//! OpenCode-compatible source projection.
//!
//! The adapter covers real OpenCode input shapes: `opencode.json` npm plugin
//! entries and project-local `.opencode/plugins/*.ts` source files. It does not
//! execute JavaScript, install packages, or become the runtime host.

use async_trait::async_trait;
use bitfun_plugin_runtime_host::PluginHostAdapter;
use bitfun_runtime_ports::{
    PermissionPromptDenyState, PermissionPromptDescriptor, PermissionPromptEffectKind,
    PluginArtifactRef, PluginCapabilityRef, PluginDataClassification, PluginEffectCandidatePayload,
    PluginOwnerKind, PluginOwnerRef, PluginPermissionGate, PluginRiskLevel, PluginRollbackMode,
    PluginRollbackPolicy, PluginTargetRef,
};
use bitfun_runtime_ports::{
    PluginAuditRef, PluginConfigValidationIssue, PluginConfigValidationState,
    PluginConfigValidationStatus, PluginDiagnostic, PluginDiagnosticDetail,
    PluginDiagnosticSeverity, PluginDispatchEnvelope, PluginEffectCandidate, PluginManifestRef,
    PluginResponseEnvelope, PluginRuntimeAvailability, PluginRuntimeEpochs,
    PluginRuntimeReadRequest, PluginRuntimeReadResponse, PluginRuntimeUnavailableReason,
    PluginSourceKind, PluginSourceRef, PluginStatusKind, PluginStatusSnapshot, PluginTrustLevel,
    PortError, PortErrorKind, PortResult,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

const OPENCODE_ADAPTER_ID: &str = "opencode-compatible";
const OPENCODE_CONFIG_SCHEMA: &str = "https://opencode.ai/config.json";
const OPENCODE_LOCAL_PLUGIN_SCHEMA_VERSION: &str = "opencode.plugin.module.ts";
const PLUGIN_EFFECT_SCHEMA_VERSION: &str = "plugin.effect.v1";
const CUSTOM_TOOL_CONTRACT_ID: &str = "opencode.custom-tool.v1";
const CUSTOM_TOOL_CAPABILITY_ID: &str = "opencode.custom_tool";
const CUSTOM_TOOL_CAPABILITY_OWNER_ID: &str = "opencode.custom-tools";
const CUSTOM_TOOL_EXTENSION_POINT: &str = "tool";
const OPENCODE_WORKSPACE_PLUGIN_DIR: &str = ".opencode/plugins";
const MAX_OPENCODE_PLUGIN_SOURCE_BYTES: u64 = 1_048_576;

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

impl OpenCodeAdapterError {
    fn field(&self) -> &'static str {
        match self {
            Self::Json(_) => "json",
            Self::InvalidConfig { field, .. } | Self::InvalidPluginSource { field, .. } => field,
        }
    }
}

struct OpenCodePluginHostAdapter {
    projections: Vec<OpenCodeProjection>,
    observed_at_ms: u64,
}

impl OpenCodePluginHostAdapter {
    fn from_workspace(
        project_root: impl AsRef<Path>,
        observed_at_ms: u64,
        source_trust_epoch: u64,
        source_trust_refs: &[PluginSourceRef],
    ) -> PortResult<Self> {
        let project_root = project_root.as_ref();
        let config_path = project_root.join("opencode.json");
        let (config_json, config_uri) = read_opencode_config(&config_path)?;
        let mut projections = Vec::new();
        let config = match parse_opencode_config(&config_json, &config_uri) {
            Ok(config) => config,
            Err(error) => {
                projections.push(OpenCodeProjection::Invalid(
                    OpenCodeInvalidProjection::config(
                        &config_uri,
                        &config_json,
                        "opencode.config_invalid",
                        "opencode.json",
                        error.to_string(),
                        observed_at_ms,
                    ),
                ));
                OpenCodeConfig::empty(config_uri.clone())
            }
        };

        for plugin_path in workspace_plugin_paths(project_root, observed_at_ms)? {
            let plugin_path = match plugin_path {
                Ok(plugin_path) => plugin_path,
                Err(projection) => {
                    projections.push(OpenCodeProjection::Invalid(projection));
                    continue;
                }
            };
            let plugin_source = fs::read_to_string(&plugin_path).map_err(|error| {
                OpenCodeProjection::Invalid(OpenCodeInvalidProjection::local_path(
                    &plugin_path,
                    "opencode.local_plugin_unreadable",
                    "source",
                    format!(
                        "failed to read OpenCode plugin source {}: {error}",
                        plugin_path.display()
                    ),
                    observed_at_ms,
                ))
            });
            let plugin_source = match plugin_source {
                Ok(plugin_source) => plugin_source,
                Err(projection) => {
                    projections.push(projection);
                    continue;
                }
            };
            match OpenCodeSourceProjection::from_local_plugin_source(
                &plugin_source,
                OpenCodeAdapterSource::project_local(
                    config_uri.clone(),
                    plugin_path.to_string_lossy().into_owned(),
                    PluginTrustLevel::Unknown,
                    observed_at_ms,
                ),
                config.clone(),
            )
            .map(|projection| {
                projection
                    .with_source_trust_refs(source_trust_epoch, source_trust_refs)
                    .without_config_package_diagnostics()
            }) {
                Ok(projection) => projections.push(OpenCodeProjection::Local(projection)),
                Err(error) => projections.push(OpenCodeProjection::Invalid(
                    OpenCodeInvalidProjection::local_source(
                        &plugin_path,
                        &plugin_source,
                        "opencode.local_plugin_invalid",
                        error.field(),
                        error.to_string(),
                        observed_at_ms,
                    ),
                )),
            }
        }

        projections.extend(config.npm_plugins.iter().map(|package| {
            OpenCodeProjection::Package(OpenCodePackageProjection::new(
                package,
                &config_uri,
                observed_at_ms,
            ))
        }));

        Ok(Self {
            projections,
            observed_at_ms,
        })
    }

    fn projection_for_source(&self, source: &PluginSourceRef) -> Option<&OpenCodeProjection> {
        self.projections
            .iter()
            .find(|projection| source_identity_matches(projection.source_ref(), source))
    }

    fn source_mismatch_response(&self, envelope: PluginDispatchEnvelope) -> PluginResponseEnvelope {
        let diagnostic_id = format!(
            "diag:{}:dispatch:{}:source_mismatch",
            envelope.source.plugin_id, envelope.event_id
        );
        let diagnostic = PluginDiagnostic {
            diagnostic_id: diagnostic_id.clone(),
            severity: PluginDiagnosticSeverity::Warning,
            source: envelope.source.clone(),
            code: "opencode.source_mismatch".to_string(),
            message: "OpenCode dispatch source does not match a loaded source snapshot".to_string(),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: audit_ref(&envelope),
            retryable: false,
        };

        PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: self.observed_at_ms,
            effects: Vec::new(),
            diagnostics: vec![diagnostic],
            quarantine: None,
            plugin_statuses: vec![PluginStatusSnapshot {
                source: envelope.source.clone(),
                status: PluginStatusKind::Unavailable,
                availability: PluginRuntimeAvailability::Unavailable {
                    reason: PluginRuntimeUnavailableReason::HostUnavailable,
                },
                config_validation: None,
                quarantine: None,
                diagnostic_ids: vec![diagnostic_id],
                updated_at_ms: self.observed_at_ms,
            }],
            observed_epochs: envelope.epochs,
        }
    }
}

#[async_trait]
impl PluginHostAdapter for OpenCodePluginHostAdapter {
    fn adapter_id(&self) -> &str {
        OPENCODE_ADAPTER_ID
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        let mut sources = Vec::new();
        let mut plugin_statuses = Vec::new();
        let mut diagnostics = Vec::new();

        for projection in self.projections.iter().filter(|projection| {
            request.plugin_ids.is_empty()
                || request
                    .plugin_ids
                    .iter()
                    .any(|plugin_id| plugin_id == &projection.source_ref().plugin_id)
        }) {
            let projection_diagnostics = projection.read_diagnostics(&request.epochs);
            let diagnostic_ids = projection_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.diagnostic_id.clone())
                .collect();
            sources.push(projection.source_ref_for_epochs(&request.epochs));
            plugin_statuses.push(projection.status_snapshot(
                request.include_config_validation,
                diagnostic_ids,
                &request.epochs,
            ));
            diagnostics.extend(projection_diagnostics);
        }

        Ok(PluginRuntimeReadResponse {
            request_id: request.request_id,
            project_domain_id: request.project_domain_id,
            workspace_id: request.workspace_id,
            sources,
            plugin_statuses,
            diagnostics,
            observed_epochs: request.epochs,
        })
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        match self.projection_for_source(&envelope.source) {
            Some(projection) => projection.project_dispatch_response(envelope),
            None => Ok(self.source_mismatch_response(envelope)),
        }
    }
}

pub fn load_opencode_workspace_adapter(
    project_root: impl AsRef<Path>,
    observed_at_ms: u64,
    source_trust_epoch: u64,
    source_trust_refs: &[PluginSourceRef],
) -> PortResult<Arc<dyn PluginHostAdapter>> {
    Ok(Arc::new(OpenCodePluginHostAdapter::from_workspace(
        project_root,
        observed_at_ms,
        source_trust_epoch,
        source_trust_refs,
    )?))
}

fn source_identity_matches(left: &PluginSourceRef, right: &PluginSourceRef) -> bool {
    left.plugin_id == right.plugin_id
        && left.source_kind == right.source_kind
        && left.source == right.source
        && left.version == right.version
        && left.content_hash == right.content_hash
}

fn plugin_owner_kind_name(kind: PluginOwnerKind) -> &'static str {
    match kind {
        PluginOwnerKind::ProductFeature => "product_feature",
        PluginOwnerKind::ExtensionContract => "extension_contract",
        PluginOwnerKind::AssemblyPolicy => "assembly_policy",
        _ => "unknown",
    }
}

enum OpenCodeProjection {
    Local(OpenCodeSourceProjection),
    Package(OpenCodePackageProjection),
    Invalid(OpenCodeInvalidProjection),
}

impl OpenCodeProjection {
    fn source_ref(&self) -> &PluginSourceRef {
        match self {
            Self::Local(projection) => projection.source_ref(),
            Self::Package(projection) => projection.source_ref(),
            Self::Invalid(projection) => projection.source_ref(),
        }
    }

    fn source_ref_for_epochs(&self, epochs: &PluginRuntimeEpochs) -> PluginSourceRef {
        match self {
            Self::Local(projection) => projection.source_ref_for_epochs(epochs),
            Self::Package(projection) => projection.source_ref().clone(),
            Self::Invalid(projection) => projection.source_ref().clone(),
        }
    }

    fn read_diagnostics(&self, epochs: &PluginRuntimeEpochs) -> Vec<PluginDiagnostic> {
        match self {
            Self::Local(projection) => projection.read_diagnostics(epochs),
            Self::Package(projection) => projection.read_diagnostics(),
            Self::Invalid(projection) => projection.read_diagnostics(),
        }
    }

    fn status_snapshot(
        &self,
        include_config_validation: bool,
        diagnostic_ids: Vec<String>,
        epochs: &PluginRuntimeEpochs,
    ) -> PluginStatusSnapshot {
        match self {
            Self::Local(projection) => {
                let (availability, status) = projection.trust_status_for_epochs(epochs);
                projection.status_snapshot(
                    projection.source_ref_for_epochs(epochs),
                    availability,
                    include_config_validation,
                    status,
                    diagnostic_ids,
                )
            }
            Self::Package(projection) => {
                projection.status_snapshot(include_config_validation, diagnostic_ids)
            }
            Self::Invalid(projection) => {
                projection.status_snapshot(include_config_validation, diagnostic_ids)
            }
        }
    }

    fn project_dispatch_response(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        match self {
            Self::Local(projection) => projection.project_dispatch_response(envelope),
            Self::Package(projection) => projection.project_dispatch_response(envelope),
            Self::Invalid(projection) => projection.project_dispatch_response(envelope),
        }
    }
}

#[derive(Debug, Clone)]
struct OpenCodePackageProjection {
    config_uri: String,
    package: String,
    source: PluginSourceRef,
    observed_at_ms: u64,
}

impl OpenCodePackageProjection {
    fn new(package: &str, config_uri: &str, observed_at_ms: u64) -> Self {
        let plugin_id = format!("opencode.npm.{}", sanitize_plugin_id_component(package));
        Self {
            config_uri: config_uri.to_string(),
            package: package.to_string(),
            source: PluginSourceRef {
                plugin_id: plugin_id.clone(),
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: format!("npm:{package}"),
                version: None,
                content_hash: sha256_content_hash(&format!("npm:{package}")),
                trust_level: PluginTrustLevel::Unknown,
                manifest: Some(PluginManifestRef {
                    manifest_id: format!("{plugin_id}:opencode.config"),
                    schema_version: OPENCODE_CONFIG_SCHEMA.to_string(),
                    path: Some(config_uri.to_string()),
                }),
            },
            observed_at_ms,
        }
    }

    fn source_ref(&self) -> &PluginSourceRef {
        &self.source
    }

    fn read_diagnostics(&self) -> Vec<PluginDiagnostic> {
        vec![self.trust_diagnostic(), self.package_diagnostic()]
    }

    fn status_snapshot(
        &self,
        include_config_validation: bool,
        diagnostic_ids: Vec<String>,
    ) -> PluginStatusSnapshot {
        PluginStatusSnapshot {
            source: self.source.clone(),
            status: PluginStatusKind::TrustRequired,
            availability: PluginRuntimeAvailability::projection_only(
                PluginRuntimeUnavailableReason::DisabledByPolicy,
            ),
            config_validation: include_config_validation.then(|| PluginConfigValidationState {
                status: PluginConfigValidationStatus::Valid,
                issues: Vec::new(),
            }),
            quarantine: None,
            diagnostic_ids,
            updated_at_ms: self.observed_at_ms,
        }
    }

    fn project_dispatch_response(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        if envelope.source.plugin_id != self.source.plugin_id {
            return Err(PortError::new(
                PortErrorKind::NotFound,
                format!(
                    "OpenCode package source {} is not loaded by this adapter",
                    envelope.source.plugin_id
                ),
            ));
        }
        let diagnostics = self.dispatch_diagnostics(&envelope);
        let diagnostic_ids = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.diagnostic_id.clone())
            .collect();

        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: self.observed_at_ms,
            effects: Vec::new(),
            diagnostics,
            quarantine: None,
            plugin_statuses: vec![PluginStatusSnapshot {
                source: envelope.source.clone(),
                status: PluginStatusKind::TrustRequired,
                availability: PluginRuntimeAvailability::projection_only(
                    PluginRuntimeUnavailableReason::DisabledByPolicy,
                ),
                config_validation: None,
                quarantine: None,
                diagnostic_ids,
                updated_at_ms: self.observed_at_ms,
            }],
            observed_epochs: envelope.epochs,
        })
    }

    fn dispatch_diagnostics(&self, envelope: &PluginDispatchEnvelope) -> Vec<PluginDiagnostic> {
        let mut trust = self.trust_diagnostic();
        trust.diagnostic_id = format!(
            "diag:{}:dispatch:{}:trust",
            self.source.plugin_id, envelope.event_id
        );
        trust.source = envelope.source.clone();
        trust.audit = audit_ref(envelope);

        let mut package = self.package_diagnostic();
        package.diagnostic_id = format!(
            "diag:{}:dispatch:{}:npm:{}",
            self.source.plugin_id, envelope.event_id, self.package
        );
        package.source = envelope.source.clone();
        package.audit = audit_ref(envelope);

        vec![trust, package]
    }

    fn package_diagnostic(&self) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:npm:{}", self.source.plugin_id, self.package),
            severity: PluginDiagnosticSeverity::Info,
            source: self.source.clone(),
            code: "opencode.npm_plugin_projection_only".to_string(),
            message: format!(
                "OpenCode npm plugin is discovered from opencode.json but is not installed or executed by BitFun: {}",
                self.package
            ),
            detail: PluginDiagnosticDetail::Manifest {
                manifest: PluginManifestRef {
                    manifest_id: "opencode.config".to_string(),
                    schema_version: OPENCODE_CONFIG_SCHEMA.to_string(),
                    path: Some(self.config_uri.clone()),
                },
            },
            audit: PluginAuditRef {
                correlation_id: format!("config:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: false,
        }
    }

    fn trust_diagnostic(&self) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:trust", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Warning,
            source: self.source.clone(),
            code: "opencode.trust_required".to_string(),
            message: "OpenCode package source is not trusted for projection".to_string(),
            detail: PluginDiagnosticDetail::Trust {
                trust_level: self.source.trust_level,
            },
            audit: PluginAuditRef {
                correlation_id: format!("trust:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: false,
        }
    }
}

struct OpenCodeInvalidProjection {
    source: PluginSourceRef,
    validation: PluginConfigValidationState,
    diagnostic_code: String,
    diagnostic_message: String,
    diagnostic_detail_manifest: PluginManifestRef,
    observed_at_ms: u64,
}

impl OpenCodeInvalidProjection {
    fn config(
        config_uri: &str,
        config_json: &str,
        code: &str,
        field: &str,
        message: String,
        observed_at_ms: u64,
    ) -> Self {
        let manifest = PluginManifestRef {
            manifest_id: "opencode.config".to_string(),
            schema_version: OPENCODE_CONFIG_SCHEMA.to_string(),
            path: Some(config_uri.to_string()),
        };
        Self {
            source: PluginSourceRef {
                plugin_id: "opencode.config".to_string(),
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: config_uri.to_string(),
                version: None,
                content_hash: sha256_content_hash(config_json),
                trust_level: PluginTrustLevel::Unknown,
                manifest: Some(manifest.clone()),
            },
            validation: invalid_validation(field, code, &message),
            diagnostic_code: code.to_string(),
            diagnostic_message: message,
            diagnostic_detail_manifest: manifest,
            observed_at_ms,
        }
    }

    fn local_path(
        path: &Path,
        code: &str,
        field: &str,
        message: String,
        observed_at_ms: u64,
    ) -> Self {
        Self::local(
            path,
            sha256_content_hash(&format!("{}:{message}", path.to_string_lossy())),
            code,
            field,
            message,
            observed_at_ms,
        )
    }

    fn local_source(
        path: &Path,
        source: &str,
        code: &str,
        field: &str,
        message: String,
        observed_at_ms: u64,
    ) -> Self {
        Self::local(
            path,
            sha256_content_hash(source),
            code,
            field,
            message,
            observed_at_ms,
        )
    }

    fn local(
        path: &Path,
        content_hash: String,
        code: &str,
        field: &str,
        message: String,
        observed_at_ms: u64,
    ) -> Self {
        let plugin_id = format!("opencode.local.{}", path_stem_path(path));
        let path_string = path.to_string_lossy().into_owned();
        let manifest = PluginManifestRef {
            manifest_id: format!("{plugin_id}:{OPENCODE_LOCAL_PLUGIN_SCHEMA_VERSION}"),
            schema_version: OPENCODE_LOCAL_PLUGIN_SCHEMA_VERSION.to_string(),
            path: Some(path_string.clone()),
        };
        Self {
            source: PluginSourceRef {
                plugin_id,
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: source_file_uri(&path_string),
                version: None,
                content_hash,
                trust_level: PluginTrustLevel::Unknown,
                manifest: Some(manifest.clone()),
            },
            validation: invalid_validation(field, code, &message),
            diagnostic_code: code.to_string(),
            diagnostic_message: message,
            diagnostic_detail_manifest: manifest,
            observed_at_ms,
        }
    }

    fn source_ref(&self) -> &PluginSourceRef {
        &self.source
    }

    fn read_diagnostics(&self) -> Vec<PluginDiagnostic> {
        vec![self.diagnostic(None)]
    }

    fn status_snapshot(
        &self,
        include_config_validation: bool,
        diagnostic_ids: Vec<String>,
    ) -> PluginStatusSnapshot {
        PluginStatusSnapshot {
            source: self.source.clone(),
            status: PluginStatusKind::InvalidConfig,
            availability: PluginRuntimeAvailability::projection_only(
                PluginRuntimeUnavailableReason::DisabledByPolicy,
            ),
            config_validation: include_config_validation.then(|| self.validation.clone()),
            quarantine: None,
            diagnostic_ids,
            updated_at_ms: self.observed_at_ms,
        }
    }

    fn project_dispatch_response(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        if envelope.source.plugin_id != self.source.plugin_id {
            return Err(PortError::new(
                PortErrorKind::NotFound,
                format!(
                    "OpenCode source {} is not loaded by this adapter",
                    envelope.source.plugin_id
                ),
            ));
        }
        let diagnostics = vec![self.diagnostic(Some(&envelope))];
        let diagnostic_ids = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.diagnostic_id.clone())
            .collect();
        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: self.observed_at_ms,
            effects: Vec::new(),
            diagnostics,
            quarantine: None,
            plugin_statuses: vec![PluginStatusSnapshot {
                source: envelope.source.clone(),
                status: PluginStatusKind::InvalidConfig,
                availability: PluginRuntimeAvailability::projection_only(
                    PluginRuntimeUnavailableReason::DisabledByPolicy,
                ),
                config_validation: None,
                quarantine: None,
                diagnostic_ids,
                updated_at_ms: self.observed_at_ms,
            }],
            observed_epochs: envelope.epochs,
        })
    }

    fn diagnostic(&self, envelope: Option<&PluginDispatchEnvelope>) -> PluginDiagnostic {
        let diagnostic_id = match envelope {
            Some(envelope) => format!(
                "diag:{}:dispatch:{}:{}",
                self.source.plugin_id, envelope.event_id, self.diagnostic_code
            ),
            None => format!("diag:{}:{}", self.source.plugin_id, self.diagnostic_code),
        };
        PluginDiagnostic {
            diagnostic_id,
            severity: PluginDiagnosticSeverity::Error,
            source: envelope
                .map_or_else(|| self.source.clone(), |envelope| envelope.source.clone()),
            code: self.diagnostic_code.clone(),
            message: self.diagnostic_message.clone(),
            detail: PluginDiagnosticDetail::ConfigValidation {
                manifest: self.diagnostic_detail_manifest.clone(),
                validation: self.validation.clone(),
            },
            audit: envelope.map_or_else(
                || PluginAuditRef {
                    correlation_id: format!("invalid:{}", self.source.plugin_id),
                    event_id: None,
                },
                audit_ref,
            ),
            retryable: false,
        }
    }
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
    source_trust_snapshot: Option<OpenCodeMatchedTrustSnapshot>,
    source_trust_conflict: bool,
    observed_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenCodeMatchedTrustSnapshot {
    trust_level: PluginTrustLevel,
    trust_epoch: u64,
}

impl OpenCodeSourceProjection {
    fn from_local_plugin_source(
        local_plugin_source: &str,
        source: OpenCodeAdapterSource,
        mut config: OpenCodeConfig,
    ) -> Result<Self, OpenCodeAdapterError> {
        config.config_uri = source.config_uri.clone();
        let local_plugin =
            OpenCodeLocalPlugin::from_source(&source.local_plugin_path, local_plugin_source)?;
        let source_ref = PluginSourceRef {
            plugin_id: local_plugin.plugin_id.clone(),
            source_kind: PluginSourceKind::OpenCodeCompatible,
            source: source_file_uri(&source.local_plugin_path),
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
            source_trust_snapshot: None,
            source_trust_conflict: false,
            observed_at_ms: source.observed_at_ms,
        })
    }

    fn with_source_trust_refs(
        mut self,
        source_trust_epoch: u64,
        source_trust_refs: &[PluginSourceRef],
    ) -> Self {
        let matching_refs = source_trust_refs
            .iter()
            .filter(|trust_ref| source_identity_matches(trust_ref, &self.source))
            .collect::<Vec<_>>();
        self.source_trust_conflict = matching_refs
            .windows(2)
            .any(|window| window[0].trust_level != window[1].trust_level);

        if let Some(trust_ref) = matching_refs.last() {
            self.source_trust_snapshot = Some(OpenCodeMatchedTrustSnapshot {
                trust_level: trust_ref.trust_level,
                trust_epoch: source_trust_epoch,
            });
        }
        self
    }

    #[cfg(test)]
    fn from_opencode_sources(
        config_json: &str,
        local_plugin_source: &str,
        source: OpenCodeAdapterSource,
    ) -> Result<Self, OpenCodeAdapterError> {
        let config_doc: OpenCodeConfigDoc = serde_json::from_str(config_json)?;
        let config = OpenCodeConfig::try_from_doc(config_doc)?;
        Self::from_local_plugin_source(local_plugin_source, source, config)
    }

    fn source_ref(&self) -> &PluginSourceRef {
        &self.source
    }

    fn source_ref_for_epochs(&self, epochs: &PluginRuntimeEpochs) -> PluginSourceRef {
        let mut source = self.source.clone();
        source.trust_level = self.effective_trust_level(epochs);
        source
    }

    fn effective_trust_level(&self, epochs: &PluginRuntimeEpochs) -> PluginTrustLevel {
        match self.source_trust_snapshot {
            Some(trust_snapshot) if trust_snapshot.trust_epoch == epochs.trust_epoch => {
                trust_snapshot.trust_level
            }
            Some(_) => PluginTrustLevel::Unknown,
            None => self.source.trust_level,
        }
    }

    fn trust_epoch_mismatch(
        &self,
        epochs: &PluginRuntimeEpochs,
    ) -> Option<OpenCodeMatchedTrustSnapshot> {
        self.source_trust_snapshot
            .filter(|trust_snapshot| trust_snapshot.trust_epoch != epochs.trust_epoch)
    }

    fn without_config_package_diagnostics(mut self) -> Self {
        self.config.npm_plugins.clear();
        self
    }

    #[cfg(test)]
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
                workspace_id: request.workspace_id,
                sources: Vec::new(),
                plugin_statuses: Vec::new(),
                diagnostics: Vec::new(),
                observed_epochs: request.epochs,
            });
        }

        let diagnostics = self.read_diagnostics(&request.epochs);
        let diagnostic_ids = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.diagnostic_id.clone())
            .collect();
        let (availability, status) = self.trust_status_for_epochs(&request.epochs);
        let source = self.source_ref_for_epochs(&request.epochs);

        Ok(PluginRuntimeReadResponse {
            request_id: request.request_id,
            project_domain_id: request.project_domain_id,
            workspace_id: request.workspace_id,
            sources: vec![source.clone()],
            plugin_statuses: vec![self.status_snapshot(
                source,
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
                    "OpenCode source {} is not loaded by this adapter",
                    envelope.source.plugin_id
                ),
            ));
        }

        let effective_trust_level = self.effective_trust_level(&envelope.epochs);
        if effective_trust_level != PluginTrustLevel::Trusted {
            let mut diagnostics = Vec::new();
            if let Some(trust_fact) = self.trust_epoch_mismatch(&envelope.epochs) {
                diagnostics
                    .push(self.trust_epoch_mismatch_dispatch_diagnostic(&envelope, trust_fact));
            }
            diagnostics.push(self.trust_dispatch_diagnostic(&envelope, effective_trust_level));
            return Ok(self.response(
                &envelope,
                Vec::new(),
                diagnostics,
                Self::trust_status_for_level(effective_trust_level).1,
            ));
        }

        let (effects, diagnostics) = if envelope.extension_point_id == CUSTOM_TOOL_EXTENSION_POINT
            && !self.supports_custom_tool_capability(&envelope.declared_capability)
        {
            (
                Vec::new(),
                vec![self.custom_tool_capability_mismatch_diagnostic(&envelope)],
            )
        } else if envelope.extension_point_id == CUSTOM_TOOL_EXTENSION_POINT {
            (
                self.local_plugin
                    .custom_tools
                    .iter()
                    .map(|tool| self.provider_candidate_effect(&envelope, tool))
                    .collect(),
                Vec::new(),
            )
        } else {
            (
                Vec::new(),
                vec![self.unsupported_hook_dispatch_diagnostic(&envelope)],
            )
        };

        let status = PluginStatusKind::ProjectionOnly;
        Ok(self.response(&envelope, effects, diagnostics, status))
    }

    fn read_diagnostics(&self, epochs: &PluginRuntimeEpochs) -> Vec<PluginDiagnostic> {
        let mut diagnostics = Vec::new();
        let source = self.source_ref_for_epochs(epochs);
        if let Some(trust_fact) = self.trust_epoch_mismatch(epochs) {
            diagnostics.push(self.trust_epoch_mismatch_diagnostic(
                source.clone(),
                epochs.trust_epoch,
                trust_fact,
            ));
        }
        if self.source_trust_conflict {
            diagnostics.push(self.trust_ref_conflict_diagnostic(source.clone()));
        }
        if source.trust_level != PluginTrustLevel::Trusted {
            diagnostics.push(self.trust_diagnostic(source.clone(), source.trust_level));
        }
        diagnostics.extend(
            self.config
                .npm_plugins
                .iter()
                .map(|package| self.npm_package_diagnostic(package, source.clone())),
        );
        diagnostics.extend(
            self.local_plugin
                .unsupported_hooks
                .iter()
                .map(|hook| self.unsupported_hook_diagnostic(hook, source.clone())),
        );
        diagnostics
    }

    fn supports_custom_tool_capability(&self, capability: &PluginCapabilityRef) -> bool {
        self.local_plugin
            .custom_tools
            .iter()
            .any(|tool| tool.capability_ref() == *capability)
    }

    fn trust_status_for_epochs(
        &self,
        epochs: &PluginRuntimeEpochs,
    ) -> (PluginRuntimeAvailability, PluginStatusKind) {
        Self::trust_status_for_level(self.effective_trust_level(epochs))
    }

    fn trust_status_for_level(
        trust_level: PluginTrustLevel,
    ) -> (PluginRuntimeAvailability, PluginStatusKind) {
        match trust_level {
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
        source: PluginSourceRef,
        availability: PluginRuntimeAvailability,
        include_config_validation: bool,
        status: PluginStatusKind,
        diagnostic_ids: Vec<String>,
    ) -> PluginStatusSnapshot {
        PluginStatusSnapshot {
            source,
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

        let availability = self.trust_status_for_epochs(&envelope.epochs).0;

        PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: self.observed_at_ms,
            effects,
            diagnostics,
            quarantine: None,
            plugin_statuses: vec![self.status_snapshot(
                envelope.source.clone(),
                availability,
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
            source_ref: envelope.source.clone(),
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
            plugin: envelope.source.clone(),
            requested_capability: tool.capability_ref(),
            requested_effect: PermissionPromptEffectKind::ProviderCandidate,
            target,
            risk_level: PluginRiskLevel::Medium,
            owner: tool.capability_ref().owner,
            rollback: PluginRollbackPolicy {
                mode: PluginRollbackMode::DisablePlugin,
                reason_ref: Some(format!("audit:{}", envelope.event_id)),
            },
            deny_state: PermissionPromptDenyState::CandidateDiscarded,
            audit,
        }
    }

    fn npm_package_diagnostic(&self, package: &str, source: PluginSourceRef) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:npm:{package}", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Info,
            source,
            code: "opencode.npm_plugin_projection_only".to_string(),
            message: format!(
                "OpenCode npm plugin is present in opencode.json but is not installed or executed by BitFun: {package}"
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
        }
    }

    fn unsupported_hook_diagnostic(&self, hook: &str, source: PluginSourceRef) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:hook:{hook}", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Warning,
            source,
            code: "opencode.hook_projection_only".to_string(),
            message: format!(
                "OpenCode hook is discovered but the current OpenCode adapter does not support hook execution: {hook}"
            ),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: PluginAuditRef {
                correlation_id: format!("plugin-source:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: false,
        }
    }

    fn unsupported_hook_dispatch_diagnostic(
        &self,
        envelope: &PluginDispatchEnvelope,
    ) -> PluginDiagnostic {
        let mut diagnostic =
            self.unsupported_hook_diagnostic(&envelope.extension_point_id, envelope.source.clone());
        diagnostic.diagnostic_id = format!(
            "diag:{}:dispatch:{}:hook:{}",
            self.source.plugin_id, envelope.event_id, envelope.extension_point_id
        );
        diagnostic.source = envelope.source.clone();
        diagnostic.audit = audit_ref(envelope);
        diagnostic
    }

    fn trust_diagnostic(
        &self,
        source: PluginSourceRef,
        trust_level: PluginTrustLevel,
    ) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:trust", self.source.plugin_id),
            severity: if trust_level == PluginTrustLevel::Unknown {
                PluginDiagnosticSeverity::Warning
            } else {
                PluginDiagnosticSeverity::Error
            },
            source,
            code: "opencode.trust_required".to_string(),
            message: "OpenCode plugin source is not trusted for projection".to_string(),
            detail: PluginDiagnosticDetail::Trust { trust_level },
            audit: PluginAuditRef {
                correlation_id: format!("trust:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: false,
        }
    }

    fn trust_dispatch_diagnostic(
        &self,
        envelope: &PluginDispatchEnvelope,
        trust_level: PluginTrustLevel,
    ) -> PluginDiagnostic {
        let mut diagnostic = self.trust_diagnostic(envelope.source.clone(), trust_level);
        diagnostic.diagnostic_id = format!(
            "diag:{}:dispatch:{}:trust",
            self.source.plugin_id, envelope.event_id
        );
        diagnostic.source = envelope.source.clone();
        diagnostic.audit = audit_ref(envelope);
        diagnostic
    }

    fn trust_epoch_mismatch_diagnostic(
        &self,
        source: PluginSourceRef,
        runtime_trust_epoch: u64,
        trust_snapshot: OpenCodeMatchedTrustSnapshot,
    ) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:trust_epoch", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Warning,
            source,
            code: "opencode.trust_epoch_mismatch".to_string(),
            message: format!(
                "OpenCode trust snapshot epoch {} does not match runtime trust epoch {}; source remains untrusted",
                trust_snapshot.trust_epoch, runtime_trust_epoch
            ),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: PluginAuditRef {
                correlation_id: format!("trust:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: true,
        }
    }

    fn trust_ref_conflict_diagnostic(&self, source: PluginSourceRef) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:trust_ref_conflict", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Warning,
            source,
            code: "opencode.trust_ref_conflict".to_string(),
            message: "OpenCode source has multiple trust snapshots with conflicting trust levels; the last snapshot is used".to_string(),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: PluginAuditRef {
                correlation_id: format!("trust:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: false,
        }
    }

    fn trust_epoch_mismatch_dispatch_diagnostic(
        &self,
        envelope: &PluginDispatchEnvelope,
        trust_snapshot: OpenCodeMatchedTrustSnapshot,
    ) -> PluginDiagnostic {
        let mut diagnostic = self.trust_epoch_mismatch_diagnostic(
            envelope.source.clone(),
            envelope.epochs.trust_epoch,
            trust_snapshot,
        );
        diagnostic.diagnostic_id = format!(
            "diag:{}:dispatch:{}:trust_epoch",
            self.source.plugin_id, envelope.event_id
        );
        diagnostic.audit = audit_ref(envelope);
        diagnostic
    }

    fn custom_tool_capability_mismatch_diagnostic(
        &self,
        envelope: &PluginDispatchEnvelope,
    ) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!(
                "diag:{}:dispatch:{}:custom_tool_capability",
                self.source.plugin_id, envelope.event_id
            ),
            severity: PluginDiagnosticSeverity::Warning,
            source: envelope.source.clone(),
            code: "opencode.custom_tool_capability_mismatch".to_string(),
            message: format!(
                "OpenCode custom tool dispatch requires expected capability {CUSTOM_TOOL_CAPABILITY_ID} owned by {}/{}; actual capability {} owned by {}/{}",
                plugin_owner_kind_name(PluginOwnerKind::ExtensionContract),
                CUSTOM_TOOL_CAPABILITY_OWNER_ID,
                envelope.declared_capability.capability_id,
                plugin_owner_kind_name(envelope.declared_capability.owner.kind),
                envelope.declared_capability.owner.id
            ),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: audit_ref(envelope),
            retryable: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenCodeConfig {
    config_uri: String,
    npm_plugins: Vec<String>,
}

impl OpenCodeConfig {
    fn empty(config_uri: String) -> Self {
        Self {
            config_uri,
            npm_plugins: Vec::new(),
        }
    }

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
            capability_id: CUSTOM_TOOL_CAPABILITY_ID.to_string(),
            owner: PluginOwnerRef {
                kind: PluginOwnerKind::ExtensionContract,
                id: CUSTOM_TOOL_CAPABILITY_OWNER_ID.to_string(),
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
    let mut hooks = UNSUPPORTED_HOOK_EVENTS
        .iter()
        .filter(|event| {
            source.contains(&format!("\"{event}\"")) || source.contains(&format!("'{event}'"))
        })
        .map(|event| (*event).to_string())
        .collect::<Vec<_>>();
    if has_event_hook(source) && !hooks.iter().any(|hook| hook == "event") {
        hooks.push("event".to_string());
    }
    hooks
}

fn has_event_hook(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("event:") || line.contains(" event:")
    })
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

fn path_stem_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace('-', "_"))
        .unwrap_or_else(|| "plugin".to_string())
}

fn sha256_content_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn read_opencode_config(config_path: &Path) -> PortResult<(String, String)> {
    let config_uri = file_uri(config_path);
    if !config_path.exists() {
        return Ok((
            format!(r#"{{"$schema":"{OPENCODE_CONFIG_SCHEMA}","plugin":[]}}"#),
            config_uri,
        ));
    }

    let config_json = fs::read_to_string(config_path).map_err(|error| {
        adapter_port_error(format!(
            "failed to read OpenCode config {}: {error}",
            config_path.display()
        ))
    })?;
    Ok((config_json, config_uri))
}

fn parse_opencode_config(
    config_json: &str,
    config_uri: &str,
) -> Result<OpenCodeConfig, OpenCodeAdapterError> {
    let config_doc: OpenCodeConfigDoc = serde_json::from_str(config_json)?;
    let mut config = OpenCodeConfig::try_from_doc(config_doc)?;
    config.config_uri = config_uri.to_string();
    Ok(config)
}

fn workspace_plugin_paths(
    project_root: &Path,
    observed_at_ms: u64,
) -> PortResult<Vec<Result<PathBuf, OpenCodeInvalidProjection>>> {
    let plugin_dir = project_root.join(OPENCODE_WORKSPACE_PLUGIN_DIR);
    if !plugin_dir.exists() {
        return Ok(Vec::new());
    }
    let plugin_dir_metadata = fs::symlink_metadata(&plugin_dir).map_err(|error| {
        adapter_port_error(format!(
            "failed to inspect OpenCode plugin directory {}: {error}",
            plugin_dir.display()
        ))
    })?;
    if is_unsupported_link(&plugin_dir_metadata) {
        return Ok(vec![Err(OpenCodeInvalidProjection::local_path(
            &plugin_dir,
            "opencode.local_plugin_directory_link_unsupported",
            "source",
            format!(
                "OpenCode plugin directory is a symlink or reparse point and is not scanned by this OpenCode adapter: {}",
                plugin_dir.display()
            ),
            observed_at_ms,
        ))]);
    }
    let canonical_plugin_dir = plugin_dir.canonicalize().map_err(|error| {
        adapter_port_error(format!(
            "failed to resolve OpenCode plugin directory {}: {error}",
            plugin_dir.display()
        ))
    })?;

    let mut paths = fs::read_dir(&plugin_dir)
        .map_err(|error| {
            adapter_port_error(format!(
                "failed to read OpenCode plugin directory {}: {error}",
                plugin_dir.display()
            ))
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            adapter_port_error(format!(
                "failed to inspect OpenCode plugin directory {}: {error}",
                plugin_dir.display()
            ))
        })?;

    paths.sort();
    let candidates = paths
        .into_iter()
        .filter(|path| {
            matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("js" | "ts")
            )
        })
        .map(|path| plugin_file_candidate(path, &canonical_plugin_dir, observed_at_ms))
        .collect();
    Ok(candidates)
}

fn plugin_file_candidate(
    path: PathBuf,
    canonical_plugin_dir: &Path,
    observed_at_ms: u64,
) -> Result<PathBuf, OpenCodeInvalidProjection> {
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        OpenCodeInvalidProjection::local_path(
            &path,
            "opencode.local_plugin_unreadable",
            "source",
            format!(
                "failed to inspect OpenCode plugin source {}: {error}",
                path.display()
            ),
            observed_at_ms,
        )
    })?;

    if is_unsupported_link(&metadata) {
        return Err(OpenCodeInvalidProjection::local_path(
            &path,
            "opencode.local_plugin_symlink_unsupported",
            "source",
            format!(
                "OpenCode plugin source is a symlink and is not scanned by this OpenCode adapter: {}",
                path.display()
            ),
            observed_at_ms,
        ));
    }
    if !metadata.is_file() {
        return Err(OpenCodeInvalidProjection::local_path(
            &path,
            "opencode.local_plugin_not_file",
            "source",
            format!(
                "OpenCode plugin source is not a regular file and is not scanned by this OpenCode adapter: {}",
                path.display()
            ),
            observed_at_ms,
        ));
    }
    if metadata.len() > MAX_OPENCODE_PLUGIN_SOURCE_BYTES {
        return Err(OpenCodeInvalidProjection::local_path(
            &path,
            "opencode.local_plugin_too_large",
            "source",
            format!(
                "OpenCode plugin source exceeds the OpenCode adapter size limit of {MAX_OPENCODE_PLUGIN_SOURCE_BYTES} bytes: {}",
                path.display()
            ),
            observed_at_ms,
        ));
    }

    let canonical_path = path.canonicalize().map_err(|error| {
        OpenCodeInvalidProjection::local_path(
            &path,
            "opencode.local_plugin_unreadable",
            "source",
            format!(
                "failed to resolve OpenCode plugin source {}: {error}",
                path.display()
            ),
            observed_at_ms,
        )
    })?;
    if !canonical_path.starts_with(canonical_plugin_dir) {
        return Err(OpenCodeInvalidProjection::local_path(
            &path,
            "opencode.local_plugin_outside_workspace",
            "source",
            format!(
                "OpenCode plugin source resolves outside .opencode/plugins and is not scanned: {}",
                path.display()
            ),
            observed_at_ms,
        ));
    }

    Ok(path)
}

fn is_unsupported_link(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || is_windows_reparse_point(metadata)
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn file_uri(path: &Path) -> String {
    source_file_uri(&path.to_string_lossy())
}

fn source_file_uri(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        format!("file:///{normalized}")
    }
}

fn sanitize_plugin_id_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut previous_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator {
            sanitized.push('_');
            previous_separator = true;
        }
    }

    let sanitized = sanitized.trim_matches('_').to_string();
    if sanitized.is_empty() {
        "plugin".to_string()
    } else {
        sanitized
    }
}

fn adapter_port_error(message: String) -> PortError {
    PortError::new(PortErrorKind::InvalidRequest, message)
}

fn invalid_validation(field: &str, code: &str, message: &str) -> PluginConfigValidationState {
    PluginConfigValidationState {
        status: PluginConfigValidationStatus::Invalid,
        issues: vec![PluginConfigValidationIssue {
            field: field.to_string(),
            code: code.to_string(),
            message: message.to_string(),
        }],
    }
}

fn audit_ref(envelope: &PluginDispatchEnvelope) -> PluginAuditRef {
    PluginAuditRef {
        correlation_id: envelope.correlation_id.clone(),
        event_id: Some(envelope.event_id.clone()),
    }
}

#[cfg(test)]
mod opencode_projection_contracts {
    use super::*;
    use bitfun_plugin_runtime_host::PluginRuntimeHost;
    use bitfun_runtime_ports::{
        PermissionPromptDenyState, PermissionPromptEffectKind, PluginPayloadRedaction,
        PluginPayloadRef, PluginRuntimeClient, PluginRuntimeEpochs,
    };
    use std::sync::Arc;

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
            capability_id: CUSTOM_TOOL_CAPABILITY_ID.to_string(),
            owner: PluginOwnerRef {
                kind: PluginOwnerKind::ExtensionContract,
                id: CUSTOM_TOOL_CAPABILITY_OWNER_ID.to_string(),
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
    fn p0_c2_fixture_projects_custom_tool_candidate_with_permission_prompt() {
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
                assert_eq!(prompt.owner.kind, PluginOwnerKind::ExtensionContract);
                assert_eq!(
                    prompt.deny_state,
                    PermissionPromptDenyState::CandidateDiscarded
                );
                assert_eq!(prompt.audit.event_id.as_deref(), Some("event-tool"));
            }
            other => panic!("expected permission prompt, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn host_path_projects_trusted_custom_tool_candidate_with_permission_prompt() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter)],
            observed_at_ms: 1_720_000_001,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(dispatch)
            .await
            .expect("host dispatch should preserve trusted custom tool candidate");

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
            CUSTOM_TOOL_CAPABILITY_ID
        );
        match &effect.payload {
            PluginEffectCandidatePayload::ProviderCandidate {
                provider_id,
                tool_contract_id,
            } => {
                assert_eq!(
                    provider_id,
                    "opencode.local.workspace_tools.workspaceSummary"
                );
                assert_eq!(tool_contract_id, CUSTOM_TOOL_CONTRACT_ID);
            }
            other => panic!("expected provider candidate, got {other:?}"),
        }
        match &effect.permission {
            PluginPermissionGate::PermissionRequired { prompt } => {
                assert_eq!(
                    prompt.requested_effect,
                    PermissionPromptEffectKind::ProviderCandidate
                );
                assert_eq!(
                    prompt.target.target_id,
                    "opencode.local.workspace_tools.workspaceSummary"
                );
                assert_eq!(
                    prompt.deny_state,
                    PermissionPromptDenyState::CandidateDiscarded
                );
            }
            other => panic!("expected permission prompt, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn host_path_rejects_mismatched_custom_tool_capability_without_effect() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let mut dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        dispatch.declared_capability.capability_id = "opencode.permission_hook".to_string();
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter)],
            observed_at_ms: 1_720_000_001,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(dispatch)
            .await
            .expect("host dispatch should reject mismatched custom tool capability");

        assert!(response.effects.is_empty());
        assert_eq!(
            response.diagnostics[0].code,
            "opencode.custom_tool_capability_mismatch"
        );
        assert_eq!(
            response.diagnostics[0].message,
            "OpenCode custom tool dispatch requires expected capability opencode.custom_tool owned by extension_contract/opencode.custom-tools; actual capability opencode.permission_hook owned by extension_contract/opencode.custom-tools"
        );
        assert_eq!(
            response.plugin_statuses[0].status,
            PluginStatusKind::ProjectionOnly
        );
    }

    #[tokio::test]
    async fn host_path_rejects_custom_tool_capability_owner_mismatch_without_quarantine() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let mut dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        dispatch.declared_capability.owner = PluginOwnerRef {
            kind: PluginOwnerKind::ExtensionContract,
            id: "opencode.wrong-owner".to_string(),
        };
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter.clone())],
            observed_at_ms: 1_720_000_001,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(dispatch)
            .await
            .expect("host dispatch should reject full capability ref mismatch");

        assert!(response.effects.is_empty());
        assert_eq!(
            response.diagnostics[0].code,
            "opencode.custom_tool_capability_mismatch"
        );
        assert_eq!(
            response.diagnostics[0].message,
            "OpenCode custom tool dispatch requires expected capability opencode.custom_tool owned by extension_contract/opencode.custom-tools; actual capability opencode.custom_tool owned by extension_contract/opencode.wrong-owner"
        );
        let follow_up = host
            .dispatch(envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT))
            .await
            .expect("capability mismatch diagnostic should not quarantine the plugin");
        assert_eq!(follow_up.effects.len(), 1);
    }

    #[tokio::test]
    async fn host_path_rejects_custom_tool_capability_owner_kind_mismatch_without_quarantine() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let mut dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        dispatch.declared_capability.owner = PluginOwnerRef {
            kind: PluginOwnerKind::ProductFeature,
            id: CUSTOM_TOOL_CAPABILITY_OWNER_ID.to_string(),
        };
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter.clone())],
            observed_at_ms: 1_720_000_001,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(dispatch)
            .await
            .expect("host dispatch should reject capability owner kind mismatch");

        assert!(response.effects.is_empty());
        assert_eq!(
            response.diagnostics[0].code,
            "opencode.custom_tool_capability_mismatch"
        );
        assert_eq!(
            response.diagnostics[0].message,
            "OpenCode custom tool dispatch requires expected capability opencode.custom_tool owned by extension_contract/opencode.custom-tools; actual capability opencode.custom_tool owned by product_feature/opencode.custom-tools"
        );
        let follow_up = host
            .dispatch(envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT))
            .await
            .expect("owner kind mismatch diagnostic should not quarantine the plugin");
        assert_eq!(follow_up.effects.len(), 1);
    }

    #[tokio::test]
    async fn host_path_accepts_source_identity_with_different_read_model_fields() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let mut dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        dispatch.source.trust_level = PluginTrustLevel::Unknown;
        dispatch.source.manifest = None;
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter)],
            observed_at_ms: 1_720_000_001,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(dispatch.clone())
            .await
            .expect("read-model-only source fields should not break dispatch routing");

        assert_eq!(response.effects.len(), 1);
        assert!(response.diagnostics.is_empty());
        assert_eq!(response.effects[0].source_ref, dispatch.source);
        assert_eq!(response.plugin_statuses[0].source, dispatch.source);
        assert_eq!(
            response.plugin_statuses[0].status,
            PluginStatusKind::ProjectionOnly
        );
    }

    #[tokio::test]
    async fn host_path_revoked_source_snapshot_overrides_stale_dispatch_source_without_quarantine()
    {
        let adapter = adapter(PluginTrustLevel::Revoked);
        let mut dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        dispatch.source.trust_level = PluginTrustLevel::Trusted;
        dispatch.source.manifest = None;
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter.clone())],
            observed_at_ms: 1_720_000_001,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(dispatch.clone())
            .await
            .expect("revoked trust snapshot should project a typed diagnostic");

        assert!(response.effects.is_empty());
        assert_eq!(
            response.plugin_statuses[0].status,
            PluginStatusKind::Disabled
        );
        assert_eq!(response.diagnostics[0].source, dispatch.source);
        assert_eq!(response.plugin_statuses[0].source, dispatch.source);

        let follow_up = host
            .dispatch(dispatch)
            .await
            .expect("revoked trust diagnostic should not quarantine the plugin");
        assert!(follow_up.effects.is_empty());
    }

    #[tokio::test]
    async fn host_path_rejects_stale_source_ref_without_quarantine() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let mut dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        dispatch.source.content_hash = "sha256:stale".to_string();
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter.clone())],
            observed_at_ms: 1_720_000_001,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(dispatch.clone())
            .await
            .expect("host dispatch should convert stale source refs into diagnostics");

        assert!(response.effects.is_empty());
        assert_eq!(response.diagnostics[0].code, "opencode.source_mismatch");
        assert_eq!(response.diagnostics[0].source, dispatch.source);
        assert_eq!(response.plugin_statuses[0].source, dispatch.source);
        assert_eq!(
            response.plugin_statuses[0].availability,
            PluginRuntimeAvailability::Unavailable {
                reason: PluginRuntimeUnavailableReason::HostUnavailable
            }
        );

        let follow_up = host
            .dispatch(envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT))
            .await
            .expect("stale source diagnostic should not quarantine the plugin");
        assert_eq!(follow_up.effects.len(), 1);
    }

    #[tokio::test]
    async fn host_path_projects_unsupported_hook_diagnostic_without_quarantine() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter.clone())],
            observed_at_ms: 1_720_000_001,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(envelope(&adapter, "tool.execute.before"))
            .await
            .expect("host dispatch should preserve unsupported hook diagnostic");

        assert!(response.effects.is_empty());
        assert_eq!(response.diagnostics.len(), 1);
        assert_eq!(
            response.diagnostics[0].code,
            "opencode.hook_projection_only"
        );
        assert_eq!(
            response.diagnostics[0].audit.event_id.as_deref(),
            Some("event-tool.execute.before")
        );

        let follow_up = host
            .dispatch(envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT))
            .await
            .expect("unsupported hook diagnostic should not quarantine the plugin");
        assert_eq!(follow_up.effects.len(), 1);
    }

    #[test]
    fn custom_tool_dispatch_rejects_mismatched_declared_capability_without_effect() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let mut dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        dispatch.declared_capability.capability_id = "opencode.permission_hook".to_string();

        let response = adapter
            .project_dispatch_response(dispatch)
            .expect("project dispatch response");

        assert!(response.effects.is_empty());
        assert_eq!(
            response.diagnostics[0].code,
            "opencode.custom_tool_capability_mismatch"
        );
        assert_eq!(
            response.plugin_statuses[0].status,
            PluginStatusKind::ProjectionOnly
        );
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
    fn unsupported_opencode_hook_projects_typed_diagnostic_without_effect() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let response = adapter
            .project_dispatch_response(envelope(&adapter, "tool.execute.before"))
            .expect("project dispatch response");

        assert!(response.effects.is_empty());
        assert_eq!(
            response.diagnostics[0].code,
            "opencode.hook_projection_only"
        );
        assert_eq!(
            response.diagnostics[0].source.plugin_id,
            "opencode.local.workspace_tools"
        );
        assert_eq!(
            response.plugin_statuses[0].status,
            PluginStatusKind::ProjectionOnly
        );
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
