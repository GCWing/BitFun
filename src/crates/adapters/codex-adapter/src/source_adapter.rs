//! Codex Plugin Host Adapter.
//!
//! Standalone `PluginHostAdapter` for Codex `.codex-plugin/plugin.json` sources.
//! Implements the projection model like opencode-adapter.

use async_trait::async_trait;
use bitfun_plugin_runtime_host::PluginHostAdapter;
use bitfun_runtime_ports::{
    PluginAuditRef, PluginDiagnostic, PluginDiagnosticDetail, PluginDiagnosticSeverity,
    PluginDispatchEnvelope, PluginResponseEnvelope, PluginRuntimeAvailability,
    PluginRuntimeReadRequest, PluginRuntimeReadResponse, PluginRuntimeUnavailableReason,
    PluginSourceKind, PluginSourceRef,
    PluginStatusKind, PluginStatusSnapshot, PluginTrustLevel, PortResult,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::discovery;

const ADAPTER_ID: &str = "codex-compatible";

// ============================================================================
// Projection
// ============================================================================

struct CodexProjection {
    source: PluginSourceRef,
    plugin_name: String,
    load_diagnostics: Vec<PluginDiagnostic>,
    has_hooks: bool,
    has_mcp: bool,
    skill_roots: Vec<PathBuf>,
}

impl CodexProjection {
    fn read_diagnostics(&self) -> Vec<PluginDiagnostic> {
        self.load_diagnostics.clone()
    }

    fn status_snapshot(&self) -> PluginStatusSnapshot {
        PluginStatusSnapshot {
            source: self.source.clone(),
            status: PluginStatusKind::Enabled,
            availability: PluginRuntimeAvailability::projection_only(
                bitfun_runtime_ports::PluginRuntimeUnavailableReason::NotBuilt,
            ),
            config_validation: None,
            quarantine: None,
            diagnostic_ids: Vec::new(),
            updated_at_ms: 0,
        }
    }

    fn dispatch_response(&self, envelope: &PluginDispatchEnvelope) -> PluginResponseEnvelope {
        let mut diagnostics = Vec::new();

        if !super::event_map::CODEX_EVENT_NAMES.contains(&envelope.extension_point_id.as_str()) {
            diagnostics.push(PluginDiagnostic {
                diagnostic_id: format!("codex.unsupported.{}", envelope.extension_point_id),
                severity: PluginDiagnosticSeverity::Info,
                source: self.source.clone(),
                code: "codex.event_unsupported".to_string(),
                message: format!("event '{}' not supported", envelope.extension_point_id),
                detail: PluginDiagnosticDetail::Adapter {
                    adapter_id: ADAPTER_ID.to_string(),
                },
                audit: PluginAuditRef {
                    correlation_id: envelope.correlation_id.clone(),
                    event_id: Some(envelope.event_id.clone()),
                },
                retryable: false,
            });
        }

        PluginResponseEnvelope {
            envelope_version: 1,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: ADAPTER_ID.to_string(),
            plugin_id: Some(self.source.plugin_id.clone()),
            completed_at_ms: 0,
            effects: Vec::new(),
            diagnostics,
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs.clone(),
        }
    }
}

// ============================================================================
// Adapter
// ============================================================================

pub struct CodexPluginHostAdapter {
    projections: Vec<CodexProjection>,
}

#[async_trait]
impl PluginHostAdapter for CodexPluginHostAdapter {
    fn adapter_id(&self) -> &str {
        ADAPTER_ID
    }

    async fn read_plugins(
        &self,
        request: PluginRuntimeReadRequest,
    ) -> PortResult<PluginRuntimeReadResponse> {
        let mut sources = Vec::new();
        let mut plugin_statuses = Vec::new();
        let mut diagnostics = Vec::new();

        for p in self
            .projections
            .iter()
            .filter(|p| request.plugin_ids.is_empty() || request.plugin_ids.contains(&p.source.plugin_id))
        {
            let ds = p.read_diagnostics();
            sources.push(p.source.clone());
            plugin_statuses.push(p.status_snapshot());
            diagnostics.extend(ds);
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
        for p in &self.projections {
            if p.source.plugin_id == envelope.source.plugin_id {
                return Ok(p.dispatch_response(&envelope));
            }
        }
        Ok(PluginResponseEnvelope {
            envelope_version: 1,
            request_event_id: envelope.event_id.clone(),
            project_domain_id: envelope.project_domain_id.clone(),
            workspace_id: envelope.workspace_id.clone(),
            adapter_id: ADAPTER_ID.to_string(),
            plugin_id: Some(envelope.source.plugin_id.clone()),
            completed_at_ms: 0,
            effects: Vec::new(),
            diagnostics: vec![PluginDiagnostic {
                diagnostic_id: "codex.source_mismatch".to_string(),
                severity: PluginDiagnosticSeverity::Info,
                source: envelope.source.clone(),
                code: "codex.source_mismatch".to_string(),
                message: format!("no projection for '{}'", envelope.source.plugin_id),
                detail: PluginDiagnosticDetail::Adapter {
                    adapter_id: ADAPTER_ID.to_string(),
                },
                audit: PluginAuditRef {
                    correlation_id: envelope.correlation_id.clone(),
                    event_id: Some(envelope.event_id.clone()),
                },
                retryable: false,
            }],
            quarantine: None,
            plugin_statuses: Vec::new(),
            observed_epochs: envelope.epochs.clone(),
        })
    }
}

// ============================================================================
// Factory
// ============================================================================

fn compute_content_hash(plugin_id: &str) -> String {
    let mut h = Sha256::new();
    h.update(plugin_id.as_bytes());
    h.update(b":codex-plugin:v1");
    format!("sha256:{:x}", h.finalize())
}

pub fn load_codex_compatible_adapter(
    project_root: impl AsRef<Path>,
    _observed_at_ms: u64,
    _source_trust_epoch: u64,
    source_trust_refs: &[PluginSourceRef],
) -> PortResult<Arc<dyn PluginHostAdapter>> {
    use std::collections::HashMap;

    let project = project_root.as_ref();
    let trust: HashMap<String, PluginTrustLevel> = source_trust_refs
        .iter()
        .map(|r| (r.plugin_id.clone(), r.trust_level))
        .collect();

    let discoveries = discovery::discover_all(Some(project));
    let mut projections = Vec::new();

    for d in &discoveries {
        match discovery::load_plugin_manifest(d) {
            Ok(plugin) => {
                let tl = trust
                    .get(&plugin.plugin_id)
                    .copied()
                    .unwrap_or(PluginTrustLevel::Unknown);
                projections.push(CodexProjection {
                    source: PluginSourceRef {
                        plugin_id: plugin.plugin_id.clone(),
                        source_kind: PluginSourceKind::OpenCodeCompatible,
                        source: plugin.root.to_string_lossy().to_string(),
                        version: plugin.version.clone(),
                        content_hash: compute_content_hash(&plugin.plugin_id),
                        trust_level: tl,
                        manifest: None,
                    },
                    load_diagnostics: Vec::new(),
                    plugin_name: plugin.name.clone(),
                    has_hooks: !plugin.hook_paths.is_empty() || plugin.hooks_inline.is_some(),
                    has_mcp: plugin.mcp_servers.is_some(),
                    skill_roots: plugin.skill_roots.clone(),
                });
            }
            Err(e) => log::warn!(
                "Codex adapter: failed to load {}: {e}",
                d.manifest_path.display()
            ),
        }
    }

    Ok(Arc::new(CodexPluginHostAdapter { projections }))
}
