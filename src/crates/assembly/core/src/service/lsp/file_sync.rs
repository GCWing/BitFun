//! LSP file sync service
//!
//! Responsibilities:
//! - Watch filesystem changes in the workspace
//! - Sync file edits to the LSP server (didOpen/didChange)
//! - Batch + debounce to reduce LSP calls
//! - Support graceful degradation

use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::workspace_manager::WorkspaceLspManager;

/// File sync configuration.
#[derive(Debug, Clone)]
pub struct FileSyncConfig {
    /// Whether file sync is enabled (global downgrade switch).
    pub enabled: bool,

    /// Debounce delay (milliseconds).
    pub debounce_ms: u64,

    /// File extensions to watch (allowlist).
    pub watch_extensions: Vec<String>,

    /// Path patterns to ignore (denylist).
    pub ignore_patterns: Vec<String>,

    /// Max file size (bytes). Files larger than this will not be synced.
    pub max_file_size: u64,
}

impl Default for FileSyncConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_ms: 300,
            watch_extensions: vec![
                ".rs".to_string(),
                ".ts".to_string(),
                ".tsx".to_string(),
                ".js".to_string(),
                ".jsx".to_string(),
                ".py".to_string(),
                ".go".to_string(),
                ".java".to_string(),
                ".cpp".to_string(),
                ".c".to_string(),
                ".h".to_string(),
                ".hpp".to_string(),
                ".cs".to_string(),
                ".rb".to_string(),
                ".php".to_string(),
                ".swift".to_string(),
                ".kt".to_string(),
                ".scala".to_string(),
            ],
            ignore_patterns: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".next".to_string(),
                ".vscode".to_string(),
                ".idea".to_string(),
            ],
            max_file_size: 10 * 1024 * 1024,
        }
    }
}

/// File change record.
#[derive(Debug, Clone)]
struct FileChange {
    #[allow(dead_code)]
    path: PathBuf,
    #[allow(dead_code)]
    last_modified: SystemTime,
    change_count: u32,
}

/// LSP file sync manager.
pub struct LspFileSync {
    /// Configuration.
    config: FileSyncConfig,

    /// File watcher.
    watcher: Arc<RwLock<Option<RecommendedWatcher>>>,

    /// Workspace LSP manager mapping (`workspace_path -> manager`).
    workspace_managers: Arc<RwLock<HashMap<PathBuf, Arc<WorkspaceLspManager>>>>,

    /// Pending file change queue (debounced).
    pending_changes: Arc<RwLock<HashMap<PathBuf, FileChange>>>,

    /// Debounce worker task handle.
    debounce_worker: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl LspFileSync {
    /// Creates a new file sync manager.
    pub fn new(config: FileSyncConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            watcher: Arc::new(RwLock::new(None)),
            workspace_managers: Arc::new(RwLock::new(HashMap::new())),
            pending_changes: Arc::new(RwLock::new(HashMap::new())),
            debounce_worker: Arc::new(RwLock::new(None)),
        })
    }

    /// Starts watching a workspace.
    pub async fn watch_workspace(
        self: &Arc<Self>,
        workspace: PathBuf,
        manager: Arc<WorkspaceLspManager>,
    ) -> Result<()> {
        if !self.config.enabled {
            info!("LSP file sync is disabled, skipping workspace watch");
            return Ok(());
        }

        info!("Starting file sync for workspace: {:?}", workspace);

        {
            let mut managers = self.workspace_managers.write().await;
            managers.insert(workspace.clone(), manager);
        }

        {
            let mut watcher = self.watcher.write().await;
            if watcher.is_none() {
                *watcher = Some(self.start_watcher().await?);
            }
        }

        {
            let mut watcher = self.watcher.write().await;
            if let Some(w) = watcher.as_mut() {
                w.watch(&workspace, RecursiveMode::Recursive)
                    .map_err(|e| anyhow!("Failed to watch workspace: {}", e))?;
            }
        }

        {
            let mut worker = self.debounce_worker.write().await;
            if worker.is_none() {
                *worker = Some(self.start_debounce_worker());
            }
        }

        Ok(())
    }

    /// Stops watching a workspace.
    pub async fn unwatch_workspace(&self, workspace: &Path) -> Result<()> {
        info!("Stopping file sync for workspace: {:?}", workspace);

        {
            let mut watcher = self.watcher.write().await;
            if let Some(w) = watcher.as_mut() {
                w.unwatch(workspace)
                    .map_err(|e| anyhow!("Failed to unwatch workspace: {}", e))?;
            }
        }

        {
            let mut managers = self.workspace_managers.write().await;
            managers.remove(workspace);
        }

        Ok(())
    }

    /// Starts the filesystem watcher.
    async fn start_watcher(self: &Arc<Self>) -> Result<RecommendedWatcher> {
        let sync_clone = self.clone();

        let handle = tokio::runtime::Handle::current();

        let watcher =
            notify::recommended_watcher(move |event: notify::Result<Event>| match event {
                Ok(event) => {
                    let sync = sync_clone.clone();

                    handle.spawn(async move {
                        if let Err(e) = sync.handle_fs_event(event).await {
                            debug!("Failed to handle fs event: {}", e);
                        }
                    });
                }
                Err(e) => {
                    warn!("File watcher error: {}", e);
                }
            })
            .map_err(|e| anyhow!("Failed to create watcher: {}", e))?;

        Ok(watcher)
    }

    /// Handles filesystem events.
    async fn handle_fs_event(&self, event: Event) -> Result<()> {
        match event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
                for path in event.paths {
                    if self.should_sync(&path).await {
                        self.queue_sync(path).await;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Returns whether a file should be synced.
    async fn should_sync(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        if let Ok(metadata) = tokio::fs::metadata(path).await {
            if metadata.len() > self.config.max_file_size {
                return false;
            }
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_with_dot = format!(".{}", ext);
            if !self.config.watch_extensions.contains(&ext_with_dot) {
                return false;
            }
        } else {
            return false;
        }

        if let Some(path_str) = path.to_str() {
            for pattern in &self.config.ignore_patterns {
                if path_str.contains(pattern) {
                    return false;
                }
            }
        }

        true
    }

    /// Queues a file change for syncing (debounced).
    async fn queue_sync(&self, path: PathBuf) {
        let mut pending = self.pending_changes.write().await;

        let change = FileChange {
            path: path.clone(),
            last_modified: SystemTime::now(),
            change_count: pending.get(&path).map(|c| c.change_count + 1).unwrap_or(1),
        };

        pending.insert(path.clone(), change);
    }

    /// Flushes all pending syncs immediately (skips debounce).
    pub async fn flush_pending_syncs(self: &Arc<Self>) -> Result<()> {
        let changes = {
            let mut pending = self.pending_changes.write().await;
            pending.drain().collect::<Vec<_>>()
        };

        if !changes.is_empty() {
            info!("Flushing {} pending file sync(s)", changes.len());
            self.process_changes(changes).await?;
        }

        Ok(())
    }

    /// Starts the debounce worker.
    fn start_debounce_worker(self: &Arc<Self>) -> JoinHandle<()> {
        let sync = self.clone();
        let debounce_ms = self.config.debounce_ms;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(debounce_ms)).await;

                let changes = {
                    let mut pending = sync.pending_changes.write().await;
                    if pending.is_empty() {
                        continue;
                    }
                    pending.drain().collect::<Vec<_>>()
                };

                if let Err(e) = sync.process_changes(changes).await {
                    warn!("Failed to process file changes: {}", e);
                }
            }
        })
    }

    /// Processes file changes (batch sync).
    async fn process_changes(self: &Arc<Self>, changes: Vec<(PathBuf, FileChange)>) -> Result<()> {
        let grouped = self.group_by_workspace(changes).await;

        let mut tasks = Vec::new();

        for (workspace, files) in grouped {
            let sync = self.clone();
            let task = tokio::spawn(async move {
                if let Err(e) = sync.sync_workspace_files(workspace.clone(), files).await {
                    warn!("Failed to sync workspace {:?}: {}", workspace, e);
                }
            });
            tasks.push(task);
        }

        for task in tasks {
            let _ = task.await;
        }

        Ok(())
    }

    /// Groups files by workspace.
    async fn group_by_workspace(
        self: &Arc<Self>,
        changes: Vec<(PathBuf, FileChange)>,
    ) -> HashMap<PathBuf, Vec<PathBuf>> {
        let mut grouped: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        let managers = self.workspace_managers.read().await;

        for (path, _change) in changes {
            let workspace = managers.keys().find(|ws| path.starts_with(ws));

            if let Some(ws) = workspace {
                grouped.entry(ws.clone()).or_default().push(path);
            }
        }

        grouped
    }

    /// Syncs workspace files to LSP.
    async fn sync_workspace_files(
        self: &Arc<Self>,
        workspace: PathBuf,
        files: Vec<PathBuf>,
    ) -> Result<()> {
        let manager = {
            let managers = self.workspace_managers.read().await;
            managers
                .get(&workspace)
                .cloned()
                .ok_or_else(|| anyhow!("Workspace manager not found: {:?}", workspace))?
        };

        info!(
            "Syncing {} file(s) to LSP for workspace: {:?}",
            files.len(),
            workspace
        );

        let mut tasks = Vec::new();

        for file in files {
            let manager = manager.clone();
            let file = file.clone();

            let task = tokio::spawn(async move {
                if let Err(e) = Self::sync_single_file(manager, file.clone()).await {
                    debug!("Failed to sync file {:?}: {}", file, e);
                }
            });

            tasks.push(task);
        }

        for task in tasks {
            let _ = task.await;
        }

        Ok(())
    }

    /// Syncs a single file to LSP.
    async fn sync_single_file(manager: Arc<WorkspaceLspManager>, file: PathBuf) -> Result<()> {
        let content = tokio::fs::read_to_string(&file)
            .await
            .map_err(|e| anyhow!("Failed to read file {:?}: {}", file, e))?;

        let uri = format!("file://{}", file.display());

        let language = detect_language(&file);

        let is_opened = manager.is_document_opened(&uri).await;

        if is_opened {
            manager.change_document(uri, content).await?;
        } else {
            manager.open_document(uri, language, content).await?;
        }

        Ok(())
    }
}

/// Detects a file language.
fn detect_language(path: &Path) -> String {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext {
            "rs" => "rust",
            "ts" => "typescript",
            "tsx" => "typescriptreact",
            "js" => "javascript",
            "jsx" => "javascriptreact",
            "py" => "python",
            "go" => "go",
            "java" => "java",
            "c" => "c",
            "cpp" | "cc" | "cxx" => "cpp",
            "h" | "hpp" => "cpp",
            "cs" => "csharp",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" => "kotlin",
            "scala" => "scala",
            _ => "plaintext",
        }
        .to_string()
    } else {
        "plaintext".to_string()
    }
}

impl Drop for LspFileSync {
    fn drop(&mut self) {
        debug!("Dropping LspFileSync");
    }
}
