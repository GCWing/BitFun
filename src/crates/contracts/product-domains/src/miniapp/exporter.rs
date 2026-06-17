//! MiniApp export DTOs.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::miniapp::runtime::RuntimeKind;

pub const MISSING_JS_RUNTIME_MESSAGE: &str = "No JS runtime (install Bun or Node.js)";

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

pub fn export_runtime_label(kind: &RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Bun => "bun",
        RuntimeKind::Node => "node",
    }
}

pub fn build_export_check_result(runtime: Option<&RuntimeKind>) -> ExportCheckResult {
    let runtime = runtime.map(export_runtime_label).map(str::to_string);
    let mut missing = Vec::new();
    if runtime.is_none() {
        missing.push(MISSING_JS_RUNTIME_MESSAGE.to_string());
    }
    ExportCheckResult {
        ready: missing.is_empty(),
        runtime,
        missing,
        warnings: Vec::new(),
    }
}
