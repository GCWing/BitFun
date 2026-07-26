use bitfun_events::EventEmitter;
use log::{debug, error};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use tokio::sync::{broadcast, Mutex};

use super::types::{FileWatchEvent, FileWatchEventKind, FileWatcherConfig};

type WatchedPaths = StdRwLock<HashMap<PathBuf, FileWatcherConfig>>;

fn read_watched_paths(
    lock: &WatchedPaths,
) -> std::sync::RwLockReadGuard<'_, HashMap<PathBuf, FileWatcherConfig>> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write_watched_paths(
    lock: &WatchedPaths,
) -> std::sync::RwLockWriteGuard<'_, HashMap<PathBuf, FileWatcherConfig>> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl From<&EventKind> for FileWatchEventKind {
    fn from(kind: &EventKind) -> Self {
        match kind {
            EventKind::Create(_) => FileWatchEventKind::Create,
            EventKind::Modify(_) => FileWatchEventKind::Modify,
            EventKind::Remove(_) => FileWatchEventKind::Remove,
            EventKind::Any => FileWatchEventKind::Other,
            _ => FileWatchEventKind::Other,
        }
    }
}

pub struct FileWatchService {
    emitter: Arc<Mutex<Option<Arc<dyn EventEmitter>>>>,
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    /// Path table shared with the watcher event thread. Uses a synchronous
    /// RwLock so per-event filtering never needs `block_on` into the async
    /// runtime; guards are only held for short, await-free sections.
    watched_paths: Arc<WatchedPaths>,
    event_buffer: Arc<StdMutex<Vec<FileWatchEvent>>>,
    event_sender: broadcast::Sender<Vec<FileWatchEvent>>,
    config: FileWatcherConfig,
}

fn lock_event_buffer(
    event_buffer: &StdMutex<Vec<FileWatchEvent>>,
) -> std::sync::MutexGuard<'_, Vec<FileWatchEvent>> {
    match event_buffer.lock() {
        Ok(buffer) => buffer,
        Err(poisoned) => {
            error!("File watcher event buffer mutex was poisoned, recovering lock");
            poisoned.into_inner()
        }
    }
}

impl FileWatchService {
    pub fn new(config: FileWatcherConfig) -> Self {
        let (event_sender, _) = broadcast::channel(64);
        Self {
            emitter: Arc::new(Mutex::new(None)),
            watcher: Arc::new(Mutex::new(None)),
            watched_paths: Arc::new(StdRwLock::new(HashMap::new())),
            event_buffer: Arc::new(StdMutex::new(Vec::new())),
            event_sender,
            config,
        }
    }

    /// Subscribe to the same debounced event batches emitted to product surfaces.
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<FileWatchEvent>> {
        self.event_sender.subscribe()
    }

    pub async fn set_emitter(&self, emitter: Arc<dyn EventEmitter>) {
        let mut e = self.emitter.lock().await;
        *e = Some(emitter);
    }

    pub async fn watch_path(
        &self,
        path: &str,
        config: Option<FileWatcherConfig>,
    ) -> Result<(), String> {
        let path_buf = PathBuf::from(path);

        if !path_buf.exists() {
            return Err("Path does not exist".to_string());
        }

        let (recursive, is_new) = {
            let mut watched_paths = write_watched_paths(&self.watched_paths);
            let config = config.unwrap_or_else(|| self.config.clone());
            match watched_paths.entry(path_buf.clone()) {
                Entry::Occupied(mut entry) => {
                    // Multiple product services may share a root. Registering a
                    // narrower observer must not silently downgrade an existing
                    // recursive or hidden-file-aware watch.
                    let existing = entry.get_mut();
                    existing.watch_recursively |= config.watch_recursively;
                    existing.ignore_hidden_files &= config.ignore_hidden_files;
                    existing.debounce_interval_ms = existing
                        .debounce_interval_ms
                        .min(config.debounce_interval_ms);
                    existing.max_events_per_interval = existing
                        .max_events_per_interval
                        .max(config.max_events_per_interval);
                    (existing.watch_recursively, false)
                }
                Entry::Vacant(entry) => {
                    let recursive = config.watch_recursively;
                    entry.insert(config);
                    (recursive, true)
                }
            }
        };

        // Register incrementally on the existing watcher instead of rebuilding
        // the whole watcher (recursive registrations are expensive on Windows).
        let mut watcher_guard = self.watcher.lock().await;
        match watcher_guard.as_mut() {
            Some(watcher) => {
                let mode = if recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };
                // A repeat registration must always re-register with the OS
                // watcher, not just when the recursion mode changed: the path
                // may have been absent when the watcher was built (missing
                // paths are skipped), or removed and recreated since, leaving a
                // stale/absent OS-level watch. `ensure_watch_roots` relies on
                // exactly this to resume watching a root that reappeared, and
                // the pre-incremental implementation got it for free by
                // rebuilding the whole watcher on every call.
                if !is_new {
                    // May never have been registered; ignore unwatch failures.
                    let _ = watcher.unwatch(&path_buf);
                }
                watcher
                    .watch(&path_buf, mode)
                    .map_err(|e| format!("Failed to watch path {}: {}", path_buf.display(), e))?;
                Ok(())
            }
            None => {
                drop(watcher_guard);
                self.create_watcher().await
            }
        }
    }

    pub async fn unwatch_path(&self, path: &str) -> Result<(), String> {
        let path_buf = PathBuf::from(path);

        let (removed, is_empty) = {
            let mut watched_paths = write_watched_paths(&self.watched_paths);
            let removed = watched_paths.remove(&path_buf).is_some();
            (removed, watched_paths.is_empty())
        };

        let mut watcher_guard = self.watcher.lock().await;
        if is_empty {
            // Dropping the watcher disconnects its channel; the event thread
            // exits on its own.
            *watcher_guard = None;
        } else if removed {
            if let Some(watcher) = watcher_guard.as_mut() {
                // The path may never have been registered (e.g. it was missing
                // when the watcher was created); ignore unwatch failures.
                let _ = watcher.unwatch(&path_buf);
            }
        }

        Ok(())
    }

    async fn create_watcher(&self) -> Result<(), String> {
        // Keep the sync read guard scopes free of awaits: check emptiness with
        // a short-lived guard before touching the async watcher mutex.
        if read_watched_paths(&self.watched_paths).is_empty() {
            let mut watcher_guard = self.watcher.lock().await;
            *watcher_guard = None;
            return Ok(());
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())
            .map_err(|e| format!("Failed to create watcher: {}", e))?;

        {
            let watched_paths = read_watched_paths(&self.watched_paths);

            for (path, config) in watched_paths.iter() {
                // A watched source directory may be removed between events. Its
                // stable parent watch remains active and the missing path will be
                // re-registered when it reappears.
                if !path.exists() {
                    continue;
                }
                let mode = if config.watch_recursively {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };

                watcher
                    .watch(path, mode)
                    .map_err(|e| format!("Failed to watch path {}: {}", path.display(), e))?;
            }
        }

        {
            let mut watcher_guard = self.watcher.lock().await;
            *watcher_guard = Some(watcher);
        }

        let event_buffer = self.event_buffer.clone();
        let emitter_arc = self.emitter.clone();
        let default_debounce_ms = self.config.debounce_interval_ms;
        let watched_paths = self.watched_paths.clone();
        let event_sender = self.event_sender.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            let poll = std::time::Duration::from_millis(50);
            let mut last_event_time: Option<std::time::Instant> = None;

            loop {
                match rx.recv_timeout(poll) {
                    Ok(Ok(event)) => {
                        // Synchronous path-table snapshot: no block_on needed
                        // per event.
                        let file_events = Self::convert_events(&event, &watched_paths);
                        if !file_events.is_empty() {
                            lock_event_buffer(&event_buffer).extend(file_events);
                            last_event_time = Some(std::time::Instant::now());
                        }
                    }
                    Ok(Err(e)) => error!("File watch error: {}", e),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }

                if let Some(t) = last_event_time {
                    let debounce_ms = read_watched_paths(&watched_paths)
                        .values()
                        .map(|config| config.debounce_interval_ms)
                        .min()
                        .unwrap_or(default_debounce_ms);
                    if t.elapsed() >= std::time::Duration::from_millis(debounce_ms) {
                        rt.block_on(Self::flush_events_static(
                            &event_buffer,
                            &emitter_arc,
                            &event_sender,
                        ));
                        last_event_time = None;
                    }
                }
            }
        });

        Ok(())
    }

    fn convert_events(event: &Event, watched_paths: &WatchedPaths) -> Vec<FileWatchEvent> {
        let paths = read_watched_paths(watched_paths);
        event
            .paths
            .iter()
            .filter_map(|event_path| {
                let config = paths
                    .iter()
                    .filter(|(watch_path, _)| event_path.starts_with(watch_path))
                    .max_by_key(|(watch_path, _)| watch_path.components().count())
                    .map(|(_, config)| config)?;
                if Self::is_in_excluded_directory(event_path)
                    || Self::is_temporary_file(event_path)
                    || config.ignore_hidden_files
                        && event_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.starts_with('.'))
                {
                    return None;
                }
                Self::convert_event(&event.kind, event_path)
            })
            .collect()
    }

    fn is_in_excluded_directory(path: &Path) -> bool {
        const EXCLUDED_DIRS: &[&str] = &[
            "node_modules",
            ".git",
            ".svn",
            ".hg",
            "target",
            "dist",
            "build",
            "out",
            ".next",
            ".nuxt",
            "vendor",
            "__pycache__",
            ".pytest_cache",
            ".mypy_cache",
            "venv",
            ".venv",
            "env",
            ".env",
            "bower_components",
            ".idea",
            ".vscode",
            ".vs",
            "bin",
            "obj",
            ".terraform",
            "coverage",
            ".coverage",
            "htmlcov",
        ];

        for component in path.components() {
            if let Some(os_str) = component.as_os_str().to_str() {
                if EXCLUDED_DIRS.contains(&os_str) {
                    return true;
                }
            }
        }

        false
    }

    fn is_temporary_file(path: &Path) -> bool {
        if let Some(file_name) = path.file_name() {
            if let Some(name_str) = file_name.to_str() {
                return name_str.ends_with('~')
                    || name_str.ends_with(".swp")
                    || name_str.ends_with(".swo")
                    || name_str.ends_with(".swn")
                    || name_str.starts_with(".#")
                    || name_str.ends_with(".tmp")
                    || name_str.ends_with(".temp")
                    || name_str.ends_with(".bak")
                    || name_str.ends_with(".old")
                    || name_str.starts_with('#') && name_str.ends_with('#')
                    || name_str == ".DS_Store"
                    || name_str == "Thumbs.db"
                    || name_str == "desktop.ini"
                    || name_str.ends_with(".crdownload")
                    || name_str.ends_with(".part");
            }
        }

        false
    }

    fn convert_event(kind: &EventKind, path: &Path) -> Option<FileWatchEvent> {
        let kind = match kind {
            EventKind::Create(_) => FileWatchEventKind::Create,
            EventKind::Modify(_) => FileWatchEventKind::Modify,
            EventKind::Remove(_) => FileWatchEventKind::Remove,
            EventKind::Other => FileWatchEventKind::Other,
            _ => return None,
        };

        Some(FileWatchEvent {
            path: path.to_string_lossy().to_string(),
            kind,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }

    async fn flush_events_static(
        event_buffer: &Arc<StdMutex<Vec<FileWatchEvent>>>,
        emitter_arc: &Arc<Mutex<Option<Arc<dyn EventEmitter>>>>,
        event_sender: &broadcast::Sender<Vec<FileWatchEvent>>,
    ) {
        let events = {
            let mut buffer = lock_event_buffer(event_buffer);
            if buffer.is_empty() {
                return;
            }
            buffer.drain(..).collect::<Vec<_>>()
        };

        // No active backend subscriber is a normal state; the frontend emitter
        // may still consume this batch.
        let _ = event_sender.send(events.clone());

        let emitter_guard = emitter_arc.lock().await;
        if let Some(emitter) = emitter_guard.as_ref() {
            let mut event_array = Vec::new();

            for event in &events {
                let kind = match event.kind {
                    FileWatchEventKind::Create => "create",
                    FileWatchEventKind::Modify => "modify",
                    FileWatchEventKind::Remove => "remove",
                    FileWatchEventKind::Rename { ref from, ref to } => {
                        event_array.push(serde_json::json!({
                            "path": to,
                            "kind": "rename",
                            "from": from,
                            "to": to,
                            "timestamp": event.timestamp
                        }));
                        continue;
                    }
                    FileWatchEventKind::Other => "other",
                };

                event_array.push(serde_json::json!({
                    "path": event.path,
                    "kind": kind,
                    "timestamp": event.timestamp
                }));
            }

            if let Err(e) = emitter
                .emit("file-system-changed", serde_json::json!(event_array))
                .await
            {
                error!("Failed to emit file-system-changed events: {}", e);
            } else {
                debug!("Emitted {} file system change events", event_array.len());
            }
        } else {
            debug!("EventEmitter not configured, skipping file watch events");
        }
    }

    pub async fn get_watched_paths(&self) -> Vec<String> {
        let watched_paths = read_watched_paths(&self.watched_paths);
        watched_paths
            .keys()
            .map(|path| path.to_string_lossy().to_string())
            .collect()
    }
}

static GLOBAL_FILE_WATCH_SERVICE: std::sync::OnceLock<Arc<FileWatchService>> =
    std::sync::OnceLock::new();

pub fn get_global_file_watch_service() -> Arc<FileWatchService> {
    GLOBAL_FILE_WATCH_SERVICE
        .get_or_init(|| Arc::new(FileWatchService::new(FileWatcherConfig::default())))
        .clone()
}

pub async fn start_file_watch(path: String, recursive: Option<bool>) -> Result<(), String> {
    let watcher = get_global_file_watch_service();
    let mut config = FileWatcherConfig::default();
    if let Some(rec) = recursive {
        config.watch_recursively = rec;
    }

    watcher.watch_path(&path, Some(config)).await
}

pub async fn stop_file_watch(path: String) -> Result<(), String> {
    let watcher = get_global_file_watch_service();
    watcher.unwatch_path(&path).await
}

pub async fn get_watched_paths() -> Result<Vec<String>, String> {
    let watcher = get_global_file_watch_service();
    Ok(watcher.get_watched_paths().await)
}

pub fn initialize_file_watch_service(emitter: Arc<dyn EventEmitter>) {
    let watcher = get_global_file_watch_service();

    tokio::spawn(async move {
        watcher.set_emitter(emitter).await;
    });
}
