//! MiniApp storage — persist and load MiniApp data under user data dir (V2: ui.js, worker.js, package.json).

use crate::miniapp::types::{MiniApp, MiniAppMeta, MiniAppSource, NpmDep};
use crate::util::errors::{BitFunError, BitFunResult};
use bitfun_product_domains::miniapp::ports::{
    MiniAppPortError, MiniAppPortErrorKind, MiniAppPortFuture, MiniAppStoragePort,
};
use bitfun_product_domains::miniapp::storage::{
    build_package_json, parse_npm_dependencies, MiniAppStorageLayout, ESM_DEPS_JSON, INDEX_HTML,
    PACKAGE_JSON, STYLE_CSS, UI_JS, WORKER_JS,
};
use serde_json;
use std::path::PathBuf;
use std::sync::Arc;

/// MiniApp storage service (file-based under path_manager.miniapps_dir).
pub struct MiniAppStorage {
    path_manager: Arc<crate::infrastructure::PathManager>,
}

impl MiniAppStorage {
    pub fn new(path_manager: Arc<crate::infrastructure::PathManager>) -> Self {
        Self { path_manager }
    }

    fn layout(&self, app_id: &str) -> MiniAppStorageLayout {
        MiniAppStorageLayout::new(self.path_manager.miniapps_dir(), app_id)
    }

    fn app_dir(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).app_dir()
    }

    fn meta_path(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).meta_path()
    }

    fn source_dir(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).source_dir()
    }

    fn compiled_path(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).compiled_path()
    }

    fn storage_path(&self, app_id: &str) -> PathBuf {
        self.layout(app_id).storage_path()
    }

    fn version_path(&self, app_id: &str, version: u32) -> PathBuf {
        self.layout(app_id).version_path(version)
    }

    /// Ensure app directory and source subdir exist.
    pub async fn ensure_app_dir(&self, app_id: &str) -> BitFunResult<()> {
        let dir = self.app_dir(app_id);
        let source = self.source_dir(app_id);
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create miniapp dir {}: {}",
                dir.display(),
                e
            ))
        })?;
        tokio::fs::create_dir_all(&source).await.map_err(|e| {
            BitFunError::io(format!(
                "Failed to create source dir {}: {}",
                source.display(),
                e
            ))
        })?;
        Ok(())
    }

    /// List all app IDs (directories under miniapps_dir).
    pub async fn list_app_ids(&self) -> BitFunResult<Vec<String>> {
        let root = self.path_manager.miniapps_dir();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&root)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read miniapps dir: {}", e)))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read miniapps entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !name.starts_with('.') {
                        ids.push(name.to_string());
                    }
                }
            }
        }
        Ok(ids)
    }

    /// Load full MiniApp by id (meta + source + compiled_html).
    pub async fn load(&self, app_id: &str) -> BitFunResult<MiniApp> {
        let meta_path = self.meta_path(app_id);
        let meta_content = tokio::fs::read_to_string(&meta_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("MiniApp not found: {}", app_id))
            } else {
                BitFunError::io(format!("Failed to read meta: {}", e))
            }
        })?;
        let meta: MiniAppMeta = serde_json::from_str(&meta_content)
            .map_err(|e| BitFunError::parse(format!("Invalid meta.json: {}", e)))?;

        let source = self.load_source(app_id).await?;
        let compiled_html = self.load_compiled_html(app_id).await?;

        Ok(MiniApp {
            id: meta.id,
            name: meta.name,
            description: meta.description,
            icon: meta.icon,
            category: meta.category,
            tags: meta.tags,
            version: meta.version,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            source,
            compiled_html,
            permissions: meta.permissions,
            ai_context: meta.ai_context,
            runtime: meta.runtime,
            i18n: meta.i18n,
        })
    }

    /// Load only metadata (for list views).
    pub async fn load_meta(&self, app_id: &str) -> BitFunResult<MiniAppMeta> {
        let meta_path = self.meta_path(app_id);
        let content = tokio::fs::read_to_string(&meta_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("MiniApp not found: {}", app_id))
            } else {
                BitFunError::io(format!("Failed to read meta: {}", e))
            }
        })?;
        serde_json::from_str(&content)
            .map_err(|e| BitFunError::parse(format!("Invalid meta.json: {}", e)))
    }

    async fn load_source(&self, app_id: &str) -> BitFunResult<MiniAppSource> {
        let sd = self.source_dir(app_id);
        let html = tokio::fs::read_to_string(sd.join(INDEX_HTML))
            .await
            .unwrap_or_default();
        let css = tokio::fs::read_to_string(sd.join(STYLE_CSS))
            .await
            .unwrap_or_default();
        let ui_js = tokio::fs::read_to_string(sd.join(UI_JS))
            .await
            .unwrap_or_default();
        let worker_js = tokio::fs::read_to_string(sd.join(WORKER_JS))
            .await
            .unwrap_or_default();

        let esm_dependencies = if sd.join(ESM_DEPS_JSON).exists() {
            let c = tokio::fs::read_to_string(sd.join(ESM_DEPS_JSON))
                .await
                .unwrap_or_default();
            serde_json::from_str(&c).unwrap_or_default()
        } else {
            Vec::new()
        };

        let npm_dependencies = self.load_npm_dependencies(app_id).await?;

        Ok(MiniAppSource {
            html,
            css,
            ui_js,
            esm_dependencies,
            worker_js,
            npm_dependencies,
        })
    }

    /// Load only source files and package dependencies from disk.
    pub async fn load_source_only(&self, app_id: &str) -> BitFunResult<MiniAppSource> {
        self.load_source(app_id).await
    }

    async fn load_npm_dependencies(&self, app_id: &str) -> BitFunResult<Vec<NpmDep>> {
        let p = self.layout(app_id).package_json_path();
        if !p.exists() {
            return Ok(Vec::new());
        }
        let c = tokio::fs::read_to_string(&p)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read package.json: {}", e)))?;
        parse_npm_dependencies(&c)
            .map_err(|e| BitFunError::parse(format!("Invalid package.json: {}", e)))
    }

    async fn load_compiled_html(&self, app_id: &str) -> BitFunResult<String> {
        let p = self.compiled_path(app_id);
        tokio::fs::read_to_string(&p).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("Compiled HTML not found: {}", app_id))
            } else {
                BitFunError::io(format!("Failed to read compiled.html: {}", e))
            }
        })
    }

    /// Save full MiniApp (meta, source files, compiled.html).
    pub async fn save(&self, app: &MiniApp) -> BitFunResult<()> {
        self.ensure_app_dir(&app.id).await?;

        let meta = MiniAppMeta::from(app);
        let meta_path = self.meta_path(&app.id);
        let meta_json = serde_json::to_string_pretty(&meta).map_err(BitFunError::from)?;
        tokio::fs::write(&meta_path, meta_json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write meta: {}", e)))?;

        let sd = self.source_dir(&app.id);
        tokio::fs::write(sd.join(INDEX_HTML), &app.source.html)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write index.html: {}", e)))?;
        tokio::fs::write(sd.join(STYLE_CSS), &app.source.css)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write style.css: {}", e)))?;
        tokio::fs::write(sd.join(UI_JS), &app.source.ui_js)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write ui.js: {}", e)))?;
        tokio::fs::write(sd.join(WORKER_JS), &app.source.worker_js)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write worker.js: {}", e)))?;

        let esm_json = serde_json::to_string_pretty(&app.source.esm_dependencies)
            .map_err(BitFunError::from)?;
        tokio::fs::write(sd.join(ESM_DEPS_JSON), esm_json)
            .await
            .map_err(|e| {
                BitFunError::io(format!("Failed to write esm_dependencies.json: {}", e))
            })?;

        self.write_package_json(&app.id, &app.source.npm_dependencies)
            .await?;

        tokio::fs::write(self.compiled_path(&app.id), &app.compiled_html)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write compiled.html: {}", e)))?;

        Ok(())
    }

    async fn write_package_json(&self, app_id: &str, deps: &[NpmDep]) -> BitFunResult<()> {
        let pkg = build_package_json(app_id, deps);
        let p = self.app_dir(app_id).join(PACKAGE_JSON);
        let json = serde_json::to_string_pretty(&pkg).map_err(BitFunError::from)?;
        tokio::fs::write(&p, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write package.json: {}", e)))?;
        Ok(())
    }

    /// Save a version snapshot (for rollback).
    pub async fn save_version(
        &self,
        app_id: &str,
        version: u32,
        app: &MiniApp,
    ) -> BitFunResult<()> {
        let versions_dir = self.layout(app_id).versions_dir();
        tokio::fs::create_dir_all(&versions_dir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to create versions dir: {}", e)))?;
        let path = self.version_path(app_id, version);
        let json = serde_json::to_string_pretty(app).map_err(BitFunError::from)?;
        tokio::fs::write(&path, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write version file: {}", e)))?;
        Ok(())
    }

    /// Load app storage (KV JSON). Returns empty object if missing.
    pub async fn load_app_storage(&self, app_id: &str) -> BitFunResult<serde_json::Value> {
        let p = self.storage_path(app_id);
        if !p.exists() {
            return Ok(serde_json::json!({}));
        }
        let c = tokio::fs::read_to_string(&p)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read storage: {}", e)))?;
        Ok(serde_json::from_str(&c).unwrap_or_else(|_| serde_json::json!({})))
    }

    /// Save app storage (merge with existing or replace).
    pub async fn save_app_storage(
        &self,
        app_id: &str,
        key: &str,
        value: serde_json::Value,
    ) -> BitFunResult<()> {
        self.ensure_app_dir(app_id).await?;
        let mut current = self.load_app_storage(app_id).await?;
        let obj = current
            .as_object_mut()
            .ok_or_else(|| BitFunError::validation("App storage is not an object".to_string()))?;
        obj.insert(key.to_string(), value);
        let p = self.storage_path(app_id);
        let json = serde_json::to_string_pretty(&current).map_err(BitFunError::from)?;
        tokio::fs::write(&p, json)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to write storage: {}", e)))?;
        Ok(())
    }

    /// Delete MiniApp directory entirely.
    pub async fn delete(&self, app_id: &str) -> BitFunResult<()> {
        let dir = self.app_dir(app_id);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir)
                .await
                .map_err(|e| BitFunError::io(format!("Failed to delete miniapp dir: {}", e)))?;
        }
        Ok(())
    }

    /// List version numbers that have snapshots.
    pub async fn list_versions(&self, app_id: &str) -> BitFunResult<Vec<u32>> {
        let vdir = self.layout(app_id).versions_dir();
        if !vdir.exists() {
            return Ok(Vec::new());
        }
        let mut versions = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&vdir)
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read versions dir: {}", e)))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| BitFunError::io(format!("Failed to read versions entry: {}", e)))?
        {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('v') && name.ends_with(".json") {
                if let Ok(n) = name[1..name.len() - 5].parse::<u32>() {
                    versions.push(n);
                }
            }
        }
        versions.sort();
        Ok(versions)
    }

    /// Load a specific version snapshot.
    pub async fn load_version(&self, app_id: &str, version: u32) -> BitFunResult<MiniApp> {
        let p = self.version_path(app_id, version);
        let c = tokio::fs::read_to_string(&p).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                BitFunError::NotFound(format!("Version v{} not found", version))
            } else {
                BitFunError::io(format!("Failed to read version: {}", e))
            }
        })?;
        serde_json::from_str(&c)
            .map_err(|e| BitFunError::parse(format!("Invalid version file: {}", e)))
    }
}

impl MiniAppStoragePort for MiniAppStorage {
    fn list_app_ids(&self) -> MiniAppPortFuture<'_, Vec<String>> {
        Box::pin(async move { self.list_app_ids().await.map_err(map_miniapp_port_error) })
    }

    fn load(&self, app_id: String) -> MiniAppPortFuture<'_, MiniApp> {
        Box::pin(async move { self.load(&app_id).await.map_err(map_miniapp_port_error) })
    }

    fn load_meta(&self, app_id: String) -> MiniAppPortFuture<'_, MiniAppMeta> {
        Box::pin(async move {
            self.load_meta(&app_id)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn load_source(&self, app_id: String) -> MiniAppPortFuture<'_, MiniAppSource> {
        Box::pin(async move {
            self.load_source_only(&app_id)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn save(&self, app: MiniApp) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move { self.save(&app).await.map_err(map_miniapp_port_error) })
    }

    fn save_version(
        &self,
        app_id: String,
        version: u32,
        app: MiniApp,
    ) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move {
            self.save_version(&app_id, version, &app)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn load_app_storage(&self, app_id: String) -> MiniAppPortFuture<'_, serde_json::Value> {
        Box::pin(async move {
            self.load_app_storage(&app_id)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn save_app_storage(
        &self,
        app_id: String,
        key: String,
        value: serde_json::Value,
    ) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move {
            self.save_app_storage(&app_id, &key, value)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn delete(&self, app_id: String) -> MiniAppPortFuture<'_, ()> {
        Box::pin(async move { self.delete(&app_id).await.map_err(map_miniapp_port_error) })
    }

    fn list_versions(&self, app_id: String) -> MiniAppPortFuture<'_, Vec<u32>> {
        Box::pin(async move {
            self.list_versions(&app_id)
                .await
                .map_err(map_miniapp_port_error)
        })
    }

    fn load_version(&self, app_id: String, version: u32) -> MiniAppPortFuture<'_, MiniApp> {
        Box::pin(async move {
            self.load_version(&app_id, version)
                .await
                .map_err(map_miniapp_port_error)
        })
    }
}

fn map_miniapp_port_error(error: BitFunError) -> MiniAppPortError {
    let kind = match &error {
        BitFunError::NotFound(_) => MiniAppPortErrorKind::NotFound,
        BitFunError::Validation(_) | BitFunError::Deserialization(_) => {
            MiniAppPortErrorKind::InvalidInput
        }
        BitFunError::Io(io_error) if io_error.kind() == std::io::ErrorKind::PermissionDenied => {
            MiniAppPortErrorKind::PermissionDenied
        }
        BitFunError::Io(_) => MiniAppPortErrorKind::Io,
        _ => MiniAppPortErrorKind::Backend,
    };
    MiniAppPortError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn storage_port_adapter_preserves_existing_file_lifecycle() {
        let root = std::env::temp_dir().join(format!(
            "bitfun-miniapp-storage-port-{}",
            uuid::Uuid::new_v4()
        ));
        let path_manager =
            Arc::new(crate::infrastructure::PathManager::with_user_root_for_tests(root));
        let storage = MiniAppStorage::new(path_manager);
        let port: &dyn MiniAppStoragePort = &storage;
        let app = sample_app("demo_app");

        port.save(app.clone()).await.unwrap();

        let ids = port.list_app_ids().await.unwrap();
        assert_eq!(ids, vec!["demo_app".to_string()]);

        let meta = port.load_meta("demo_app".to_string()).await.unwrap();
        assert_eq!(meta.name, "Demo");

        let source = port.load_source("demo_app".to_string()).await.unwrap();
        assert_eq!(source.ui_js, "console.log('ui');");

        let loaded = port.load("demo_app".to_string()).await.unwrap();
        assert_eq!(loaded.compiled_html, "<html></html>");

        port.save_app_storage(
            "demo_app".to_string(),
            "answer".to_string(),
            serde_json::json!(42),
        )
        .await
        .unwrap();
        let app_storage = port.load_app_storage("demo_app".to_string()).await.unwrap();
        assert_eq!(app_storage["answer"], 42);

        port.save_version("demo_app".to_string(), 1, app)
            .await
            .unwrap();
        assert_eq!(
            port.list_versions("demo_app".to_string()).await.unwrap(),
            vec![1]
        );
        assert_eq!(
            port.load_version("demo_app".to_string(), 1)
                .await
                .unwrap()
                .id,
            "demo_app"
        );

        port.delete("demo_app".to_string()).await.unwrap();
        assert!(port.list_app_ids().await.unwrap().is_empty());
    }

    fn sample_app(id: &str) -> MiniApp {
        MiniApp {
            id: id.to_string(),
            name: "Demo".to_string(),
            description: "Demo app".to_string(),
            icon: "sparkles".to_string(),
            category: "tools".to_string(),
            tags: vec!["demo".to_string()],
            version: 1,
            created_at: 1,
            updated_at: 2,
            source: MiniAppSource {
                html: "<div id=\"app\"></div>".to_string(),
                css: "body {}".to_string(),
                ui_js: "console.log('ui');".to_string(),
                esm_dependencies: Vec::new(),
                worker_js: "export default {};".to_string(),
                npm_dependencies: vec![NpmDep {
                    name: "lodash".to_string(),
                    version: "^4.17.21".to_string(),
                }],
            },
            compiled_html: "<html></html>".to_string(),
            permissions: Default::default(),
            ai_context: None,
            runtime: Default::default(),
            i18n: None,
        }
    }
}
