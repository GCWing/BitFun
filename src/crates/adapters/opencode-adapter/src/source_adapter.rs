//! OpenCode-compatible source projection.
//!
//! The adapter covers real OpenCode input shapes: `opencode.json` npm plugin
//! entries and project-local `.opencode/plugins/*.ts` source files. It does not
//! execute JavaScript, install packages, or become the runtime host.

use crate::hook_contributions::{
    map_hook_contributions, OpenCodeHookDescriptor, OPENCODE_PLUGIN_PROVIDER_ID,
};
use async_trait::async_trait;
use bitfun_plugin_runtime_host::PluginHostAdapter;
use bitfun_product_domains::external_hook_contributions::{
    ExternalHookContributionDeclaration, ExternalHookPoint, ExternalHookRiskCapability,
};
use bitfun_product_domains::external_sources::SourceKey;
use bitfun_product_domains::plugin_source::{PluginActivationAuthority, PluginPackageInput};
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
use oxc_parse::{
    allocator::Allocator,
    ast::ast::{
        ArrowFunctionExpression, Declaration, Expression, ObjectExpression, ObjectPropertyKind,
        Statement,
    },
    parser::Parser,
    span::SourceType,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::Path, sync::Arc};

const OPENCODE_ADAPTER_ID: &str = "opencode-compatible";
const OPENCODE_CONFIG_SCHEMA: &str = "https://opencode.ai/config.json";
const OPENCODE_LOCAL_PLUGIN_SCHEMA_VERSION: &str = "opencode.plugin.module.ts";
const PLUGIN_EFFECT_SCHEMA_VERSION: &str = "plugin.effect.v1";
const CUSTOM_TOOL_CONTRACT_ID: &str = "opencode.custom-tool.v1";
const CUSTOM_TOOL_CAPABILITY_ID: &str = "opencode.custom_tool";
const CUSTOM_TOOL_CAPABILITY_OWNER_ID: &str = "opencode.custom-tools";
const CUSTOM_TOOL_EXTENSION_POINT: &str = "tool";
const MAX_PLUGIN_ID_COMPONENT_LEN: usize = 40;
const MAX_CUSTOM_TOOLS_PER_SOURCE: usize = 128;
const MAX_CUSTOM_TOOLS_PER_PACKAGE: usize = 256;
const MAX_CUSTOM_TOOL_ID_BYTES: usize = 64;
const MAX_NPM_PLUGINS: usize = 128;
const MAX_NPM_PLUGIN_NAME_BYTES: usize = 256;
const MAX_NPM_PLUGIN_METADATA_BYTES: usize = 16 * 1024;

// Frozen from the @opencode-ai/plugin Hooks interface. `tool` is handled by
// the existing custom-tool projection and event-bus event types belong under
// the top-level `event` Hook rather than in this property set.
const DISCOVERABLE_HOOK_PROPERTIES: &[&str] = &[
    "auth",
    "chat.headers",
    "chat.message",
    "chat.params",
    "command.execute.before",
    "config",
    "dispose",
    "event",
    "experimental.chat.messages.transform",
    "experimental.chat.system.transform",
    "experimental.compaction.autocontinue",
    "experimental.provider.small_model",
    "experimental.session.compacting",
    "experimental.text.complete",
    "permission.ask",
    "provider",
    "shell.env",
    "tool.execute.after",
    "tool.execute.before",
    "tool.definition",
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
    activation: Option<OpenCodeActivationContext>,
}

struct OpenCodeActivationContext {
    project_domain_id: String,
    workspace_id: String,
    activation_epoch: u64,
}

impl OpenCodePluginHostAdapter {
    fn from_package(input: PluginPackageInput, observed_at_ms: u64) -> PortResult<Self> {
        let (manifest, source, files) = input.into_parts();
        if manifest.adapter != "opencode_compatible" {
            return Err(adapter_port_error(format!(
                "managed package adapter is not OpenCode-compatible: {}",
                manifest.adapter
            )));
        }
        let provenance_id = sha256_content_hash(&source.source_path)
            .trim_start_matches("sha256:")
            .to_string();
        let package_path = format!("/managed-plugins/{provenance_id}/{}", source.package_id);
        let package_uri = format!(
            "bitfun://managed-plugins/{provenance_id}/{}",
            urlencoding::encode(&source.package_id)
        );
        let config_uri = format!(
            "bitfun://managed-plugins/{provenance_id}/{}/opencode.json",
            source.package_id
        );
        let mut projections = Vec::new();
        let config = match files.get("opencode.json") {
            Some(bytes) => match std::str::from_utf8(bytes) {
                Ok(config_json) => match parse_opencode_config(config_json, &config_uri) {
                    Ok(config) => config,
                    Err(error) => {
                        projections.push(OpenCodeProjection::Invalid(
                            OpenCodeInvalidProjection::config(
                                &config_uri,
                                config_json,
                                "opencode.config_invalid",
                                "opencode.json",
                                error.to_string(),
                                observed_at_ms,
                            )
                            .with_package_identity(&source.version, &source.content_hash),
                        ));
                        OpenCodeConfig::empty(config_uri.clone())
                    }
                },
                Err(error) => {
                    let config_json = String::from_utf8_lossy(bytes);
                    projections.push(OpenCodeProjection::Invalid(
                        OpenCodeInvalidProjection::config(
                            &config_uri,
                            &config_json,
                            "opencode.config_invalid",
                            "opencode.json",
                            format!("opencode.json must be UTF-8: {error}"),
                            observed_at_ms,
                        )
                        .with_package_identity(&source.version, &source.content_hash),
                    ));
                    OpenCodeConfig::empty(config_uri.clone())
                }
            },
            None => OpenCodeConfig::empty(config_uri.clone()),
        };

        let mut package_custom_tool_count = 0usize;
        for (relative_path, bytes) in files.iter().filter(|(path, _)| {
            path.starts_with(".opencode/plugins/")
                && (path.ends_with(".js") || path.ends_with(".ts"))
        }) {
            let plugin_path = format!("{package_path}/{relative_path}");
            let plugin_uri = managed_source_uri(&provenance_id, &source.package_id, relative_path);
            let plugin_source = match std::str::from_utf8(bytes) {
                Ok(source) => source,
                Err(error) => {
                    projections.push(OpenCodeProjection::Invalid(
                        OpenCodeInvalidProjection::local_source(
                            Path::new(&plugin_path),
                            "",
                            "opencode.local_plugin_invalid",
                            "source",
                            format!("OpenCode plugin source must be UTF-8: {error}"),
                            observed_at_ms,
                        )
                        .with_package_identity(&source.version, &source.content_hash)
                        .with_source_uri(plugin_uri.clone()),
                    ));
                    continue;
                }
            };
            match OpenCodeSourceProjection::from_local_plugin_source(
                plugin_source,
                OpenCodeAdapterSource::project_local(
                    config_uri.clone(),
                    plugin_path.clone(),
                    PluginTrustLevel::Unknown,
                    observed_at_ms,
                )
                .with_source_uri(plugin_uri.clone()),
                config.clone(),
            )
            .map(|mut projection| {
                projection.source.version = Some(source.version.clone());
                projection.source.content_hash = source.content_hash.clone();
                projection.without_config_package_diagnostics()
            }) {
                Ok(projection) => {
                    package_custom_tool_count = package_custom_tool_count
                        .checked_add(projection.local_plugin.custom_tools.len())
                        .ok_or_else(|| {
                            adapter_port_error("custom tool count overflow".to_string())
                        })?;
                    if package_custom_tool_count > MAX_CUSTOM_TOOLS_PER_PACKAGE {
                        return Err(adapter_port_error(format!(
                            "managed package declares more than {MAX_CUSTOM_TOOLS_PER_PACKAGE} custom tools"
                        )));
                    }
                    projections.push(OpenCodeProjection::Local(projection));
                }
                Err(error) => projections.push(OpenCodeProjection::Invalid(
                    OpenCodeInvalidProjection::local_source(
                        Path::new(&plugin_path),
                        plugin_source,
                        "opencode.local_plugin_invalid",
                        error.field(),
                        error.to_string(),
                        observed_at_ms,
                    )
                    .with_package_identity(&source.version, &source.content_hash)
                    .with_source_uri(plugin_uri),
                )),
            }
        }

        projections.extend(config.npm_plugins.iter().map(|package| {
            OpenCodeProjection::Package(OpenCodePackageProjection::new(
                package,
                &config_uri,
                &source.version,
                &source.content_hash,
                observed_at_ms,
            ))
        }));

        if projections.is_empty() {
            projections.push(OpenCodeProjection::Invalid(
                OpenCodeInvalidProjection::package(
                    &package_uri,
                    &source.package_id,
                    &source.version,
                    &source.content_hash,
                    "opencode.package_no_supported_entry",
                    "files",
                    "managed package has no recognized OpenCode config or local plugin entry"
                        .to_string(),
                    observed_at_ms,
                ),
            ));
        }

        Ok(Self {
            projections,
            observed_at_ms,
            activation: None,
        })
    }

    fn from_activated_package(
        input: PluginPackageInput,
        authority: PluginActivationAuthority,
        observed_at_ms: u64,
    ) -> PortResult<Self> {
        let (project_domain_id, workspace_id, authority_source, activation_epoch) =
            authority.into_parts();
        let (manifest, source, files) = input.into_parts();
        if source != authority_source {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "OpenCode package input does not match its activation authority",
            ));
        }
        let input = PluginPackageInput::new(manifest, source, files)
            .map_err(|error| PortError::new(PortErrorKind::InvalidRequest, error.to_string()))?;
        let mut adapter = Self::from_package(input, observed_at_ms)?;
        for projection in &mut adapter.projections {
            projection.activate_supported_source();
        }
        adapter.activation = Some(OpenCodeActivationContext {
            project_domain_id,
            workspace_id,
            activation_epoch,
        });
        Ok(adapter)
    }

    fn custom_tool_dispatch_targets(
        &self,
    ) -> Vec<(
        PluginSourceRef,
        String,
        PluginCapabilityRef,
        Vec<(PluginTargetRef, PluginRiskLevel)>,
    )> {
        self.projections
            .iter()
            .filter_map(OpenCodeProjection::custom_tool_dispatch_target)
            .collect()
    }

    fn validate_activation_scope(
        &self,
        project_domain_id: &str,
        workspace_id: &str,
        epochs: &PluginRuntimeEpochs,
    ) -> PortResult<()> {
        let Some(activation) = &self.activation else {
            return Ok(());
        };
        if activation.project_domain_id != project_domain_id
            || activation.workspace_id != workspace_id
            || activation.activation_epoch != epochs.trust_epoch
        {
            return Err(PortError::new(
                PortErrorKind::NotAvailable,
                "OpenCode package activation scope or epoch is stale",
            ));
        }
        Ok(())
    }

    fn activation_matches(
        &self,
        project_domain_id: &str,
        workspace_id: &str,
        epochs: &PluginRuntimeEpochs,
    ) -> bool {
        self.activation.as_ref().is_none_or(|activation| {
            activation.project_domain_id == project_domain_id
                && activation.workspace_id == workspace_id
                && activation.activation_epoch == epochs.trust_epoch
        })
    }

    fn projection_for_source(&self, source: &PluginSourceRef) -> Option<&OpenCodeProjection> {
        self.projections
            .iter()
            .find(|projection| source_identity_matches(projection.source_ref(), source))
    }

    fn source_mismatch_response(&self, envelope: PluginDispatchEnvelope) -> PluginResponseEnvelope {
        self.unavailable_response(
            envelope,
            "opencode.source_mismatch",
            "OpenCode dispatch source does not match a loaded source snapshot",
            false,
        )
    }

    fn activation_stale_response(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PluginResponseEnvelope {
        self.unavailable_response(
            envelope,
            "opencode.activation_stale",
            "OpenCode package activation scope or epoch is stale",
            true,
        )
    }

    fn unavailable_response(
        &self,
        envelope: PluginDispatchEnvelope,
        code: &str,
        message: &str,
        retryable: bool,
    ) -> PluginResponseEnvelope {
        let diagnostic_id = format!(
            "diag:{}:dispatch:{}:{}",
            envelope.source.plugin_id, envelope.event_id, code
        );
        let diagnostic = PluginDiagnostic {
            diagnostic_id: diagnostic_id.clone(),
            severity: PluginDiagnosticSeverity::Warning,
            source: envelope.source.clone(),
            code: code.to_string(),
            message: message.to_string(),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: audit_ref(&envelope),
            retryable,
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
        self.validate_activation_scope(
            &request.project_domain_id,
            &request.workspace_id,
            &request.epochs,
        )?;
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
        if !self.activation_matches(
            &envelope.project_domain_id,
            &envelope.workspace_id,
            &envelope.epochs,
        ) {
            return Ok(self.activation_stale_response(envelope));
        }
        match self.projection_for_source(&envelope.source) {
            Some(projection) => projection.project_dispatch_response(envelope),
            None => Ok(self.source_mismatch_response(envelope)),
        }
    }
}

pub fn load_opencode_package_adapter(
    input: PluginPackageInput,
    activation: Option<PluginActivationAuthority>,
    observed_at_ms: u64,
) -> PortResult<(
    Arc<dyn PluginHostAdapter>,
    Vec<(
        PluginSourceRef,
        String,
        PluginCapabilityRef,
        Vec<(PluginTargetRef, PluginRiskLevel)>,
    )>,
)> {
    let adapter = match activation {
        Some(authority) => {
            OpenCodePluginHostAdapter::from_activated_package(input, authority, observed_at_ms)?
        }
        None => OpenCodePluginHostAdapter::from_package(input, observed_at_ms)?,
    };
    let targets = adapter.custom_tool_dispatch_targets();
    Ok((Arc::new(adapter), targets))
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
    fn activate_supported_source(&mut self) {
        if let Self::Local(projection) = self {
            projection.source.trust_level = PluginTrustLevel::Trusted;
        }
    }

    fn source_ref(&self) -> &PluginSourceRef {
        match self {
            Self::Local(projection) => projection.source_ref(),
            Self::Package(projection) => projection.source_ref(),
            Self::Invalid(projection) => projection.source_ref(),
        }
    }

    fn custom_tool_dispatch_target(
        &self,
    ) -> Option<(
        PluginSourceRef,
        String,
        PluginCapabilityRef,
        Vec<(PluginTargetRef, PluginRiskLevel)>,
    )> {
        let Self::Local(projection) = self else {
            return None;
        };
        let capability = projection
            .local_plugin
            .custom_tools
            .first()
            .map(OpenCodeCustomTool::capability_ref)?;
        Some((
            projection.source_ref().clone(),
            CUSTOM_TOOL_EXTENSION_POINT.to_string(),
            capability,
            projection
                .local_plugin
                .custom_tools
                .iter()
                .map(|tool| {
                    (
                        tool.target_ref(&projection.source.plugin_id),
                        PluginRiskLevel::High,
                    )
                })
                .collect(),
        ))
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
    fn new(
        package: &str,
        config_uri: &str,
        package_version: &str,
        package_content_hash: &str,
        observed_at_ms: u64,
    ) -> Self {
        let source_uri = format!("{config_uri}#npm={}", urlencoding::encode(package));
        let plugin_id = stable_plugin_id(
            "opencode.npm",
            &sanitize_plugin_id_component(package),
            &source_uri,
        );
        Self {
            config_uri: config_uri.to_string(),
            package: package.to_string(),
            source: PluginSourceRef {
                plugin_id: plugin_id.clone(),
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: source_uri,
                version: Some(package_version.to_string()),
                content_hash: package_content_hash.to_string(),
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
    fn with_package_identity(mut self, version: &str, content_hash: &str) -> Self {
        self.source.version = Some(version.to_string());
        self.source.content_hash = content_hash.to_string();
        self
    }

    fn with_source_uri(mut self, source_uri: String) -> Self {
        self.source.source = source_uri;
        self
    }

    fn package(
        package_uri: &str,
        package_id: &str,
        version: &str,
        content_hash: &str,
        code: &str,
        field: &str,
        message: String,
        observed_at_ms: u64,
    ) -> Self {
        let plugin_id = stable_plugin_id(
            "opencode.package",
            &sanitize_plugin_id_component(package_id),
            package_uri,
        );
        let manifest = PluginManifestRef {
            manifest_id: format!("{plugin_id}:bitfun.plugin"),
            schema_version: "bitfun.plugin.package.v1".to_string(),
            path: Some(package_uri.to_string()),
        };
        Self {
            source: PluginSourceRef {
                plugin_id,
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: package_uri.to_string(),
                version: Some(version.to_string()),
                content_hash: content_hash.to_string(),
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

    fn config(
        config_uri: &str,
        config_json: &str,
        code: &str,
        field: &str,
        message: String,
        observed_at_ms: u64,
    ) -> Self {
        let plugin_id = stable_plugin_id("opencode.config", "source", config_uri);
        let manifest = PluginManifestRef {
            manifest_id: format!("{plugin_id}:opencode.config"),
            schema_version: OPENCODE_CONFIG_SCHEMA.to_string(),
            path: Some(config_uri.to_string()),
        };
        Self {
            source: PluginSourceRef {
                plugin_id,
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
        let plugin_id = local_plugin_id(&path.to_string_lossy());
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
    source_uri: String,
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
        let local_plugin_path = local_plugin_path.into();
        Self {
            config_uri: config_uri.into(),
            source_uri: source_file_uri(&local_plugin_path),
            local_plugin_path,
            trust_level,
            observed_at_ms,
        }
    }

    fn with_source_uri(mut self, source_uri: String) -> Self {
        self.source_uri = source_uri;
        self
    }
}

#[derive(Debug, Clone)]
struct OpenCodeSourceProjection {
    config: OpenCodeConfig,
    local_plugin: OpenCodeLocalPlugin,
    hook_contributions: Vec<ExternalHookContributionDeclaration>,
    source: PluginSourceRef,
    observed_at_ms: u64,
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
            source: source.source_uri.clone(),
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
        let source_key =
            SourceKey::new(OPENCODE_PLUGIN_PROVIDER_ID, local_plugin.plugin_id.clone()).map_err(
                |error| OpenCodeAdapterError::InvalidPluginSource {
                    field: "plugin.hooks",
                    message: format!("invalid Hook source identity: {error}"),
                },
            )?;
        let descriptors = local_plugin
            .statically_mapped_hooks
            .iter()
            .filter_map(|event| OpenCodeHookDescriptor::from_static_projection_event(event))
            .collect();
        let hook_contributions =
            map_hook_contributions(source_key, descriptors).map_err(|error| {
                OpenCodeAdapterError::InvalidPluginSource {
                    field: "plugin.hooks",
                    message: format!("Hook mapping failed: {}", error.as_str()),
                }
            })?;

        Ok(Self {
            config,
            local_plugin,
            hook_contributions,
            source: source_ref,
            observed_at_ms: source.observed_at_ms,
        })
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

    fn source_ref_for_epochs(&self, _epochs: &PluginRuntimeEpochs) -> PluginSourceRef {
        self.source.clone()
    }

    fn effective_trust_level(&self, _epochs: &PluginRuntimeEpochs) -> PluginTrustLevel {
        self.source.trust_level
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
            return Ok(self.response(
                &envelope,
                Vec::new(),
                vec![self.trust_dispatch_diagnostic(&envelope, effective_trust_level)],
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
            self.hook_contributions
                .iter()
                .map(|hook| self.mapped_hook_diagnostic(hook, source.clone())),
        );
        diagnostics.extend(
            self.local_plugin
                .discovered_hooks
                .iter()
                .filter(|hook| !self.local_plugin.statically_mapped_hooks.contains(hook))
                .map(|hook| self.unsupported_hook_diagnostic(hook, source.clone())),
        );
        diagnostics.extend(
            self.local_plugin
                .hook_projection_error
                .as_ref()
                .map(|error| self.hook_projection_error_diagnostic(*error, source.clone())),
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
            risk_level: PluginRiskLevel::High,
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
            risk_level: PluginRiskLevel::High,
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

    fn mapped_hook_diagnostic(
        &self,
        hook: &ExternalHookContributionDeclaration,
        source: PluginSourceRef,
    ) -> PluginDiagnostic {
        let event_name = match hook.hook_point {
            ExternalHookPoint::ToolBefore => "tool.execute.before",
            ExternalHookPoint::ToolAfter => "tool.execute.after",
        };
        let declared_risks = format_natural_list(
            hook.safety
                .declared_risks
                .iter()
                .map(|risk| match risk {
                    ExternalHookRiskCapability::ReadToolArguments => "read tool arguments",
                    ExternalHookRiskCapability::ModifyToolArguments => "modify tool arguments",
                    ExternalHookRiskCapability::ReadToolResult => "read tool results",
                    ExternalHookRiskCapability::ModifyToolResult => "modify tool results",
                })
                .collect(),
        );
        let safety_statement = if hook.safety.complete {
            "its declared safety facts are complete"
        } else {
            "its safety declaration is incomplete"
        };
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:hook:{}", self.source.plugin_id, hook.contribution_id),
            severity: PluginDiagnosticSeverity::Warning,
            source,
            code: "opencode.hook_mapped_runtime_unavailable".to_string(),
            message: format!(
                "OpenCode Hook {event_name} was recognized from the plugin return object. It may {declared_risks}, but {safety_statement} and Hook execution is unavailable in this BitFun version"
            ),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: PluginAuditRef {
                correlation_id: hook.contribution_id.to_string(),
                event_id: None,
            },
            retryable: false,
        }
    }

    fn hook_projection_error_diagnostic(
        &self,
        error: OpenCodeHookProjectionError,
        source: PluginSourceRef,
    ) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:hook-projection", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Warning,
            source,
            code: "opencode.hook_projection_parse_failed".to_string(),
            message: "OpenCode Hook projection could not parse the plugin as JavaScript or TypeScript; Hook declarations are unavailable"
                .to_string(),
            detail: PluginDiagnosticDetail::Adapter {
                adapter_id: OPENCODE_ADAPTER_ID.to_string(),
            },
            audit: PluginAuditRef {
                correlation_id: format!(
                    "plugin-source:{}:hook-projection:{}",
                    self.source.plugin_id,
                    error.as_str()
                ),
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

        if doc.plugin.len() > MAX_NPM_PLUGINS {
            return Err(OpenCodeAdapterError::InvalidConfig {
                field: "plugin",
                message: format!("at most {MAX_NPM_PLUGINS} npm plugins may be declared"),
            });
        }

        let mut npm_plugins = Vec::new();
        let mut seen_packages = HashSet::new();
        let mut metadata_bytes = 0usize;
        for package in doc.plugin {
            let package = package.trim().to_string();
            if package.is_empty() {
                return Err(OpenCodeAdapterError::InvalidConfig {
                    field: "plugin",
                    message: "package names must not be empty".to_string(),
                });
            }
            if package.len() > MAX_NPM_PLUGIN_NAME_BYTES {
                return Err(OpenCodeAdapterError::InvalidConfig {
                    field: "plugin",
                    message: format!(
                        "npm plugin names may contain at most {MAX_NPM_PLUGIN_NAME_BYTES} bytes"
                    ),
                });
            }
            metadata_bytes = metadata_bytes.checked_add(package.len()).ok_or_else(|| {
                OpenCodeAdapterError::InvalidConfig {
                    field: "plugin",
                    message: "npm plugin metadata size overflow".to_string(),
                }
            })?;
            if metadata_bytes > MAX_NPM_PLUGIN_METADATA_BYTES {
                return Err(OpenCodeAdapterError::InvalidConfig {
                    field: "plugin",
                    message: format!(
                        "npm plugin metadata may contain at most {MAX_NPM_PLUGIN_METADATA_BYTES} bytes"
                    ),
                });
            }
            if seen_packages.insert(package.clone()) {
                npm_plugins.push(package);
            }
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
    discovered_hooks: Vec<String>,
    statically_mapped_hooks: Vec<String>,
    hook_projection_error: Option<OpenCodeHookProjectionError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenCodeHookProjectionError {
    ParseFailed,
}

impl OpenCodeHookProjectionError {
    fn as_str(self) -> &'static str {
        match self {
            Self::ParseFailed => "parse_failed",
        }
    }
}

impl OpenCodeLocalPlugin {
    fn from_source(path: &str, source: &str) -> Result<Self, OpenCodeAdapterError> {
        let source = strip_js_comments(source)?;
        let export_name =
            exported_plugin_name(&source).ok_or(OpenCodeAdapterError::InvalidPluginSource {
                field: "plugin.export",
                message: "expected an exported OpenCode plugin function".to_string(),
            })?;
        let custom_tools = discover_custom_tools(&source)?;
        let (discovered_hooks, hook_projection_error) =
            match discover_exported_hooks(&source, path, &export_name) {
                Ok(hooks) => (hooks, None),
                Err(error) => (Vec::new(), Some(error)),
            };
        let statically_mapped_hooks = discovered_hooks
            .iter()
            .filter(|event| OpenCodeHookDescriptor::from_static_projection_event(event).is_some())
            .cloned()
            .collect();
        if custom_tools.is_empty() && discovered_hooks.is_empty() && hook_projection_error.is_none()
        {
            return Err(OpenCodeAdapterError::InvalidPluginSource {
                field: "plugin.contributions",
                message: "expected a custom tool or hook contribution".to_string(),
            });
        }

        Ok(Self {
            plugin_id: local_plugin_id(path),
            export_name,
            custom_tools,
            discovered_hooks,
            statically_mapped_hooks,
            hook_projection_error,
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

fn discover_custom_tools(source: &str) -> Result<Vec<OpenCodeCustomTool>, OpenCodeAdapterError> {
    let mut ids = HashSet::new();
    let tools = source
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.trim().split_once(':')?;
            rest.trim_start()
                .starts_with("tool({")
                .then(|| name.trim())
                .filter(|candidate| is_identifier(candidate))
        })
        .map(|id| {
            if id.len() > MAX_CUSTOM_TOOL_ID_BYTES {
                return Err(OpenCodeAdapterError::InvalidPluginSource {
                    field: "plugin.custom_tools",
                    message: format!(
                        "custom tool identifiers may contain at most {MAX_CUSTOM_TOOL_ID_BYTES} bytes"
                    ),
                });
            }
            if !ids.insert(id) {
                return Err(OpenCodeAdapterError::InvalidPluginSource {
                    field: "plugin.custom_tools",
                    message: format!("duplicate custom tool declaration: {id}"),
                });
            }
            Ok(OpenCodeCustomTool {
                id: id.to_string(),
                tool_contract_id: CUSTOM_TOOL_CONTRACT_ID.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if tools.len() > MAX_CUSTOM_TOOLS_PER_SOURCE {
        return Err(OpenCodeAdapterError::InvalidPluginSource {
            field: "plugin.custom_tools",
            message: format!(
                "a plugin source may declare at most {MAX_CUSTOM_TOOLS_PER_SOURCE} custom tools"
            ),
        });
    }
    Ok(tools)
}

fn strip_js_comments(source: &str) -> Result<String, OpenCodeAdapterError> {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut quote = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;

    while let Some(ch) = chars.next() {
        if line_comment {
            if ch == '\n' {
                line_comment = false;
                output.push(ch);
            } else {
                output.push(' ');
            }
            continue;
        }
        if block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                output.push_str("  ");
                block_comment = false;
            } else if ch == '\n' {
                output.push(ch);
            } else {
                output.push(' ');
            }
            continue;
        }
        if let Some(delimiter) = quote {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == delimiter {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            output.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            output.push_str("  ");
            line_comment = true;
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            output.push_str("  ");
            block_comment = true;
        } else {
            output.push(ch);
        }
    }

    if block_comment {
        return Err(OpenCodeAdapterError::InvalidPluginSource {
            field: "plugin.comments",
            message: "unterminated block comment".to_string(),
        });
    }
    Ok(output)
}

fn discover_exported_hooks(
    source: &str,
    path: &str,
    export_name: &str,
) -> Result<Vec<String>, OpenCodeHookProjectionError> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(Path::new(path)).unwrap_or(SourceType::ts());
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(OpenCodeHookProjectionError::ParseFailed);
    }

    Ok(parsed
        .program
        .body
        .iter()
        .find_map(|statement| exported_hook_object(statement, export_name))
        .map(|object| {
            let mut seen = HashSet::new();
            object
                .properties
                .iter()
                .filter_map(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) => property.key.static_name(),
                    ObjectPropertyKind::SpreadProperty(_) => None,
                })
                .filter(|name| DISCOVERABLE_HOOK_PROPERTIES.contains(&name.as_ref()))
                .filter(|name| seen.insert(name.to_string()))
                .map(|name| name.into_owned())
                .collect()
        })
        .unwrap_or_default())
}

pub(crate) struct StaticHookEvent {
    pub registration_id: String,
    pub native_event: String,
}

pub(crate) struct StaticHookEventDiscovery {
    pub events: Vec<StaticHookEvent>,
    pub opaque_events: Vec<StaticHookEvent>,
    pub dynamic_registrations: Vec<String>,
}

pub(crate) fn statically_discover_hook_events(
    path: &Path,
    source: &str,
) -> Result<StaticHookEventDiscovery, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or(SourceType::ts());
    // OXC already understands comments and regular-expression literals. A
    // text-level comment pass can corrupt valid JavaScript such as `/https?:\/\//`.
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(OpenCodeHookProjectionError::ParseFailed
            .as_str()
            .to_string());
    }
    let mut events = Vec::new();
    let mut opaque_events = Vec::new();
    let mut dynamic_registrations = Vec::new();
    let mut dynamic_seen = HashSet::new();
    let mut exported_declaration_seen = false;
    for statement in &parsed.program.body {
        if let Statement::ExportDefaultDeclaration(export) = statement {
            exported_declaration_seen = true;
            let has_runtime_value = match &export.declaration {
                oxc_parse::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(
                    function,
                ) => !function.declare,
                oxc_parse::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                    !class.declare
                }
                oxc_parse::ast::ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
                    false
                }
                _ => true,
            };
            if has_runtime_value {
                push_dynamic_registration(
                    "default".to_string(),
                    &mut dynamic_registrations,
                    &mut dynamic_seen,
                );
            }
            continue;
        }
        if let Statement::ExportAllDeclaration(export) = statement {
            exported_declaration_seen = true;
            if matches!(
                export.export_kind,
                oxc_parse::ast::ast::ImportOrExportKind::Value
            ) {
                let registration_id = export
                    .exported
                    .as_ref()
                    .map(|name| name.name().to_string())
                    .unwrap_or_else(|| format!("<export-all:{}>", export.source.value));
                push_dynamic_registration(
                    registration_id,
                    &mut dynamic_registrations,
                    &mut dynamic_seen,
                );
            }
            continue;
        }
        let Statement::ExportNamedDeclaration(export) = statement else {
            continue;
        };
        let Some(declaration) = &export.declaration else {
            if !export.specifiers.is_empty() {
                exported_declaration_seen = true;
                if matches!(
                    export.export_kind,
                    oxc_parse::ast::ast::ImportOrExportKind::Value
                ) {
                    for specifier in &export.specifiers {
                        if matches!(
                            specifier.export_kind,
                            oxc_parse::ast::ast::ImportOrExportKind::Value
                        ) {
                            push_dynamic_registration(
                                specifier.exported.name().to_string(),
                                &mut dynamic_registrations,
                                &mut dynamic_seen,
                            );
                        }
                    }
                }
            }
            continue;
        };
        exported_declaration_seen = true;
        if declaration.declare() {
            continue;
        }
        let Declaration::VariableDeclaration(declaration) = declaration else {
            let registration_id = match declaration {
                Declaration::FunctionDeclaration(function) => {
                    function.id.as_ref().map(|id| id.name.to_string())
                }
                Declaration::ClassDeclaration(class) => {
                    class.id.as_ref().map(|id| id.name.to_string())
                }
                Declaration::TSEnumDeclaration(declaration)
                    if !declaration.declare && !declaration.r#const =>
                {
                    Some(declaration.id.name.to_string())
                }
                _ => None,
            };
            if let Some(registration_id) = registration_id {
                push_dynamic_registration(
                    registration_id,
                    &mut dynamic_registrations,
                    &mut dynamic_seen,
                );
            }
            continue;
        };
        for declarator in &declaration.declarations {
            let is_simple_identifier = declarator.id.get_identifier_name().is_some();
            let mut registration_ids = declarator
                .id
                .get_binding_identifiers()
                .into_iter()
                .map(|identifier| identifier.name.to_string())
                .collect::<Vec<_>>();
            if !is_simple_identifier {
                if registration_ids.is_empty() {
                    registration_ids.push("<destructured-export>".to_string());
                }
                for registration_id in registration_ids {
                    push_dynamic_registration(
                        registration_id,
                        &mut dynamic_registrations,
                        &mut dynamic_seen,
                    );
                }
                continue;
            }
            let registration_id = registration_ids
                .pop()
                .expect("identifier binding must contain its registration name");
            let Some(Expression::ArrowFunctionExpression(arrow)) = declarator
                .init
                .as_ref()
                .map(Expression::get_inner_expression)
            else {
                push_dynamic_registration(
                    registration_id,
                    &mut dynamic_registrations,
                    &mut dynamic_seen,
                );
                continue;
            };
            let Some(object) = arrow_return_object(arrow) else {
                push_dynamic_registration(
                    registration_id,
                    &mut dynamic_registrations,
                    &mut dynamic_seen,
                );
                continue;
            };
            if collect_static_hook_properties(
                object,
                &registration_id,
                &mut events,
                &mut opaque_events,
            ) {
                push_dynamic_registration(
                    registration_id,
                    &mut dynamic_registrations,
                    &mut dynamic_seen,
                );
            }
        }
    }
    if !exported_declaration_seen {
        push_dynamic_registration(
            "<module>".to_string(),
            &mut dynamic_registrations,
            &mut dynamic_seen,
        );
    }
    Ok(StaticHookEventDiscovery {
        events,
        opaque_events,
        dynamic_registrations,
    })
}

fn collect_static_hook_properties(
    object: &ObjectExpression<'_>,
    registration_id: &str,
    events: &mut Vec<StaticHookEvent>,
    opaque_events: &mut Vec<StaticHookEvent>,
) -> bool {
    let mut seen = HashSet::new();
    let mut has_dynamic_registration = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            has_dynamic_registration = true;
            continue;
        };
        let Some(name) = property.key.static_name() else {
            has_dynamic_registration = true;
            continue;
        };
        let name = name.into_owned();
        if name == CUSTOM_TOOL_EXTENSION_POINT || !seen.insert(name.clone()) {
            continue;
        }
        if name.is_empty() || name.len() > 160 || name.chars().any(char::is_control) {
            has_dynamic_registration = true;
        } else if DISCOVERABLE_HOOK_PROPERTIES.contains(&name.as_str()) {
            events.push(StaticHookEvent {
                registration_id: registration_id.to_string(),
                native_event: name,
            });
        } else {
            opaque_events.push(StaticHookEvent {
                registration_id: registration_id.to_string(),
                native_event: name,
            });
        }
    }
    has_dynamic_registration
}

fn push_dynamic_registration(
    registration_id: String,
    dynamic_registrations: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if seen.insert(registration_id.clone()) {
        dynamic_registrations.push(registration_id);
    }
}

fn exported_hook_object<'a>(
    statement: &'a Statement<'a>,
    export_name: &str,
) -> Option<&'a ObjectExpression<'a>> {
    let Statement::ExportNamedDeclaration(export) = statement else {
        return None;
    };
    let Some(Declaration::VariableDeclaration(declaration)) = &export.declaration else {
        return None;
    };
    let declarator = declaration
        .declarations
        .iter()
        .find(|declarator| declarator.id.get_identifier_name().as_deref() == Some(export_name))?;
    let Expression::ArrowFunctionExpression(arrow) =
        declarator.init.as_ref()?.get_inner_expression()
    else {
        return None;
    };
    arrow_return_object(arrow)
}

fn arrow_return_object<'a>(
    arrow: &'a ArrowFunctionExpression<'a>,
) -> Option<&'a ObjectExpression<'a>> {
    let expression = if arrow.expression {
        let Statement::ExpressionStatement(statement) = arrow.body.statements.first()? else {
            return None;
        };
        &statement.expression
    } else {
        let return_statement =
            arrow
                .body
                .statements
                .iter()
                .find_map(|statement| match statement {
                    Statement::ReturnStatement(statement) => Some(statement),
                    _ => None,
                })?;
        return_statement.argument.as_ref()?
    };
    match expression.get_inner_expression() {
        Expression::ObjectExpression(object) => Some(object),
        _ => None,
    }
}

fn format_natural_list(items: Vec<&str>) -> String {
    match items.as_slice() {
        [] => "use no declared data".to_string(),
        [item] => (*item).to_string(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let (last, preceding) = items.split_last().expect("non-empty risk list");
            format!("{}, and {last}", preceding.join(", "))
        }
    }
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

fn local_plugin_id(path: &str) -> String {
    stable_plugin_id(
        "opencode.local",
        &sanitize_plugin_id_component(&path_stem(path)),
        &path.replace('\\', "/"),
    )
}

fn stable_plugin_id(prefix: &str, component: &str, identity: &str) -> String {
    let digest = hex::encode(Sha256::digest(identity.as_bytes()));
    format!("{prefix}.{component}.{}", &digest[..32])
}

fn sha256_content_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
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

fn source_file_uri(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/') {
        format!("file://{normalized}")
    } else {
        format!("file:///{normalized}")
    }
}

fn managed_source_uri(provenance_id: &str, package_id: &str, relative_path: &str) -> String {
    let encoded_path = relative_path
        .split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect::<Vec<_>>()
        .join("/");
    format!(
        "bitfun://managed-plugins/{provenance_id}/{}/{encoded_path}",
        urlencoding::encode(package_id)
    )
}

fn sanitize_plugin_id_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len().min(MAX_PLUGIN_ID_COMPONENT_LEN));
    let mut previous_separator = false;
    for ch in value.chars() {
        if sanitized.len() >= MAX_PLUGIN_ID_COMPONENT_LEN {
            break;
        }
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

    #[test]
    fn commented_custom_tool_declarations_are_not_discovered() {
        let source = r#"
export const DemoPlugin = async () => ({
  /*
  ghost: tool({
  */
  // lineGhost: tool({
})
"#;

        let error = OpenCodeLocalPlugin::from_source(LOCAL_PLUGIN_PATH, source)
            .expect_err("comments must not create contributions");
        assert!(error.to_string().contains("expected a custom tool or hook"));
    }

    #[test]
    fn duplicate_custom_tool_declarations_are_rejected() {
        let source = r#"
export const DemoPlugin = async () => ({
  tool: {
    duplicate: tool({
    duplicate: tool({
  }
})
"#;

        let error = OpenCodeLocalPlugin::from_source(LOCAL_PLUGIN_PATH, source)
            .expect_err("duplicate declarations must be ambiguous");
        assert!(error
            .to_string()
            .contains("duplicate custom tool declaration"));
    }

    #[test]
    fn custom_tool_declaration_limit_accepts_boundary_and_rejects_overflow() {
        let source = |count| {
            let declarations = (0..count)
                .map(|index| format!("  tool{index}: tool({{"))
                .collect::<Vec<_>>()
                .join("\n");
            format!("export const DemoPlugin = async () => ({{\n{declarations}\n}})")
        };

        let boundary = source(MAX_CUSTOM_TOOLS_PER_SOURCE);
        let plugin = OpenCodeLocalPlugin::from_source(LOCAL_PLUGIN_PATH, &boundary)
            .expect("boundary declaration count");
        assert_eq!(plugin.custom_tools.len(), MAX_CUSTOM_TOOLS_PER_SOURCE);

        let overflow = source(MAX_CUSTOM_TOOLS_PER_SOURCE + 1);
        let error = OpenCodeLocalPlugin::from_source(LOCAL_PLUGIN_PATH, &overflow)
            .expect_err("overflow declaration count");
        assert!(error.to_string().contains("may declare at most"));

        let long_id = "a".repeat(MAX_CUSTOM_TOOL_ID_BYTES + 1);
        let long_source =
            format!("export const DemoPlugin = async () => ({{\n  {long_id}: tool({{\n}})");
        let error = OpenCodeLocalPlugin::from_source(LOCAL_PLUGIN_PATH, &long_source)
            .expect_err("long custom tool identifier");
        assert!(error
            .to_string()
            .contains("identifiers may contain at most"));
    }

    #[test]
    fn npm_plugin_declaration_limits_reject_count_and_metadata_amplification() {
        let doc = |plugin| OpenCodeConfigDoc {
            schema: Some(OPENCODE_CONFIG_SCHEMA.to_string()),
            plugin,
        };

        let count_error = OpenCodeConfig::try_from_doc(doc((0..=MAX_NPM_PLUGINS)
            .map(|index| format!("package-{index}"))
            .collect()))
        .expect_err("npm plugin count overflow");
        assert!(count_error.to_string().contains("at most 128 npm plugins"));

        let metadata_error = OpenCodeConfig::try_from_doc(doc((0..65)
            .map(|index| format!("{index:03}{}", "a".repeat(253)))
            .collect()))
        .expect_err("npm plugin metadata overflow");
        assert!(metadata_error
            .to_string()
            .contains("metadata may contain at most"));
    }

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
        let plugin_id = adapter.source.plugin_id.clone();

        assert_eq!(
            adapter.config.npm_plugins,
            ["opencode-wakatime", "@my-org/custom-plugin"]
        );
        assert_eq!(adapter.local_plugin.export_name, "WorkspaceToolsPlugin");
        assert_eq!(adapter.local_plugin.custom_tools[0].id, "workspaceSummary");
        assert_eq!(
            adapter.local_plugin.discovered_hooks,
            ["tool.execute.before"]
        );

        let response = adapter
            .project_read_model(PluginRuntimeReadRequest {
                request_id: "read-1".to_string(),
                project_domain_id: "project-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                plugin_ids: vec![plugin_id.clone()],
                include_config_validation: true,
                epochs: epochs(),
            })
            .expect("project read model");

        assert_eq!(response.sources.len(), 1);
        assert_eq!(response.sources[0].plugin_id, plugin_id);
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
        assert!(response.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "opencode.hook_mapped_runtime_unavailable"
                && diagnostic.message.contains("tool.execute.before")
        }));
    }

    #[test]
    fn p0_c2_fixture_projects_custom_tool_candidate_with_permission_prompt() {
        let adapter = adapter(PluginTrustLevel::Trusted);
        let plugin_id = adapter.source.plugin_id.clone();
        let provider_id = format!("{plugin_id}.workspaceSummary");
        let response = adapter
            .project_dispatch_response(envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT))
            .expect("project dispatch response");

        assert_eq!(response.adapter_id, OPENCODE_ADAPTER_ID);
        assert_eq!(response.plugin_id.as_deref(), Some(plugin_id.as_str()));
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
        assert_eq!(effect.target_ref.target_id, provider_id);
        assert_eq!(effect.source_ref.plugin_id, plugin_id);
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
                    &format!("{}.workspaceSummary", effect.source_ref.plugin_id)
                );
                assert_eq!(tool_contract_id, "opencode.custom-tool.v1");
            }
            other => panic!("expected provider candidate, got {other:?}"),
        }

        match &effect.permission {
            PluginPermissionGate::PermissionRequired { prompt } => {
                assert_eq!(prompt.plugin.plugin_id, effect.source_ref.plugin_id);
                assert_eq!(
                    prompt.requested_effect,
                    PermissionPromptEffectKind::ProviderCandidate
                );
                assert_eq!(prompt.target.target_id, effect.target_ref.target_id);
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
        let plugin_id = adapter.source.plugin_id.clone();
        let dispatch = envelope(&adapter, CUSTOM_TOOL_EXTENSION_POINT);
        let host_adapter: Arc<dyn PluginHostAdapter> = Arc::new(OpenCodePluginHostAdapter {
            projections: vec![OpenCodeProjection::Local(adapter)],
            observed_at_ms: 1_720_000_001,
            activation: None,
        });
        let host = PluginRuntimeHost::new(host_adapter);

        let response = host
            .dispatch(dispatch)
            .await
            .expect("host dispatch should preserve trusted custom tool candidate");

        assert_eq!(response.adapter_id, OPENCODE_ADAPTER_ID);
        assert_eq!(response.plugin_id.as_deref(), Some(plugin_id.as_str()));
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
                assert_eq!(provider_id, &format!("{plugin_id}.workspaceSummary"));
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
                    format!("{plugin_id}.workspaceSummary")
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
            activation: None,
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
            activation: None,
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
            activation: None,
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
            activation: None,
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
            activation: None,
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
            activation: None,
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
            activation: None,
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
        let plugin_id = adapter.source.plugin_id.clone();

        let read = adapter
            .project_read_model(PluginRuntimeReadRequest {
                request_id: "read-trust".to_string(),
                project_domain_id: "project-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                plugin_ids: vec![plugin_id],
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
        let plugin_id = adapter.source.plugin_id.clone();
        let response = adapter
            .project_dispatch_response(envelope(&adapter, "tool.execute.before"))
            .expect("project dispatch response");

        assert!(response.effects.is_empty());
        assert_eq!(
            response.diagnostics[0].code,
            "opencode.hook_projection_only"
        );
        assert_eq!(response.diagnostics[0].source.plugin_id, plugin_id);
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
