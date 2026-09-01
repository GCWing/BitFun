use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceSearchAutoIndexPriority {
    Background,
    Focused,
}

#[derive(Debug, Default)]
struct QueueState {
    pending: VecDeque<PathBuf>,
    queued: HashMap<PathBuf, WorkspaceSearchAutoIndexPriority>,
    in_flight: HashMap<PathBuf, WorkspaceSearchAutoIndexPriority>,
    driver_running: bool,
}

#[derive(Debug, Default)]
pub(crate) struct AutoIndexQueue {
    state: Mutex<QueueState>,
}

impl AutoIndexQueue {
    pub(crate) async fn enqueue(
        &self,
        repo_root: PathBuf,
        priority: WorkspaceSearchAutoIndexPriority,
    ) -> bool {
        let mut state = self.state.lock().await;
        if let Some(existing_priority) = state.in_flight.get_mut(&repo_root) {
            if priority == WorkspaceSearchAutoIndexPriority::Focused {
                *existing_priority = WorkspaceSearchAutoIndexPriority::Focused;
            }
            return false;
        }

        if let Some(existing_priority) = state.queued.get_mut(&repo_root) {
            if priority == WorkspaceSearchAutoIndexPriority::Focused {
                *existing_priority = WorkspaceSearchAutoIndexPriority::Focused;
                state.pending.retain(|path| path != &repo_root);
                state.pending.push_front(repo_root);
            }
            return false;
        }

        state.queued.insert(repo_root.clone(), priority);
        match priority {
            WorkspaceSearchAutoIndexPriority::Background => state.pending.push_back(repo_root),
            WorkspaceSearchAutoIndexPriority::Focused => state.pending.push_front(repo_root),
        }

        if state.driver_running {
            false
        } else {
            state.driver_running = true;
            true
        }
    }

    pub(crate) async fn next(&self) -> Option<PathBuf> {
        let mut state = self.state.lock().await;
        let repo_root = state.pending.pop_front();
        if let Some(repo_root) = repo_root.as_ref() {
            let priority = state
                .queued
                .remove(repo_root)
                .unwrap_or(WorkspaceSearchAutoIndexPriority::Background);
            state.in_flight.insert(repo_root.clone(), priority);
        } else {
            state.driver_running = false;
        }
        repo_root
    }

    pub(crate) async fn complete(
        &self,
        repo_root: &PathBuf,
    ) -> Option<WorkspaceSearchAutoIndexPriority> {
        self.state.lock().await.in_flight.remove(repo_root)
    }

    pub(crate) async fn protected_roots(&self) -> Vec<PathBuf> {
        let state = self.state.lock().await;
        state
            .queued
            .keys()
            .chain(state.in_flight.keys())
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn focused_workspaces_jump_ahead_of_background_items() {
        let queue = AutoIndexQueue::default();
        assert!(
            queue
                .enqueue(
                    PathBuf::from("background-a"),
                    WorkspaceSearchAutoIndexPriority::Background,
                )
                .await
        );
        assert!(
            !queue
                .enqueue(
                    PathBuf::from("background-b"),
                    WorkspaceSearchAutoIndexPriority::Background,
                )
                .await
        );
        assert!(
            !queue
                .enqueue(
                    PathBuf::from("focused"),
                    WorkspaceSearchAutoIndexPriority::Focused,
                )
                .await
        );

        assert_eq!(queue.next().await, Some(PathBuf::from("focused")));
        assert_eq!(
            queue.complete(&PathBuf::from("focused")).await,
            Some(WorkspaceSearchAutoIndexPriority::Focused)
        );
        assert_eq!(queue.next().await, Some(PathBuf::from("background-a")));
        assert_eq!(
            queue.complete(&PathBuf::from("background-a")).await,
            Some(WorkspaceSearchAutoIndexPriority::Background)
        );
    }

    #[tokio::test]
    async fn in_flight_background_item_can_be_promoted_to_focused() {
        let queue = AutoIndexQueue::default();
        assert!(
            queue
                .enqueue(
                    PathBuf::from("repo"),
                    WorkspaceSearchAutoIndexPriority::Background,
                )
                .await
        );
        assert!(
            !queue
                .enqueue(
                    PathBuf::from("repo"),
                    WorkspaceSearchAutoIndexPriority::Background,
                )
                .await
        );
        assert_eq!(queue.next().await, Some(PathBuf::from("repo")));
        assert!(
            !queue
                .enqueue(
                    PathBuf::from("repo"),
                    WorkspaceSearchAutoIndexPriority::Focused,
                )
                .await
        );
        assert_eq!(
            queue.complete(&PathBuf::from("repo")).await,
            Some(WorkspaceSearchAutoIndexPriority::Focused)
        );
        assert_eq!(queue.next().await, None);
    }

    #[tokio::test]
    async fn queued_background_item_keeps_focused_priority_after_promotion() {
        let queue = AutoIndexQueue::default();
        assert!(
            queue
                .enqueue(
                    PathBuf::from("driver"),
                    WorkspaceSearchAutoIndexPriority::Background,
                )
                .await
        );
        assert!(
            !queue
                .enqueue(
                    PathBuf::from("repo"),
                    WorkspaceSearchAutoIndexPriority::Background,
                )
                .await
        );
        assert!(
            !queue
                .enqueue(
                    PathBuf::from("repo"),
                    WorkspaceSearchAutoIndexPriority::Focused,
                )
                .await
        );

        assert_eq!(queue.next().await, Some(PathBuf::from("repo")));
        assert_eq!(
            queue.complete(&PathBuf::from("repo")).await,
            Some(WorkspaceSearchAutoIndexPriority::Focused)
        );
    }
}
