//! MiniApp export DTOs.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
