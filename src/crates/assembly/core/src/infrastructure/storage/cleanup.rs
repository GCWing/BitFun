//! Compatibility wrapper for generic storage cleanup.

use crate::infrastructure::PathManager;
use crate::util::errors::*;

pub use openbitfun_services_core::storage_cleanup::{
    CleanupCategory, CleanupPolicy, CleanupResult,
};

pub struct CleanupService {
    inner: openbitfun_services_core::storage_cleanup::CleanupService,
}

impl CleanupService {
    pub fn new(path_manager: PathManager, policy: CleanupPolicy) -> Self {
        let roots = openbitfun_services_core::storage_cleanup::CleanupRoots {
            temp_dir: path_manager.temp_dir(),
            logs_dir: path_manager.logs_dir(),
            cache_dir: path_manager.cache_root(),
        };
        Self {
            inner: openbitfun_services_core::storage_cleanup::CleanupService::new(roots, policy),
        }
    }

    pub async fn cleanup_all(&self) -> OpenBitFunResult<CleanupResult> {
        self.inner
            .cleanup_all()
            .await
            .map_err(OpenBitFunError::service)
    }
}
