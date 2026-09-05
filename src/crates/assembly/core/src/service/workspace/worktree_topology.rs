use super::WorktreeTopologyFreshness;
use crate::service::git::{GitError, GitService, GitWorktreeInfo, GitWorktreeRepositoryInfo};
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::Mutex;

const WORKTREE_TOPOLOGY_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_CACHED_REPOSITORIES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MetadataFingerprint(u64);

#[derive(Clone)]
struct CachedTopology {
    worktrees: Arc<Vec<GitWorktreeInfo>>,
    fingerprint: MetadataFingerprint,
    refreshed_at: Instant,
    refresh_version: u64,
    last_used: u64,
}

#[derive(Default)]
struct CacheState {
    entries: HashMap<PathBuf, CachedTopology>,
    gates: HashMap<PathBuf, Arc<Mutex<()>>>,
    invalidation_versions: HashMap<PathBuf, u64>,
}

pub struct WorktreeTopologyService {
    state: Mutex<CacheState>,
    refresh_version: AtomicU64,
    invalidation_version: AtomicU64,
    access_tick: AtomicU64,
    #[cfg(test)]
    query_count: std::sync::atomic::AtomicUsize,
}

impl Default for WorktreeTopologyService {
    fn default() -> Self {
        Self {
            state: Mutex::new(CacheState::default()),
            refresh_version: AtomicU64::new(1),
            invalidation_version: AtomicU64::new(1),
            access_tick: AtomicU64::new(1),
            #[cfg(test)]
            query_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl WorktreeTopologyService {
    /// Proves that `candidate` is a live worktree root for the same repository
    /// as `registered_path`. This deliberately bypasses the cached topology:
    /// cached `git worktree list` entries can be prunable after their directory
    /// is deleted and must never authorize a newly-created ordinary directory.
    pub async fn is_live_worktree_root_in_same_repository(
        &self,
        registered_path: &Path,
        candidate: &Path,
    ) -> Result<bool, GitError> {
        let (registered, candidate_repository) = tokio::join!(
            GitService::resolve_worktree_repository(registered_path),
            GitService::resolve_worktree_repository(candidate),
        );
        let registered = registered?;
        let candidate_repository = candidate_repository?;
        let candidate = dunce::canonicalize(candidate)?;
        let candidate_root = dunce::canonicalize(&candidate_repository.query_path)?;
        let registered_common_git_dir = dunce::canonicalize(&registered.common_git_dir)?;
        let candidate_common_git_dir = dunce::canonicalize(&candidate_repository.common_git_dir)?;
        Ok(candidate == candidate_root && registered_common_git_dir == candidate_common_git_dir)
    }

    pub async fn list_worktrees(
        &self,
        path: &Path,
        freshness: WorktreeTopologyFreshness,
    ) -> Result<Vec<GitWorktreeInfo>, GitError> {
        let repository = GitService::resolve_worktree_repository(path).await?;
        let fingerprint = metadata_fingerprint(&repository);
        let access_tick = self.access_tick.fetch_add(1, Ordering::Relaxed);

        let (gate, observed_refresh_version, observed_invalidation_version) = {
            let mut state = self.state.lock().await;
            if freshness == WorktreeTopologyFreshness::Cached {
                if let Some(cached) = state.entries.get_mut(&repository.common_git_dir) {
                    if cached.refreshed_at.elapsed() < WORKTREE_TOPOLOGY_TTL
                        && cached.fingerprint == fingerprint
                    {
                        cached.last_used = access_tick;
                        return Ok(cached.worktrees.as_ref().clone());
                    }
                }
            }

            let observed_refresh_version = state
                .entries
                .get(&repository.common_git_dir)
                .map(|cached| cached.refresh_version)
                .unwrap_or_default();
            let gate = state
                .gates
                .entry(repository.common_git_dir.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone();
            let observed_invalidation_version = state
                .invalidation_versions
                .get(&repository.common_git_dir)
                .copied()
                .unwrap_or_default();
            (
                gate,
                observed_refresh_version,
                observed_invalidation_version,
            )
        };

        let _refresh_guard = gate.lock().await;
        let fingerprint = metadata_fingerprint(&repository);
        {
            let mut state = self.state.lock().await;
            if let Some(cached) = state.entries.get_mut(&repository.common_git_dir) {
                let another_request_refreshed = cached.refresh_version > observed_refresh_version;
                let cached_is_fresh = cached.refreshed_at.elapsed() < WORKTREE_TOPOLOGY_TTL
                    && cached.fingerprint == fingerprint;
                if another_request_refreshed
                    || (freshness == WorktreeTopologyFreshness::Cached && cached_is_fresh)
                {
                    cached.last_used = access_tick;
                    return Ok(cached.worktrees.as_ref().clone());
                }
            }
        }

        #[cfg(test)]
        {
            self.query_count.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        let worktrees = GitService::list_worktrees(&repository.query_path).await?;
        let cached = CachedTopology {
            worktrees: Arc::new(worktrees.clone()),
            fingerprint: metadata_fingerprint(&repository),
            refreshed_at: Instant::now(),
            refresh_version: self.refresh_version.fetch_add(1, Ordering::Relaxed),
            last_used: access_tick,
        };

        let mut state = self.state.lock().await;
        let invalidated_during_query = state
            .invalidation_versions
            .get(&repository.common_git_dir)
            .copied()
            .unwrap_or_default()
            > observed_invalidation_version;
        if invalidated_during_query {
            return Ok(worktrees);
        }
        if state.entries.len() >= MAX_CACHED_REPOSITORIES
            && !state.entries.contains_key(&repository.common_git_dir)
        {
            if let Some(oldest) = state
                .entries
                .iter()
                .min_by_key(|(_, cached)| cached.last_used)
                .map(|(path, _)| path.clone())
            {
                state.entries.remove(&oldest);
                if state
                    .gates
                    .get(&oldest)
                    .map(Arc::strong_count)
                    .unwrap_or_default()
                    == 1
                {
                    state.gates.remove(&oldest);
                }
            }
        }
        state.entries.insert(repository.common_git_dir, cached);
        Ok(worktrees)
    }

    pub async fn invalidate(&self, path: &Path) {
        let Ok(repository) = GitService::resolve_worktree_repository(path).await else {
            return;
        };
        let mut state = self.state.lock().await;
        state.entries.remove(&repository.common_git_dir);
        state.invalidation_versions.insert(
            repository.common_git_dir,
            self.invalidation_version.fetch_add(1, Ordering::Relaxed),
        );
    }

    #[cfg(test)]
    fn query_count(&self) -> usize {
        self.query_count.load(Ordering::Relaxed)
    }
}

pub fn global_worktree_topology_service() -> &'static WorktreeTopologyService {
    static SERVICE: OnceLock<WorktreeTopologyService> = OnceLock::new();
    SERVICE.get_or_init(WorktreeTopologyService::default)
}

fn metadata_fingerprint(repository: &GitWorktreeRepositoryInfo) -> MetadataFingerprint {
    let mut hasher = DefaultHasher::new();
    hash_metadata_path(&repository.common_git_dir.join("config"), &mut hasher, 0);
    hash_metadata_path(&repository.common_git_dir.join("worktrees"), &mut hasher, 2);
    if repository.worktree_git_marker.is_file() {
        hash_metadata_path(&repository.worktree_git_marker, &mut hasher, 0);
    }
    MetadataFingerprint(hasher.finish())
}

fn hash_metadata_path(path: &Path, hasher: &mut DefaultHasher, remaining_depth: usize) {
    path.hash(hasher);
    match std::fs::metadata(path) {
        Ok(metadata) => {
            true.hash(hasher);
            metadata.len().hash(hasher);
            metadata.is_dir().hash(hasher);
            metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos())
                .hash(hasher);

            if metadata.is_dir() && remaining_depth > 0 {
                let mut children = std::fs::read_dir(path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>();
                children.sort();
                children.len().hash(hasher);
                for child in children {
                    hash_metadata_path(&child, hasher, remaining_depth - 1);
                }
            }
        }
        Err(_) => false.hash(hasher),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let output = crate::util::create_test_command("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("git should be available for worktree topology tests");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn initialized_repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temporary repository should be created");
        git(directory.path(), &["init"]);
        git(
            directory.path(),
            &["config", "user.name", "OpenBitFun Tests"],
        );
        git(
            directory.path(),
            &["config", "user.email", "openbitfun@example.com"],
        );
        std::fs::write(directory.path().join("tracked.txt"), "initial\n")
            .expect("fixture should be written");
        git(directory.path(), &["add", "tracked.txt"]);
        git(directory.path(), &["commit", "-m", "initial"]);
        directory
    }

    #[tokio::test]
    async fn concurrent_main_and_linked_reads_share_one_query() {
        let repository = initialized_repository();
        let linked_root = repository.path().join("linked");
        git(
            repository.path(),
            &[
                "worktree",
                "add",
                "-b",
                "linked-test",
                linked_root.to_string_lossy().as_ref(),
            ],
        );

        let service = Arc::new(WorktreeTopologyService::default());
        let barrier = Arc::new(tokio::sync::Barrier::new(8));
        let mut tasks = Vec::new();
        for index in 0..8 {
            let service = Arc::clone(&service);
            let barrier = Arc::clone(&barrier);
            let path = if index % 2 == 0 {
                repository.path().to_path_buf()
            } else {
                linked_root.clone()
            };
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                service
                    .list_worktrees(&path, WorktreeTopologyFreshness::Cached)
                    .await
                    .expect("topology should load")
            }));
        }

        for task in tasks {
            assert_eq!(task.await.expect("task should join").len(), 2);
        }
        assert_eq!(service.query_count(), 1);
    }

    #[tokio::test]
    async fn metadata_changes_and_explicit_invalidation_refresh_the_cache() {
        let repository = initialized_repository();
        let service = WorktreeTopologyService::default();

        let initial = service
            .list_worktrees(repository.path(), WorktreeTopologyFreshness::Cached)
            .await
            .expect("initial topology should load");
        assert_eq!(initial.len(), 1);
        assert_eq!(service.query_count(), 1);

        let linked_root = repository.path().join("external-linked");
        git(
            repository.path(),
            &[
                "worktree",
                "add",
                "-b",
                "external-linked-test",
                linked_root.to_string_lossy().as_ref(),
            ],
        );

        let refreshed = service
            .list_worktrees(repository.path(), WorktreeTopologyFreshness::Cached)
            .await
            .expect("metadata change should refresh topology");
        assert_eq!(refreshed.len(), 2);
        assert_eq!(service.query_count(), 2);

        service.invalidate(repository.path()).await;
        let after_invalidation = service
            .list_worktrees(repository.path(), WorktreeTopologyFreshness::Cached)
            .await
            .expect("invalidated topology should reload");
        assert_eq!(after_invalidation.len(), 2);
        assert_eq!(service.query_count(), 3);
    }

    #[tokio::test]
    async fn live_repository_identity_rejects_a_recreated_prunable_worktree_path() {
        let repository = initialized_repository();
        let linked_name = format!(
            "{}-live-membership",
            repository.path().file_name().unwrap().to_string_lossy()
        );
        let linked_root = repository.path().parent().unwrap().join(linked_name);
        git(
            repository.path(),
            &[
                "worktree",
                "add",
                "-b",
                "live-membership-test",
                linked_root.to_string_lossy().as_ref(),
            ],
        );
        let service = WorktreeTopologyService::default();

        assert!(service
            .is_live_worktree_root_in_same_repository(repository.path(), &linked_root)
            .await
            .unwrap());
        service
            .list_worktrees(repository.path(), WorktreeTopologyFreshness::Cached)
            .await
            .expect("topology should be cached before the worktree becomes stale");

        std::fs::remove_dir_all(&linked_root).expect("linked worktree should be removed manually");
        std::fs::create_dir_all(&linked_root)
            .expect("ordinary directory should replace the stale worktree path");

        assert!(!service
            .is_live_worktree_root_in_same_repository(repository.path(), &linked_root)
            .await
            .unwrap_or(false));

        std::fs::remove_dir_all(&linked_root).expect("replacement directory should be cleaned up");
    }

    #[tokio::test]
    async fn invalidation_during_query_does_not_restore_stale_cache_data() {
        let repository = initialized_repository();
        let service = Arc::new(WorktreeTopologyService::default());
        let query_service = Arc::clone(&service);
        let repository_path = repository.path().to_path_buf();
        let query = tokio::spawn(async move {
            query_service
                .list_worktrees(&repository_path, WorktreeTopologyFreshness::Cached)
                .await
                .expect("topology query should complete")
        });

        while service.query_count() == 0 {
            tokio::task::yield_now().await;
        }
        service.invalidate(repository.path()).await;
        assert_eq!(query.await.expect("query task should join").len(), 1);

        service
            .list_worktrees(repository.path(), WorktreeTopologyFreshness::Cached)
            .await
            .expect("post-invalidation topology should reload");
        assert_eq!(service.query_count(), 2);
    }
}
