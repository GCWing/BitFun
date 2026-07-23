//! Compatibility wrapper for generic JSON persistence storage.

use crate::infrastructure::{try_get_path_manager_arc, PathManager};
use crate::util::errors::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use bitfun_services_core::persistence::StorageOptions;

pub struct PersistenceService {
    inner: bitfun_services_core::persistence::PersistenceService,
    path_manager: Arc<PathManager>,
}

impl PersistenceService {
    pub async fn new(base_dir: PathBuf) -> BitFunResult<Self> {
        let path_manager = try_get_path_manager_arc()?;
        let inner = bitfun_services_core::persistence::PersistenceService::new(base_dir)
            .await
            .map_err(BitFunError::service)?;
        Ok(Self {
            inner,
            path_manager,
        })
    }

    pub async fn new_user_level(path_manager: Arc<PathManager>) -> BitFunResult<Self> {
        let base_dir = path_manager.user_data_dir();
        path_manager.ensure_dir(&base_dir).await?;
        Ok(Self {
            inner: bitfun_services_core::persistence::PersistenceService::from_base_dir(base_dir),
            path_manager,
        })
    }

    pub async fn new_project_level(
        path_manager: Arc<PathManager>,
        workspace_path: PathBuf,
    ) -> BitFunResult<Self> {
        let base_dir = path_manager.project_runtime_root(&workspace_path);
        Ok(Self {
            inner: bitfun_services_core::persistence::PersistenceService::from_base_dir(base_dir),
            path_manager,
        })
    }

    pub fn base_dir(&self) -> &Path {
        self.inner.base_dir()
    }

    pub fn path_manager(&self) -> &Arc<PathManager> {
        &self.path_manager
    }

    pub async fn save_json<T: serde::Serialize>(
        &self,
        key: &str,
        data: &T,
        options: StorageOptions,
    ) -> BitFunResult<()> {
        self.inner
            .save_json(key, data, options)
            .await
            .map_err(BitFunError::service)
    }

    pub async fn load_json<T: for<'de> serde::Deserialize<'de>>(
        &self,
        key: &str,
    ) -> BitFunResult<Option<T>> {
        self.inner
            .load_json(key)
            .await
            .map_err(BitFunError::service)
    }

    pub async fn delete(&self, key: &str) -> BitFunResult<bool> {
        self.inner.delete(key).await.map_err(BitFunError::service)
    }
}
