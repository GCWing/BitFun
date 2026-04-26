//! Workspace-level LSP manager
//!
//! Core responsibilities:
//! - Manage the lifecycle of all LSP servers within a workspace
//! - Automatically start and stop servers
//! - Manage document state
//! - Error recovery and health checks
//! - Integrate filesystem monitoring
//! - Push real-time events to the frontend

use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::config_watcher::ConfigWatcher;
use super::manager::LspManager;
use super::project_detector::{ProjectDetector, ProjectInfo};
use crate::infrastructure::events::EventEmitter;

/// LSP event types (pushed to the frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum LspEvent {
    /// Server state changed.
    ServerStateChanged {
        workspace_path: String,
        language: String,
        status: String,
        message: Option<String>,
    },
    /// Document opened.
    DocumentOpened {
        workspace_path: String,
        uri: String,
        language: String,
    },
    /// Document closed.
    DocumentClosed { workspace_path: String, uri: String },
    /// Workspace opened.
    WorkspaceOpened { workspace_path: String },
    /// Workspace closed.
    WorkspaceClosed { workspace_path: String },
    /// Server error.
    ServerError {
        workspace_path: String,
        language: String,
        error: String,
    },
    /// Project detection completed.
    ProjectDetected {
        workspace_path: String,
        project_info: ProjectInfo,
    },
    /// Indexing progress updated.
    IndexingProgress {
        workspace_path: String,
        language: String,
        plugin_name: String,
        progress: u32,
        message: String,
    },
    /// Indexing completed.
    IndexingComplete {
        workspace_path: String,
        language: String,
        plugin_name: String,
    },
    /// Diagnostics (errors, warnings, etc.).
    Diagnostics {
        workspace_path: String,
        uri: String,
        diagnostics: Vec<serde_json::Value>,
    },
}

/// Server status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ServerStatus {
    /// Stopped.
    Stopped,
    /// Starting.
    Starting,
    /// Running.
    Running,
    /// Failed.
    Failed,
    /// Restarting.
    Restarting,
}

/// Server state details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerState {
    /// Status.
    pub status: ServerStatus,
    /// Language identifier.
    pub language: String,
    /// Start time.
    pub started_at: Option<u64>,
    /// Last error message.
    pub last_error: Option<String>,
    /// Restart count.
    pub restart_count: u32,
    /// Open document count.
    pub document_count: usize,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            status: ServerStatus::Stopped,
            language: String::new(),
            started_at: None,
            last_error: None,
            restart_count: 0,
            document_count: 0,
        }
    }
}

/// Document state.
#[derive(Debug, Clone)]
struct DocumentState {
    #[allow(dead_code)]
    uri: String,
    language: String,
    version: i32,
    #[allow(dead_code)]
    opened_at: SystemTime,
}

/// Token state.
#[derive(Debug, Clone)]
enum TokenState {
    /// Created but not started.
    Created,
    /// In progress (includes percentage).
    InProgress(u32),
    /// Completed.
    Completed,
}

/// Token tracking info.
#[derive(Debug, Clone)]
struct TokenInfo {
    /// Token identifier.
    token: String,
    /// Token state.
    state: TokenState,
    /// Token title/description.
    title: String,
    /// Created time.
    created_at: SystemTime,
    /// Last updated time.
    last_updated: SystemTime,
}

/// Workspace LSP manager.
pub struct WorkspaceLspManager {
    /// Workspace path.
    workspace_path: PathBuf,
    /// LSP manager handle.
    lsp_manager: Arc<RwLock<LspManager>>,
    /// Server states.
    server_states: Arc<RwLock<HashMap<String, ServerState>>>,
    /// Document states.
    documents: Arc<RwLock<HashMap<String, DocumentState>>>,
    /// Startup synchronization locks (prevents duplicate starts).
    starting_locks: Arc<RwLock<HashMap<String, Arc<tokio::sync::Notify>>>>,
    /// Health check task handle.
    health_check_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// `EventEmitter` used to emit events to the frontend.
    emitter: Arc<RwLock<Option<Arc<dyn EventEmitter>>>>,
    /// Configuration file watcher.
    config_watcher: Arc<RwLock<Option<ConfigWatcher>>>,
    /// Indexing token tracking (`language -> token list`).
    indexing_tokens: Arc<RwLock<HashMap<String, Vec<TokenInfo>>>>,
    /// Workspace initialization complete flag (project detection + pre-start finished).
    workspace_initialized: Arc<tokio::sync::RwLock<bool>>,
}

impl WorkspaceLspManager {
    /// Creates a new workspace manager.
    pub async fn new(workspace_path: PathBuf, lsp_manager: Arc<RwLock<LspManager>>) -> Arc<Self> {
        let manager = Arc::new(Self {
            workspace_path: workspace_path.clone(),
            lsp_manager,
            server_states: Arc::new(RwLock::new(HashMap::new())),
            documents: Arc::new(RwLock::new(HashMap::new())),
            starting_locks: Arc::new(RwLock::new(HashMap::new())),
            health_check_handle: Arc::new(RwLock::new(None)),
            emitter: Arc::new(RwLock::new(None)),
            config_watcher: Arc::new(RwLock::new(None)),
            indexing_tokens: Arc::new(RwLock::new(HashMap::new())),
            workspace_initialized: Arc::new(tokio::sync::RwLock::new(false)),
        });

        manager.initialize().await;

        manager.start_config_watcher_internal();

        let manager_clone = manager.clone();
        let workspace_path_clone = workspace_path.clone();
        tokio::spawn(async move {
            manager_clone
                .detect_and_prestart(workspace_path_clone)
                .await;
        });

        manager
    }

    /// Detects project type and pre-starts servers.
    async fn detect_and_prestart(&self, workspace_path: PathBuf) {
        debug!(
            "Starting project detection and prestart for: {:?}",
            workspace_path
        );

        match ProjectDetector::detect(&workspace_path).await {
            Ok(project_info) => {
                info!("Project detected: languages={:?}", project_info.languages);

                self.emit_event(LspEvent::ProjectDetected {
                    workspace_path: workspace_path.display().to_string(),
                    project_info: project_info.clone(),
                })
                .await;

                let languages_to_start = ProjectDetector::should_prestart(&project_info);

                if !languages_to_start.is_empty() {
                    info!("Pre-starting language servers: {:?}", languages_to_start);

                    for language in languages_to_start {
                        if let Err(e) = self.prestart_server(&language).await {
                            warn!("Failed to prestart {} server: {}", language, e);
                        }
                    }
                } else {
                    debug!("Large project detected, using on-demand loading");
                }
            }
            Err(e) => {
                warn!("Failed to detect project type: {}", e);
            }
        }

        {
            let mut initialized = self.workspace_initialized.write().await;
            *initialized = true;
        }
    }

    /// Sets an `EventEmitter` to enable event emission.
    pub async fn set_emitter(&self, emitter: Arc<dyn EventEmitter>) {
        let mut e = self.emitter.write().await;
        *e = Some(emitter);
    }

    /// Emits an LSP event to the frontend.
    async fn emit_event(&self, event: LspEvent) {
        if let Some(emitter) = self.emitter.read().await.as_ref() {
            let event_data = match serde_json::to_value(&event) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to serialize LSP event: {}", e);
                    return;
                }
            };
            if let Err(e) = emitter.emit("lsp-event", event_data).await {
                error!("Failed to emit LSP event: {}", e);
            }
        }
    }

    /// Initializes the workspace.
    async fn initialize(&self) {
        self.emit_event(LspEvent::WorkspaceOpened {
            workspace_path: self.workspace_path.display().to_string(),
        })
        .await;

        self.start_health_check().await;
    }

    /// Opens a document (auto-starts the server).
    pub async fn open_document(
        &self,
        uri: String,
        language: String,
        content: String,
    ) -> Result<()> {
        {
            let docs = self.documents.read().await;
            if docs.contains_key(&uri) {
                debug!("Document already open: {}", uri);
                return Ok(());
            }
        }

        let server_language = match self.get_running_server_for_language(&language).await {
            Some(lang) => lang,
            None => {
                debug!(
                    "LSP server not running for language: {}, skipping didOpen",
                    language
                );
                return Ok(());
            }
        };

        let lsp = self.lsp_manager.read().await;
        lsp.did_open(&server_language, &uri, &content)
            .await
            .map_err(|e| {
                error!("Failed to send didOpen: {}", e);
                e
            })?;

        {
            let mut docs = self.documents.write().await;
            docs.insert(
                uri.clone(),
                DocumentState {
                    uri: uri.clone(),
                    language: language.clone(),
                    version: 0,
                    opened_at: SystemTime::now(),
                },
            );
        }

        self.update_server_document_count(&language).await;

        self.emit_event(LspEvent::DocumentOpened {
            workspace_path: self.workspace_path.display().to_string(),
            uri: uri.clone(),
            language: language.clone(),
        })
        .await;

        Ok(())
    }

    /// Updates a document.
    pub async fn change_document(&self, uri: String, content: String) -> Result<()> {
        let (language, version) = {
            let mut docs = self.documents.write().await;
            let doc = docs
                .get_mut(&uri)
                .ok_or_else(|| anyhow!("Document not open: {}", uri))?;

            doc.version += 1;
            (doc.language.clone(), doc.version)
        };

        let server_language = self.get_server_language(&language).await;

        let lsp = self.lsp_manager.read().await;
        lsp.did_change(&server_language, &uri, version, &content)
            .await?;

        Ok(())
    }

    /// Saves a document.
    pub async fn save_document(&self, uri: String) -> Result<()> {
        let language = {
            let docs = self.documents.read().await;
            let doc = docs
                .get(&uri)
                .ok_or_else(|| anyhow!("Document not open: {}", uri))?;
            doc.language.clone()
        };

        let server_language = self.get_server_language(&language).await;

        let lsp = self.lsp_manager.read().await;
        lsp.did_save(&server_language, &uri).await?;

        Ok(())
    }

    /// Closes a document.
    pub async fn close_document(&self, uri: String) -> Result<()> {
        let language = {
            let mut docs = self.documents.write().await;
            let doc = docs
                .remove(&uri)
                .ok_or_else(|| anyhow!("Document not open: {}", uri))?;
            doc.language.clone()
        };

        let server_language = self.get_server_language(&language).await;

        let lsp = self.lsp_manager.read().await;
        lsp.did_close(&server_language, &uri).await?;

        self.update_server_document_count(&server_language).await;

        Ok(())
    }

    /// Returns whether a document is open (used by `LspFileSync`).
    pub async fn is_document_opened(&self, uri: &str) -> bool {
        let docs = self.documents.read().await;
        docs.contains_key(uri)
    }

    /// Quickly checks whether a server is running (does not trigger query or startup).
    /// Returns the actual running server language key (may differ from the requested language).
    async fn get_running_server_for_language(&self, language: &str) -> Option<String> {
        let states = self.server_states.read().await;

        if let Some(state) = states.get(language) {
            if state.status == ServerStatus::Running {
                return Some(language.to_string());
            }
        }

        for (lang, state) in states.iter() {
            if state.status == ServerStatus::Running {
                let is_related = (language == "c" && lang == "cpp")
                    || (language == "cpp" && lang == "c")
                    || (language == "javascript" && lang == "typescript")
                    || (language == "typescript" && lang == "javascript")
                    || (language == "javascriptreact" && lang == "javascript")
                    || (language == "typescriptreact" && lang == "typescript");

                if is_related {
                    return Some(lang.clone());
                }
            }
        }

        None
    }

    /// Returns the actual server language key (handles aliases, e.g. c -> cpp).
    async fn get_server_language(&self, language: &str) -> String {
        {
            let states = self.server_states.read().await;
            if states.contains_key(language) {
                return language.to_string();
            }
        }

        let states = self.server_states.read().await;
        for (lang, state) in states.iter() {
            if state.status == ServerStatus::Running {
                let is_related = (language == "c" && lang == "cpp")
                    || (language == "cpp" && lang == "c")
                    || (language == "javascript" && lang == "typescript")
                    || (language == "typescript" && lang == "javascript")
                    || (language == "javascriptreact" && lang == "javascript")
                    || (language == "typescriptreact" && lang == "typescript");

                if is_related {
                    return lang.clone();
                }
            }
        }

        language.to_string()
    }

    /// Ensures the server is running (prevents duplicate starts).
    /// Returns the actual server language key in use (may differ from the requested one, e.g. c -> cpp).
    #[allow(dead_code)]
    async fn ensure_server_running(&self, language: &str) -> Result<String> {
        let status = {
            let states = self.server_states.read().await;

            states.get(language).map(|state| state.status.clone())
        };

        if let Some(status) = status {
            match status {
                ServerStatus::Running => {
                    return Ok(language.to_string());
                }
                ServerStatus::Starting | ServerStatus::Restarting => {
                    debug!("Server is starting, waiting: {}", language);
                    self.wait_for_server_start(language).await?;
                    return Ok(language.to_string());
                }
                _ => {}
            }
        }

        let related_lang = {
            let states = self.server_states.read().await;

            let mut result = None;
            for (lang, state) in states.iter() {
                if state.status == ServerStatus::Running {
                    let is_related = (language == "c" && lang == "cpp")
                        || (language == "cpp" && lang == "c")
                        || (language == "javascript" && lang == "typescript")
                        || (language == "typescript" && lang == "javascript");

                    if is_related {
                        result = Some(lang.clone());
                        break;
                    }
                }
            }
            result
        };

        if let Some(related_lang) = related_lang {
            debug!("Using {} server for {}", related_lang, language);
            return Ok(related_lang);
        }

        info!("Starting {} server", language);
        self.start_server(language).await?;
        Ok(language.to_string())
    }

    /// Starts a server (with retries).
    async fn start_server(&self, language: &str) -> Result<()> {
        let notify = Arc::new(tokio::sync::Notify::new());
        {
            let mut locks = self.starting_locks.write().await;
            locks.insert(language.to_string(), notify.clone());
        }

        {
            let mut states = self.server_states.write().await;
            states.insert(
                language.to_string(),
                ServerState {
                    status: ServerStatus::Starting,
                    language: language.to_string(),
                    started_at: None,
                    last_error: None,
                    restart_count: 0,
                    document_count: 0,
                },
            );
        }

        self.emit_server_state_changed(language).await;

        let result = self.start_server_internal(language).await;

        let final_result = match result {
            Ok(_) => {
                {
                    let mut states = self.server_states.write().await;
                    if let Some(state) = states.get_mut(language) {
                        state.status = ServerStatus::Running;
                        state.started_at = match SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                        {
                            Ok(duration) => Some(duration.as_secs()),
                            Err(e) => {
                                warn!(
                                    "Failed to compute LSP server start timestamp: language={}, error={}",
                                    language, e
                                );
                                Some(0)
                            }
                        };
                    }
                    info!("LSP server started: {}", language);
                }

                self.emit_server_state_changed(language).await;

                notify.notify_waiters();

                Ok(())
            }
            Err(e) => {
                {
                    let mut states = self.server_states.write().await;
                    if let Some(state) = states.get_mut(language) {
                        state.status = ServerStatus::Failed;
                        state.last_error = Some(e.to_string());
                    }

                    error!("Failed to start LSP server {}: {}", language, e);
                }

                self.emit_server_state_changed(language).await;

                notify.notify_waiters();

                Err(e)
            }
        };

        {
            let mut locks = self.starting_locks.write().await;
            locks.remove(language);
        }

        final_result
    }

    /// Sends an aggregated overall progress event.
    async fn emit_aggregated_progress(
        tokens: Arc<RwLock<HashMap<String, Vec<TokenInfo>>>>,
        workspace: PathBuf,
        language: String,
        emitter: Arc<RwLock<Option<Arc<dyn EventEmitter>>>>,
        lsp_manager: Arc<RwLock<LspManager>>,
    ) {
        let tokens_map = tokens.read().await;

        if let Some(lang_tokens) = tokens_map.get(&language) {
            if lang_tokens.is_empty() {
                return;
            }

            let active_tokens: Vec<_> = lang_tokens
                .iter()
                .filter(|t| !matches!(t.state, TokenState::Created))
                .collect();

            if active_tokens.is_empty() {
                return;
            }

            let total = active_tokens.len();
            let completed = active_tokens
                .iter()
                .filter(|t| matches!(t.state, TokenState::Completed))
                .count();
            let in_progress_tokens: Vec<_> = active_tokens
                .iter()
                .filter(|t| matches!(t.state, TokenState::InProgress(_)))
                .collect();

            let progress_sum: u32 = active_tokens
                .iter()
                .map(|t| match t.state {
                    TokenState::Created => 0,
                    TokenState::InProgress(p) => p,
                    TokenState::Completed => 100,
                })
                .sum();

            let overall_progress = if total > 0 {
                progress_sum / total as u32
            } else {
                0
            };

            let message = if completed == total {
                format!("Indexing completed ({} tasks)", total)
            } else if let Some(active) = in_progress_tokens.first() {
                let title = if active.title.is_empty() {
                    "..."
                } else {
                    &active.title
                };
                format!("{} ({}/{})", title, completed, total)
            } else {
                format!("Indexing ({}/{})", completed, total)
            };

            let plugin_name = {
                let lsp_mgr = lsp_manager.read().await;
                lsp_mgr
                    .find_plugin_by_language(&language)
                    .await
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| language.clone())
            };

            let is_completed = completed == total && total > 0;
            if let Some(emit) = emitter.read().await.as_ref() {
                let progress_event = LspEvent::IndexingProgress {
                    workspace_path: workspace.display().to_string(),
                    language: language.clone(),
                    plugin_name: plugin_name.clone(),
                    progress: overall_progress,
                    message: message.clone(),
                };
                if let Ok(event_data) = serde_json::to_value(&progress_event) {
                    let _ = emit.emit("lsp-event", event_data).await;
                }

                if is_completed {
                    info!("[{}] Indexing completed", language);
                    let complete_event = LspEvent::IndexingComplete {
                        workspace_path: workspace.display().to_string(),
                        language: language.clone(),
                        plugin_name: plugin_name.clone(),
                    };
                    if let Ok(event_data) = serde_json::to_value(&complete_event) {
                        let _ = emit.emit("lsp-event", event_data).await;
                    }
                }
            }

            if is_completed {
                drop(tokens_map);
                let mut tokens_map_mut = tokens.write().await;
                if let Some(lang_tokens) = tokens_map_mut.get_mut(&language) {
                    lang_tokens.clear();
                }
            }
        }
    }

    /// Internal server startup implementation.
    async fn start_server_internal(&self, language: &str) -> Result<()> {
        let language_clone = language.to_string();
        let server_states = self.server_states.clone();
        let workspace_path = self.workspace_path.clone();
        let emitter = self.emitter.clone();

        let crash_callback = Arc::new(move |plugin_id: String| {
            let language = language_clone.clone();
            let states = server_states.clone();
            let workspace = workspace_path.clone();
            let emitter_clone = emitter.clone();

            tokio::spawn(async move {
                error!("LSP server crashed: {} (plugin: {})", language, plugin_id);

                {
                    let mut states = states.write().await;
                    if let Some(state) = states.get_mut(&language) {
                        state.status = ServerStatus::Failed;
                        state.last_error =
                            Some("Server process crashed or became unresponsive".to_string());
                    }
                }

                if let Some(emitter) = emitter_clone.read().await.as_ref() {
                    let error_event = LspEvent::ServerError {
                        workspace_path: workspace.display().to_string(),
                        language: language.clone(),
                        error: "Server process crashed or became unresponsive".to_string(),
                    };
                    if let Ok(event_data) = serde_json::to_value(&error_event) {
                        let _ = emitter.emit("lsp-event", event_data).await;
                    }

                    let state_event = LspEvent::ServerStateChanged {
                        workspace_path: workspace.display().to_string(),
                        language: language.clone(),
                        status: "failed".to_string(),
                        message: Some("Server crashed".to_string()),
                    };
                    if let Ok(event_data) = serde_json::to_value(&state_event) {
                        let _ = emitter.emit("lsp-event", event_data).await;
                    }
                }
            });
        }) as Arc<dyn Fn(String) + Send + Sync>;

        let language_clone2 = language.to_string();
        let indexing_tokens2 = self.indexing_tokens.clone();
        let workspace_path_for_token = self.workspace_path.clone();
        let emitter_for_token = self.emitter.clone();
        let lsp_manager_for_token = self.lsp_manager.clone();

        let token_create_callback = Arc::new(move |token: String| {
            let language = language_clone2.clone();
            let tokens = indexing_tokens2.clone();
            let workspace = workspace_path_for_token.clone();
            let emitter_clone = emitter_for_token.clone();
            let lsp_mgr = lsp_manager_for_token.clone();

            tokio::spawn(async move {
                {
                    let mut tokens_map = tokens.write().await;
                    let lang_tokens = tokens_map.entry(language.clone()).or_insert_with(Vec::new);

                    if !lang_tokens.iter().any(|t| t.token == token) {
                        let now = SystemTime::now();
                        lang_tokens.push(TokenInfo {
                            token: token.clone(),
                            state: TokenState::Created,
                            title: String::new(),
                            created_at: now,
                            last_updated: now,
                        });
                    } else {
                        return;
                    }
                }

                Self::emit_aggregated_progress(tokens, workspace, language, emitter_clone, lsp_mgr)
                    .await;
            });
        }) as Arc<dyn Fn(String) + Send + Sync>;

        let language_clone3 = language.to_string();
        let workspace_path3 = self.workspace_path.clone();
        let emitter_for_progress = self.emitter.clone();
        let indexing_tokens3 = self.indexing_tokens.clone();
        let lsp_manager_for_progress = self.lsp_manager.clone();

        let progress_callback = Arc::new(
            move |kind: String, token: String, percentage: Option<u32>, message: String| {
                let language = language_clone3.clone();
                let workspace = workspace_path3.clone();
                let emitter_clone = emitter_for_progress.clone();
                let tokens = indexing_tokens3.clone();
                let lsp_mgr = lsp_manager_for_progress.clone();

                tokio::spawn(async move {
                    {
                        let mut tokens_map = tokens.write().await;
                        if let Some(lang_tokens) = tokens_map.get_mut(&language) {
                            if let Some(token_info) =
                                lang_tokens.iter_mut().find(|t| t.token == token)
                            {
                                token_info.last_updated = SystemTime::now();
                                match kind.as_str() {
                                    "begin" => {
                                        token_info.state = TokenState::InProgress(0);
                                        token_info.title = message.clone();
                                        info!("[{}] Indexing started: {}", language, message);
                                    }
                                    "report" => {
                                        let progress = percentage.unwrap_or(0);
                                        token_info.state = TokenState::InProgress(progress);
                                    }
                                    "end" => {
                                        token_info.state = TokenState::Completed;
                                        info!("[{}] Indexing task completed", language);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    {
                        let mut tokens_map = tokens.write().await;
                        if let Some(lang_tokens) = tokens_map.get_mut(&language) {
                            let now = SystemTime::now();
                            lang_tokens.retain(|t| {
                                if matches!(t.state, TokenState::Created) {
                                    if let Ok(elapsed) = now.duration_since(t.created_at) {
                                        return elapsed.as_secs() <= 5;
                                    }
                                }
                                true
                            });
                        }
                    }

                    Self::emit_aggregated_progress(
                        tokens,
                        workspace,
                        language,
                        emitter_clone,
                        lsp_mgr,
                    )
                    .await;
                });
            },
        )
            as Arc<dyn Fn(String, String, Option<u32>, String) + Send + Sync>;

        let _language_clone4 = language.to_string();
        let workspace_path4 = self.workspace_path.clone();
        let emitter_for_diagnostics = self.emitter.clone();
        let lsp_manager_for_cache = self.lsp_manager.clone();

        let diagnostics_callback =
            Arc::new(move |uri: String, diagnostics: Vec<serde_json::Value>| {
                let workspace = workspace_path4.clone();
                let emitter_clone = emitter_for_diagnostics.clone();
                let lsp_mgr = lsp_manager_for_cache.clone();

                tokio::spawn(async move {
                    {
                        let lsp = lsp_mgr.read().await;
                        lsp.update_diagnostics_cache(uri.clone(), diagnostics.clone())
                            .await;
                    }

                    let event = LspEvent::Diagnostics {
                        workspace_path: workspace.display().to_string(),
                        uri: uri.clone(),
                        diagnostics: diagnostics.clone(),
                    };

                    let emitter_guard = emitter_clone.read().await;
                    if let Some(emitter) = emitter_guard.as_ref() {
                        debug!(
                            "Emitting diagnostics event: uri={}, count={}",
                            uri,
                            diagnostics.len()
                        );
                        if let Ok(event_data) = serde_json::to_value(&event) {
                            if let Err(e) = emitter.emit("lsp-event", event_data).await {
                                error!("Failed to emit diagnostics event: {}", e);
                            }
                        }
                    }
                });
            }) as Arc<dyn Fn(String, Vec<serde_json::Value>) + Send + Sync>;

        let lsp = self.lsp_manager.read().await;
        lsp.start_server(
            language,
            Some(self.workspace_path.clone()),
            Some(crash_callback),
            Some(progress_callback),
            Some(token_create_callback),
            Some(diagnostics_callback),
        )
        .await
    }

    /// Waits for server startup to complete.
    #[allow(dead_code)]
    async fn wait_for_server_start(&self, language: &str) -> Result<()> {
        let notify = {
            let locks = self.starting_locks.read().await;
            locks.get(language).cloned()
        };

        if let Some(notify) = notify {
            let timeout_duration = Duration::from_secs(60);
            tokio::select! {
                _ = notify.notified() => {

                    let states = self.server_states.read().await;
                    if let Some(state) = states.get(language) {
                        if state.status == ServerStatus::Running {
                            return Ok(());
                        } else {
                            return Err(anyhow!(
                                "Server failed to start: {}",
                                state.last_error.as_deref().unwrap_or("Unknown error")
                            ));
                        }
                    }
                    Err(anyhow!("Server state not found after start"))
                }
                _ = tokio::time::sleep(timeout_duration) => {
                    Err(anyhow!("Server start timeout"))
                }
            }
        } else {
            Ok(())
        }
    }

    /// Pre-starts a server (used during workspace initialization).
    pub async fn prestart_server(&self, language: &str) -> Result<()> {
        info!("Pre-starting LSP server for language: {}", language);

        self.start_server(language).await?;

        Ok(())
    }

    /// Stops a server.
    pub async fn stop_server(&self, language: &str) -> Result<()> {
        info!("Stopping LSP server: {}", language);

        let docs_to_close: Vec<String> = {
            let docs = self.documents.read().await;
            docs.iter()
                .filter(|(_, doc)| doc.language == language)
                .map(|(uri, _)| uri.clone())
                .collect()
        };

        for uri in docs_to_close {
            let _ = self.close_document(uri).await;
        }

        let lsp = self.lsp_manager.read().await;
        lsp.stop_server(language).await?;

        {
            let mut states = self.server_states.write().await;
            states.remove(language);
        }

        self.emit_server_state_changed(language).await;

        info!("LSP server stopped: {}", language);
        Ok(())
    }

    /// Returns server state.
    pub async fn get_server_state(&self, language: &str) -> ServerState {
        let states = self.server_states.read().await;
        states
            .get(language)
            .cloned()
            .unwrap_or_else(|| ServerState {
                status: ServerStatus::Stopped,
                language: language.to_string(),
                ..Default::default()
            })
    }

    /// Returns all server states.
    pub async fn get_all_server_states(&self) -> HashMap<String, ServerState> {
        let states = self.server_states.read().await;
        states.clone()
    }

    /// Updates the server document count.
    async fn update_server_document_count(&self, language: &str) {
        let count = {
            let docs = self.documents.read().await;
            docs.values().filter(|doc| doc.language == language).count()
        };

        let mut states = self.server_states.write().await;
        if let Some(state) = states.get_mut(language) {
            state.document_count = count;
        }
    }

    /// Starts health checks.
    async fn start_health_check(&self) {
        let server_states = self.server_states.clone();
        let lsp_manager = self.lsp_manager.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let languages: Vec<String> = {
                    let states = server_states.read().await;
                    states
                        .iter()
                        .filter(|(_, state)| state.status == ServerStatus::Running)
                        .map(|(lang, _)| lang.clone())
                        .collect()
                };

                for language in languages {
                    let states = server_states.read().await;
                    let needs_check = states
                        .get(&language)
                        .map(|s| matches!(s.status, ServerStatus::Running))
                        .unwrap_or(false);
                    drop(states);

                    if needs_check {
                        let lsp = lsp_manager.read().await;
                        let is_alive = lsp.is_server_alive(&language).await;
                        drop(lsp);

                        if !is_alive {
                            error!("Health check detected dead process: {}", language);

                            let mut states = server_states.write().await;
                            if let Some(state) = states.get_mut(&language) {
                                state.status = ServerStatus::Failed;
                                state.last_error =
                                    Some("Server process died unexpectedly".to_string());
                            }
                            drop(states);

                            let lsp = lsp_manager.read().await;
                            if let Err(e) = lsp.stop_server(&language).await {
                                warn!("Failed to cleanup dead server {}: {}", language, e);
                            }
                        }
                    }
                }
            }
        });

        let mut handle_lock = self.health_check_handle.write().await;
        *handle_lock = Some(handle);
    }

    /// Emits a server state change event.
    async fn emit_server_state_changed(&self, language: &str) {
        let state = self.get_server_state(language).await;

        debug!("Server state changed: {} -> {:?}", language, state.status);
    }

    /// Gets code completion (via business layer).
    pub async fn get_completions(
        &self,
        language: &str,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Vec<super::types::CompletionItem>> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;

        let lsp = self.lsp_manager.read().await;
        lsp.get_completions(&server_language, uri, line, character)
            .await
    }

    /// Gets hover information.
    pub async fn get_hover(
        &self,
        language: &str,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.get_hover(&server_language, uri, line, character).await
    }

    /// Go to definition.
    pub async fn goto_definition(
        &self,
        language: &str,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.goto_definition(&server_language, uri, line, character)
            .await
    }

    /// Finds references.
    pub async fn find_references(
        &self,
        language: &str,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.find_references(&server_language, uri, line, character)
            .await
    }

    /// Gets code actions.
    pub async fn get_code_actions(
        &self,
        language: &str,
        uri: &str,
        range: serde_json::Value,
        context: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.get_code_actions(&server_language, uri, range, context)
            .await
    }

    /// Formats a document.
    pub async fn format_document(
        &self,
        language: &str,
        uri: &str,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.format_document(&server_language, uri, tab_size, insert_spaces)
            .await
    }

    /// Gets inlay hints.
    pub async fn get_inlay_hints(
        &self,
        language: &str,
        uri: &str,
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    ) -> Result<Vec<super::types::InlayHint>> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.get_inlay_hints(
            &server_language,
            uri,
            start_line,
            start_character,
            end_line,
            end_character,
        )
        .await
    }

    /// Renames a symbol.
    pub async fn rename(
        &self,
        language: &str,
        uri: &str,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.rename(&server_language, uri, line, character, new_name)
            .await
    }

    /// Gets document highlights.
    pub async fn get_document_highlight(
        &self,
        language: &str,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.get_document_highlight(&server_language, uri, line, character)
            .await
    }

    /// Starts the config file watcher (internal; requires `Arc<Self>`).
    fn start_config_watcher_internal(self: &Arc<Self>) {
        let workspace_path = self.workspace_path.clone();
        let manager_weak = Arc::downgrade(self);

        let on_config_changed = Arc::new(move |language: String, _config_file: String| {
            if let Some(manager) = manager_weak.upgrade() {
                info!(
                    "Config file changed for {}, scheduling server restart",
                    language
                );

                let manager_clone = manager.clone();
                let language_clone = language.clone();
                tokio::spawn(async move {
                    info!("Restarting {} server due to config change", language_clone);

                    if let Err(e) = manager_clone.stop_server(&language_clone).await {
                        warn!("Failed to stop {} server: {}", language_clone, e);
                        return;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;

                    if let Err(e) = manager_clone.start_server(&language_clone).await {
                        error!("Failed to restart {} server: {}", language_clone, e);
                    } else {
                        info!("{} server restarted successfully", language_clone);

                        manager_clone
                            .emit_event(LspEvent::ServerStateChanged {
                                workspace_path: manager_clone.workspace_path.display().to_string(),
                                language: language_clone,
                                status: "running".to_string(),
                                message: Some("Config file updated, server restarted".to_string()),
                            })
                            .await;
                    }
                });
            }
        });

        let config_watcher = self.config_watcher.clone();
        let workspace_path_clone = workspace_path.clone();
        tokio::spawn(async move {
            match ConfigWatcher::new(workspace_path_clone, on_config_changed) {
                Ok(watcher) => {
                    let mut config_watcher_lock = config_watcher.write().await;
                    *config_watcher_lock = Some(watcher);
                }
                Err(e) => {
                    warn!("Failed to start config file watcher: {}", e);
                }
            }
        });
    }

    /// Cleans up resources.
    pub async fn dispose(&self) -> Result<()> {
        info!("Disposing workspace LSP manager");

        {
            let mut handle = self.health_check_handle.write().await;
            if let Some(h) = handle.take() {
                h.abort();
            }
        }

        {
            let mut watcher = self.config_watcher.write().await;
            *watcher = None;
        }

        let docs: Vec<String> = {
            let docs = self.documents.read().await;
            docs.keys().cloned().collect()
        };

        for uri in docs {
            let _ = self.close_document(uri).await;
        }

        let languages: Vec<String> = {
            let states = self.server_states.read().await;
            states.keys().cloned().collect()
        };

        for language in languages {
            let _ = self.stop_server(&language).await;
        }

        info!("Workspace LSP manager disposed");
        Ok(())
    }

    /// Gets document symbols.
    pub async fn get_document_symbols(
        &self,
        language: &str,
        uri: &str,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.get_document_symbols(&server_language, uri).await
    }

    /// Gets diagnostics for a file (e.g. for UI or other callers).
    /// Returns cached diagnostics without triggering new LSP requests.
    pub async fn get_diagnostics(&self, uri: &str) -> Result<Vec<serde_json::Value>> {
        let lsp = self.lsp_manager.read().await;
        Ok(lsp.get_diagnostics(uri).await)
    }

    /// Gets semantic tokens (used for semantic-level syntax highlighting).
    pub async fn get_semantic_tokens(
        &self,
        language: &str,
        uri: &str,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.get_semantic_tokens(&server_language, uri).await
    }

    /// Gets semantic tokens range (for incremental updates).
    pub async fn get_semantic_tokens_range(
        &self,
        language: &str,
        uri: &str,
        range: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let server_language = self
            .get_running_server_for_language(language)
            .await
            .ok_or_else(|| anyhow!("LSP server not running for language: {}", language))?;
        let lsp = self.lsp_manager.read().await;
        lsp.get_semantic_tokens_range(&server_language, uri, range)
            .await
    }
}

impl Drop for WorkspaceLspManager {
    fn drop(&mut self) {
        debug!("WorkspaceLspManager dropped");
    }
}
