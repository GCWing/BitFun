use super::types::AnnouncementState;
use crate::infrastructure::app_paths::PathManager;
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use std::sync::Arc;

pub struct AnnouncementStateStore {
    inner: openbitfun_services_integrations::announcement::AnnouncementStateStore,
}

impl AnnouncementStateStore {
    pub fn new(path_manager: &Arc<PathManager>) -> Self {
        Self {
            inner: openbitfun_services_integrations::announcement::AnnouncementStateStore::new(
                path_manager.user_config_dir(),
            ),
        }
    }

    /// Load state from disk.  Returns a default state if the file does not exist.
    pub async fn load(&self) -> OpenBitFunResult<AnnouncementState> {
        self.inner.load().await.map_err(map_state_store_error)
    }

    /// Persist state to disk.
    pub async fn save(&self, state: &AnnouncementState) -> OpenBitFunResult<()> {
        self.inner.save(state).await.map_err(map_state_store_error)
    }
}

fn map_state_store_error(
    err: openbitfun_services_integrations::announcement::AnnouncementStateStoreError,
) -> OpenBitFunError {
    match err {
        openbitfun_services_integrations::announcement::AnnouncementStateStoreError::Io(err) => {
            OpenBitFunError::Io(err)
        }
        openbitfun_services_integrations::announcement::AnnouncementStateStoreError::Serialization(
            err,
        ) => OpenBitFunError::Serialization(err),
    }
}
