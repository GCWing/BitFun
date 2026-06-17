//! LSP plugin loader
//!
//! Responsible for loading and installing plugins from the filesystem.

use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::path::{Path, PathBuf};
use tokio::fs;

use super::types::LspPlugin;

/// Plugin loader.
pub struct PluginLoader {
    /// Plugins directory.
    plugins_dir: PathBuf,
}

impl PluginLoader {
    /// Creates a new plugin loader.
    pub fn new(plugins_dir: PathBuf) -> Self {
        Self { plugins_dir }
    }

    /// Loads a specific plugin.
    pub async fn load_plugin(&self, plugin_id: &str) -> Result<LspPlugin> {
        let plugin_dir = self.plugins_dir.join(plugin_id);
        let manifest_path = plugin_dir.join("manifest.json");

        if !manifest_path.exists() {
            return Err(anyhow!(
                "Plugin manifest not found: {}",
                manifest_path.display()
            ));
        }

        let content = fs::read_to_string(&manifest_path).await?;
        let plugin: LspPlugin = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse manifest: {}", e))?;

        if plugin.id != plugin_id {
            return Err(anyhow!(
                "Plugin ID mismatch: expected '{}', found '{}'",
                plugin_id,
                plugin.id
            ));
        }

        info!("Plugin loaded: {} v{}", plugin.name, plugin.version);
        debug!("Supported languages: {:?}", plugin.languages);
        debug!("File extensions: {:?}", plugin.file_extensions);

        Ok(plugin)
    }

    /// Loads all installed plugins.
    pub async fn load_all_plugins(&self) -> Result<Vec<LspPlugin>> {
        if !self.plugins_dir.exists() {
            fs::create_dir_all(&self.plugins_dir).await?;
            info!("Created plugins directory: {:?}", self.plugins_dir);
            return Ok(vec![]);
        }

        let mut plugins = Vec::new();
        let mut entries = fs::read_dir(&self.plugins_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                if let Some(plugin_id) = path.file_name().and_then(|n| n.to_str()) {
                    if plugin_id.starts_with('.') {
                        continue;
                    }

                    if plugin_id == "temp" || plugin_id == "cache" || plugin_id == "backup" {
                        continue;
                    }

                    match self.load_plugin(plugin_id).await {
                        Ok(plugin) => {
                            plugins.push(plugin);
                        }
                        Err(e) => {
                            error!("Failed to load plugin '{}': {}", plugin_id, e);
                        }
                    }
                }
            }
        }

        info!("Successfully loaded {} plugin(s)", plugins.len());

        Ok(plugins)
    }

    /// Installs a plugin package (a `.vcpkg` file).
    pub async fn install_plugin_package(&self, package_path: &Path) -> Result<String> {
        info!("Installing plugin package: {:?}", package_path);

        if !package_path.exists() {
            error!("Plugin package not found: {:?}", package_path);
            return Err(anyhow!("Plugin package not found: {:?}", package_path));
        }

        if package_path.extension().and_then(|e| e.to_str()) != Some("vcpkg") {
            error!("Invalid plugin package format (expected .vcpkg)");
            return Err(anyhow!("Invalid plugin package format (expected .vcpkg)"));
        }

        let temp_id = format!(".temp-{}", std::process::id());
        let temp_dir = self.plugins_dir.join(&temp_id);

        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).await?;
        }

        fs::create_dir_all(&temp_dir).await?;

        let file = std::fs::File::open(package_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut manifest_content = String::new();
        {
            let mut manifest_file = archive.by_name("manifest.json")?;
            std::io::Read::read_to_string(&mut manifest_file, &mut manifest_content)?;
        }

        let plugin: LspPlugin = serde_json::from_str(&manifest_content)?;
        let plugin_id = plugin.id.clone();

        let plugin_dir = self.plugins_dir.join(&plugin_id);
        if plugin_dir.exists() {
            return Err(anyhow!("Plugin already installed: {}", plugin_id));
        }

        archive.extract(&plugin_dir)?;

        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir).await;
        }

        info!(
            "Plugin installed: {} v{} (id: {})",
            plugin.name, plugin.version, plugin_id
        );

        Ok(plugin_id)
    }

    /// Uninstalls a plugin.
    pub async fn uninstall_plugin(&self, plugin_id: &str) -> Result<()> {
        info!("Uninstalling plugin: {}", plugin_id);

        let plugin_dir = self.plugins_dir.join(plugin_id);

        if !plugin_dir.exists() {
            error!("Plugin not found: {}", plugin_id);
            return Err(anyhow!("Plugin not found: {}", plugin_id));
        }

        fs::remove_dir_all(&plugin_dir).await?;

        info!("Plugin uninstalled successfully: {}", plugin_id);

        Ok(())
    }

    /// Cleans up temporary directories.
    pub async fn cleanup_temp_dirs(&self) -> Result<()> {
        let mut entries = fs::read_dir(&self.plugins_dir).await?;
        let mut cleaned_count = 0;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if dir_name.starts_with(".temp") {
                        if let Err(e) = fs::remove_dir_all(&path).await {
                            warn!("Failed to remove temp directory {}: {}", dir_name, e);
                        } else {
                            cleaned_count += 1;
                        }
                    }
                }
            }
        }

        if cleaned_count > 0 {
            info!("Cleaned {} temporary director(ies)", cleaned_count);
        }

        Ok(())
    }

    /// Returns the plugin server executable path.
    pub fn get_server_path(&self, plugin: &LspPlugin) -> Result<PathBuf> {
        let plugin_dir = self.plugins_dir.join(&plugin.id);

        let command = self.resolve_command(&plugin.server.command)?;

        let command = command.replace('/', std::path::MAIN_SEPARATOR_STR);

        let server_path = plugin_dir.join(&command);

        if !server_path.exists() {
            #[cfg(windows)]
            {
                let mut server_path = server_path.clone();
                let extensions = vec![".exe", ".bat", ".cmd"];
                let mut found = false;

                for ext in extensions {
                    let path_with_ext = plugin_dir.join(format!("{}{}", command, ext));

                    if path_with_ext.exists() {
                        server_path = path_with_ext;
                        found = true;
                        break;
                    }
                }

                if !found {
                    error!("LSP server binary not found at: {:?}", server_path);
                    error!("Tried extensions: .exe, .bat, .cmd");
                    error!("Plugin directory: {:?}", plugin_dir);
                    return Err(anyhow!(
                        "LSP server binary not found: {}\nTried: {}.exe, {}.bat, {}.cmd",
                        server_path.display(),
                        command,
                        command,
                        command
                    ));
                }
            }

            #[cfg(not(windows))]
            {
                error!("LSP server binary not found: {:?}", server_path);
                return Err(anyhow!(
                    "LSP server binary not found: {}",
                    server_path.display()
                ));
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&server_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&server_path, perms)?;
        }

        Ok(server_path)
    }

    /// Resolves placeholders in the command.
    fn resolve_command(&self, command: &str) -> Result<String> {
        let mut resolved = command.to_string();

        let platform = if cfg!(target_os = "windows") {
            "win"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            return Err(anyhow!("Unsupported platform"));
        };

        resolved = resolved.replace("${platform}", platform);
        resolved = resolved.replace("${os}", platform);

        let arch = if cfg!(target_arch = "x86_64") {
            "x64"
        } else if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            return Err(anyhow!("Unsupported architecture"));
        };

        resolved = resolved.replace("${arch}", arch);

        Ok(resolved)
    }

    /// Returns the plugin directory path.
    pub fn get_plugin_dir(&self, plugin_id: &str) -> PathBuf {
        self.plugins_dir.join(plugin_id)
    }

    /// Returns the plugins root directory.
    pub fn get_plugins_root(&self) -> &Path {
        &self.plugins_dir
    }
}
