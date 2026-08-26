//! Managed plugin composition.
//!
//! This is the only plugin-runtime root that selects managed-package ecosystem
//! adapters and PluginRuntimeClient. Workspace identity here scopes source,
//! approval, and logical client state; it is not a process ownership boundary.
//! This module projects candidates for product surfaces; it does not register
//! tools or execute plugin code.

use async_trait::async_trait;
use bitfun_dsh_adapter::load_dsh_package_adapter;
use bitfun_opencode_adapter::load_opencode_package_adapter;
use bitfun_plugin_runtime_client::{DefaultPluginRuntimeClient, PluginRuntimeAdapter};
use bitfun_product_domains::plugin_source::{
    PluginActivationAuthority, PluginPackageInput, PluginPackageSourceIdentity,
};
use bitfun_runtime_ports::{
    PluginCapabilityRef, PluginDispatchEnvelope, PluginEffectCandidatePayload,
    PluginPermissionGate, PluginResponseEnvelope, PluginRiskLevel, PluginRuntimeAvailability,
    PluginRuntimeBinding, PluginRuntimeClient, PluginRuntimeEpochs, PluginRuntimeReadRequest,
    PluginRuntimeReadResponse, PluginSourceRef, PluginStatusKind, PluginTargetRef, PortError,
    PortErrorKind, PortResult,
};
use bitfun_services_integrations::plugin_source::{
    ManagedPluginSourceError, ManagedPluginSourceIssue, ManagedPluginSourceService,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(feature = "opencode-plugin-host")]
pub(crate) fn opencode_config_snapshot(
    workspace: &Path,
) -> Result<bitfun_opencode_adapter::OpenCodeConfigSnapshot, String> {
    bitfun_opencode_adapter::load_opencode_config_snapshot(workspace)
        .map_err(|error| error.to_string())
}

const PREVIEW_PROJECT_ID: &str = "managed-plugin-preview";
const PREVIEW_WORKSPACE_ID: &str = "managed-plugin-preview";
const DSH_MANIFEST_ADAPTER_ID: &str = "dsh_compatible";
const OPENCODE_MANIFEST_ADAPTER_ID: &str = "opencode_compatible";
type PluginDispatchTarget = (
    PluginSourceRef,
    String,
    PluginCapabilityRef,
    Vec<(PluginTargetRef, PluginRiskLevel)>,
);

fn load_package_adapter(
    input: PluginPackageInput,
    activation: Option<PluginActivationAuthority>,
    observed_at_ms: u64,
) -> Result<(Arc<dyn PluginRuntimeAdapter>, Vec<PluginDispatchTarget>), ManagedPluginSourceError> {
    let (adapter, package_id) = {
        let (manifest, source, _files) = input.clone().into_parts();
        (manifest.adapter, source.package_id)
    };
    match adapter.as_str() {
        OPENCODE_MANIFEST_ADAPTER_ID => {
            load_opencode_package_adapter(input, activation, observed_at_ms)
                .map_err(|error| invalid_package(&package_id, error.to_string()))
        }
        DSH_MANIFEST_ADAPTER_ID => load_dsh_package_adapter(input, activation, observed_at_ms)
            .map_err(|error| invalid_package(&package_id, error.to_string())),
        other => Err(invalid_package(
            &package_id,
            format!("unsupported managed package adapter: {other}"),
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedPluginCandidateView {
    pub entry_id: String,
    pub target: String,
    pub risk_level: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedPluginActivationView {
    pub package_id: String,
    pub version: String,
    pub adapter: String,
    pub content_hash: String,
    pub activated: bool,
    pub activation_epoch: Option<u64>,
    pub entry_ids: Vec<String>,
    pub provider_candidates_supported: bool,
    pub permission_required: bool,
    pub candidates: Vec<ManagedPluginCandidateView>,
    pub diagnostics: Vec<String>,
}

struct ManagedPluginPreview {
    view: ManagedPluginActivationView,
    activation_config_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedPluginDeactivationResult {
    Deactivated {
        package_id: String,
        diagnostics: Vec<ManagedPluginSourceIssue>,
    },
    ResidualActivationCleared {
        package_id: String,
        current_package_available: Option<bool>,
        diagnostics: Vec<ManagedPluginSourceIssue>,
    },
    AlreadyInactive {
        package_id: String,
        current_package_available: Option<bool>,
        diagnostics: Vec<ManagedPluginSourceIssue>,
    },
}

pub async fn preview_managed_plugin_activation(
    workspace: &Path,
    package_id: &str,
) -> Result<ManagedPluginActivationView, ManagedPluginSourceError> {
    let service = Arc::new(crate::plugin_source::managed_plugin_source_service(
        workspace,
    )?);
    preview_with_service(service, workspace, package_id).await
}

pub async fn activate_managed_plugin(
    workspace: &Path,
    package_id: &str,
    expected_content_hash: Option<&str>,
) -> Result<ManagedPluginActivationView, ManagedPluginSourceError> {
    let service = Arc::new(crate::plugin_source::managed_plugin_source_service(
        workspace,
    )?);
    activate_with_service(service, workspace, package_id, expected_content_hash).await
}

pub async fn deactivate_managed_plugin(
    workspace: &Path,
    package_id: &str,
) -> Result<ManagedPluginDeactivationResult, ManagedPluginSourceError> {
    let service = Arc::new(crate::plugin_source::managed_plugin_source_service(
        workspace,
    )?);
    deactivate_with_service(service, workspace, package_id).await
}

async fn preview_with_service(
    service: Arc<ManagedPluginSourceService>,
    workspace: &Path,
    package_id: &str,
) -> Result<ManagedPluginActivationView, ManagedPluginSourceError> {
    Ok(project_preview_with_service(service, workspace, package_id)
        .await?
        .view)
}

async fn project_preview_with_service(
    service: Arc<ManagedPluginSourceService>,
    workspace: &Path,
    package_id: &str,
) -> Result<ManagedPluginPreview, ManagedPluginSourceError> {
    let input = service.load_package(workspace, package_id).await?;
    let source = input.clone().into_parts().1;
    let (adapter, dispatch_targets) = load_package_adapter(input, None, current_time_ms())?;
    let binding = PluginRuntimeBinding::client(Arc::new(DefaultPluginRuntimeClient::new(adapter)));
    let response = binding
        .as_client()
        .read_plugins(read_request(PREVIEW_PROJECT_ID, PREVIEW_WORKSPACE_ID, 1))
        .await
        .map_err(|error| unavailable(package_id, error.to_string()))?;
    let activation_config_valid = response
        .plugin_statuses
        .iter()
        .all(|status| status.status != PluginStatusKind::InvalidConfig);
    let candidates = preview_candidates(&dispatch_targets);
    Ok(ManagedPluginPreview {
        view: project_view(
            source,
            false,
            None,
            !dispatch_targets.is_empty(),
            response,
            candidates,
        ),
        activation_config_valid,
    })
}

async fn activate_with_service(
    service: Arc<ManagedPluginSourceService>,
    workspace: &Path,
    package_id: &str,
    expected_content_hash: Option<&str>,
) -> Result<ManagedPluginActivationView, ManagedPluginSourceError> {
    let expected_content_hash = expected_content_hash.ok_or_else(|| {
        invalid_package(
            package_id,
            "activation requires the exact content hash from the preview".to_string(),
        )
    })?;
    let ManagedPluginPreview {
        view: preview,
        activation_config_valid,
    } = project_preview_with_service(Arc::clone(&service), workspace, package_id).await?;
    if preview.content_hash != expected_content_hash {
        return Err(invalid_package(
            package_id,
            "activation confirmation does not match the current package content".to_string(),
        ));
    }
    if preview.adapter == DSH_MANIFEST_ADAPTER_ID && !activation_config_valid {
        return Err(invalid_package(
            package_id,
            "the package projection contains invalid configuration".to_string(),
        ));
    }
    if preview.adapter == OPENCODE_MANIFEST_ADAPTER_ID && !preview.provider_candidates_supported {
        return Err(invalid_package(
            package_id,
            "the package contains no supported OpenCode custom tool declaration".to_string(),
        ));
    }

    let (activation, activation_changed) = service
        .activate(workspace, package_id, Some(expected_content_hash))
        .await?;
    let activation_epoch = activation.activation_epoch.ok_or_else(|| {
        unavailable(
            package_id,
            "activation state did not provide a generation".to_string(),
        )
    })?;
    let activation_diagnostics = activation
        .issues
        .into_iter()
        .map(|issue| issue.message)
        .collect::<Vec<_>>();

    let projection = async {
        let (input, authority) = service
            .load_activated_package(workspace, package_id)
            .await?;
        project_activated(
            Arc::clone(&service),
            workspace,
            package_id,
            input,
            authority,
            activation_diagnostics,
        )
        .await
    }
    .await;
    match projection {
        Ok(view) => Ok(view),
        Err(error) if activation_changed => {
            let rollback = service
                .deactivate(workspace, package_id, Some(activation_epoch))
                .await;
            match rollback {
                Ok(_) => Err(error),
                Err(rollback_error) => Err(unavailable(
                    package_id,
                    format!("{error}; activation rollback failed: {rollback_error}"),
                )),
            }
        }
        Err(error) => Err(error),
    }
}

async fn deactivate_with_service(
    service: Arc<ManagedPluginSourceService>,
    workspace: &Path,
    package_id: &str,
) -> Result<ManagedPluginDeactivationResult, ManagedPluginSourceError> {
    let (snapshot, changed, cleared_source_available) =
        service.deactivate(workspace, package_id, None).await?;
    let current_package_available = snapshot.discovery_complete.then(|| {
        snapshot
            .packages
            .iter()
            .any(|package| package.package_id == package_id)
    });
    let diagnostics = snapshot.issues;
    if changed && cleared_source_available == Some(true) {
        Ok(ManagedPluginDeactivationResult::Deactivated {
            package_id: package_id.to_string(),
            diagnostics,
        })
    } else if changed {
        Ok(ManagedPluginDeactivationResult::ResidualActivationCleared {
            package_id: package_id.to_string(),
            current_package_available,
            diagnostics,
        })
    } else {
        Ok(ManagedPluginDeactivationResult::AlreadyInactive {
            package_id: package_id.to_string(),
            current_package_available,
            diagnostics,
        })
    }
}

async fn project_activated(
    service: Arc<ManagedPluginSourceService>,
    workspace: &Path,
    package_id: &str,
    input: PluginPackageInput,
    authority: PluginActivationAuthority,
    initial_diagnostics: Vec<String>,
) -> Result<ManagedPluginActivationView, ManagedPluginSourceError> {
    let (project_domain_id, workspace_id, source, activation_epoch) =
        authority.clone().into_parts();
    let (binding, dispatch_targets) =
        activated_binding(service, workspace, package_id, input, authority)?;
    let provider_candidates_supported = !dispatch_targets.is_empty();
    let client = binding.as_client();
    let read = client
        .read_plugins(read_request(
            &project_domain_id,
            &workspace_id,
            activation_epoch,
        ))
        .await
        .map_err(|error| unavailable(package_id, error.to_string()))?;
    let mut diagnostics = initial_diagnostics;
    diagnostics.extend(
        read.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone()),
    );
    let mut candidates = Vec::new();
    for (index, (plugin_source, extension_point_id, declared_capability, _)) in
        dispatch_targets.into_iter().enumerate()
    {
        let response = client
            .dispatch(dispatch_envelope(
                plugin_source,
                extension_point_id,
                declared_capability,
                &project_domain_id,
                &workspace_id,
                activation_epoch,
                index,
            ))
            .await
            .map_err(|error| unavailable(package_id, error.to_string()))?;
        diagnostics.extend(
            response
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone()),
        );
        candidates.extend(response.effects.iter().filter_map(project_candidate));
    }
    diagnostics.sort();
    diagnostics.dedup();
    Ok(project_view(
        source,
        true,
        Some(activation_epoch),
        provider_candidates_supported,
        read,
        candidates,
    )
    .with_diagnostics(diagnostics))
}

fn preview_candidates(
    dispatch_targets: &[PluginDispatchTarget],
) -> Vec<ManagedPluginCandidateView> {
    dispatch_targets
        .iter()
        .flat_map(|(source, _, _, targets)| {
            targets
                .iter()
                .map(|(target, risk_level)| ManagedPluginCandidateView {
                    entry_id: source.plugin_id.clone(),
                    target: target.display_name.clone(),
                    risk_level: risk_level_name(*risk_level).to_string(),
                })
        })
        .collect()
}

fn activated_binding(
    service: Arc<ManagedPluginSourceService>,
    workspace: &Path,
    package_id: &str,
    input: PluginPackageInput,
    authority: PluginActivationAuthority,
) -> Result<(PluginRuntimeBinding, Vec<PluginDispatchTarget>), ManagedPluginSourceError> {
    let (adapter, dispatch_targets) =
        load_package_adapter(input, Some(authority.clone()), current_time_ms())?;
    let client: Arc<dyn PluginRuntimeClient> = Arc::new(DefaultPluginRuntimeClient::new(adapter));
    Ok((
        PluginRuntimeBinding::client(Arc::new(ActivationGatedPluginRuntimeClient {
            inner: client,
            service,
            workspace: workspace.to_path_buf(),
            package_id: package_id.to_string(),
            authority,
        })),
        dispatch_targets,
    ))
}

fn project_view(
    source: PluginPackageSourceIdentity,
    activated: bool,
    activation_epoch: Option<u64>,
    provider_candidates_supported: bool,
    response: PluginRuntimeReadResponse,
    candidates: Vec<ManagedPluginCandidateView>,
) -> ManagedPluginActivationView {
    ManagedPluginActivationView {
        package_id: source.package_id,
        version: source.version,
        adapter: source.adapter,
        content_hash: source.content_hash,
        activated,
        activation_epoch,
        entry_ids: response
            .sources
            .iter()
            .map(|source| source.plugin_id.clone())
            .collect(),
        provider_candidates_supported,
        permission_required: provider_candidates_supported,
        candidates,
        diagnostics: response
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    }
}

impl ManagedPluginActivationView {
    fn with_diagnostics(mut self, diagnostics: Vec<String>) -> Self {
        self.diagnostics = diagnostics;
        self
    }
}

fn project_candidate(
    effect: &bitfun_runtime_ports::PluginEffectCandidate,
) -> Option<ManagedPluginCandidateView> {
    let PluginPermissionGate::PermissionRequired { .. } = &effect.permission else {
        return None;
    };
    let PluginEffectCandidatePayload::ProviderCandidate { .. } = &effect.payload else {
        return None;
    };
    Some(ManagedPluginCandidateView {
        entry_id: effect.source_ref.plugin_id.clone(),
        target: effect.target_ref.display_name.clone(),
        risk_level: risk_level_name(effect.risk_level).to_string(),
    })
}

fn read_request(
    project_domain_id: &str,
    workspace_id: &str,
    trust_epoch: u64,
) -> PluginRuntimeReadRequest {
    PluginRuntimeReadRequest {
        request_id: "managed-plugin-read".to_string(),
        project_domain_id: project_domain_id.to_string(),
        workspace_id: workspace_id.to_string(),
        plugin_ids: Vec::new(),
        include_config_validation: true,
        epochs: runtime_epochs(trust_epoch),
    }
}

fn dispatch_envelope(
    source: PluginSourceRef,
    extension_point_id: String,
    declared_capability: PluginCapabilityRef,
    project_domain_id: &str,
    workspace_id: &str,
    trust_epoch: u64,
    index: usize,
) -> PluginDispatchEnvelope {
    PluginDispatchEnvelope {
        envelope_version: 1,
        event_id: format!("managed-plugin-candidate-{index}"),
        event_type: "plugin.activation.candidates.requested".to_string(),
        event_version: "v1".to_string(),
        project_domain_id: project_domain_id.to_string(),
        workspace_id: workspace_id.to_string(),
        extension_point_id,
        source,
        declared_capability,
        correlation_id: "managed-plugin-activation".to_string(),
        causation_id: None,
        idempotency_key: format!("managed-plugin-candidate-{index}"),
        deadline_ms: 30_000,
        epochs: runtime_epochs(trust_epoch),
        payload_ref: None,
    }
}

fn runtime_epochs(trust_epoch: u64) -> PluginRuntimeEpochs {
    PluginRuntimeEpochs {
        project_epoch: 0,
        trust_epoch,
        policy_epoch: 0,
        tool_registry_epoch: None,
    }
}

fn risk_level_name(risk: PluginRiskLevel) -> &'static str {
    match risk {
        PluginRiskLevel::Low => "low",
        PluginRiskLevel::Medium => "medium",
        PluginRiskLevel::High => "high",
        _ => "unknown",
    }
}

fn invalid_package(package_id: &str, diagnostic: String) -> ManagedPluginSourceError {
    ManagedPluginSourceError::PackageInvalid {
        package_id: package_id.to_string(),
        diagnostic,
    }
}

fn unavailable(package_id: &str, diagnostic: String) -> ManagedPluginSourceError {
    ManagedPluginSourceError::TemporarilyUnavailable {
        package_id: package_id.to_string(),
        diagnostic,
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

struct ActivationGatedPluginRuntimeClient {
    inner: Arc<dyn PluginRuntimeClient>,
    service: Arc<ManagedPluginSourceService>,
    workspace: PathBuf,
    package_id: String,
    authority: PluginActivationAuthority,
}

impl ActivationGatedPluginRuntimeClient {
    async fn check_authority(&self) -> PortResult<()> {
        match self
            .service
            .has_activation_authority(&self.workspace, &self.package_id, &self.authority)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(PortError::new(
                PortErrorKind::NotAvailable,
                "managed plugin activation is no longer current",
            )),
            Err(error) => Err(PortError::new(
                PortErrorKind::NotAvailable,
                error.to_string(),
            )),
        }
    }
}

#[async_trait]
impl PluginRuntimeClient for ActivationGatedPluginRuntimeClient {
    fn availability(&self) -> PluginRuntimeAvailability {
        self.inner.availability()
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        self.check_authority().await?;
        self.inner.read_plugins(request).await
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        let deadline = Duration::from_millis(envelope.deadline_ms);
        tokio::time::timeout(deadline, async {
            self.check_authority().await?;
            let response = self.inner.dispatch(envelope).await?;
            self.check_authority().await?;
            Ok(response)
        })
        .await
        .map_err(|_| {
            PortError::new(
                PortErrorKind::Timeout,
                "managed plugin dispatch exceeded its end-to-end deadline",
            )
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::plugin_source::{PluginPackageFile, PluginPackageManifest};
    use bitfun_services_integrations::plugin_source::ManagedPluginTrustDecision;
    use sha2::{Digest, Sha256};
    use std::fs;
    use tokio::sync::Notify;

    const PLUGIN_SOURCE: &str = r#"
import { type Plugin, tool } from "@opencode-ai/plugin"
export const WorkspaceToolsPlugin: Plugin = async () => ({
  tool: {
    workspaceSummary: tool({
      description: "Summarize the workspace",
      args: { topic: tool.schema.string() },
      async execute(args, context) { return `${context.directory}: ${args.topic}` },
    }),
  },
})
"#;

    struct Fixture {
        _temp: tempfile::TempDir,
        workspace: PathBuf,
        source_path: PathBuf,
        service: Arc<ManagedPluginSourceService>,
    }

    struct BlockingDispatchClient {
        inner: Arc<dyn PluginRuntimeClient>,
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    #[async_trait]
    impl PluginRuntimeClient for BlockingDispatchClient {
        fn availability(&self) -> PluginRuntimeAvailability {
            self.inner.availability()
        }

        async fn read_plugins(
            &self,
            request: PluginRuntimeReadRequest,
        ) -> PortResult<PluginRuntimeReadResponse> {
            self.inner.read_plugins(request).await
        }

        async fn dispatch(
            &self,
            envelope: PluginDispatchEnvelope,
        ) -> PortResult<PluginResponseEnvelope> {
            self.started.notify_one();
            self.release.notified().await;
            self.inner.dispatch(envelope).await
        }
    }

    impl Fixture {
        async fn new() -> Self {
            Self::with_source(PLUGIN_SOURCE).await
        }

        async fn with_source(plugin_source: &str) -> Self {
            Self::with_managed_package(
                "opencode_compatible",
                &[(".opencode/plugins/workspace-tools.ts", plugin_source)],
            )
            .await
        }

        async fn with_managed_package(adapter: &str, files: &[(&str, &str)]) -> Self {
            let temp = tempfile::tempdir().expect("tempdir");
            let workspace = temp.path().join("workspace");
            let user = temp.path().join("user");
            let package = workspace.join(".bitfun/plugins/acme.demo");
            let source_path = package.join(files[0].0);
            fs::create_dir_all(user.join("plugins")).expect("create user plugins");
            let mut manifest_files = Vec::new();
            for (relative_path, contents) in files {
                let path = package.join(relative_path);
                fs::create_dir_all(path.parent().expect("source parent")).expect("create package");
                fs::write(&path, contents).expect("write plugin source");
                manifest_files.push(serde_json::json!({
                    "path": relative_path,
                    "sha256": format!(
                        "sha256:{}",
                        hex::encode(Sha256::digest(contents.as_bytes()))
                    )
                }));
            }
            fs::write(
                package.join("bitfun.plugin.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "schemaVersion": 1,
                    "id": "acme.demo",
                    "version": "1.0.0",
                    "adapter": adapter,
                    "files": manifest_files
                }))
                .expect("serialize manifest"),
            )
            .expect("write manifest");
            let service = Arc::new(ManagedPluginSourceService::new(
                user.join("plugins"),
                user.clone(),
                workspace.join(".bitfun/plugins"),
                workspace.clone(),
                user.join("runtime/plugin-trust.json"),
            ));
            service
                .set_trust(
                    &workspace,
                    "acme.demo",
                    ManagedPluginTrustDecision::ApproveSource,
                )
                .await
                .expect("approve package source");
            Self {
                _temp: temp,
                workspace,
                source_path,
                service,
            }
        }

        async fn activate(&self) {
            let preview =
                preview_with_service(Arc::clone(&self.service), &self.workspace, "acme.demo")
                    .await
                    .expect("preview package");
            activate_with_service(
                Arc::clone(&self.service),
                &self.workspace,
                "acme.demo",
                Some(&preview.content_hash),
            )
            .await
            .expect("activate package");
        }
    }

    #[tokio::test]
    async fn preview_and_activation_project_only_permission_required_candidates() {
        let fixture = Fixture::new().await;
        let preview = preview_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("preview package");
        assert!(!preview.activated);
        assert!(!preview.candidates.is_empty());
        assert!(preview
            .candidates
            .iter()
            .all(|candidate| candidate.risk_level == "high"));
        assert!(preview.provider_candidates_supported);
        assert!(preview.permission_required);

        let activated = activate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
            Some(&preview.content_hash),
        )
        .await
        .expect("activate package");
        assert!(activated.activated);
        assert!(!activated.entry_ids.is_empty());
        assert!(!activated.candidates.is_empty());
        assert!(activated.permission_required);

        let deactivated = deactivate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("deactivate package");
        assert!(matches!(
            deactivated,
            ManagedPluginDeactivationResult::Deactivated { .. }
        ));
    }

    #[tokio::test]
    async fn deactivation_distinguishes_current_residual_and_inactive_states() {
        let fixture = Fixture::new().await;
        fixture.activate().await;

        let result = deactivate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("deactivate package");

        assert!(matches!(
            result,
            ManagedPluginDeactivationResult::Deactivated { package_id, .. }
                if package_id == "acme.demo"
        ));
        fixture.activate().await;
        fs::remove_dir_all(fixture.workspace.join(".bitfun/plugins")).expect("remove plugin root");
        fs::write(fixture.workspace.join(".bitfun/plugins"), "not a directory")
            .expect("make plugin root unreadable");

        let cleared = deactivate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("clear residual activation");
        assert!(matches!(
            cleared,
            ManagedPluginDeactivationResult::ResidualActivationCleared {
                current_package_available: None,
                ..
            }
        ));

        let repeated = deactivate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("repeat deactivation");
        assert!(matches!(
            repeated,
            ManagedPluginDeactivationResult::AlreadyInactive {
                current_package_available: None,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn activation_without_supported_custom_tools_does_not_persist_state() {
        let fixture = Fixture::with_source(
            r#"export const MetadataPlugin = async () => ({ config: async () => {} })"#,
        )
        .await;
        let preview = preview_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("preview unsupported package");
        assert!(!preview.provider_candidates_supported);

        let error = activate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
            Some(&preview.content_hash),
        )
        .await
        .expect_err("unsupported package must not activate");
        assert!(matches!(
            error,
            ManagedPluginSourceError::PackageInvalid { .. }
        ));
        let snapshot = fixture.service.refresh(&fixture.workspace).await;
        assert!(!snapshot.packages[0].activated);

        let inactive = deactivate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("keep package inactive");
        assert!(matches!(
            inactive,
            ManagedPluginDeactivationResult::AlreadyInactive { .. }
        ));
    }

    #[tokio::test]
    async fn existing_binding_fails_after_deactivation_or_source_change() {
        let fixture = Fixture::new().await;
        let first_content_hash = preview_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("preview package")
        .content_hash;
        fixture
            .service
            .activate(&fixture.workspace, "acme.demo", Some(&first_content_hash))
            .await
            .expect("activate package");
        let (input, authority) = fixture
            .service
            .load_activated_package(&fixture.workspace, "acme.demo")
            .await
            .expect("load activation authority");
        let (project_domain_id, workspace_id, _, activation_epoch) = authority.clone().into_parts();
        let (binding, _) = activated_binding(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
            input,
            authority,
        )
        .expect("create binding");
        binding
            .as_client()
            .read_plugins(read_request(
                &project_domain_id,
                &workspace_id,
                activation_epoch,
            ))
            .await
            .expect("binding is initially current");

        fixture
            .service
            .deactivate(&fixture.workspace, "acme.demo", None)
            .await
            .expect("deactivate package");
        assert!(binding
            .as_client()
            .read_plugins(read_request(
                &project_domain_id,
                &workspace_id,
                activation_epoch,
            ))
            .await
            .is_err());

        let current_content_hash = preview_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("preview package")
        .content_hash;
        fixture
            .service
            .activate(&fixture.workspace, "acme.demo", Some(&current_content_hash))
            .await
            .expect("reactivate package");
        let (current_input, current_authority) = fixture
            .service
            .load_activated_package(&fixture.workspace, "acme.demo")
            .await
            .expect("load current activation authority");
        let (project_domain_id, workspace_id, _, activation_epoch) =
            current_authority.clone().into_parts();
        let (current_binding, _) = activated_binding(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
            current_input,
            current_authority,
        )
        .expect("create current binding");
        fs::write(&fixture.source_path, "changed without manifest update")
            .expect("change package source");
        assert!(current_binding
            .as_client()
            .read_plugins(read_request(
                &project_domain_id,
                &workspace_id,
                activation_epoch,
            ))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn dispatch_rechecks_activation_after_adapter_returns() {
        let fixture = Fixture::new().await;
        let preview = preview_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("preview package");
        fixture
            .service
            .activate(&fixture.workspace, "acme.demo", Some(&preview.content_hash))
            .await
            .expect("activate package");
        let (input, authority) = fixture
            .service
            .load_activated_package(&fixture.workspace, "acme.demo")
            .await
            .expect("load activation authority");
        let (project_domain_id, workspace_id, _, activation_epoch) = authority.clone().into_parts();
        let (adapter, mut targets) =
            load_opencode_package_adapter(input, Some(authority.clone()), current_time_ms())
                .expect("create activated adapter");
        let (source, extension_point_id, capability, _) =
            targets.pop().expect("custom tool dispatch target");
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let inner: Arc<dyn PluginRuntimeClient> = Arc::new(BlockingDispatchClient {
            inner: Arc::new(DefaultPluginRuntimeClient::new(adapter)),
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        });
        let client: Arc<dyn PluginRuntimeClient> = Arc::new(ActivationGatedPluginRuntimeClient {
            inner,
            service: Arc::clone(&fixture.service),
            workspace: fixture.workspace.clone(),
            package_id: "acme.demo".to_string(),
            authority,
        });
        let dispatch = tokio::spawn({
            let client = Arc::clone(&client);
            async move {
                client
                    .dispatch(dispatch_envelope(
                        source,
                        extension_point_id,
                        capability,
                        &project_domain_id,
                        &workspace_id,
                        activation_epoch,
                        0,
                    ))
                    .await
            }
        });

        started.notified().await;
        fixture
            .service
            .deactivate(&fixture.workspace, "acme.demo", None)
            .await
            .expect("deactivate while dispatch is in flight");
        release.notify_one();

        let error = dispatch
            .await
            .expect("dispatch task")
            .expect_err("revoked dispatch result must be discarded");
        assert_eq!(error.kind, PortErrorKind::NotAvailable);
    }

    #[tokio::test]
    async fn dsh_packages_route_to_the_dsh_adapter() {
        let manifest = PluginPackageManifest {
            schema_version: 1,
            id: "acme.dsh".to_string(),
            version: "1.0.0".to_string(),
            adapter: "dsh_compatible".to_string(),
            files: vec![PluginPackageFile {
                path: "package.json".to_string(),
                sha256: format!(
                    "sha256:{}",
                    hex::encode(Sha256::digest(
                        br#"{ "dsh": { "profile": { "bundles": ["@acme/dsh-base"] } } }"#
                    ))
                ),
            }],
        };
        let source = PluginPackageSourceIdentity {
            package_id: "acme.dsh".to_string(),
            version: "1.0.0".to_string(),
            adapter: "dsh_compatible".to_string(),
            source_path: "/managed/acme.dsh".to_string(),
            content_hash: manifest.content_hash().expect("content hash"),
        };
        let files = std::collections::BTreeMap::from([(
            "package.json".to_string(),
            br#"{ "dsh": { "profile": { "bundles": ["@acme/dsh-base"] } } }"#.to_vec(),
        )]);
        let input = PluginPackageInput::new(manifest, source, files).expect("dsh input");

        let (adapter, dispatch_targets) =
            load_package_adapter(input, None, current_time_ms()).expect("route dsh adapter");
        assert_eq!(adapter.adapter_id(), "dsh-compatible");
        assert!(dispatch_targets.is_empty());
    }

    #[tokio::test]
    async fn valid_dsh_projection_can_activate_without_tool_candidates() {
        let fixture = Fixture::with_managed_package(
            "dsh_compatible",
            &[(
                "package.json",
                r#"{ "dsh": { "profile": { "bundles": ["@acme/dsh-base"] } } }"#,
            )],
        )
        .await;
        let preview = preview_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("preview dsh package");
        assert!(!preview.provider_candidates_supported);
        assert!(!preview.entry_ids.is_empty());

        let activated = activate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
            Some(&preview.content_hash),
        )
        .await
        .expect("activate valid dsh projection");
        assert!(activated.activated);
        assert!(!activated.provider_candidates_supported);
        assert!(!activated.permission_required);
    }

    #[tokio::test]
    async fn invalid_dsh_projection_does_not_persist_activation() {
        let fixture = Fixture::with_managed_package(
            "dsh_compatible",
            &[("package.json", r#"{ "name": "plain-package" }"#)],
        )
        .await;
        let preview = preview_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
        )
        .await
        .expect("preview invalid dsh package");
        let activation_epoch_before = fixture
            .service
            .refresh(&fixture.workspace)
            .await
            .activation_epoch;

        let error = activate_with_service(
            Arc::clone(&fixture.service),
            &fixture.workspace,
            "acme.demo",
            Some(&preview.content_hash),
        )
        .await
        .expect_err("invalid dsh package must not remain active");
        assert!(matches!(
            error,
            ManagedPluginSourceError::PackageInvalid { .. }
        ));
        let snapshot = fixture.service.refresh(&fixture.workspace).await;
        assert!(!snapshot.packages[0].activated);
        assert_eq!(snapshot.activation_epoch, activation_epoch_before);
    }

    #[tokio::test]
    async fn unsupported_managed_package_adapter_is_rejected() {
        let manifest = PluginPackageManifest {
            schema_version: 1,
            id: "acme.unknown".to_string(),
            version: "1.0.0".to_string(),
            adapter: "unknown_compatible".to_string(),
            files: vec![PluginPackageFile {
                path: "package.json".to_string(),
                sha256: format!("sha256:{}", hex::encode(Sha256::digest(b"{}"))),
            }],
        };
        let source = PluginPackageSourceIdentity {
            package_id: "acme.unknown".to_string(),
            version: "1.0.0".to_string(),
            adapter: "unknown_compatible".to_string(),
            source_path: "/managed/acme.unknown".to_string(),
            content_hash: manifest.content_hash().expect("content hash"),
        };
        let files =
            std::collections::BTreeMap::from([("package.json".to_string(), b"{}".to_vec())]);
        let input = PluginPackageInput::new(manifest, source, files).expect("unknown input");
        let error = load_package_adapter(input, None, current_time_ms())
            .err()
            .expect("unsupported adapter must fail");
        assert!(error
            .to_string()
            .contains("unsupported managed package adapter"));
    }
}
