//! Local workspace runtime services.
//!
//! This module owns local filesystem and shell implementations for the
//! workspace runtime ports. Product crates remain responsible for selecting
//! when these providers are used.

use async_trait::async_trait;
use bitfun_runtime_ports::{
    WorkspaceCommandOptions, WorkspaceCommandResult, WorkspaceDirEntry, WorkspaceFileSystem,
    WorkspacePathKind, WorkspaceServices, WorkspaceShell,
};
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

/// Local filesystem implementation of [`WorkspaceFileSystem`].
pub struct LocalWorkspaceFs;

#[async_trait]
impl WorkspaceFileSystem for LocalWorkspaceFs {
    async fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        Ok(tokio::fs::read(path).await?)
    }

    async fn read_file_bounded(
        &self,
        path: &str,
        max_bytes: usize,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let metadata = tokio::fs::metadata(path).await?;
        if metadata.len() > max_bytes as u64 {
            return Ok(None);
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        tokio::fs::File::open(path)
            .await?
            .take(max_bytes.saturating_add(1) as u64)
            .read_to_end(&mut bytes)
            .await?;
        Ok((bytes.len() <= max_bytes).then_some(bytes))
    }

    async fn read_file_text(&self, path: &str) -> anyhow::Result<String> {
        Ok(tokio::fs::read_to_string(path).await?)
    }

    async fn read_file_text_bounded(
        &self,
        path: &str,
        max_bytes: usize,
    ) -> anyhow::Result<Option<String>> {
        self.read_file_bounded(path, max_bytes)
            .await?
            .map(String::from_utf8)
            .transpose()
            .map_err(Into::into)
    }

    async fn write_file(&self, path: &str, contents: &[u8]) -> anyhow::Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        Ok(tokio::fs::write(path, contents).await?)
    }

    async fn exists(&self, path: &str) -> anyhow::Result<bool> {
        Ok(tokio::fs::try_exists(path).await.unwrap_or(false))
    }

    async fn is_file(&self, path: &str) -> anyhow::Result<bool> {
        match tokio::fs::metadata(path).await {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(_) => Ok(false),
        }
    }

    async fn is_dir(&self, path: &str) -> anyhow::Result<bool> {
        match tokio::fs::metadata(path).await {
            Ok(metadata) => Ok(metadata.is_dir()),
            Err(_) => Ok(false),
        }
    }

    async fn path_kind_no_follow(&self, path: &str) -> anyhow::Result<Option<WorkspacePathKind>> {
        let metadata = match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let file_type = metadata.file_type();
        Ok(Some(if file_type.is_symlink() {
            WorkspacePathKind::Symlink
        } else if file_type.is_dir() {
            WorkspacePathKind::Directory
        } else if file_type.is_file() {
            WorkspacePathKind::File
        } else {
            WorkspacePathKind::Other
        }))
    }

    async fn read_dir(&self, path: &str) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
        self.read_dir_bounded(path, usize::MAX).await
    }

    async fn read_dir_bounded(
        &self,
        path: &str,
        max_entries: usize,
    ) -> anyhow::Result<Vec<WorkspaceDirEntry>> {
        let mut entries = Vec::new();
        let mut scanned_entries = 0usize;
        let mut read_dir = tokio::fs::read_dir(path).await?;
        while scanned_entries < max_entries {
            let Ok(Some(entry)) = read_dir.next_entry().await else {
                break;
            };
            scanned_entries += 1;
            let path = entry.path();
            let metadata = tokio::fs::symlink_metadata(&path).await?;
            if metadata.file_type().is_symlink() {
                continue;
            }

            entries.push(WorkspaceDirEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
                is_dir: metadata.is_dir(),
                is_symlink: false,
            });
        }
        Ok(entries)
    }
}

/// Local shell implementation of [`WorkspaceShell`].
pub struct LocalWorkspaceShell {
    workspace_root: String,
}

impl LocalWorkspaceShell {
    pub fn new(workspace_root: String) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl WorkspaceShell for LocalWorkspaceShell {
    async fn exec_with_options(
        &self,
        command: &str,
        options: WorkspaceCommandOptions,
    ) -> anyhow::Result<WorkspaceCommandResult> {
        use std::process::Stdio;
        use tokio::io::AsyncReadExt;

        let mut cmd = crate::process_manager::create_tokio_command("sh");
        cmd.arg("-c").arg(command);
        cmd.current_dir(&self.workspace_root);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture command stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture command stderr"))?;

        let stdout_task = tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer).await?;
            Ok::<Vec<u8>, std::io::Error>(buffer)
        });
        let stderr_task = tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer).await?;
            Ok::<Vec<u8>, std::io::Error>(buffer)
        });

        let mut interrupted = false;
        let mut timed_out = false;
        let mut exit_code = -1;
        let deadline = options
            .timeout_ms
            .map(|ms| tokio::time::Instant::now() + std::time::Duration::from_millis(ms));

        loop {
            if let Some(token) = options.cancellation_token.as_ref() {
                if token.is_cancelled() {
                    interrupted = true;
                    let _ = child.start_kill();
                    break;
                }
            }

            if let Some(deadline) = deadline {
                if tokio::time::Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.start_kill();
                    break;
                }
            }

            if let Some(status) = child.try_wait()? {
                exit_code = status.code().unwrap_or(-1);
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }

        if interrupted || timed_out {
            let _ = child.wait().await;
            if interrupted {
                #[cfg(windows)]
                {
                    exit_code = -1073741510;
                }
                #[cfg(not(windows))]
                {
                    exit_code = 130;
                }
            } else if timed_out {
                exit_code = 124;
            }
        }

        let stdout = String::from_utf8_lossy(
            &stdout_task
                .await
                .map_err(|error| anyhow::anyhow!("Failed to join stdout task: {}", error))??,
        )
        .to_string();
        let stderr = String::from_utf8_lossy(
            &stderr_task
                .await
                .map_err(|error| anyhow::anyhow!("Failed to join stderr task: {}", error))??,
        )
        .to_string();

        Ok(WorkspaceCommandResult {
            stdout,
            stderr,
            exit_code,
            interrupted,
            timed_out,
        })
    }
}

/// Build [`WorkspaceServices`] backed by the local filesystem and shell.
pub fn local_workspace_services(workspace_root: String) -> WorkspaceServices {
    WorkspaceServices {
        fs: Arc::new(LocalWorkspaceFs),
        shell: Arc::new(LocalWorkspaceShell::new(workspace_root)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_runtime_ports::WorkspaceFileSystem;

    #[tokio::test]
    async fn local_workspace_fs_writes_parent_dirs_and_reads_text() {
        let temp = tempfile::tempdir().expect("temp dir");
        let file = temp.path().join("nested").join("file.txt");
        let path = file.to_string_lossy().to_string();
        let fs = LocalWorkspaceFs;

        fs.write_file(&path, b"hello").await.unwrap();

        assert!(fs.exists(&path).await.unwrap());
        assert!(fs.is_file(&path).await.unwrap());
        assert_eq!(fs.read_file_text(&path).await.unwrap(), "hello");
        assert!(fs.read_file_bounded(&path, 4).await.unwrap().is_none());
        assert_eq!(
            fs.read_file_bounded(&path, 5).await.unwrap(),
            Some(b"hello".to_vec())
        );
    }

    #[tokio::test]
    async fn local_workspace_shell_timeout_preserves_legacy_result_shape() {
        if which::which("sh").is_err() {
            return;
        }

        let temp = tempfile::tempdir().expect("temp dir");
        let shell = LocalWorkspaceShell::new(temp.path().to_string_lossy().to_string());

        let result = shell
            .exec_with_options(
                "sleep 2",
                WorkspaceCommandOptions {
                    timeout_ms: Some(50),
                    cancellation_token: None,
                },
            )
            .await
            .unwrap();

        assert!(result.timed_out);
        assert!(!result.interrupted);
        assert_eq!(result.exit_code, 124);
    }
}
