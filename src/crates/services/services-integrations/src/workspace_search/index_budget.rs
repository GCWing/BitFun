use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) const DEFAULT_INDEX_DISK_BUDGET_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Single definition of where a workspace's local flashgrep index lives.
///
/// Budget maintenance and session creation must agree on this path, otherwise
/// maintenance silently stops finding the indexes it is meant to reclaim.
pub(crate) fn storage_root(repo_root: &Path) -> PathBuf {
    repo_root
        .join(openbitfun_services_core::product_identity::hidden_data_directory())
        .join("search")
        .join("flashgrep-index")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexBudgetReport {
    pub total_before: u64,
    pub total_after: u64,
    pub removed: Vec<PathBuf>,
    pub over_budget: bool,
}

#[derive(Debug)]
struct IndexDirectory {
    repo_root: PathBuf,
    path: PathBuf,
    bytes: u64,
    modified: SystemTime,
    recency_rank: usize,
}

pub(crate) fn enforce(
    workspace_roots: Vec<PathBuf>,
    protected_roots: HashSet<PathBuf>,
) -> Result<IndexBudgetReport, String> {
    enforce_with_budget(
        workspace_roots,
        protected_roots,
        DEFAULT_INDEX_DISK_BUDGET_BYTES,
    )
}

pub(crate) fn remove_for_repo(repo_root: PathBuf) -> Result<bool, String> {
    let path = storage_root(&repo_root);
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(&path).map_err(|error| {
                format!(
                    "failed to remove workspace search index {}: {error}",
                    path.display()
                )
            })?;
            Ok(true)
        }
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "failed to inspect workspace search index {}: {error}",
            path.display()
        )),
    }
}

fn enforce_with_budget(
    workspace_roots: Vec<PathBuf>,
    protected_roots: HashSet<PathBuf>,
    budget_bytes: u64,
) -> Result<IndexBudgetReport, String> {
    let mut seen = HashSet::new();
    let mut indexes = Vec::new();
    let recency = workspace_roots
        .iter()
        .enumerate()
        .map(|(rank, root)| (root.clone(), rank))
        .collect::<HashMap<_, _>>();

    for repo_root in workspace_roots {
        if !seen.insert(repo_root.clone()) {
            continue;
        }
        let path = storage_root(&repo_root);
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if !metadata.is_dir() {
            continue;
        }
        let bytes = directory_size(&path)?;
        indexes.push(IndexDirectory {
            repo_root: repo_root.clone(),
            path,
            bytes,
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            recency_rank: recency.get(&repo_root).copied().unwrap_or(usize::MAX),
        });
    }

    let total_before = indexes.iter().map(|index| index.bytes).sum::<u64>();
    let mut total_after = total_before;
    let mut candidates = indexes;
    candidates.sort_by(|left, right| {
        right
            .recency_rank
            .cmp(&left.recency_rank)
            .then_with(|| left.modified.cmp(&right.modified))
    });

    let mut removed = Vec::new();
    for index in candidates {
        if total_after <= budget_bytes {
            break;
        }
        if protected_roots.contains(&index.repo_root) {
            continue;
        }
        fs::remove_dir_all(&index.path).map_err(|error| {
            format!(
                "failed to remove workspace search index {}: {error}",
                index.path.display()
            )
        })?;
        total_after = total_after.saturating_sub(index.bytes);
        removed.push(index.repo_root);
    }

    Ok(IndexBudgetReport {
        total_before,
        total_after,
        removed,
        over_budget: total_after > budget_bytes,
    })
}

fn directory_size(path: &Path) -> Result<u64, String> {
    let mut total = 0_u64;
    for entry in fs::read_dir(path)
        .map_err(|error| format!("failed to read index directory {}: {error}", path.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect index directory {}: {error}",
                path.display()
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect index entry {}: {error}",
                entry.path().display()
            )
        })?;
        if file_type.is_dir() {
            total = total.saturating_add(directory_size(&entry.path())?);
        } else if file_type.is_file() {
            total = total.saturating_add(
                entry
                    .metadata()
                    .map_err(|error| {
                        format!(
                            "failed to read index entry metadata {}: {error}",
                            entry.path().display()
                        )
                    })?
                    .len(),
            );
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::TempDir;

    fn make_index(root: &Path, bytes: usize) {
        let index = root
            .join(openbitfun_services_core::product_identity::hidden_data_directory())
            .join("search")
            .join("flashgrep-index");
        fs::create_dir_all(&index).expect("index directory should be created");
        write(index.join("payload"), vec![b'x'; bytes]).expect("index payload should be written");
    }

    #[test]
    fn evicts_oldest_unprotected_indexes_until_budget_is_met() {
        let temp = TempDir::new().expect("temp dir should be created");
        let newest = temp.path().join("newest");
        let older = temp.path().join("older");
        let oldest = temp.path().join("oldest");
        make_index(&newest, 4);
        make_index(&older, 4);
        make_index(&oldest, 4);

        let report = enforce_with_budget(
            vec![newest.clone(), older.clone(), oldest.clone()],
            HashSet::from([newest.clone()]),
            8,
        )
        .expect("budget enforcement should succeed");

        assert_eq!(report.total_before, 12);
        assert_eq!(report.total_after, 8);
        assert_eq!(report.removed, vec![oldest]);
        assert!(storage_root(&newest).exists());
        assert!(storage_root(&older).exists());
        assert!(!report.over_budget);
    }

    #[test]
    fn reports_unavoidable_over_budget_when_all_indexes_are_protected() {
        let temp = TempDir::new().expect("temp dir should be created");
        let root = temp.path().join("root");
        make_index(&root, 4);

        let report = enforce_with_budget(vec![root.clone()], HashSet::from([root]), 1)
            .expect("budget enforcement should succeed");

        assert_eq!(report.total_before, 4);
        assert_eq!(report.total_after, 4);
        assert!(report.over_budget);
        assert!(report.removed.is_empty());
    }
}
