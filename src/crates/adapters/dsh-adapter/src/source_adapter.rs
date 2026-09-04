//! DeepSeek Harness (`dsh`) bundle source projection.
//!
//! The adapter covers the `dsh` distribution shape: a `package.json` whose
//! `dsh` field declares a bundle (`dsh.bundle.patch` -> a
//! `cordis.patch.yml` list of Cordis plugin rows), a profile
//! (`dsh.profile.bundles` -> an ordered list of bundle names). It projects the
//! discovered entries for either or both roles as projection-only plugin
//! sources. It does not execute JavaScript, install packages, or become the
//! runtime client.

use async_trait::async_trait;
use openbitfun_plugin_runtime_client::PluginRuntimeAdapter;
use openbitfun_product_domains::plugin_source::{
    PluginActivationAuthority, PluginPackageInput, PluginPackageSourceIdentity,
};
use openbitfun_runtime_ports::{
    PluginAuditRef, PluginConfigValidationIssue, PluginConfigValidationState,
    PluginConfigValidationStatus, PluginDiagnostic, PluginDiagnosticDetail,
    PluginDiagnosticSeverity, PluginDispatchEnvelope, PluginManifestRef, PluginResponseEnvelope,
    PluginRuntimeAvailability, PluginRuntimeEpochs, PluginRuntimeReadRequest,
    PluginRuntimeReadResponse, PluginRuntimeUnavailableReason, PluginSourceKind, PluginSourceRef,
    PluginStatusKind, PluginStatusSnapshot, PluginTrustLevel, PortError, PortErrorKind, PortResult,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

const DSH_ADAPTER_ID: &str = "dsh-compatible";
const DSH_MANIFEST_ADAPTER_ID: &str = "dsh_compatible";
const DSH_PACKAGE_SCHEMA_VERSION: &str = "dsh.package.v1";
const DSH_BUNDLE_SCHEMA_VERSION: &str = "dsh.bundle.v1";
const DSH_PROFILE_SCHEMA_VERSION: &str = "dsh.profile.v1";
const PACKAGE_JSON: &str = "package.json";
const MAX_PLUGIN_ID_COMPONENT_LEN: usize = 40;
const MAX_ENTRIES_PER_PACKAGE: usize = 256;
const MAX_ENTRY_ID_BYTES: usize = 256;
const MAX_PROFILE_BUNDLES: usize = 128;
const MAX_PROFILE_BUNDLE_NAME_BYTES: usize = 256;

struct DshPluginRuntimeAdapter {
    projections: Vec<DshProjection>,
    observed_at_ms: u64,
    activation: Option<DshActivationContext>,
}

struct DshActivationContext {
    project_domain_id: String,
    workspace_id: String,
    activation_epoch: u64,
}

impl DshPluginRuntimeAdapter {
    fn from_package(input: PluginPackageInput, observed_at_ms: u64) -> PortResult<Self> {
        let (manifest, source, files) = input.into_parts();
        if manifest.adapter != DSH_MANIFEST_ADAPTER_ID {
            return Err(adapter_port_error(format!(
                "managed package adapter is not dsh-compatible: {}",
                manifest.adapter
            )));
        }
        let provenance_id = sha256_content_hash(&source.source_path)
            .trim_start_matches("sha256:")
            .to_string();
        let package_uri = format!(
            "openbitfun://managed-plugins/{provenance_id}/{}",
            urlencoding::encode(&source.package_id)
        );
        let package_json_uri = format!(
            "openbitfun://managed-plugins/{provenance_id}/{}/{PACKAGE_JSON}",
            source.package_id
        );
        let projections = project_package(
            &files,
            &provenance_id,
            &source,
            &package_uri,
            &package_json_uri,
            observed_at_ms,
        )?;
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
                "dsh package input does not match its activation authority",
            ));
        }
        let input = PluginPackageInput::new(manifest, source, files)
            .map_err(|error| PortError::new(PortErrorKind::InvalidRequest, error.to_string()))?;
        let mut adapter = Self::from_package(input, observed_at_ms)?;
        if adapter.projections.iter().any(DshProjection::is_invalid) {
            return Err(adapter_port_error(
                "invalid dsh package projection cannot be activated".to_string(),
            ));
        }
        for projection in &mut adapter.projections {
            projection.activate_supported_source();
        }
        adapter.activation = Some(DshActivationContext {
            project_domain_id,
            workspace_id,
            activation_epoch,
        });
        Ok(adapter)
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
                "dsh package activation scope or epoch is stale",
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

    fn projection_for_source(&self, source: &PluginSourceRef) -> Option<&DshProjection> {
        self.projections
            .iter()
            .find(|projection| source_identity_matches(projection.source_ref(), source))
    }

    fn source_mismatch_response(&self, envelope: PluginDispatchEnvelope) -> PluginResponseEnvelope {
        self.unavailable_response(
            envelope,
            "dsh.source_mismatch",
            "dsh dispatch source does not match a loaded source snapshot",
            false,
        )
    }

    fn activation_stale_response(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PluginResponseEnvelope {
        self.unavailable_response(
            envelope,
            "dsh.activation_stale",
            "dsh package activation scope or epoch is stale",
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
                adapter_id: DSH_ADAPTER_ID.to_string(),
            },
            audit: audit_ref(&envelope),
            retryable,
        };

        PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: DSH_ADAPTER_ID.to_string(),
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
impl PluginRuntimeAdapter for DshPluginRuntimeAdapter {
    fn adapter_id(&self) -> &str {
        DSH_ADAPTER_ID
    }

    fn availability(&self) -> PluginRuntimeAvailability {
        PluginRuntimeAvailability::projection_only(PluginRuntimeUnavailableReason::HostUnavailable)
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
            let projection_diagnostics = projection.read_diagnostics();
            let diagnostic_ids = projection_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.diagnostic_id.clone())
                .collect();
            sources.push(projection.source_ref().clone());
            plugin_statuses.push(
                projection.status_snapshot(request.include_config_validation, diagnostic_ids),
            );
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

// Product Assembly consumes the same compatibility tuple exposed by the
// sibling OpenCode adapter.
#[allow(clippy::type_complexity)]
pub fn load_dsh_package_adapter(
    input: PluginPackageInput,
    activation: Option<PluginActivationAuthority>,
    observed_at_ms: u64,
) -> PortResult<(
    Arc<dyn PluginRuntimeAdapter>,
    Vec<(
        PluginSourceRef,
        String,
        openbitfun_runtime_ports::PluginCapabilityRef,
        Vec<(
            openbitfun_runtime_ports::PluginTargetRef,
            openbitfun_runtime_ports::PluginRiskLevel,
        )>,
    )>,
)> {
    let adapter = match activation {
        Some(authority) => {
            DshPluginRuntimeAdapter::from_activated_package(input, authority, observed_at_ms)?
        }
        None => DshPluginRuntimeAdapter::from_package(input, observed_at_ms)?,
    };
    // Cordis rows can mount services that later register model-facing tools in
    // dsh, but static row metadata is not a safe executable OpenBitFun provider
    // candidate. This runtime-free adapter therefore exposes no dispatch target.
    Ok((Arc::new(adapter), Vec::new()))
}

fn source_identity_matches(left: &PluginSourceRef, right: &PluginSourceRef) -> bool {
    left.plugin_id == right.plugin_id
        && left.source_kind == right.source_kind
        && left.source == right.source
        && left.version == right.version
        && left.content_hash == right.content_hash
}

enum DshProjection {
    Entry(DshEntryProjection),
    Invalid(DshInvalidProjection),
}

impl DshProjection {
    fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid(_))
    }

    fn activate_supported_source(&mut self) {
        if let Self::Entry(projection) = self {
            projection.source.trust_level = PluginTrustLevel::Trusted;
        }
    }

    fn source_ref(&self) -> &PluginSourceRef {
        match self {
            Self::Entry(projection) => projection.source_ref(),
            Self::Invalid(projection) => projection.source_ref(),
        }
    }

    fn read_diagnostics(&self) -> Vec<PluginDiagnostic> {
        match self {
            Self::Entry(projection) => projection.read_diagnostics(),
            Self::Invalid(projection) => projection.read_diagnostics(),
        }
    }

    fn status_snapshot(
        &self,
        include_config_validation: bool,
        diagnostic_ids: Vec<String>,
    ) -> PluginStatusSnapshot {
        match self {
            Self::Entry(projection) => {
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
            Self::Entry(projection) => projection.project_dispatch_response(envelope),
            Self::Invalid(projection) => projection.project_dispatch_response(envelope),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DshEntryKind {
    Bundle,
    ProfileBundle,
}

impl DshEntryKind {
    fn diagnostic_code(self) -> &'static str {
        match self {
            Self::Bundle => "dsh.bundle_entry_projection_only",
            Self::ProfileBundle => "dsh.profile_bundle_projection_only",
        }
    }

    fn diagnostic_message(self) -> &'static str {
        match self {
            Self::Bundle => {
                "dsh bundle entry is discovered from cordis.patch.yml but is not installed or executed by OpenBitFun"
            }
            Self::ProfileBundle => {
                "dsh profile bundle reference is discovered from package.json but is not installed or executed by OpenBitFun"
            }
        }
    }

    fn schema_version(self) -> &'static str {
        match self {
            Self::Bundle => DSH_BUNDLE_SCHEMA_VERSION,
            Self::ProfileBundle => DSH_PROFILE_SCHEMA_VERSION,
        }
    }
}

struct DshEntryProjection {
    entry_id: String,
    source: PluginSourceRef,
    manifest: PluginManifestRef,
    entry_kind: DshEntryKind,
    observed_at_ms: u64,
}

impl DshEntryProjection {
    #[allow(clippy::too_many_arguments)]
    fn new(
        entry_id: String,
        source_uri: String,
        manifest_path: String,
        entry_kind: DshEntryKind,
        version: &str,
        content_hash: &str,
        observed_at_ms: u64,
    ) -> Self {
        let plugin_id = stable_plugin_id(
            "dsh.entry",
            &sanitize_plugin_id_component(&entry_id),
            &source_uri,
        );
        let manifest = PluginManifestRef {
            manifest_id: format!("{plugin_id}:{}", entry_kind.schema_version()),
            schema_version: entry_kind.schema_version().to_string(),
            path: Some(manifest_path),
        };
        Self {
            entry_id,
            source: PluginSourceRef {
                plugin_id: plugin_id.clone(),
                source_kind: PluginSourceKind::DeepSeekHarnessCompatible,
                source: source_uri,
                version: Some(version.to_string()),
                content_hash: content_hash.to_string(),
                trust_level: PluginTrustLevel::Unknown,
                manifest: Some(manifest.clone()),
            },
            manifest,
            entry_kind,
            observed_at_ms,
        }
    }

    fn source_ref(&self) -> &PluginSourceRef {
        &self.source
    }

    fn read_diagnostics(&self) -> Vec<PluginDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.source.trust_level != PluginTrustLevel::Trusted {
            diagnostics.push(self.trust_diagnostic());
        }
        diagnostics.push(self.entry_diagnostic());
        diagnostics
    }

    fn status_snapshot(
        &self,
        include_config_validation: bool,
        diagnostic_ids: Vec<String>,
    ) -> PluginStatusSnapshot {
        let (availability, status) = Self::trust_status_for_level(self.source.trust_level);
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

    fn project_dispatch_response(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        if envelope.source.plugin_id != self.source.plugin_id {
            return Err(PortError::new(
                PortErrorKind::NotFound,
                format!(
                    "dsh source {} is not loaded by this adapter",
                    envelope.source.plugin_id
                ),
            ));
        }
        let diagnostics = self.dispatch_diagnostics(&envelope);
        let diagnostic_ids = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.diagnostic_id.clone())
            .collect();
        let (availability, status) = Self::trust_status_for_level(self.source.trust_level);

        Ok(PluginResponseEnvelope {
            envelope_version: envelope.envelope_version,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: DSH_ADAPTER_ID.to_string(),
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: self.observed_at_ms,
            effects: Vec::new(),
            diagnostics,
            quarantine: None,
            plugin_statuses: vec![PluginStatusSnapshot {
                source: envelope.source.clone(),
                status,
                availability,
                config_validation: None,
                quarantine: None,
                diagnostic_ids,
                updated_at_ms: self.observed_at_ms,
            }],
            observed_epochs: envelope.epochs,
        })
    }

    fn dispatch_diagnostics(&self, envelope: &PluginDispatchEnvelope) -> Vec<PluginDiagnostic> {
        let mut entry = self.entry_diagnostic();
        entry.diagnostic_id = format!(
            "diag:{}:dispatch:{}:entry",
            self.source.plugin_id, envelope.event_id
        );
        entry.source = envelope.source.clone();
        entry.audit = audit_ref(envelope);

        if self.source.trust_level == PluginTrustLevel::Trusted {
            return vec![entry];
        }

        let mut trust = self.trust_diagnostic();
        trust.diagnostic_id = format!(
            "diag:{}:dispatch:{}:trust",
            self.source.plugin_id, envelope.event_id
        );
        trust.source = envelope.source.clone();
        trust.audit = audit_ref(envelope);
        vec![trust, entry]
    }

    fn trust_diagnostic(&self) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:trust", self.source.plugin_id),
            severity: PluginDiagnosticSeverity::Warning,
            source: self.source.clone(),
            code: "dsh.trust_required".to_string(),
            message: "dsh bundle entry is not trusted for projection".to_string(),
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

    fn entry_diagnostic(&self) -> PluginDiagnostic {
        PluginDiagnostic {
            diagnostic_id: format!("diag:{}:entry:{}", self.source.plugin_id, self.entry_id),
            severity: PluginDiagnosticSeverity::Info,
            source: self.source.clone(),
            code: self.entry_kind.diagnostic_code().to_string(),
            message: format!(
                "{}: {}",
                self.entry_kind.diagnostic_message(),
                self.entry_id
            ),
            detail: PluginDiagnosticDetail::Manifest {
                manifest: self.manifest.clone(),
            },
            audit: PluginAuditRef {
                correlation_id: format!("config:{}", self.source.plugin_id),
                event_id: None,
            },
            retryable: false,
        }
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
}

struct DshInvalidProjection {
    source: PluginSourceRef,
    validation: PluginConfigValidationState,
    diagnostic_code: String,
    diagnostic_message: String,
    diagnostic_detail_manifest: PluginManifestRef,
    observed_at_ms: u64,
}

impl DshInvalidProjection {
    fn invalid(
        source_uri: &str,
        package_source: &PluginPackageSourceIdentity,
        code: &str,
        field: &str,
        message: String,
        observed_at_ms: u64,
    ) -> Self {
        let plugin_id = stable_plugin_id(
            "dsh.package",
            &sanitize_plugin_id_component(code),
            &format!("{source_uri}#{code}"),
        );
        let manifest = PluginManifestRef {
            manifest_id: format!("{plugin_id}:{DSH_PACKAGE_SCHEMA_VERSION}"),
            schema_version: DSH_PACKAGE_SCHEMA_VERSION.to_string(),
            path: Some(source_uri.to_string()),
        };
        Self {
            source: PluginSourceRef {
                plugin_id,
                source_kind: PluginSourceKind::DeepSeekHarnessCompatible,
                source: source_uri.to_string(),
                version: Some(package_source.version.clone()),
                content_hash: package_source.content_hash.clone(),
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
                    "dsh source {} is not loaded by this adapter",
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
            adapter_id: DSH_ADAPTER_ID.to_string(),
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

#[derive(Debug, Deserialize)]
struct DshPackageDoc {
    #[serde(default)]
    dsh: Option<DshPackageField>,
}

#[derive(Debug, Deserialize)]
struct DshPackageField {
    #[serde(default)]
    bundle: Option<DshBundleDecl>,
    #[serde(default)]
    profile: Option<DshProfileDecl>,
}

#[derive(Debug, Deserialize)]
struct DshBundleDecl {
    #[serde(default)]
    patch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DshProfileDecl {
    #[serde(default)]
    bundles: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DshPatchOperation {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    insert: Option<Vec<DshCordisEntry>>,
    #[serde(default)]
    group: Option<bool>,
    #[serde(default)]
    config: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
struct DshCordisEntry {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    group: Option<bool>,
    #[serde(default)]
    config: Option<serde_yaml::Value>,
}

fn project_package(
    files: &BTreeMap<String, Vec<u8>>,
    provenance_id: &str,
    source: &PluginPackageSourceIdentity,
    package_uri: &str,
    package_json_uri: &str,
    observed_at_ms: u64,
) -> PortResult<Vec<DshProjection>> {
    let mut projections = Vec::new();

    let Some(package_doc) = read_package_doc(
        files,
        source,
        package_json_uri,
        observed_at_ms,
        &mut projections,
    ) else {
        return Ok(projections);
    };

    let Some(field) = package_doc.dsh.as_ref() else {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            package_json_uri,
            source,
            "dsh.package_no_dsh_field",
            "dsh",
            "package.json declares no dsh bundle or profile".to_string(),
            observed_at_ms,
        )));
        return Ok(projections);
    };

    let mut declared_role = false;
    if let Some(bundle) = field.bundle.as_ref() {
        declared_role = true;
        project_bundle(
            bundle,
            files,
            provenance_id,
            source,
            package_json_uri,
            observed_at_ms,
            &mut projections,
        )?;
    }
    if let Some(profile) = field.profile.as_ref() {
        declared_role = true;
        project_profile(
            profile,
            source,
            package_uri,
            package_json_uri,
            observed_at_ms,
            &mut projections,
        )?;
    }
    if !declared_role {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            package_json_uri,
            source,
            "dsh.package_no_supported_role",
            "dsh",
            "dsh declaration has no supported bundle or profile role".to_string(),
            observed_at_ms,
        )));
    }

    Ok(projections)
}

fn read_package_doc(
    files: &BTreeMap<String, Vec<u8>>,
    source: &PluginPackageSourceIdentity,
    package_json_uri: &str,
    observed_at_ms: u64,
    projections: &mut Vec<DshProjection>,
) -> Option<DshPackageDoc> {
    let Some(bytes) = files.get(PACKAGE_JSON) else {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            package_json_uri,
            source,
            "dsh.package_json_missing",
            "package.json",
            "managed package has no package.json".to_string(),
            observed_at_ms,
        )));
        return None;
    };
    let json = match std::str::from_utf8(bytes) {
        Ok(json) => json,
        Err(error) => {
            projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
                package_json_uri,
                source,
                "dsh.package_json_invalid",
                "package.json",
                format!("package.json must be UTF-8: {error}"),
                observed_at_ms,
            )));
            return None;
        }
    };
    match serde_json::from_str::<DshPackageDoc>(json) {
        Ok(doc) => Some(doc),
        Err(error) => {
            projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
                package_json_uri,
                source,
                "dsh.package_json_invalid",
                "package.json",
                error.to_string(),
                observed_at_ms,
            )));
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn project_bundle(
    bundle: &DshBundleDecl,
    files: &BTreeMap<String, Vec<u8>>,
    provenance_id: &str,
    source: &PluginPackageSourceIdentity,
    package_json_uri: &str,
    observed_at_ms: u64,
    projections: &mut Vec<DshProjection>,
) -> PortResult<()> {
    let Some(declared_patch) = bundle.patch.as_deref().filter(|patch| !patch.is_empty()) else {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            package_json_uri,
            source,
            "dsh.bundle_patch_missing",
            "dsh.bundle.patch",
            "dsh bundle declaration must name its patch file".to_string(),
            observed_at_ms,
        )));
        return Ok(());
    };
    let Some(patch_rel) = normalize_relative_path(declared_patch) else {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            package_json_uri,
            source,
            "dsh.bundle_patch_invalid",
            "dsh.bundle.patch",
            "dsh bundle patch must resolve to a file inside the managed package".to_string(),
            observed_at_ms,
        )));
        return Ok(());
    };
    let patch_uri = managed_source_uri(provenance_id, &source.package_id, &patch_rel);

    let Some(bytes) = files.get(&patch_rel) else {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            package_json_uri,
            source,
            "dsh.bundle_patch_missing",
            "dsh.bundle.patch",
            format!("declared bundle patch file is not part of the package: {patch_rel}"),
            observed_at_ms,
        )));
        return Ok(());
    };

    let patch_yaml = match std::str::from_utf8(bytes) {
        Ok(yaml) => yaml,
        Err(error) => {
            projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
                package_json_uri,
                source,
                "dsh.bundle_patch_invalid",
                "dsh.bundle.patch",
                format!("bundle patch must be UTF-8: {error}"),
                observed_at_ms,
            )));
            return Ok(());
        }
    };

    if patch_yaml.trim().is_empty() {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            &patch_uri,
            source,
            "dsh.bundle_no_entries",
            "cordis.patch.yml",
            "bundle patch declares no cordis entries".to_string(),
            observed_at_ms,
        )));
        return Ok(());
    }

    let entries = match serde_yaml::from_str::<Vec<DshPatchOperation>>(patch_yaml) {
        Ok(entries) => entries,
        Err(error) => {
            projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
                &patch_uri,
                source,
                "dsh.bundle_patch_invalid",
                "cordis.patch.yml",
                format!("bundle patch must be a YAML list: {error}"),
                observed_at_ms,
            )));
            return Ok(());
        }
    };

    let mut entry_ids = Vec::new();
    let mut seen = HashSet::new();
    let mut visited_entries = 0usize;
    let mut unprojectable_entries = 0usize;
    collect_entry_ids(
        &entries,
        &mut entry_ids,
        &mut seen,
        &mut visited_entries,
        &mut unprojectable_entries,
    );
    if visited_entries > MAX_ENTRIES_PER_PACKAGE {
        return Err(adapter_port_error(format!(
            "managed package declares more than {MAX_ENTRIES_PER_PACKAGE} dsh bundle entries"
        )));
    }
    if unprojectable_entries > 0 {
        let invalid_uri = format!("{patch_uri}#unprojectable-entry");
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            &invalid_uri,
            source,
            "dsh.bundle_entry_invalid",
            "cordis.patch.yml",
            format!(
                "bundle patch contains {unprojectable_entries} Cordis entry or group value(s) without a valid stable identity"
            ),
            observed_at_ms,
        )));
    }
    if entry_ids.is_empty() {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            &patch_uri,
            source,
            "dsh.bundle_no_entries",
            "cordis.patch.yml",
            "bundle patch declares no cordis entries".to_string(),
            observed_at_ms,
        )));
        return Ok(());
    }

    for entry_id in entry_ids {
        let source_uri = format!("{patch_uri}#entry={}", urlencoding::encode(&entry_id));
        projections.push(DshProjection::Entry(DshEntryProjection::new(
            entry_id,
            source_uri,
            patch_uri.clone(),
            DshEntryKind::Bundle,
            &source.version,
            &source.content_hash,
            observed_at_ms,
        )));
    }
    Ok(())
}

fn project_profile(
    profile: &DshProfileDecl,
    source: &PluginPackageSourceIdentity,
    package_uri: &str,
    package_json_uri: &str,
    observed_at_ms: u64,
    projections: &mut Vec<DshProjection>,
) -> PortResult<()> {
    if profile.bundles.len() > MAX_PROFILE_BUNDLES {
        return Err(adapter_port_error(format!(
            "managed package declares more than {MAX_PROFILE_BUNDLES} dsh profile bundles"
        )));
    }
    if profile.bundles.is_empty() {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            package_json_uri,
            source,
            "dsh.profile_no_bundles",
            "dsh.profile.bundles",
            "dsh profile declares no bundles".to_string(),
            observed_at_ms,
        )));
        return Ok(());
    }

    let mut projected = 0usize;
    for (index, name) in profile.bundles.iter().enumerate() {
        if name.is_empty()
            || name.trim() != name
            || name.len() > MAX_PROFILE_BUNDLE_NAME_BYTES
            || name.chars().any(|ch| ch.is_control() || ch.is_whitespace())
        {
            let invalid_uri = format!("{package_json_uri}#bundle-index={index}");
            projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
                &invalid_uri,
                source,
                "dsh.profile_bundle_invalid",
                &format!("dsh.profile.bundles[{index}]"),
                "dsh profile bundle name must be a non-empty package name without whitespace"
                    .to_string(),
                observed_at_ms,
            )));
            continue;
        }
        projected += 1;
        let source_uri = format!(
            "{package_uri}#bundle-index={index}&bundle={}",
            urlencoding::encode(name)
        );
        projections.push(DshProjection::Entry(DshEntryProjection::new(
            name.to_string(),
            source_uri,
            package_json_uri.to_string(),
            DshEntryKind::ProfileBundle,
            &source.version,
            &source.content_hash,
            observed_at_ms,
        )));
    }

    if projected == 0 {
        projections.push(DshProjection::Invalid(DshInvalidProjection::invalid(
            package_json_uri,
            source,
            "dsh.profile_no_bundles",
            "dsh.profile.bundles",
            "dsh profile declares no valid bundle names".to_string(),
            observed_at_ms,
        )));
    }
    Ok(())
}

fn collect_entry_ids(
    entries: &[DshPatchOperation],
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
    visited_entries: &mut usize,
    unprojectable_entries: &mut usize,
) {
    for operation in entries {
        *visited_entries = (*visited_entries).saturating_add(1);
        if *visited_entries > MAX_ENTRIES_PER_PACKAGE {
            return;
        }
        if let Some(id) = operation.id.as_deref() {
            collect_stable_entry_id(id, out, seen, unprojectable_entries);
        }
        if let Some(insert) = operation.insert.as_deref() {
            collect_cordis_entries(insert, out, seen, visited_entries, unprojectable_entries);
            if *visited_entries > MAX_ENTRIES_PER_PACKAGE {
                return;
            }
        }
        collect_group_entries(
            operation.group,
            operation.config.as_ref(),
            out,
            seen,
            visited_entries,
            unprojectable_entries,
        );
    }
}

fn collect_cordis_entries(
    entries: &[DshCordisEntry],
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
    visited_entries: &mut usize,
    unprojectable_entries: &mut usize,
) {
    for entry in entries {
        *visited_entries = (*visited_entries).saturating_add(1);
        if *visited_entries > MAX_ENTRIES_PER_PACKAGE {
            return;
        }
        match entry.id.as_deref() {
            Some(id) => collect_stable_entry_id(id, out, seen, unprojectable_entries),
            None => *unprojectable_entries = (*unprojectable_entries).saturating_add(1),
        }
        collect_group_entries(
            entry.group,
            entry.config.as_ref(),
            out,
            seen,
            visited_entries,
            unprojectable_entries,
        );
    }
}

fn collect_group_entries(
    group: Option<bool>,
    config: Option<&serde_yaml::Value>,
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
    visited_entries: &mut usize,
    unprojectable_entries: &mut usize,
) {
    if group != Some(true) {
        return;
    }
    let Some(serde_yaml::Value::Sequence(values)) = config else {
        if config.is_some() {
            *unprojectable_entries = (*unprojectable_entries).saturating_add(1);
        }
        return;
    };
    let mut entries = Vec::with_capacity(values.len());
    for value in values {
        match serde_yaml::from_value::<DshCordisEntry>(value.clone()) {
            Ok(entry) => entries.push(entry),
            Err(_) => *unprojectable_entries = (*unprojectable_entries).saturating_add(1),
        }
    }
    collect_cordis_entries(&entries, out, seen, visited_entries, unprojectable_entries);
}

fn collect_stable_entry_id(
    id: &str,
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
    unprojectable_entries: &mut usize,
) {
    if id.is_empty() || id.len() > MAX_ENTRY_ID_BYTES || id.chars().any(char::is_control) {
        *unprojectable_entries = (*unprojectable_entries).saturating_add(1);
        return;
    }
    if seen.insert(id.to_string()) {
        out.push(id.to_string());
    }
}

fn normalize_relative_path(path: &str) -> Option<String> {
    if path.starts_with('/') || path.contains('\\') {
        return None;
    }

    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            _ if segment.contains(':') || segment.chars().any(char::is_control) => return None,
            _ => segments.push(segment),
        }
    }
    (!segments.is_empty()).then(|| segments.join("/"))
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

fn managed_source_uri(provenance_id: &str, package_id: &str, relative_path: &str) -> String {
    let encoded_path = relative_path
        .split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect::<Vec<_>>()
        .join("/");
    format!(
        "openbitfun://managed-plugins/{provenance_id}/{}/{encoded_path}",
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
