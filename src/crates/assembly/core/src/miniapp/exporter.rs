//! MiniApp export engine — export to Electron or Tauri standalone app (skeleton).

pub use bitfun_product_domains::miniapp::exporter::{
    build_export_check_result, ExportCheckResult, ExportOptions, ExportResult, ExportTarget,
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
        Ok(build_export_check_result(
            runtime.as_ref().map(|runtime| &runtime.kind),
        ))
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
