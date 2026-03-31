use super::PersistenceManager;
use crate::util::errors::BitFunResult;
use dashmap::{DashMap, DashSet};
use log::info;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionWorkspaceMaintenanceReport {
    pub scanned_sessions: usize,
    pub hidden_sessions: usize,
    pub deleted_sessions: usize,
    pub skipped: bool,
}

pub struct SessionWorkspaceMaintenanceService {
    persistence_manager: Arc<PersistenceManager>,
    cleaned_workspaces: DashSet<PathBuf>,
    workspace_locks: DashMap<PathBuf, Arc<Mutex<()>>>,
}

impl SessionWorkspaceMaintenanceService {
    pub fn new(persistence_manager: Arc<PersistenceManager>) -> Self {
        Self {
            persistence_manager,
            cleaned_workspaces: DashSet::new(),
            workspace_locks: DashMap::new(),
        }
    }

    pub async fn ensure_workspace_maintained(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<SessionWorkspaceMaintenanceReport> {
        let workspace_key = workspace_path.to_path_buf();

        if self.cleaned_workspaces.contains(&workspace_key) {
            return Ok(SessionWorkspaceMaintenanceReport {
                skipped: true,
                ..Default::default()
            });
        }

        let workspace_lock = self
            .workspace_locks
            .entry(workspace_key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _guard = workspace_lock.lock().await;

        if self.cleaned_workspaces.contains(&workspace_key) {
            return Ok(SessionWorkspaceMaintenanceReport {
                skipped: true,
                ..Default::default()
            });
        }

        let report = self.run_workspace_maintenance(workspace_path).await?;
        self.cleaned_workspaces.insert(workspace_key);

        Ok(report)
    }

    async fn run_workspace_maintenance(
        &self,
        workspace_path: &Path,
    ) -> BitFunResult<SessionWorkspaceMaintenanceReport> {
        if !workspace_path.exists() {
            return Ok(SessionWorkspaceMaintenanceReport::default());
        }

        let all_metadata = self
            .persistence_manager
            .list_session_metadata_including_internal(workspace_path)
            .await?;
        let hidden_session_ids = all_metadata
            .iter()
            .filter(|metadata| metadata.should_hide_from_user_lists())
            .map(|metadata| metadata.session_id.clone())
            .collect::<Vec<_>>();

        let mut report = SessionWorkspaceMaintenanceReport {
            scanned_sessions: all_metadata.len(),
            hidden_sessions: hidden_session_ids.len(),
            deleted_sessions: 0,
            skipped: false,
        };

        for session_id in hidden_session_ids {
            self.persistence_manager
                .delete_session(workspace_path, &session_id)
                .await?;
            report.deleted_sessions += 1;
        }

        if report.deleted_sessions > 0 {
            info!(
                "Workspace session maintenance removed hidden sessions: workspace_path={}, scanned_sessions={}, hidden_sessions={}, deleted_sessions={}",
                workspace_path.display(),
                report.scanned_sessions,
                report.hidden_sessions,
                report.deleted_sessions
            );
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::SessionWorkspaceMaintenanceService;
    use crate::agentic::core::SessionKind;
    use crate::agentic::persistence::PersistenceManager;
    use crate::infrastructure::PathManager;
    use crate::service::session::SessionMetadata;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use uuid::Uuid;

    struct TestWorkspace {
        path: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bitfun-session-maintenance-test-{}",
                Uuid::new_v4()
            ));
            std::fs::create_dir_all(&path).expect("test workspace should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[tokio::test]
    async fn workspace_maintenance_removes_hidden_sessions_once() {
        let workspace = TestWorkspace::new();
        let persistence_manager = Arc::new(
            PersistenceManager::new(Arc::new(PathManager::new().expect("path manager")))
                .expect("persistence manager"),
        );
        let maintenance = SessionWorkspaceMaintenanceService::new(persistence_manager.clone());

        let visible = SessionMetadata::new(
            Uuid::new_v4().to_string(),
            "Visible Session".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );

        let mut legacy_hidden = SessionMetadata::new(
            Uuid::new_v4().to_string(),
            "Subagent: stale task".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        legacy_hidden.created_by = Some("session-parent".to_string());

        let mut subagent_hidden = SessionMetadata::new(
            Uuid::new_v4().to_string(),
            "Subagent: fresh task".to_string(),
            "agent".to_string(),
            "model".to_string(),
        );
        subagent_hidden.session_kind = SessionKind::Subagent;

        for metadata in [&visible, &legacy_hidden, &subagent_hidden] {
            persistence_manager
                .save_session_metadata(workspace.path(), metadata)
                .await
                .expect("metadata should save");
        }

        let first_report = maintenance
            .ensure_workspace_maintained(workspace.path())
            .await
            .expect("maintenance should succeed");

        assert_eq!(first_report.scanned_sessions, 3);
        assert_eq!(first_report.hidden_sessions, 2);
        assert_eq!(first_report.deleted_sessions, 2);
        assert!(!first_report.skipped);

        let raw_after_cleanup = persistence_manager
            .list_session_metadata_including_internal(workspace.path())
            .await
            .expect("raw metadata should load");
        assert_eq!(raw_after_cleanup.len(), 1);
        assert_eq!(raw_after_cleanup[0].session_id, visible.session_id);

        let visible_after_cleanup = persistence_manager
            .list_session_metadata(workspace.path())
            .await
            .expect("visible metadata should load");
        assert_eq!(visible_after_cleanup.len(), 1);
        assert_eq!(visible_after_cleanup[0].session_id, visible.session_id);

        let second_report = maintenance
            .ensure_workspace_maintained(workspace.path())
            .await
            .expect("second maintenance should succeed");
        assert!(second_report.skipped);
        assert_eq!(second_report.deleted_sessions, 0);
    }
}
