//! Cross-platform reusable LSP service rules.
//!
//! This module owns pure plugin registry, command-target mapping rules,
//! reusable plugin package filesystem loading, protocol helpers, project
//! detection, request debounce, configuration file watching, and LSP server
//! process/manager primitives. It does not own product workspace state,
//! frontend event emission, or global singleton wiring.

pub mod config_watcher;
pub mod debouncer;
pub mod manager;
pub mod plugin_loader;
pub mod process;
pub mod project_detector;
pub mod protocol;

use bitfun_core_types::lsp::LspPlugin;
use log::{info, warn};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use thiserror::Error;

pub type LspPluginRegistryResult<T> = Result<T, LspPluginRegistryError>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LspPluginRegistryError {
    #[error("Plugin already registered: {0}")]
    AlreadyRegistered(String),
    #[error("Plugin not found: {0}")]
    NotFound(String),
    #[error("Unsupported platform")]
    UnsupportedPlatform,
    #[error("Unsupported architecture")]
    UnsupportedArchitecture,
}

/// Current or simulated platform for plugin command placeholder expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspPluginRuntimePlatform {
    Windows,
    Macos,
    Linux,
}

impl LspPluginRuntimePlatform {
    pub const fn manifest_token(self) -> &'static str {
        match self {
            Self::Windows => "win",
            Self::Macos => "darwin",
            Self::Linux => "linux",
        }
    }
}

/// Current or simulated CPU architecture for plugin command placeholder
/// expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspPluginRuntimeArch {
    X64,
    Arm64,
}

impl LspPluginRuntimeArch {
    pub const fn manifest_token(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::Arm64 => "arm64",
        }
    }
}

/// Runtime target facts used by LSP plugin manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspPluginRuntimeTarget {
    pub platform: LspPluginRuntimePlatform,
    pub arch: LspPluginRuntimeArch,
}

impl LspPluginRuntimeTarget {
    pub const fn new(platform: LspPluginRuntimePlatform, arch: LspPluginRuntimeArch) -> Self {
        Self { platform, arch }
    }
}

/// Resolves `${platform}`, `${os}`, and `${arch}` placeholders in an LSP plugin
/// command without touching the filesystem.
pub fn resolve_plugin_command_for_target(command: &str, target: LspPluginRuntimeTarget) -> String {
    command
        .replace("${platform}", target.platform.manifest_token())
        .replace("${os}", target.platform.manifest_token())
        .replace("${arch}", target.arch.manifest_token())
}

/// Returns the current runtime target supported by LSP plugin manifests.
pub fn current_lsp_plugin_runtime_target() -> LspPluginRegistryResult<LspPluginRuntimeTarget> {
    let platform = if cfg!(target_os = "windows") {
        LspPluginRuntimePlatform::Windows
    } else if cfg!(target_os = "macos") {
        LspPluginRuntimePlatform::Macos
    } else if cfg!(target_os = "linux") {
        LspPluginRuntimePlatform::Linux
    } else {
        return Err(LspPluginRegistryError::UnsupportedPlatform);
    };

    let arch = if cfg!(target_arch = "x86_64") {
        LspPluginRuntimeArch::X64
    } else if cfg!(target_arch = "aarch64") {
        LspPluginRuntimeArch::Arm64
    } else {
        return Err(LspPluginRegistryError::UnsupportedArchitecture);
    };

    Ok(LspPluginRuntimeTarget::new(platform, arch))
}

/// Resolves plugin command placeholders for the current runtime target.
pub fn resolve_plugin_command_for_current_target(command: &str) -> LspPluginRegistryResult<String> {
    Ok(resolve_plugin_command_for_target(
        command,
        current_lsp_plugin_runtime_target()?,
    ))
}

/// Supported extension summary used by product surfaces.
pub struct LspSupportedExtensions {
    pub extension_to_language: HashMap<String, String>,
    pub supported_languages: Vec<String>,
}

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
    pub fn register(&mut self, plugin: LspPlugin) -> LspPluginRegistryResult<()> {
        let plugin_id = plugin.id.clone();

        if self.plugins.contains_key(&plugin_id) {
            return Err(LspPluginRegistryError::AlreadyRegistered(plugin_id));
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
    pub fn unregister(&mut self, plugin_id: &str) -> LspPluginRegistryResult<()> {
        let plugin = self
            .plugins
            .remove(plugin_id)
            .ok_or_else(|| LspPluginRegistryError::NotFound(plugin_id.to_string()))?;

        for language in &plugin.languages {
            remove_index_if_owned(&mut self.language_map, language, plugin_id);
        }

        for ext in &plugin.file_extensions {
            remove_index_if_owned(&mut self.extension_map, ext, plugin_id);
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
            .filter(|p| p.languages.iter().any(|candidate| candidate == language))
            .collect()
    }

    /// Returns surface-facing supported extension facts.
    pub fn supported_extensions(&self) -> LspSupportedExtensions {
        let mut extension_to_language = HashMap::new();
        let mut supported_languages = HashSet::new();

        for plugin in self.plugins.values() {
            for language in &plugin.languages {
                supported_languages.insert(language.clone());
            }

            for ext in &plugin.file_extensions {
                if let Some(language) = plugin.languages.first() {
                    extension_to_language.insert(ext.clone(), language.clone());
                }
            }
        }

        LspSupportedExtensions {
            extension_to_language,
            supported_languages: supported_languages.into_iter().collect(),
        }
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

fn remove_index_if_owned(index: &mut HashMap<String, String>, key: &str, plugin_id: &str) {
    if index
        .get(key)
        .map(|current_plugin_id| current_plugin_id == plugin_id)
        .unwrap_or(false)
    {
        index.remove(key);
    }
}
