//! Codex plugin integration — assembly layer.
//!
//! Bridges the codex-adapter output (PluginEffectCandidate metadata) into
//! BitFun's product subsystems: SkillRegistry, MCPService, and hooks lifecycle.
//!
//! This module is thin: it consumes the adapter's read-only plugin metadata
//! and wires it into the product runtime. Plugin execution (hooks commands)
//! happens here, not in the adapter.

// hooks_executor is gated behind cfg(test) until production lifecycle wiring
// is complete. See deep-review finding: dead code without production callers.
#[cfg(test)]
mod hooks_executor;

use crate::agentic::tools::implementations::skills::registry::SkillRegistry;
use log::info;
use std::path::Path;

/// Initializes plugin support (OpenCode + Codex).
///
/// Called once during agentic system startup. Discovers plugins from
/// `~/.agents/plugins/` and registers their skills with SkillRegistry.
pub async fn initialize_plugin_support(workspace_root: Option<&Path>) {
    info!("Initializing plugin support (OpenCode + Codex)...");

    // Delegate plugin discovery to the codex-adapter's sanctioned public API.
    // Sync filesystem I/O is intentionally on the caller's async context here;
    // callers should wrap this in spawn_blocking when on a latency-sensitive path.
    let discoveries = bitfun_codex_adapter::discovery::discover_all(workspace_root);
    let codex_plugins: Vec<_> = discoveries
        .iter()
        .filter_map(|d| bitfun_codex_adapter::discovery::load_plugin_manifest(d).ok())
        .collect();
    let registry = SkillRegistry::global();

    for plugin in &codex_plugins {
        info!(
            "Plugin: {} v{} — {} skill roots, {} hook files",
            plugin.name,
            plugin.version.as_deref().unwrap_or("?"),
            plugin.skill_roots.len(),
            plugin.hook_paths.len()
        );

        for root in &plugin.skill_roots {
            registry
                .add_plugin_skill_root(root.clone(), &plugin.plugin_id)
                .await;
            info!("  registered skill root: {}", root.display());
        }
    }

    if !codex_plugins.is_empty() {
        registry.refresh().await;
        let all_skills = registry.get_all_skills_for_workspace(workspace_root).await;
        let plugin_skill_names: Vec<&str> = all_skills
            .iter()
            .filter(|s| s.source_slot == "plugin")
            .map(|s| s.name.as_str())
            .collect();
        info!(
            "Plugin skills registered: {:?} ({} total skills from plugins)",
            plugin_skill_names,
            plugin_skill_names.len()
        );
    }

    info!(
        "Plugin support initialized: {} Codex plugin(s) discovered",
        codex_plugins.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_e2e_plugin_discovery_and_skill_registration() {
        // Full end-to-end: discover plugins from disk → register skills → verify in registry.
        let registry = SkillRegistry::global();

        // Discover plugins via the adapter's public API
        let discoveries = bitfun_codex_adapter::discovery::discover_all(None);
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

        // Should find at least test-codex-plugin and codex-tools-plugin
        let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"test-codex-plugin"),
            "Should find test-codex-plugin. Found: {:?}", names
        );
        assert!(
            names.contains(&"codex-tools-plugin"),
            "Should find codex-tools-plugin. Found: {:?}", names
        );

        // Register their skill roots
        for plugin in &plugins {
            for root in &plugin.skill_roots {
                registry.add_plugin_skill_root(root.clone(), &plugin.plugin_id).await;
            }
        }

        // Refresh and get all skills
        registry.refresh().await;
        let all_skills = registry.get_all_skills_for_workspace(None).await;

        // Plugin skills should be present
        let plugin_skills: Vec<&str> = all_skills
            .iter()
            .filter(|s| s.source_slot == "plugin")
            .map(|s| s.name.as_str())
            .collect();
        eprintln!("=== E2E Test: Plugin skills in registry: {:?} ===", plugin_skills);

        assert!(
            plugin_skills.contains(&"hello-skill"),
            "hello-skill should be in registry. Plugin skills: {:?}", plugin_skills
        );
        assert!(
            plugin_skills.contains(&"tool-logger"),
            "tool-logger should be in registry. Plugin skills: {:?}", plugin_skills
        );
    }

    #[tokio::test]
    async fn test_e2e_superpowers_plugin_skills() {
        let registry = SkillRegistry::global();
        let discoveries = bitfun_codex_adapter::discovery::discover_all(None);
        let plugins: Vec<_> = discoveries
            .iter()
            .filter_map(|d| bitfun_codex_adapter::discovery::load_plugin_manifest(d).ok())
            .collect();

        let superpowers = plugins.iter().find(|p| p.name == "superpowers");
        if superpowers.is_none() {
            eprintln!("=== E2E Test: superpowers plugin not installed, skipping ===");
            return;
        }
        let sp = superpowers.unwrap();

        // Register superpowers skill roots
        for root in &sp.skill_roots {
            registry.add_plugin_skill_root(root.clone(), &sp.plugin_id).await;
        }
        registry.refresh().await;

        let all_skills = registry.get_all_skills_for_workspace(None).await;
        let plugin_skills: Vec<&str> = all_skills
            .iter()
            .filter(|s| s.source_slot == "plugin")
            .map(|s| s.name.as_str())
            .collect();

        eprintln!("=== E2E Test: Superpowers skills: {:?} ===", plugin_skills);

        // Must have key superpowers skills
        for expected in &[
            "brainstorming",
            "test-driven-development",
            "writing-plans",
            "executing-plans",
            "subagent-driven-development",
        ] {
            assert!(
                plugin_skills.contains(expected),
                "superpowers should include '{}'. Plugin skills: {:?}",
                expected,
                plugin_skills
            );
        }
    }

    #[test]
    fn test_hooks_executor_echo() {
        // Verify hooks_executor can execute a basic echo command.
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let handler = hooks_executor::HookHandler {
                event_name: "tool.execute.before".to_string(),
                command: if cfg!(windows) {
                    "cmd /c echo continue:true".to_string()
                } else {
                    "echo continue:true".to_string()
                },
                timeout_secs: 10,
            };
            let result = hooks_executor::execute_hook(&handler, "{}").await;
            match result {
                hooks_executor::HookResult::Success { stdout, .. } => {
                    eprintln!("Hook stdout: '{}'", stdout.trim());
                    assert!(
                        stdout.contains("continue:true"),
                        "Hook should output 'continue:true'. Got: '{}'", stdout
                    );
                }
                other => panic!("Expected Success, got: {:?}", other),
            }
        });
    }
}
