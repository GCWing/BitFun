use crate::api::app_state::AppState;
use bitfun_core::service::search::workspace_search_runtime_available;
use bitfun_core::service::remote_ssh::workspace_state::is_remote_path;
use bitfun_core::service::workspace::{WorkspaceInfo, WorkspaceKind};
use log::{debug, info, warn};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

pub fn spawn_workspace_background_warmup(state: &AppState, workspace_info: WorkspaceInfo) {
    let workspace_path = state.workspace_path.clone();
    let agent_registry = state.agent_registry.clone();
    let workspace_search_service = state.workspace_search_service.clone();

    tokio::spawn(async move {
        warm_workspace_background_services(
            workspace_path,
            agent_registry,
            workspace_search_service,
            workspace_info,
        )
        .await;
    });
}

async fn warm_workspace_background_services(
    workspace_path: Arc<RwLock<Option<PathBuf>>>,
    agent_registry: Arc<bitfun_core::agentic::agents::AgentRegistry>,
    workspace_search_service: Arc<bitfun_core::service::search::WorkspaceSearchService>,
    workspace_info: WorkspaceInfo,
) {
    let started_at = Instant::now();
    let target_path = workspace_info.root_path.clone();
    let root_str = target_path.to_string_lossy().to_string();
    let skip_local_snapshot = workspace_info.workspace_kind == WorkspaceKind::Remote
        || is_remote_path(root_str.trim()).await;

    if !skip_local_snapshot && is_workspace_active(&workspace_path, &target_path).await {
        let snapshot_started_at = Instant::now();
        if let Err(error) =
            bitfun_core::service::snapshot::initialize_snapshot_manager_for_workspace(
                target_path.clone(),
                None,
            )
            .await
        {
            warn!(
                "Failed to initialize snapshot system during workspace warmup: path={}, error={}",
                target_path.display(),
                error
            );
        } else {
            debug!(
                "Workspace snapshot warmup completed: path={}, elapsed_ms={}",
                target_path.display(),
                snapshot_started_at.elapsed().as_millis()
            );
        }
    }

    if is_workspace_active(&workspace_path, &target_path).await {
        let subagents_started_at = Instant::now();
        agent_registry.load_custom_subagents(&target_path).await;
        debug!(
            "Workspace custom subagent warmup completed: path={}, elapsed_ms={}",
            target_path.display(),
            subagents_started_at.elapsed().as_millis()
        );
    }

    if workspace_info.workspace_kind != WorkspaceKind::Remote
        && is_workspace_active(&workspace_path, &target_path).await
        && workspace_search_runtime_available().await
    {
        let search_started_at = Instant::now();
        match workspace_search_service.open_repo(&target_path).await {
            Ok(_) => {
                let still_active = is_workspace_active(&workspace_path, &target_path).await;
                if !still_active {
                    workspace_search_service.schedule_repo_release(target_path.clone());
                    debug!(
                        "Released flashgrep warmup session for inactive workspace: path={}",
                        target_path.display()
                    );
                }
                info!(
                    "Workspace search warmup completed: path={}, elapsed_ms={}, active_after_open={}",
                    target_path.display(),
                    search_started_at.elapsed().as_millis(),
                    still_active
                );
            }
            Err(error) => {
                warn!(
                    "Failed to open workspace search repository session during warmup: path={}, error={}",
                    target_path.display(),
                    error
                );
            }
        }
    }

    debug!(
        "Workspace background warmup completed: path={}, total_elapsed_ms={}",
        target_path.display(),
        started_at.elapsed().as_millis()
    );
}

async fn is_workspace_active(
    workspace_path: &Arc<RwLock<Option<PathBuf>>>,
    target_path: &Path,
) -> bool {
    workspace_path
        .read()
        .await
        .as_ref()
        .is_some_and(|current| current == target_path)
}
