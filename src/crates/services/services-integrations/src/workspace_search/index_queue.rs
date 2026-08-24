use std::collections::{HashSet, VecDeque};
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
    queued: HashSet<PathBuf>,
    in_flight: HashSet<PathBuf>,
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
        if state.in_flight.contains(&repo_root) {
            return false;
        }

        if state.queued.contains(&repo_root) {
            if priority == WorkspaceSearchAutoIndexPriority::Focused {
                state.pending.retain(|path| path != &repo_root);
                state.pending.push_front(repo_root);
            }
            return false;
        }

        state.queued.insert(repo_root.clone());
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
            state.queued.remove(repo_root);
            state.in_flight.insert(repo_root.clone());
        } else {
            state.driver_running = false;
        }
        repo_root
    }

    pub(crate) async fn complete(&self, repo_root: &PathBuf) {
        self.state.lock().await.in_flight.remove(repo_root);
    }

    pub(crate) async fn protected_roots(&self) -> Vec<PathBuf> {
        let state = self.state.lock().await;
        state
            .queued
            .iter()
            .chain(state.in_flight.iter())
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
        queue.complete(&PathBuf::from("focused")).await;
        assert_eq!(queue.next().await, Some(PathBuf::from("background-a")));
    }

    #[tokio::test]
    async fn duplicate_items_are_deduplicated_and_in_flight_items_are_ignored() {
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
        queue.complete(&PathBuf::from("repo")).await;
        assert_eq!(queue.next().await, None);
    }
}
