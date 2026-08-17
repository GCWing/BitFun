use super::service::GitService;
use super::types::{GitLocalChangeSummary, GitWorktreeInfo};
use super::utils::{execute_git_command, open_repository};
use super::GitError;
use bitfun_services_core::process_manager;
use git2::Repository;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::task;

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_nul_paths(bytes: &[u8]) -> Result<Vec<String>, GitError> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|value| !value.is_empty())
        .map(|value| {
            String::from_utf8(value.to_vec()).map_err(|error| {
                GitError::ParseError(format!("Git returned a non-UTF-8 path: {error}"))
            })
        })
        .collect()
}

fn validate_relative_file_path(path: &str) -> Result<PathBuf, GitError> {
    if path.trim().is_empty() || path.contains('\0') {
        return Err(GitError::InvalidPath(
            "Worktree copy paths must be non-empty relative paths".to_string(),
        ));
    }
    let path = PathBuf::from(path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || path.components().any(|component| {
            matches!(
                component,
                Component::Normal(name)
                    if name.to_string_lossy().eq_ignore_ascii_case(".git")
            )
        })
    {
        return Err(GitError::InvalidPath(format!(
            "Worktree copy path escapes the repository: {}",
            path.display()
        )));
    }
    Ok(path)
}

async fn git_output_bytes(repo_path: &Path, args: &[&str]) -> Result<Vec<u8>, GitError> {
    let output = process_manager::create_tokio_command("git")
        .current_dir(repo_path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .await
        .map_err(|error| {
            GitError::CommandFailed(format!("Failed to execute git command: {error}"))
        })?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(GitError::CommandFailed(if stderr.is_empty() {
            stdout
        } else {
            stderr
        }))
    }
}

async fn git_with_stdin(repo_path: &Path, args: &[&str], input: &[u8]) -> Result<(), GitError> {
    if input.is_empty() {
        return Ok(());
    }

    let mut child = process_manager::create_tokio_command("git");
    child
        .current_dir(repo_path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = child.spawn().map_err(|error| {
        GitError::CommandFailed(format!("Failed to execute git command: {error}"))
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| GitError::CommandFailed("Failed to open git stdin".to_string()))?;
    stdin
        .write_all(input)
        .await
        .map_err(|error| GitError::CommandFailed(format!("Failed to write git stdin: {error}")))?;
    drop(stdin);

    let output = child.wait_with_output().await.map_err(|error| {
        GitError::CommandFailed(format!("Failed to wait for git command: {error}"))
    })?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(GitError::CommandFailed(if stderr.is_empty() {
            stdout
        } else {
            stderr
        }))
    }
}

async fn ignored_include_paths(source: &Path) -> Result<Vec<String>, GitError> {
    let include_path = source.join(".worktreeinclude");
    let metadata = match tokio::fs::symlink_metadata(&include_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(GitError::IoError(error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(GitError::InvalidPath(
            ".worktreeinclude must be a regular file".to_string(),
        ));
    }
    let contents = tokio::fs::read_to_string(&include_path)
        .await
        .map_err(GitError::IoError)?;
    let patterns = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect::<Vec<_>>();
    for pattern in &patterns {
        validate_relative_file_path(pattern)?;
    }
    if patterns.is_empty() {
        return Ok(Vec::new());
    }

    let mut owned_args = vec![
        "ls-files".to_string(),
        "--others".to_string(),
        "--ignored".to_string(),
        "--exclude-standard".to_string(),
        "-z".to_string(),
        "--".to_string(),
    ];
    owned_args.extend(patterns);
    let args = owned_args.iter().map(String::as_str).collect::<Vec<_>>();
    parse_nul_paths(&git_output_bytes(source, &args).await?)
}

async fn copy_regular_files(
    source_root: &Path,
    target_root: &Path,
    paths: &[String],
) -> Result<(), GitError> {
    let source_root = source_root.to_path_buf();
    let target_root = target_root.to_path_buf();
    let paths = paths.to_vec();
    task::spawn_blocking(move || {
        fn validate_ancestors(
            root: &Path,
            relative: &Path,
            require_existing: bool,
        ) -> Result<(), GitError> {
            let root_metadata = std::fs::symlink_metadata(root).map_err(GitError::IoError)?;
            if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
                return Err(GitError::InvalidPath(format!(
                    "Worktree copy root must be a regular directory: {}",
                    root.display()
                )));
            }

            let mut current = root.to_path_buf();
            if let Some(parent) = relative.parent() {
                for component in parent.components() {
                    current.push(component.as_os_str());
                    match std::fs::symlink_metadata(&current) {
                        Ok(metadata) => {
                            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                                return Err(GitError::InvalidPath(format!(
                                    "Worktree copy does not follow symlink or non-directory ancestors: {}",
                                    current.display()
                                )));
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            if require_existing {
                                return Err(GitError::IoError(error));
                            }
                            break;
                        }
                        Err(error) => return Err(GitError::IoError(error)),
                    }
                }
            }
            Ok(())
        }

        for relative in paths {
            let relative = validate_relative_file_path(&relative)?;
            validate_ancestors(&source_root, &relative, true)?;
            validate_ancestors(&target_root, &relative, false)?;
            let source = source_root.join(&relative);
            let target = target_root.join(&relative);
            let metadata = std::fs::symlink_metadata(&source).map_err(GitError::IoError)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(GitError::InvalidPath(format!(
                    "Worktree copy only accepts regular files: {}",
                    relative.display()
                )));
            }
            match std::fs::symlink_metadata(&target) {
                Ok(_) => {
                    return Err(GitError::InvalidPath(format!(
                        "Worktree copy would overwrite an existing file: {}",
                        relative.display()
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(GitError::IoError(error)),
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(GitError::IoError)?;
            }
            std::fs::copy(&source, &target).map_err(GitError::IoError)?;
        }
        Ok(())
    })
    .await
    .map_err(|error| GitError::CommandFailed(format!("spawn_blocking join: {error}")))?
}

impl GitService {
    /// Creates an explicit-path detached worktree at an immutable commit.
    pub async fn add_detached_worktree<P: AsRef<Path>, Q: AsRef<Path>>(
        repository_path: P,
        target_path: Q,
        commit: &str,
    ) -> Result<GitWorktreeInfo, GitError> {
        let repository_path = repository_path.as_ref().to_path_buf();
        let target_path = target_path.as_ref().to_path_buf();
        let commit = commit.trim().to_string();
        if commit.is_empty() {
            return Err(GitError::CommandFailed(
                "Detached worktree requires an immutable commit".to_string(),
            ));
        }
        match tokio::fs::symlink_metadata(&target_path).await {
            Ok(_) => {
                return Err(GitError::InvalidPath(format!(
                    "Worktree target already exists: {}",
                    target_path.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(GitError::IoError(error)),
        }
        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(GitError::IoError)?;
        }

        let repository = normalized_path(&repository_path);
        let target = normalized_path(&target_path);
        execute_git_command(
            &repository,
            &["worktree", "add", "--detach", &target, &commit],
        )
        .await?;

        let inspect_path = target_path.clone();
        task::spawn_blocking(move || {
            let repository = Repository::open(&inspect_path).map_err(|error| {
                GitError::CommandFailed(format!(
                    "Failed to inspect newly created detached worktree: {error}"
                ))
            })?;
            let head = repository
                .head()
                .ok()
                .and_then(|head| head.target())
                .map(|target| target.to_string())
                .unwrap_or_default();
            Ok(GitWorktreeInfo {
                path: normalized_path(&inspect_path),
                branch: None,
                head,
                is_main: false,
                is_locked: false,
                is_prunable: false,
            })
        })
        .await
        .map_err(|error| GitError::CommandFailed(format!("spawn_blocking join: {error}")))?
    }

    /// Creates a local branch for an existing detached worktree.
    pub async fn create_worktree_branch<P: AsRef<Path>>(
        worktree_path: P,
        branch: &str,
    ) -> Result<GitWorktreeInfo, GitError> {
        let worktree_path = worktree_path.as_ref().to_path_buf();
        let path = normalized_path(&worktree_path);
        execute_git_command(&path, &["check-ref-format", "--branch", branch]).await?;
        execute_git_command(&path, &["switch", "-c", branch]).await?;
        let head = execute_git_command(&path, &["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_string();
        Ok(GitWorktreeInfo {
            path,
            branch: Some(branch.to_string()),
            head,
            is_main: false,
            is_locked: false,
            is_prunable: false,
        })
    }

    /// Reattaches an existing branch while recreating a missing registered
    /// worktree.
    pub async fn attach_worktree_branch<P: AsRef<Path>>(
        worktree_path: P,
        branch: &str,
    ) -> Result<GitWorktreeInfo, GitError> {
        let worktree_path = worktree_path.as_ref().to_path_buf();
        let path = normalized_path(&worktree_path);
        execute_git_command(&path, &["check-ref-format", "--branch", branch]).await?;
        execute_git_command(&path, &["switch", branch]).await?;
        let head = execute_git_command(&path, &["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_string();
        Ok(GitWorktreeInfo {
            path,
            branch: Some(branch.to_string()),
            head,
            is_main: false,
            is_locked: false,
            is_prunable: false,
        })
    }

    pub async fn prune_worktrees<P: AsRef<Path>>(repository_path: P) -> Result<(), GitError> {
        execute_git_command(
            &normalized_path(repository_path.as_ref()),
            &["worktree", "prune"],
        )
        .await?;
        Ok(())
    }

    pub async fn worktree_is_dirty<P: AsRef<Path>>(worktree_path: P) -> Result<bool, GitError> {
        let worktree_path = worktree_path.as_ref();
        let status = Self::get_status(worktree_path).await?;
        Ok(!status.staged.is_empty()
            || !status.unstaged.is_empty()
            || !status.untracked.is_empty()
            || !status.conflicts.is_empty()
            // Files explicitly selected by `.worktreeinclude` are copied user
            // state even though Git ignores them. Treat them as dirty so safe
            // removal cannot silently discard the copied state.
            || !ignored_include_paths(worktree_path).await?.is_empty())
    }

    /// Returns true when detached HEAD contains commits unreachable from every
    /// local or remote ref.
    pub async fn worktree_has_unpublished_commits<P: AsRef<Path>>(
        worktree_path: P,
    ) -> Result<bool, GitError> {
        let worktree_path = worktree_path.as_ref();
        let repository = open_repository(worktree_path)?;
        if repository.head().ok().is_some_and(|head| head.is_branch()) {
            return Ok(false);
        }
        let path = normalized_path(worktree_path);
        let refs = execute_git_command(
            &path,
            &["for-each-ref", "--format=%(refname)", "--contains", "HEAD"],
        )
        .await?;
        Ok(refs.trim().is_empty())
    }

    pub async fn local_change_summary<P: AsRef<Path>>(
        source_path: P,
    ) -> Result<GitLocalChangeSummary, GitError> {
        let source = source_path.as_ref();
        let staged = parse_nul_paths(
            &git_output_bytes(
                source,
                &["diff", "--name-only", "-z", "--cached", "HEAD", "--"],
            )
            .await?,
        )?;
        let unstaged = parse_nul_paths(
            &git_output_bytes(source, &["diff", "--name-only", "-z", "--"]).await?,
        )?;
        let untracked = parse_nul_paths(
            &git_output_bytes(
                source,
                &["ls-files", "--others", "--exclude-standard", "-z"],
            )
            .await?,
        )?;
        let included_ignored = ignored_include_paths(source).await?;
        Ok(GitLocalChangeSummary {
            staged,
            unstaged,
            untracked,
            included_ignored,
        })
    }

    /// Copies source changes into a fresh worktree while preserving index state.
    /// Both worktrees must still point at the same immutable HEAD.
    pub async fn copy_local_changes<P: AsRef<Path>, Q: AsRef<Path>>(
        source_path: P,
        target_path: Q,
    ) -> Result<GitLocalChangeSummary, GitError> {
        let source = source_path.as_ref();
        let target = target_path.as_ref();
        let source_head = execute_git_command(&normalized_path(source), &["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_string();
        let target_head = execute_git_command(&normalized_path(target), &["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_string();
        if source_head != target_head {
            return Err(GitError::CommandFailed(
                "Local changes can only be copied when source and target HEAD match".to_string(),
            ));
        }

        let summary = Self::local_change_summary(source).await?;
        let staged_patch =
            git_output_bytes(source, &["diff", "--binary", "--cached", "HEAD", "--"]).await?;
        git_with_stdin(
            target,
            &["apply", "--binary", "--index", "--whitespace=nowarn", "-"],
            &staged_patch,
        )
        .await?;

        let unstaged_patch = git_output_bytes(source, &["diff", "--binary", "--"]).await?;
        git_with_stdin(
            target,
            &["apply", "--binary", "--whitespace=nowarn", "-"],
            &unstaged_patch,
        )
        .await?;

        copy_regular_files(source, target, &summary.untracked).await?;
        copy_regular_files(source, target, &summary.included_ignored).await?;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::GitService;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(path)
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(args)
            .output()
            .expect("git command should start");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn initialized_repository() -> (TempDir, std::path::PathBuf) {
        let temp = TempDir::new().expect("temp dir");
        let repository = temp.path().join("repository");
        fs::create_dir_all(&repository).expect("create repository");
        git(&repository, &["init"]);
        git(&repository, &["config", "user.name", "BitFun Test"]);
        git(
            &repository,
            &["config", "user.email", "bitfun-test@example.invalid"],
        );
        fs::write(repository.join("shared.txt"), "base\n").expect("write base file");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "base"]);
        (temp, repository)
    }

    #[tokio::test]
    async fn detached_worktrees_from_the_same_commit_are_independent() {
        let (temp, repository) = initialized_repository();
        let base = git(&repository, &["rev-parse", "HEAD"]);
        let first = temp.path().join("first");
        let second = temp.path().join("second");

        let first_result = GitService::add_detached_worktree(&repository, &first, &base)
            .await
            .expect("first worktree");
        let second_result = GitService::add_detached_worktree(&repository, &second, &base)
            .await
            .expect("second worktree");
        assert_eq!(first_result.head, base);
        assert_eq!(second_result.head, base);

        fs::write(first.join("shared.txt"), "first\n").expect("write first");
        fs::write(second.join("shared.txt"), "second\n").expect("write second");

        assert_eq!(
            fs::read_to_string(repository.join("shared.txt")).unwrap(),
            "base\n"
        );
        assert_eq!(
            fs::read_to_string(first.join("shared.txt")).unwrap(),
            "first\n"
        );
        assert_eq!(
            fs::read_to_string(second.join("shared.txt")).unwrap(),
            "second\n"
        );

        git(&first, &["add", "shared.txt"]);
        git(&first, &["commit", "-m", "detached change"]);
        assert!(GitService::worktree_has_unpublished_commits(&first)
            .await
            .expect("unpublished check"));
        assert!(!GitService::worktree_has_unpublished_commits(&second)
            .await
            .expect("base commit is referenced"));
    }

    #[tokio::test]
    async fn local_changes_copy_preserves_index_binary_and_selected_files() {
        let (temp, repository) = initialized_repository();
        fs::write(repository.join("binary.dat"), [0_u8, 1, 2, 3]).expect("write binary");
        fs::write(repository.join(".gitignore"), "secret.env\n").expect("write ignore");
        fs::write(repository.join(".worktreeinclude"), "secret.env\n").expect("write include");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "copy fixtures"]);

        fs::write(repository.join("shared.txt"), "staged\n").expect("write staged");
        fs::write(repository.join("binary.dat"), [0_u8, 9, 8, 7, 6]).expect("write binary change");
        git(&repository, &["add", "shared.txt", "binary.dat"]);
        fs::write(repository.join("shared.txt"), "staged\nunstaged\n").expect("write unstaged");
        fs::write(repository.join("untracked.txt"), "untracked\n").expect("write untracked");
        fs::write(repository.join("secret.env"), "included ignored\n")
            .expect("write selected ignored");

        let base = git(&repository, &["rev-parse", "HEAD"]);
        let target = temp.path().join("copy-target");
        GitService::add_detached_worktree(&repository, &target, &base)
            .await
            .expect("create target");
        let summary = GitService::copy_local_changes(&repository, &target)
            .await
            .expect("copy changes");

        assert!(summary.staged.contains(&"shared.txt".to_string()));
        assert!(summary.staged.contains(&"binary.dat".to_string()));
        assert!(summary.unstaged.contains(&"shared.txt".to_string()));
        assert!(summary.untracked.contains(&"untracked.txt".to_string()));
        assert!(summary.included_ignored.contains(&"secret.env".to_string()));
        assert_eq!(
            fs::read_to_string(target.join("shared.txt"))
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            vec!["staged", "unstaged"]
        );
        assert_eq!(
            fs::read(target.join("binary.dat")).unwrap(),
            [0_u8, 9, 8, 7, 6]
        );
        assert_eq!(
            fs::read_to_string(target.join("untracked.txt")).unwrap(),
            "untracked\n"
        );
        assert_eq!(
            fs::read_to_string(target.join("secret.env")).unwrap(),
            "included ignored\n"
        );

        let staged = git(&target, &["diff", "--cached", "--name-only"]);
        let unstaged = git(&target, &["diff", "--name-only"]);
        assert!(staged.lines().any(|path| path == "shared.txt"));
        assert!(staged.lines().any(|path| path == "binary.dat"));
        assert!(unstaged.lines().any(|path| path == "shared.txt"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_changes_copy_rejects_selected_symlinks() {
        use std::os::unix::fs::symlink;

        let (temp, repository) = initialized_repository();
        fs::write(repository.join(".gitignore"), "selected-link\n").expect("write ignore");
        fs::write(repository.join(".worktreeinclude"), "selected-link\n").expect("write include");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "symlink fixtures"]);

        let outside = temp.path().join("outside-secret");
        fs::write(&outside, "must not copy\n").expect("write outside file");
        symlink(&outside, repository.join("selected-link")).expect("create selected symlink");

        let base = git(&repository, &["rev-parse", "HEAD"]);
        let target = temp.path().join("symlink-target");
        GitService::add_detached_worktree(&repository, &target, &base)
            .await
            .expect("create target");
        let error = GitService::copy_local_changes(&repository, &target)
            .await
            .expect_err("selected symlink must be rejected");

        assert!(error.to_string().contains("regular files"));
        assert!(!target.join("selected-link").exists());
    }

    #[tokio::test]
    async fn selected_ignored_files_block_safe_clean_removal() {
        let (temp, repository) = initialized_repository();
        fs::write(repository.join(".gitignore"), "secret.env\n").expect("write ignore");
        fs::write(repository.join(".worktreeinclude"), "secret.env\n").expect("write include");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "include policy"]);

        let base = git(&repository, &["rev-parse", "HEAD"]);
        let target = temp.path().join("ignored-state-target");
        GitService::add_detached_worktree(&repository, &target, &base)
            .await
            .expect("create target");
        fs::write(target.join("secret.env"), "user state\n").expect("write selected ignored file");

        assert!(
            GitService::worktree_is_dirty(&target)
                .await
                .expect("dirty check"),
            "selected ignored state must be protected by safe removal"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn detached_worktree_rejects_a_dangling_symlink_target() {
        use std::os::unix::fs::symlink;

        let (temp, repository) = initialized_repository();
        let base = git(&repository, &["rev-parse", "HEAD"]);
        let target = temp.path().join("dangling-target");
        symlink(temp.path().join("missing-target"), &target).expect("create dangling symlink");

        let error = GitService::add_detached_worktree(&repository, &target, &base)
            .await
            .expect_err("dangling targets must not be followed");

        assert!(error.to_string().contains("already exists"));
    }
}
