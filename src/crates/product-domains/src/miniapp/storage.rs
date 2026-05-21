//! MiniApp storage-shape helpers.

use crate::miniapp::types::NpmDep;
use std::path::{Path, PathBuf};

pub const META_JSON: &str = "meta.json";
pub const SOURCE_DIR: &str = "source";
pub const INDEX_HTML: &str = "index.html";
pub const STYLE_CSS: &str = "style.css";
pub const UI_JS: &str = "ui.js";
pub const WORKER_JS: &str = "worker.js";
pub const REQUIRED_SOURCE_FILES: &[&str] = &[INDEX_HTML, STYLE_CSS, UI_JS, WORKER_JS];
pub const PACKAGE_JSON: &str = "package.json";
pub const ESM_DEPS_JSON: &str = "esm_dependencies.json";
pub const COMPILED_HTML: &str = "compiled.html";
pub const STORAGE_JSON: &str = "storage.json";
pub const EMPTY_ESM_DEPENDENCIES_JSON: &str = "[]";
pub const EMPTY_STORAGE_JSON: &str = "{}";
pub const PLACEHOLDER_COMPILED_HTML: &str =
    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>Loading...</body></html>";
pub const VERSIONS_DIR: &str = "versions";
pub const DRAFTS_DIR: &str = ".drafts";
pub const DRAFTS_CLEANUP_PREFIX: &str = ".drafts.cleanup-";
pub const DRAFTS_CLEANUP_MARKER: &str = ".cleanup-pending";
pub const DRAFT_JSON: &str = "draft.json";
pub const CUSTOMIZATION_JSON: &str = ".customization.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppImportLayout {
    root: PathBuf,
}

impl MiniAppImportLayout {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn meta_path(&self) -> PathBuf {
        self.root.join(META_JSON)
    }

    pub fn source_dir(&self) -> PathBuf {
        self.root.join(SOURCE_DIR)
    }

    pub fn source_file_path(&self, file_name: &str) -> PathBuf {
        self.source_dir().join(file_name)
    }

    pub fn required_source_file_paths(&self) -> Vec<(&'static str, PathBuf)> {
        REQUIRED_SOURCE_FILES
            .iter()
            .map(|file_name| (*file_name, self.source_file_path(file_name)))
            .collect()
    }

    pub fn esm_dependencies_path(&self) -> PathBuf {
        self.source_file_path(ESM_DEPS_JSON)
    }

    pub fn package_json_path(&self) -> PathBuf {
        self.root.join(PACKAGE_JSON)
    }

    pub fn storage_json_path(&self) -> PathBuf {
        self.root.join(STORAGE_JSON)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppStorageLayout {
    miniapps_root: PathBuf,
    app_id: String,
}

impl MiniAppStorageLayout {
    pub fn new(miniapps_root: impl AsRef<Path>, app_id: impl Into<String>) -> Self {
        Self {
            miniapps_root: miniapps_root.as_ref().to_path_buf(),
            app_id: app_id.into(),
        }
    }

    pub fn app_dir(&self) -> PathBuf {
        self.miniapps_root.join(&self.app_id)
    }

    pub fn source_dir(&self) -> PathBuf {
        self.app_dir().join(SOURCE_DIR)
    }

    pub fn meta_path(&self) -> PathBuf {
        self.app_dir().join(META_JSON)
    }

    pub fn compiled_path(&self) -> PathBuf {
        self.app_dir().join(COMPILED_HTML)
    }

    pub fn storage_path(&self) -> PathBuf {
        self.app_dir().join(STORAGE_JSON)
    }

    pub fn customization_path(&self) -> PathBuf {
        self.app_dir().join(CUSTOMIZATION_JSON)
    }

    pub fn source_file_path(&self, file_name: &str) -> PathBuf {
        self.source_dir().join(file_name)
    }

    pub fn package_json_path(&self) -> PathBuf {
        self.app_dir().join(PACKAGE_JSON)
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.app_dir().join(VERSIONS_DIR)
    }

    pub fn version_path(&self, version: u32) -> PathBuf {
        self.versions_dir().join(format!("v{}.json", version))
    }

    pub fn drafts_root(miniapps_root: impl AsRef<Path>) -> PathBuf {
        miniapps_root.as_ref().join(DRAFTS_DIR)
    }

    pub fn app_drafts_dir(miniapps_root: impl AsRef<Path>, app_id: &str) -> PathBuf {
        Self::drafts_root(miniapps_root).join(app_id)
    }

    pub fn draft_dir(miniapps_root: impl AsRef<Path>, app_id: &str, draft_id: &str) -> PathBuf {
        Self::app_drafts_dir(miniapps_root, app_id).join(draft_id)
    }

    pub fn draft_source_dir(
        miniapps_root: impl AsRef<Path>,
        app_id: &str,
        draft_id: &str,
    ) -> PathBuf {
        Self::draft_dir(miniapps_root, app_id, draft_id).join(SOURCE_DIR)
    }

    pub fn draft_manifest_path(
        miniapps_root: impl AsRef<Path>,
        app_id: &str,
        draft_id: &str,
    ) -> PathBuf {
        Self::draft_dir(miniapps_root, app_id, draft_id).join(DRAFT_JSON)
    }

    pub fn cleanup_drafts_root(miniapps_root: impl AsRef<Path>, cleanup_id: &str) -> PathBuf {
        miniapps_root
            .as_ref()
            .join(format!("{}{}", DRAFTS_CLEANUP_PREFIX, cleanup_id))
    }
}

/// Parse package.json dependencies using the legacy MiniApp storage contract.
pub fn parse_npm_dependencies(package_json: &str) -> Result<Vec<NpmDep>, serde_json::Error> {
    let package: serde_json::Value = serde_json::from_str(package_json)?;
    let Some(deps) = package
        .get("dependencies")
        .and_then(|deps| deps.as_object())
    else {
        return Ok(Vec::new());
    };

    Ok(deps
        .iter()
        .map(|(name, version)| NpmDep {
            name: name.clone(),
            version: version.as_str().unwrap_or("*").to_string(),
        })
        .collect())
}

/// Build package.json using the legacy MiniApp storage contract.
pub fn build_package_json(app_id: &str, deps: &[NpmDep]) -> serde_json::Value {
    let mut dependencies = serde_json::Map::new();
    for dep in deps {
        dependencies.insert(
            dep.name.clone(),
            serde_json::Value::String(dep.version.clone()),
        );
    }

    serde_json::json!({
        "name": format!("miniapp-{}", app_id),
        "private": true,
        "dependencies": dependencies
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppImportFallbacks {
    pub esm_dependencies_json: &'static str,
    pub storage_json: &'static str,
    pub compiled_html: &'static str,
    pub package_json: serde_json::Value,
}

pub fn build_import_fallbacks(app_id: &str) -> MiniAppImportFallbacks {
    MiniAppImportFallbacks {
        esm_dependencies_json: EMPTY_ESM_DEPENDENCIES_JSON,
        storage_json: EMPTY_STORAGE_JSON,
        compiled_html: PLACEHOLDER_COMPILED_HTML,
        package_json: build_package_json(app_id, &[]),
    }
}
