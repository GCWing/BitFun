//! Configuration file watcher
//!
//! Features:
//! - Watches configuration file changes (tsconfig.json, package.json, etc.)
//! - Automatically restarts the corresponding LSP server when config changes

use anyhow::Result;
use log::{debug, info, warn};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Configuration file watcher.
pub struct ConfigWatcher {
    workspace_path: PathBuf,
    _watcher: RecommendedWatcher, // Keep the watcher alive (prevent it from being dropped)
}

impl ConfigWatcher {
    /// Creates a configuration file watcher.
    pub fn new(
        workspace_path: PathBuf,
        on_config_changed: Arc<dyn Fn(String, String) + Send + Sync>,
    ) -> Result<Self> {
        info!(
            "Setting up config file watcher for workspace: {:?}",
            workspace_path
        );

        let (tx, mut rx) = mpsc::channel(100);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default(),
        )?;

        let config_files = vec![
            "tsconfig.json",
            "package.json",
            "Cargo.toml",
            ".eslintrc.json",
            ".eslintrc.js",
            "pyproject.toml",
            "setup.py",
            "go.mod",
            "pom.xml",
            "build.gradle",
            "CMakeLists.txt",
        ];

        for file_name in config_files {
            let file_path = workspace_path.join(file_name);
            if file_path.exists() {
                if let Err(e) = watcher.watch(&file_path, RecursiveMode::NonRecursive) {
                    warn!("Failed to watch config file {}: {}", file_name, e);
                }
            }
        }

        let workspace_path_clone = workspace_path.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                Self::handle_file_event(event, &workspace_path_clone, &on_config_changed);
            }
        });

        info!("Config file watcher started");

        Ok(Self {
            workspace_path,
            _watcher: watcher,
        })
    }

    /// Handles file change events.
    fn handle_file_event(
        event: Event,
        _workspace_path: &Path,
        on_config_changed: &Arc<dyn Fn(String, String) + Send + Sync>,
    ) {
        if !matches!(event.kind, EventKind::Modify(_)) {
            return;
        }

        for path in event.paths {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                let language = Self::config_file_to_language(file_name);

                if let Some(lang) = language {
                    info!(
                        "Config file changed: {}, restarting {} server",
                        file_name, lang
                    );
                    on_config_changed(lang.to_string(), file_name.to_string());
                }
            }
        }
    }

    /// Infers a language from a configuration filename.
    fn config_file_to_language(file_name: &str) -> Option<&'static str> {
        match file_name {
            "tsconfig.json" | "package.json" => Some("typescript"),
            "Cargo.toml" => Some("rust"),
            "pyproject.toml" | "setup.py" => Some("python"),
            "go.mod" => Some("go"),
            "pom.xml" | "build.gradle" => Some("java"),
            "CMakeLists.txt" => Some("cpp"),
            ".eslintrc.json" | ".eslintrc.js" => Some("javascript"),
            _ => None,
        }
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        debug!(
            "ConfigWatcher dropped for workspace: {:?}",
            self.workspace_path
        );
    }
}
