//! Codex plugin integration — assembly layer.
//!
//! Bridges the codex-adapter output into BitFun's product subsystems
//! (SkillRegistry) through the PluginRuntimeHost trust chain.
//!
//! Flow: discover → build trust refs → load adapter → PluginRuntimeHost →
//! read_plugins → register only Trusted skill roots in SkillRegistry.

// hooks_executor is gated behind cfg(test) until production lifecycle wiring
// is complete. See review finding: dead code without production callers.
#[cfg(test)]
mod hooks_executor;

use crate::agentic::tools::implementations::skills::registry::SkillRegistry;
use bitfun_codex_adapter::load_codex_compatible_adapter;
use bitfun_plugin_runtime_host::PluginRuntimeHost;
use bitfun_runtime_ports::{
    PluginRuntimeReadRequest, PluginRuntimeEpochs, PluginSourceKind,
    PluginSourceRef, PluginTrustLevel,
};
use log::{info, warn};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Initializes plugin support (OpenCode + Codex).
///
/// Called once during agentic system startup. Discovers Codex plugins from the
/// filesystem, routes them through the PluginRuntimeHost trust chain, and
/// registers only Trusted plugin skills in the SkillRegistry.
///
/// Untrusted, Denied, or Revoked plugin sources are reported via diagnostics
/// but their skills are never loaded into the agent context.
pub async fn initialize_plugin_support(workspace_root: Option<&Path>) {
    info!("Initializing plugin support (Codex) via PluginRuntimeHost...");

    // ── Phase 1: discover available plugins ────────────────────────────────
    let discoveries = bitfun_codex_adapter::discovery::discover_all(workspace_root);
    let discovered_plugins: Vec<_> = discoveries
        .iter()
        .filter_map(|d| bitfun_codex_adapter::discovery::load_plugin_manifest(d).ok())
        .collect();

    if discovered_plugins.is_empty() {
        info!("No Codex plugins discovered — plugin support idle.");
        return;
    }

    // ── Phase 2: build trust refs — local filesystem plugins are Trusted ────
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // For local filesystem plugins, we trust them by default.
    // A future managed source/config layer may override this with
    // Denied/Revoked entries for specific paths.
    let trust_refs: Vec<PluginSourceRef> = discovered_plugins
        .iter()
        .map(|p| PluginSourceRef {
            plugin_id: p.plugin_id.clone(),
            source_kind: PluginSourceKind::OpenCodeCompatible,
            source: p.root.to_string_lossy().to_string(),
            version: p.version.clone(),
            content_hash: String::new(), // trust refs match by full identity
            trust_level: PluginTrustLevel::Trusted,
            manifest: None,
        })
        .collect();

    let trust_epoch = now_ms;

    // ── Phase 3: load adapter through PluginRuntimeHost ────────────────────
    let project_root = workspace_root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let adapter = match load_codex_compatible_adapter(
        &project_root,
        now_ms,
        trust_epoch,
        &trust_refs,
    ) {
        Ok(a) => a,
        Err(e) => {
            warn!("Failed to load Codex adapter: {e}");
            return;
        }
    };

    let host: Arc<dyn bitfun_runtime_ports::PluginRuntimeClient> =
        Arc::new(PluginRuntimeHost::new(adapter));

    // ── Phase 4: read plugins through the host ─────────────────────────────
    let epochs = PluginRuntimeEpochs {
        project_epoch: 0,
        trust_epoch,
        policy_epoch: 0,
        tool_registry_epoch: None,
    };

    let read_result = host
        .read_plugins(PluginRuntimeReadRequest {
            request_id: "codex-init".to_string(),
            project_domain_id: String::new(),
            workspace_id: workspace_root
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            plugin_ids: Vec::new(),
            epochs: epochs.clone(),
            include_config_validation: false,
        })
        .await;

    let response = match read_result {
        Ok(r) => r,
        Err(e) => {
            warn!("PluginRuntimeHost::read_plugins failed: {e}");
            return;
        }
    };

    // Log diagnostics for visibility (non-blocking).
    for d in &response.diagnostics {
        warn!(
            "Codex plugin diagnostic [{}]: {}",
            d.code, d.message
        );
    }

    // ── Phase 5: register only Trusted plugin skills ───────────────────────
    let registry = SkillRegistry::global();
    let mut trusted_count = 0u32;

    for source in &response.sources {
        if source.trust_level != PluginTrustLevel::Trusted {
            info!(
                "Codex plugin '{}' is {:?} — skills gated.",
                source.plugin_id, source.trust_level
            );
            continue;
        }

        // Map the source back to the plugin to extract skill roots.
        if let Some(plugin) = discovered_plugins
            .iter()
            .find(|p| p.plugin_id == source.plugin_id)
        {
            info!(
                "Codex plugin '{}' v{} (trusted) — {} skill roots",
                plugin.name,
                plugin.version.as_deref().unwrap_or("?"),
                plugin.skill_roots.len()
            );

            for root in &plugin.skill_roots {
                registry
                    .add_plugin_skill_root(root.clone(), &plugin.plugin_id, workspace_root)
                    .await;
                info!("  registered skill root: {}", root.display());
            }
            trusted_count += 1;
        }
    }

    if trusted_count > 0 {
        registry.refresh().await;
        let all_skills = registry.get_all_skills_for_workspace(workspace_root).await;
        let plugin_skill_names: Vec<&str> = all_skills
            .iter()
            .filter(|s| s.source_slot == "plugin")
            .map(|s| s.name.as_str())
            .collect();
        info!(
            "Plugin skills registered: {:?} ({} total)",
            plugin_skill_names,
            plugin_skill_names.len()
        );
    }

    info!(
        "Plugin support initialized: {} trusted codex plugin(s), {} total discovered",
        trusted_count,
        discovered_plugins.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Build a minimal Codex plugin in a tempdir for reproducible testing.
    fn build_test_plugin(dir: &Path, name: &str, skill_name: &str, skill_content: &str) {
        let plugin_dir = dir.join(name);
        let codex_dir = plugin_dir.join(".codex-plugin");
        fs::create_dir_all(&codex_dir).unwrap();

        let manifest = format!(
            r#"{{
                "name": "{name}",
                "version": "1.0.0",
                "skills": ["./skills/"]
            }}"#
        );
        fs::write(codex_dir.join("plugin.json"), manifest).unwrap();

        let skills_dir = plugin_dir.join("skills").join(skill_name);
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("SKILL.md"), skill_content).unwrap();
    }

    /// Creates plugins in a workspace-like tempdir.
    fn setup_workspace_with_plugins() -> (TempDir, PathBuf) {
        let workspace = TempDir::new().unwrap();
        let plugins_dir = workspace.path().join(".agents").join("plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        build_test_plugin(
            &plugins_dir,
            "test-plugin-a",
            "hello-skill",
            "---\nname: hello-skill\ndescription: A test hello skill\n---\n\n# Hello Skill\n\nSay hello.",
        );

        build_test_plugin(
            &plugins_dir,
            "test-plugin-b",
            "tool-skill",
            "---\nname: tool-skill\ndescription: A test tool skill\n---\n\n# Tool Skill\n\nTool usage.",
        );

        let ws_root = workspace.path().to_path_buf();
        (workspace, ws_root)
    }

    #[tokio::test]
    async fn test_e2e_plugin_discovery_and_skill_registration() {
        let (_workspace, ws_root) = setup_workspace_with_plugins();

        // Discover plugins from the workspace's .agents/plugins/
        let discoveries = bitfun_codex_adapter::discovery::discover_all(Some(&ws_root));
        let plugins: Vec<_> = discoveries
            .iter()
            .filter_map(|d| bitfun_codex_adapter::discovery::load_plugin_manifest(d).ok())
            .collect();

        eprintln!("=== E2E Test: Discovered {} plugin(s) ===", plugins.len());
        for p in &plugins {
            eprintln!(
                "  {} v{} — skills at: {:?}",
                p.name,
                p.version.as_deref().unwrap_or("?"),
                p.skill_roots
            );
        }

        assert!(
            !plugins.is_empty(),
            "Should discover at least one test plugin"
        );

        // Compute content_hash the same way the adapter does.
        fn content_hash_for(root: &Path, version: &Option<String>) -> String {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(root.to_string_lossy().as_bytes());
            h.update(b":");
            if let Some(v) = version {
                h.update(v.as_bytes());
            } else {
                h.update(b"0.0.0");
            }
            h.update(b":codex-plugin:v1");
            format!("sha256:{:x}", h.finalize())
        }

        // Build trust refs with correct content_hash (Trusted for local plugins).
        let trust_refs: Vec<PluginSourceRef> = plugins
            .iter()
            .map(|p| PluginSourceRef {
                plugin_id: p.plugin_id.clone(),
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: p.root.to_string_lossy().to_string(),
                version: p.version.clone(),
                content_hash: content_hash_for(&p.root, &p.version),
                trust_level: PluginTrustLevel::Trusted,
                manifest: None,
            })
            .collect();

        // Load through adapter.
        let adapter = load_codex_compatible_adapter(
            &ws_root,
            1_000_000,
            1_000_000,
            &trust_refs,
        )
        .expect("adapter should load");

        let host: Arc<dyn bitfun_runtime_ports::PluginRuntimeClient> =
            Arc::new(PluginRuntimeHost::new(adapter));

        let read_resp = host
            .read_plugins(PluginRuntimeReadRequest {
                request_id: "test-init".to_string(),
                project_domain_id: String::new(),
                workspace_id: String::new(),
                plugin_ids: Vec::new(),
                epochs: PluginRuntimeEpochs {
                    project_epoch: 0,
                    trust_epoch: 1_000_000,
                    policy_epoch: 0,
                    tool_registry_epoch: None,
                },
                include_config_validation: false,
            })
            .await
            .expect("read_plugins should succeed");

        let trusted: Vec<_> = read_resp
            .sources
            .iter()
            .filter(|s| s.trust_level == PluginTrustLevel::Trusted)
            .collect();
        assert!(!trusted.is_empty(), "All plugins should be Trusted");

        // Register trusted plugin skill roots into registry.
        let registry = SkillRegistry::global();
        for plugin in &plugins {
            for root in &plugin.skill_roots {
                registry
                    .add_plugin_skill_root(root.clone(), &plugin.plugin_id, Some(&ws_root))
                    .await;
            }
        }
        registry.refresh().await;
        let all_skills = registry.get_all_skills_for_workspace(Some(&ws_root)).await;
        let plugin_skills: Vec<&str> = all_skills
            .iter()
            .filter(|s| s.source_slot == "plugin")
            .map(|s| s.name.as_str())
            .collect();

        eprintln!("=== Plugin skills in registry: {:?} ===", plugin_skills);
        assert!(
            plugin_skills.contains(&"hello-skill"),
            "hello-skill should be registered. Got: {:?}",
            plugin_skills
        );

        // Clean up workspace plugin roots.
        registry.remove_plugin_skill_roots_for_workspace(Some(&ws_root)).await;
    }

    #[tokio::test]
    async fn test_denied_plugin_skills_are_gated() {
        let (_workspace, ws_root) = setup_workspace_with_plugins();

        let discoveries = bitfun_codex_adapter::discovery::discover_all(Some(&ws_root));
        let plugins: Vec<_> = discoveries
            .iter()
            .filter_map(|d| bitfun_codex_adapter::discovery::load_plugin_manifest(d).ok())
            .collect();
        assert!(!plugins.is_empty());

        fn content_hash_for(root: &Path, version: &Option<String>) -> String {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(root.to_string_lossy().as_bytes());
            h.update(b":");
            if let Some(v) = version {
                h.update(v.as_bytes());
            } else {
                h.update(b"0.0.0");
            }
            h.update(b":codex-plugin:v1");
            format!("sha256:{:x}", h.finalize())
        }

        // All refs as Denied → no skills should be registered.
        let denied_refs: Vec<PluginSourceRef> = plugins
            .iter()
            .map(|p| PluginSourceRef {
                plugin_id: p.plugin_id.clone(),
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: p.root.to_string_lossy().to_string(),
                version: p.version.clone(),
                content_hash: content_hash_for(&p.root, &p.version),
                trust_level: PluginTrustLevel::Denied,
                manifest: None,
            })
            .collect();

        let adapter = load_codex_compatible_adapter(
            &ws_root,
            2_000_000,
            2_000_000,
            &denied_refs,
        )
        .expect("adapter should load");

        let host: Arc<dyn bitfun_runtime_ports::PluginRuntimeClient> =
            Arc::new(PluginRuntimeHost::new(adapter));

        let read_resp = host
            .read_plugins(PluginRuntimeReadRequest {
                request_id: "test-deny".to_string(),
                project_domain_id: String::new(),
                workspace_id: String::new(),
                plugin_ids: Vec::new(),
                epochs: PluginRuntimeEpochs {
                    project_epoch: 0,
                    trust_epoch: 2_000_000,
                    policy_epoch: 0,
                    tool_registry_epoch: None,
                },
                include_config_validation: false,
            })
            .await
            .expect("read_plugins should succeed");

        let trusted: Vec<_> = read_resp
            .sources
            .iter()
            .filter(|s| s.trust_level == PluginTrustLevel::Trusted)
            .collect();
        assert!(trusted.is_empty(), "No plugin should be Trusted when all are Denied");
    }

    #[tokio::test]
    async fn test_epoch_stale_dispatch_is_rejected() {
        let (_workspace, ws_root) = setup_workspace_with_plugins();

        let discoveries = bitfun_codex_adapter::discovery::discover_all(Some(&ws_root));
        let plugins: Vec<_> = discoveries
            .iter()
            .filter_map(|d| bitfun_codex_adapter::discovery::load_plugin_manifest(d).ok())
            .collect();
        assert!(!plugins.is_empty());

        fn content_hash_for(root: &Path, version: &Option<String>) -> String {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(root.to_string_lossy().as_bytes());
            h.update(b":");
            if let Some(v) = version {
                h.update(v.as_bytes());
            } else {
                h.update(b"0.0.0");
            }
            h.update(b":codex-plugin:v1");
            format!("sha256:{:x}", h.finalize())
        }

        let trust_refs: Vec<PluginSourceRef> = plugins
            .iter()
            .map(|p| PluginSourceRef {
                plugin_id: p.plugin_id.clone(),
                source_kind: PluginSourceKind::OpenCodeCompatible,
                source: p.root.to_string_lossy().to_string(),
                version: p.version.clone(),
                content_hash: content_hash_for(&p.root, &p.version),
                trust_level: PluginTrustLevel::Trusted,
                manifest: None,
            })
            .collect();

        // Adapter created with epoch 3_000_000.
        let adapter = load_codex_compatible_adapter(
            &ws_root,
            3_000_000,
            3_000_000,
            &trust_refs,
        )
        .expect("adapter should load");

        let host: Arc<dyn bitfun_runtime_ports::PluginRuntimeClient> =
            Arc::new(PluginRuntimeHost::new(adapter));

        // Dispatch with a mismatched trust_epoch.
        let first_source = &plugins[0];
        let response = host
            .dispatch(bitfun_runtime_ports::PluginDispatchEnvelope {
                envelope_version: 1,
                event_id: "test-dispatch".to_string(),
                event_type: "hook".to_string(),
                event_version: "1".to_string(),
                project_domain_id: "test-project".to_string(),
                workspace_id: "test-workspace".to_string(),
                source: PluginSourceRef {
                    plugin_id: first_source.plugin_id.clone(),
                    source_kind: PluginSourceKind::OpenCodeCompatible,
                    source: first_source.root.to_string_lossy().to_string(),
                    version: first_source.version.clone(),
                    content_hash: content_hash_for(&first_source.root, &first_source.version),
                    trust_level: PluginTrustLevel::Trusted,
                    manifest: None,
                },
                extension_point_id: "SessionStart".to_string(),
                declared_capability: bitfun_runtime_ports::PluginCapabilityRef {
                    capability_id: "test.capability".to_string(),
                    owner: bitfun_runtime_ports::PluginOwnerRef {
                        kind: bitfun_runtime_ports::PluginOwnerKind::ProductFeature,
                        id: "test-owner".to_string(),
                    },
                },
                correlation_id: "test-corr".to_string(),
                causation_id: None,
                idempotency_key: "test-idem".to_string(),
                deadline_ms: 10_000,
                epochs: PluginRuntimeEpochs {
                    project_epoch: 0,
                    trust_epoch: 9_999_999, // mismatched!
                    policy_epoch: 0,
                    tool_registry_epoch: None,
                },
                payload_ref: None,
            })
            .await
            .expect("dispatch should not error");

        assert!(
            response
                .diagnostics
                .iter()
                .any(|d| d.code == "codex.trust_epoch_stale"),
            "Mismatched epoch should produce trust_epoch_stale diagnostic. Got: {:?}",
            response.diagnostics
        );
    }

    #[tokio::test]
    async fn test_workspace_isolation() {
        let registry = SkillRegistry::global();

        // Register under workspace A.
        let binding_a = PathBuf::from("/ws/a");
        let ws_a = Some(binding_a.as_path());
        registry
            .add_plugin_skill_root(
                PathBuf::from("/ws/a/plugin/skills"),
                "test.a@codex-local",
                ws_a,
            )
            .await;

        // Register under workspace B.
        let binding_b = PathBuf::from("/ws/b");
        let ws_b = Some(binding_b.as_path());
        registry
            .add_plugin_skill_root(
                PathBuf::from("/ws/b/plugin/skills"),
                "test.b@codex-local",
                ws_b,
            )
            .await;

        // Verify removal for a specific workspace.
        registry.remove_plugin_skill_roots_for_workspace(ws_a).await;

        // Workspace B's roots should still exist (not affected).
        {
            let roots = registry.plugin_skill_roots.read().await;
            assert!(
                !roots.contains_key(&ws_a.map(|p| p.to_path_buf())),
                "Workspace A roots should be removed"
            );
            assert!(
                roots.contains_key(&ws_b.map(|p| p.to_path_buf())),
                "Workspace B roots should still exist"
            );
        }

        // Clean up workspace B too.
        registry.remove_plugin_skill_roots_for_workspace(ws_b).await;
    }

    #[test]
    fn test_hooks_executor_echo() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Note: hooks_executor is #[cfg(test)], so this module can access it.
            let handler = super::hooks_executor::HookHandler {
                event_name: "tool.execute.before".to_string(),
                command: if cfg!(windows) {
                    "cmd /c echo continue:true".to_string()
                } else {
                    "echo continue:true".to_string()
                },
                timeout_secs: 10,
            };
            let result =
                super::hooks_executor::execute_hook(&handler, "{}")
                    .await;
            match result {
                super::hooks_executor::HookResult::Success {
                    stdout, ..
                } => {
                    eprintln!("Hook stdout: '{}'", stdout.trim());
                    assert!(
                        stdout.contains("continue:true"),
                        "Hook should output 'continue:true'. Got: '{}'",
                        stdout
                    );
                }
                other => panic!("Expected Success, got: {:?}", other),
            }
        });
    }
}
