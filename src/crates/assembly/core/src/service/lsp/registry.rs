//! LSP plugin registry
//!
//! Manages information about installed plugins.

use anyhow::{anyhow, Result};
use log::{info, warn};
use std::collections::HashMap;
use std::path::PathBuf;

use super::types::LspPlugin;

/// Plugin registry.
pub struct PluginRegistry {
    /// Registered plugins (`plugin_id -> plugin`).
    plugins: HashMap<String, LspPlugin>,
    /// Language-to-plugin mapping (`language -> plugin_id`).
    language_map: HashMap<String, String>,
    /// File-extension-to-plugin mapping (`extension -> plugin_id`).
    extension_map: HashMap<String, String>,
}

impl PluginRegistry {
    /// Creates a new plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            language_map: HashMap::new(),
            extension_map: HashMap::new(),
        }
    }

    /// Registers a plugin.
    pub fn register(&mut self, plugin: LspPlugin) -> Result<()> {
        let plugin_id = plugin.id.clone();

        if self.plugins.contains_key(&plugin_id) {
            return Err(anyhow!("Plugin already registered: {}", plugin_id));
        }

        for language in &plugin.languages {
            if let Some(existing) = self.language_map.get(language) {
                warn!(
                    "Language '{}' already mapped to plugin '{}', overwriting with '{}'",
                    language, existing, plugin_id
                );
            }
            self.language_map
                .insert(language.clone(), plugin_id.clone());
        }

        for ext in &plugin.file_extensions {
            if let Some(existing) = self.extension_map.get(ext) {
                warn!(
                    "Extension '{}' already mapped to plugin '{}', overwriting with '{}'",
                    ext, existing, plugin_id
                );
            }
            self.extension_map.insert(ext.clone(), plugin_id.clone());
        }

        self.plugins.insert(plugin_id.clone(), plugin);

        info!(
            "Plugin registered: {} with {} language(s) and {} extension(s)",
            plugin_id,
            self.language_map
                .values()
                .filter(|v| *v == &plugin_id)
                .count(),
            self.extension_map
                .values()
                .filter(|v| *v == &plugin_id)
                .count()
        );

        Ok(())
    }

    /// Unregisters a plugin.
    pub fn unregister(&mut self, plugin_id: &str) -> Result<()> {
        let plugin = self
            .plugins
            .remove(plugin_id)
            .ok_or_else(|| anyhow!("Plugin not found: {}", plugin_id))?;

        for language in &plugin.languages {
            self.language_map.remove(language);
        }

        for ext in &plugin.file_extensions {
            self.extension_map.remove(ext);
        }

        info!("Plugin unregistered: {}", plugin_id);

        Ok(())
    }

    /// Gets a plugin by plugin ID.
    pub fn get_plugin(&self, plugin_id: &str) -> Option<&LspPlugin> {
        self.plugins.get(plugin_id)
    }

    /// Finds a plugin by language ID.
    pub fn find_by_language(&self, language: &str) -> Option<&LspPlugin> {
        self.language_map
            .get(language)
            .and_then(|id| self.plugins.get(id))
    }

    /// Finds a plugin by file extension.
    pub fn find_by_extension(&self, extension: &str) -> Option<&LspPlugin> {
        let ext = if extension.starts_with('.') {
            extension.to_string()
        } else {
            format!(".{}", extension)
        };

        self.extension_map
            .get(&ext)
            .and_then(|id| self.plugins.get(id))
    }

    /// Finds a plugin by file path.
    pub fn find_by_file_path(&self, file_path: &str) -> Option<&LspPlugin> {
        let path = PathBuf::from(file_path);
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return self.find_by_extension(ext_str);
            }
        }
        None
    }

    /// Lists all registered plugins.
    pub fn list_all(&self) -> Vec<&LspPlugin> {
        self.plugins.values().collect()
    }

    /// Lists all plugins that support a specific language.
    pub fn list_by_language(&self, language: &str) -> Vec<&LspPlugin> {
        self.plugins
            .values()
            .filter(|p| p.languages.contains(&language.to_string()))
            .collect()
    }

    /// Returns whether a plugin is registered.
    pub fn is_registered(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    /// Returns the number of registered plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
