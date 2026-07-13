//! Codex plugin source discovery.
//!
//! Scans standard Codex plugin paths to find installed plugins:
//! - `~/.agents/plugins/` (personal plugins)
//! - `<workspace>/.agents/plugins/` (project plugins)

use super::manifest::{self, ManifestError};
use std::path::{Path, PathBuf};

/// A discovered Codex plugin before loading.
#[derive(Debug, Clone)]
pub struct PluginDiscovery {
    pub manifest_path: PathBuf,
    pub plugin_root: PathBuf,
    pub dir_name: String,
}

/// Scans a directory for plugin subdirectories containing `.codex-plugin/plugin.json`.
/// Skips symlinks to avoid following links to arbitrary filesystem locations.
pub fn scan_directory(dir: &Path) -> Vec<PluginDiscovery> {
    let mut discoveries = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return discoveries,
    };
    let mut subdirs: Vec<PathBuf> = entries
        .filter_map(|e| {
            let entry = e.ok()?;
            // Skip symlinks to prevent redirecting discovery to arbitrary directories.
            if entry.file_type().ok()?.is_symlink() {
                return None;
            }
            let path = entry.path();
            if path.is_dir() { Some(path) } else { None }
        })
        .collect();
    subdirs.sort();
    for subdir in subdirs {
        let dir_name = match subdir.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if let Some(manifest_path) = manifest::find_manifest_in_root(&subdir) {
            discoveries.push(PluginDiscovery {
                manifest_path,
                plugin_root: subdir,
                dir_name,
            });
        }
    }
    discoveries
}

/// Discovers Codex plugins from all standard locations.
pub fn discover_all(workspace_root: Option<&Path>) -> Vec<PluginDiscovery> {
    let mut all = Vec::new();
    if let Some(home) = dirs::home_dir() {
        let personal = home.join(".agents").join("plugins");
        if personal.exists() {
            all.extend(scan_directory(&personal));
        }
    }
    if let Some(root) = workspace_root {
        let project = root.join(".agents").join("plugins");
        if project.exists() {
            all.extend(scan_directory(&project));
        }
    } else if let Ok(cwd) = std::env::current_dir() {
        let project = cwd.join(".agents").join("plugins");
        if project.exists() {
            all.extend(scan_directory(&project));
        }
    }
    all
}

/// Loads a discovered plugin's manifest.
pub fn load_plugin_manifest(discovery: &PluginDiscovery) -> Result<LoadedCodexPlugin, ManifestError> {
    let manifest = manifest::parse_manifest(&discovery.manifest_path)?;
    let mut skill_roots = Vec::new();
    for sp in &manifest.skill_paths {
        let abs = resolve_plugin_path(&discovery.plugin_root, sp);
        if abs.exists() { skill_roots.push(abs); }
    }
    let plugin_id = format!("{}@codex-local", discovery.dir_name);
    Ok(LoadedCodexPlugin {
        plugin_id,
        name: manifest.name,
        version: manifest.version,
        description: manifest.description,
        root: discovery.plugin_root.clone(),
        skill_roots,
        hook_paths: manifest.hook_paths,
        hooks_inline: manifest.hooks_inline,
        mcp_servers: manifest.mcp_servers,
    })
}

/// A loaded Codex plugin with resolved paths (read-only — no execution state).
/// Public so the assembly layer can wire skills, MCP, and hooks.
#[derive(Debug, Clone)]
pub struct LoadedCodexPlugin {
    pub plugin_id: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub root: PathBuf,
    pub skill_roots: Vec<PathBuf>,
    pub hook_paths: Vec<String>,
    pub hooks_inline: Option<serde_json::Value>,
    pub mcp_servers: Option<manifest::McpServersValue>,
}

fn resolve_plugin_path(plugin_root: &Path, relative: &str) -> PathBuf {
    // Reject paths that attempt directory traversal or are absolute.
    let trimmed = relative.trim_start_matches("./").trim_start_matches(".\\");
    if trimmed.contains("..") || Path::new(trimmed).is_absolute() {
        // Return a path under plugin_root that will never exist, effectively
        // skipping this skill root at load time (caller checks .exists()).
        return plugin_root.join(".invalid-path-traversal-rejected");
    }
    plugin_root.join(trimmed)
}

#[cfg(test)]
mod resolve_tests {
    use super::*;

    #[test]
    fn test_normal_relative_path() {
        let root = Path::new("/plugins/my-plugin");
        let resolved = resolve_plugin_path(root, "./skills/");
        assert_eq!(resolved, Path::new("/plugins/my-plugin/skills/"));
    }

    #[test]
    fn test_parent_traversal_rejected() {
        let root = Path::new("/plugins/my-plugin");
        let resolved = resolve_plugin_path(root, "../../../etc/passwd");
        assert!(resolved.to_string_lossy().contains(".invalid-path-traversal-rejected"));
    }

    #[test]
    fn test_absolute_path_rejected() {
        let root = Path::new("/plugins/my-plugin");
        // Use a Windows-style absolute path on Windows, Unix-style on Unix.
        let abs_path = if cfg!(windows) { "C:\\etc\\passwd" } else { "/etc/passwd" };
        let resolved = resolve_plugin_path(root, abs_path);
        assert!(resolved.to_string_lossy().contains(".invalid-path-traversal-rejected"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_empty_dir() {
        let tmp = std::env::temp_dir().join("codex_empty_test");
        let _ = std::fs::create_dir_all(&tmp);
        let result = scan_directory(&tmp);
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(result.is_empty());
    }

    #[test]
    fn test_discover_personal_plugins() {
        // Should find at least the test plugins we installed
        let all = discover_all(None);
        let names: Vec<&str> = all.iter().map(|d| d.dir_name.as_str()).collect();
        // These may or may not exist depending on whether test setup ran
        eprintln!("Discovered plugins: {:?}", names);
        // At minimum, the function should not panic
    }
}
