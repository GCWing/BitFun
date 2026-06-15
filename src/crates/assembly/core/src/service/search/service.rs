use crate::service::bootstrap::ensure_workspace_gitignore_ignores_bitfun;
use crate::service::config::{get_global_config_service, types::WorkspaceConfig};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_services_integrations::workspace_search as owner;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex as StdMutex, Weak};

static GLOBAL_WORKSPACE_SEARCH_SERVICE: LazyLock<StdMutex<Weak<WorkspaceSearchService>>> =
    LazyLock::new(|| StdMutex::new(Weak::new()));

pub struct WorkspaceSearchService {
    inner: Arc<owner::WorkspaceSearchService>,
}

impl WorkspaceSearchService {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(owner::WorkspaceSearchService::new_with_hooks(Arc::new(
                CoreWorkspaceSearchRuntimeHooks,
            ))),
        }
    }

    pub async fn open_repo(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> BitFunResult<owner::WorkspaceIndexStatus> {
        self.inner
            .open_repo(repo_root)
            .await
            .map_err(map_workspace_search_error)
    }

    pub async fn get_index_status(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> BitFunResult<owner::WorkspaceIndexStatus> {
        self.inner
            .get_index_status(repo_root)
            .await
            .map_err(map_workspace_search_error)
    }

    pub async fn build_index(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> BitFunResult<owner::IndexTaskHandle> {
        self.inner
            .build_index(repo_root)
            .await
            .map_err(map_workspace_search_error)
    }

    pub async fn rebuild_index(
        &self,
        repo_root: impl AsRef<Path>,
    ) -> BitFunResult<owner::IndexTaskHandle> {
        self.inner
            .rebuild_index(repo_root)
            .await
            .map_err(map_workspace_search_error)
    }

    pub async fn search_content(
        &self,
        request: owner::ContentSearchRequest,
    ) -> BitFunResult<owner::ContentSearchResult> {
        self.inner
            .search_content(request)
            .await
            .map_err(map_workspace_search_error)
    }

    pub async fn glob(
        &self,
        request: owner::GlobSearchRequest,
    ) -> BitFunResult<owner::GlobSearchResult> {
        self.inner
            .glob(request)
            .await
            .map_err(map_workspace_search_error)
    }

    pub fn schedule_repo_release(self: &Arc<Self>, repo_root: impl AsRef<Path>) {
        self.inner.schedule_repo_release(repo_root);
    }

    pub async fn shutdown_all_daemons(&self) {
        self.inner.shutdown_all_daemons().await;
    }

    pub async fn stop_all_daemons(&self) {
        self.inner.stop_all_daemons().await;
    }

    pub fn shutdown_blocking(self: &Arc<Self>) {
        self.inner.shutdown_blocking();
    }
}

impl Default for WorkspaceSearchService {
    fn default() -> Self {
        Self::new()
    }
}

struct CoreWorkspaceSearchRuntimeHooks;

#[async_trait]
impl owner::WorkspaceSearchRuntimeHooks for CoreWorkspaceSearchRuntimeHooks {
    async fn repo_config(&self) -> owner::WorkspaceSearchRepoConfig {
        let max_file_size = match get_global_config_service().await {
            Ok(config_service) => match config_service
                .get_config::<WorkspaceConfig>(Some("workspace"))
                .await
            {
                Ok(workspace_config) => workspace_config.max_file_size,
                Err(error) => {
                    log::warn!(
                        "Failed to read workspace config for flashgrep repo open, using default max_file_size: {}",
                        error
                    );
                    WorkspaceConfig::default().max_file_size
                }
            },
            Err(error) => {
                log::warn!(
                    "Global config service unavailable for flashgrep repo open, using default max_file_size: {}",
                    error
                );
                WorkspaceConfig::default().max_file_size
            }
        };

        owner::WorkspaceSearchRepoConfig { max_file_size }
    }

    async fn ensure_workspace_ready(&self, repo_root: &Path) -> owner::WorkspaceSearchResult<()> {
        ensure_workspace_gitignore_ignores_bitfun(repo_root)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

pub fn set_global_workspace_search_service(service: Arc<WorkspaceSearchService>) {
    let mut global = match GLOBAL_WORKSPACE_SEARCH_SERVICE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    *global = Arc::downgrade(&service);
}

pub fn get_global_workspace_search_service() -> Option<Arc<WorkspaceSearchService>> {
    let global = match GLOBAL_WORKSPACE_SEARCH_SERVICE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    global.upgrade()
}

pub fn workspace_search_daemon_binary_names() -> &'static [&'static str] {
    owner::workspace_search_daemon_binary_names()
}

pub fn workspace_search_daemon_binary_name() -> &'static str {
    owner::workspace_search_daemon_binary_name()
}

pub fn workspace_search_daemon_missing_hint() -> String {
    owner::workspace_search_daemon_missing_hint()
}

pub fn workspace_search_daemon_available() -> bool {
    owner::workspace_search_daemon_available()
}

pub async fn workspace_search_feature_enabled() -> bool {
    match get_global_config_service().await {
        Ok(config_service) => config_service
            .get_config::<bool>(Some("app.ai_experience.enable_workspace_search"))
            .await
            .unwrap_or(false),
        Err(_) => false,
    }
}

pub async fn workspace_search_runtime_available() -> bool {
    workspace_search_feature_enabled().await && workspace_search_daemon_available()
}

pub fn resolve_workspace_search_daemon_program_path() -> Option<PathBuf> {
    owner::resolve_workspace_search_daemon_program_path()
}

fn map_workspace_search_error(error: String) -> BitFunError {
    BitFunError::service(error)
}
