//! Live App export engine — export to Electron or Tauri standalone app (skeleton).

use crate::util::errors::{BitFunError, BitFunResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportTarget {
    Electron,
    Tauri,
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub target: ExportTarget,
    pub output_dir: PathBuf,
    pub app_name: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub include_storage: bool,
    pub platforms: Vec<String>,
    pub sign: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCheckResult {
    pub ready: bool,
    pub runtime: Option<String>,
    pub missing: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub success: bool,
    pub output_path: Option<String>,
    pub size_mb: Option<f64>,
    pub duration_ms: Option<u64>,
}

/// Export engine: check prerequisites and export Live App to standalone app.
pub struct LiveAppExporter {
    #[allow(dead_code)]
    path_manager: Arc<crate::infrastructure::PathManager>,
    #[allow(dead_code)]
    templates_dir: PathBuf,
}

impl LiveAppExporter {
    pub fn new(
        path_manager: Arc<crate::infrastructure::PathManager>,
        templates_dir: PathBuf,
    ) -> Self {
        Self {
            path_manager,
            templates_dir,
        }
    }

    /// Check if export is possible (runtime, electron-builder, etc.).
    pub async fn check(&self, _app_id: &str) -> BitFunResult<ExportCheckResult> {
        let runtime = crate::live_app::runtime_detect::detect_runtime();
        let runtime_str = runtime.as_ref().map(|r| {
            match r.kind {
                crate::live_app::runtime_detect::RuntimeKind::Bun => "bun",
                crate::live_app::runtime_detect::RuntimeKind::Node => "node",
            }
            .to_string()
        });
        let mut missing = Vec::new();
        if runtime.is_none() {
            missing.push("No JS runtime (install Bun or Node.js)".to_string());
        }
        Ok(ExportCheckResult {
            ready: missing.is_empty(),
            runtime: runtime_str,
            missing,
            warnings: Vec::new(),
        })
    }

    /// Export the Live App to a standalone application.
    pub async fn export(
        &self,
        _app_id: &str,
        _options: ExportOptions,
    ) -> BitFunResult<ExportResult> {
        Err(BitFunError::validation(
            "Export not yet implemented (skeleton)".to_string(),
        ))
    }
}
