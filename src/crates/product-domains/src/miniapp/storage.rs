//! MiniApp storage-shape helpers.

use crate::miniapp::types::NpmDep;
use std::path::{Path, PathBuf};

pub const META_JSON: &str = "meta.json";
pub const SOURCE_DIR: &str = "source";
pub const INDEX_HTML: &str = "index.html";
pub const STYLE_CSS: &str = "style.css";
pub const UI_JS: &str = "ui.js";
pub const WORKER_JS: &str = "worker.js";
pub const PACKAGE_JSON: &str = "package.json";
pub const ESM_DEPS_JSON: &str = "esm_dependencies.json";
pub const COMPILED_HTML: &str = "compiled.html";
pub const STORAGE_JSON: &str = "storage.json";
pub const VERSIONS_DIR: &str = "versions";

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
