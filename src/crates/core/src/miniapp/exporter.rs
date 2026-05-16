//! MiniApp export engine — export to Electron or Tauri standalone app (skeleton).

pub use bitfun_product_domains::miniapp::exporter::{
    ExportCheckResult, ExportOptions, ExportResult, ExportTarget,
};

use crate::util::errors::{BitFunError, BitFunResult};
use std::path::PathBuf;
use std::sync::Arc;

/// Export engine: check prerequisites and export MiniApp to standalone app.
pub struct MiniAppExporter {
    #[allow(dead_code)]
    path_manager: Arc<crate::infrastructure::PathManager>,
    #[allow(dead_code)]
    templates_dir: PathBuf,
}

impl MiniAppExporter {
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
        let runtime = crate::miniapp::runtime_detect::detect_runtime();
        let runtime_str = runtime.as_ref().map(|r| {
            match r.kind {
                crate::miniapp::runtime_detect::RuntimeKind::Bun => "bun",
                crate::miniapp::runtime_detect::RuntimeKind::Node => "node",
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

    /// Export the MiniApp to a standalone application.
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
