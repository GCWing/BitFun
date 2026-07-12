//! Codex Plugin Host Adapter.
//!
//! Wraps the OpenCode adapter to additionally support Codex `.codex-plugin/plugin.json`
//! sources. Implements `PluginHostAdapter` and composes with the inner OpenCode adapter.
//!
//! # Design
//!
//! - `read_plugins()` delegates to the inner OpenCode adapter, then appends Codex
//!   plugin sources as `PluginSourceRef` entries with `OpenCodeCompatible` kind.
//! - `dispatch()` delegates entirely to the inner adapter. Codex hooks command
//!   execution happens in the assembly layer (per AGENTS.md "adapter
//!   must not execute plugins").
//! - The adapter reuses `adapter_id = "opencode-compatible"` — no new protocol ID.

use async_trait::async_trait;
use bitfun_plugin_runtime_host::PluginHostAdapter;
use bitfun_runtime_ports::{
    PluginDispatchEnvelope, PluginManifestRef, PluginResponseEnvelope,
    PluginRuntimeReadRequest, PluginRuntimeReadResponse, PluginSourceKind,
    PluginSourceRef, PluginTrustLevel, PortResult,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

use super::discovery;

const ADAPTER_ID: &str = "opencode-compatible";

/// The Codex plugin host adapter that wraps an inner OpenCode adapter.
pub struct CodexPluginHostAdapter {
    inner: Arc<dyn PluginHostAdapter>,
    codex_plugins: Vec<discovery::LoadedCodexPlugin>,
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
        let mut response = self.inner.read_plugins(request).await?;

        // Append Codex plugin sources as PluginSourceRef entries.
        for plugin in &self.codex_plugins {
            let content_hash = Self::compute_content_hash(&plugin.plugin_id);
            response.sources.push(PluginSourceRef {
                plugin_id: plugin.plugin_id.clone(),
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: plugin.root.to_string_lossy().to_string(),
                version: plugin.version.clone(),
                content_hash,
                trust_level: PluginTrustLevel::Unknown,
                manifest: Some(PluginManifestRef {
                    manifest_id: format!("codex:{}", plugin.name),
                    schema_version: "1.0.0".to_string(),
                    path: Some(plugin.root.to_string_lossy().to_string()),
                }),
            });
        }

        Ok(response)
    }

    async fn dispatch(
        &self,
        envelope: PluginDispatchEnvelope,
    ) -> PortResult<PluginResponseEnvelope> {
        self.inner.dispatch(envelope).await
    }
}

impl CodexPluginHostAdapter {
    fn compute_content_hash(plugin_id: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(plugin_id.as_bytes());
        hasher.update(b":codex-plugin");
        format!("sha256:{:x}", hasher.finalize())
    }
}

/// Public factory — the only public API entry point for this crate.
///
/// Creates an OpenCode adapter and wraps it with Codex plugin source support.
/// The returned adapter can be used with `PluginRuntimeHost`.
pub fn load_codex_workspace_adapter(
    project_root: impl AsRef<Path>,
    observed_at_ms: u64,
    source_trust_epoch: u64,
    source_trust_refs: &[PluginSourceRef],
) -> PortResult<Arc<dyn PluginHostAdapter>> {
    let inner = bitfun_opencode_adapter::load_opencode_workspace_adapter(
        &project_root,
        observed_at_ms,
        source_trust_epoch,
        source_trust_refs,
    )?;

    let project = project_root.as_ref();
    let discoveries = discovery::discover_all(Some(project));
    let mut codex_plugins = Vec::new();
    for d in &discoveries {
        match discovery::load_plugin_manifest(d) {
            Ok(plugin) => {
                log::info!(
                    "Codex adapter: loaded plugin '{}' v{}",
                    plugin.name,
                    plugin.version.as_deref().unwrap_or("?")
                );
                codex_plugins.push(plugin);
            }
            Err(e) => {
                log::warn!(
                    "Codex adapter: failed to load plugin from {}: {e}",
                    d.manifest_path.display()
                );
            }
        }
    }

    log::info!("Codex adapter: discovered {} Codex plugin(s)", codex_plugins.len());
    Ok(Arc::new(CodexPluginHostAdapter { inner, codex_plugins }))
}
