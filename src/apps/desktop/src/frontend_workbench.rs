//! Writable packaged-frontend revisions with crash-safe provisional activation.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bitfun_core::agentic::tools::frontend_workbench_host::{
    set_frontend_workbench_handler, FrontendWorkbenchHostRequest,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{Manager, Url, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

pub const FRONTEND_PROTOCOL_SCHEME: &str = "bitfun-ui";
pub const FRONTEND_URL: &str = "bitfun-ui://localhost/index.html";
const CONFIRM_WINDOW_LABEL: &str = "frontend-update-confirm";
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(15);
const STATE_SCHEMA_VERSION: u32 = 1;
const RECOVERY_HTML: &[u8] = include_bytes!("../bootstrap-ui/index.html");

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct FrontendWorkbenchState {
    schema_version: u32,
    bundled_revision: Option<String>,
    active_revision: Option<String>,
    previous_revision: Option<String>,
    pending: Option<PendingFrontendRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct PendingFrontendRevision {
    transaction_id: String,
    revision_id: String,
    previous_revision: String,
    expires_at_unix_ms: u64,
}

impl Default for PendingFrontendRevision {
    fn default() -> Self {
        Self {
            transaction_id: String::new(),
            revision_id: String::new(),
            previous_revision: String::new(),
            expires_at_unix_ms: 0,
        }
    }
}

pub struct FrontendWorkbenchManager {
    root: PathBuf,
    state: Mutex<FrontendWorkbenchState>,
    app: OnceLock<tauri::AppHandle>,
}

impl FrontendWorkbenchManager {
    pub fn new(user_data_dir: &Path) -> Self {
        Self {
            root: user_data_dir.join("frontend-workbench"),
            state: Mutex::new(FrontendWorkbenchState::default()),
            app: OnceLock::new(),
        }
    }

    fn revisions_dir(&self) -> PathBuf {
        self.root.join("revisions")
    }

    fn drafts_dir(&self) -> PathBuf {
        self.root.join("drafts")
    }

    fn state_path(&self) -> PathBuf {
        self.root.join("state.json")
    }

    fn revision_dir(&self, revision_id: &str) -> PathBuf {
        self.revisions_dir().join(revision_id)
    }

    pub fn initialize(
        self: &Arc<Self>,
        app: &tauri::AppHandle,
        bundled_frontend: &Path,
    ) -> Result<(), String> {
        validate_frontend_tree(bundled_frontend)?;
        fs::create_dir_all(self.revisions_dir()).map_err(io_error("create revision directory"))?;
        fs::create_dir_all(self.drafts_dir()).map_err(io_error("create draft directory"))?;

        let mut state = self.load_state();
        state.schema_version = STATE_SCHEMA_VERSION;

        // A pending candidate can never survive a process exit: startup restores
        // the prior revision before any editable content is served.
        if let Some(pending) = state.pending.take() {
            if self.revision_is_available(&pending.previous_revision) {
                state.active_revision = Some(pending.previous_revision.clone());
                state.previous_revision = None;
                log::warn!(
                    "Recovered an unconfirmed frontend update by rolling back: transaction_id={}",
                    pending.transaction_id
                );
            } else {
                // Never keep serving an unconfirmed candidate just because its
                // prior revision was damaged outside BitFun. The bundled copy
                // below becomes the recovery target.
                state.active_revision = None;
                state.previous_revision = None;
                log::warn!(
                    "Discarded an unconfirmed frontend update whose previous revision is unavailable: transaction_id={}",
                    pending.transaction_id
                );
            }
        }

        let bundled_revision = bundled_revision_id(bundled_frontend)?;
        let bundled_destination = self.revision_dir(&bundled_revision);
        if !bundled_destination.join("index.html").is_file() {
            copy_tree_transactional(bundled_frontend, &bundled_destination)?;
        }

        let bundled_changed = state.bundled_revision.as_deref() != Some(&bundled_revision);
        let active_is_valid = state
            .active_revision
            .as_deref()
            .is_some_and(|revision| self.revision_is_available(revision));
        if bundled_changed {
            state.previous_revision = state.active_revision.filter(|_| active_is_valid);
            state.active_revision = Some(bundled_revision.clone());
            state.bundled_revision = Some(bundled_revision);
        } else if !active_is_valid {
            state.active_revision = Some(bundled_revision.clone());
            state.bundled_revision = Some(bundled_revision);
            state.previous_revision = None;
        }

        self.save_state(&state)?;
        *self.state.lock().map_err(lock_error)? = state;
        let _ = self.app.set(app.clone());
        self.install_tool_host();
        Ok(())
    }

    fn install_tool_host(self: &Arc<Self>) {
        let manager = Arc::clone(self);
        set_frontend_workbench_handler(Arc::new(move |request| {
            let manager = Arc::clone(&manager);
            Box::pin(async move { manager.handle_tool_request(request) })
        }));
    }

    fn handle_tool_request(
        self: &Arc<Self>,
        request: FrontendWorkbenchHostRequest,
    ) -> Result<Value, String> {
        ensure_local_confirmation_surface(
            crate::api::peer_host_invoke::attached_controllers().len(),
        )?;
        match request.action.as_str() {
            "prepare" => self.prepare(),
            "status" => self.status(),
            "apply" => self.apply(
                request
                    .draft_id
                    .as_deref()
                    .ok_or_else(|| "draft_id is required for apply".to_string())?,
            ),
            "rollback" => self.rollback_confirmed(),
            other => Err(format!("Unsupported FrontendWorkbench action: {other}")),
        }
    }

    fn prepare(&self) -> Result<Value, String> {
        let active_revision = self
            .state
            .lock()
            .map_err(lock_error)?
            .active_revision
            .clone()
            .ok_or_else(|| "No active frontend revision is available".to_string())?;
        validate_revision_id(&active_revision)?;
        let source = self.revision_dir(&active_revision);
        validate_frontend_tree(&source)?;

        let draft_id = Uuid::new_v4().to_string();
        let draft_path = self.drafts_dir().join(&draft_id);
        copy_tree_transactional(&source, &draft_path)?;
        fs::write(
            draft_path.join("CREATION.md"),
            format!(
                "# BitFun frontend draft\n\nDraft id: `{draft_id}`\nBase revision: `{active_revision}`\n\nEdit only this directory. Preserve `index.html`. Prefer `bitfun-creation.css` and `bitfun-creation.js` for isolated overrides. Apply with `FrontendWorkbench` and this exact draft id; the user must confirm within 15 seconds.\n"
            ),
        )
        .map_err(io_error("write draft instructions"))?;

        Ok(json!({
            "status": "prepared",
            "draftId": draft_id,
            "draftPath": draft_path.to_string_lossy(),
            "baseRevision": active_revision,
        }))
    }

    fn apply(self: &Arc<Self>, draft_id: &str) -> Result<Value, String> {
        validate_uuid(draft_id, "draft_id")?;
        let draft_path = self.drafts_dir().join(draft_id);
        validate_frontend_tree(&draft_path)?;

        let revision_id = format!("creative-{}", Uuid::new_v4());
        copy_tree_transactional(&draft_path, &self.revision_dir(&revision_id))?;
        let transaction_id = Uuid::new_v4().to_string();
        let expires_at_unix_ms = unix_ms().saturating_add(CONFIRM_TIMEOUT.as_millis() as u64);

        {
            let mut state = self.state.lock().map_err(lock_error)?;
            if state.pending.is_some() {
                return Err(
                    "Another frontend update is awaiting confirmation; confirm or roll it back first"
                        .to_string(),
                );
            }
            let previous_revision = state
                .active_revision
                .clone()
                .ok_or_else(|| "No active frontend revision is available".to_string())?;
            let mut next = state.clone();
            next.previous_revision = Some(previous_revision.clone());
            next.active_revision = Some(revision_id.clone());
            next.pending = Some(PendingFrontendRevision {
                transaction_id: transaction_id.clone(),
                revision_id: revision_id.clone(),
                previous_revision,
                expires_at_unix_ms,
            });
            self.save_state(&next)?;
            *state = next;
        }

        // Arm the host-owned timeout before touching either webview. Even if a
        // navigation or confirmation-window operation fails, the candidate can
        // never remain active indefinitely.
        let manager = Arc::clone(self);
        let timer_transaction_id = transaction_id.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(CONFIRM_TIMEOUT).await;
            if let Err(error) = manager.rollback_pending(&timer_transaction_id, "timeout") {
                log::warn!(
                    "Failed to auto-rollback provisional frontend update: transaction_id={}, error={}",
                    timer_transaction_id,
                    error
                );
            }
        });

        let activation_result = self
            .app
            .get()
            .cloned()
            .ok_or_else(|| "Frontend workbench desktop host is not initialized".to_string())
            .and_then(|app| {
                navigate_main_to_frontend(&app)?;
                show_confirmation_window(&app, &transaction_id)
            });
        if let Err(activation_error) = activation_result {
            return match self.rollback_pending(&transaction_id, "activation_error") {
                Ok(_) => Err(activation_error),
                Err(rollback_error) => Err(format!(
                    "{activation_error}; the immediate rollback also failed: {rollback_error}"
                )),
            };
        }

        Ok(json!({
            "status": "pending_confirmation",
            "revisionId": revision_id,
            "transactionId": transaction_id,
            "expiresAtUnixMs": expires_at_unix_ms,
            "confirmationTimeoutSeconds": CONFIRM_TIMEOUT.as_secs(),
        }))
    }

    pub fn confirm_pending(&self, transaction_id: &str) -> Result<Value, String> {
        let expired = {
            let state = self.state.lock().map_err(lock_error)?;
            let pending = state
                .pending
                .as_ref()
                .ok_or_else(|| "No frontend update is awaiting confirmation".to_string())?;
            if pending.transaction_id != transaction_id {
                return Err("The frontend confirmation transaction is stale".to_string());
            }
            unix_ms() >= pending.expires_at_unix_ms
        };
        if expired {
            self.rollback_pending(transaction_id, "expired_confirmation")?;
            return Err("The 15-second frontend confirmation window has expired".to_string());
        }

        let revision_id = {
            let mut state = self.state.lock().map_err(lock_error)?;
            let pending = state
                .pending
                .as_ref()
                .ok_or_else(|| "No frontend update is awaiting confirmation".to_string())?;
            if pending.transaction_id != transaction_id {
                return Err("The frontend confirmation transaction is stale".to_string());
            }
            let revision_id = pending.revision_id.clone();
            let mut next = state.clone();
            next.pending = None;
            self.save_state(&next)?;
            *state = next;
            revision_id
        };
        self.close_confirmation_window();
        Ok(json!({"status": "confirmed", "revisionId": revision_id}))
    }

    pub fn rollback_pending(&self, transaction_id: &str, reason: &str) -> Result<Value, String> {
        let restored_revision = {
            let mut state = self.state.lock().map_err(lock_error)?;
            let Some(pending) = state.pending.as_ref() else {
                return Ok(self.status_value(&state));
            };
            if pending.transaction_id != transaction_id {
                return Ok(self.status_value(&state));
            }
            let restored_revision = pending.previous_revision.clone();
            if !self.revision_is_available(&restored_revision) {
                return Err("The previous frontend revision is unavailable".to_string());
            }
            let mut next = state.clone();
            next.active_revision = Some(restored_revision.clone());
            next.previous_revision = None;
            next.pending = None;
            self.save_state(&next)?;
            *state = next;
            restored_revision
        };
        self.close_confirmation_window();
        if let Some(app) = self.app.get() {
            navigate_main_to_frontend(app)?;
        }
        log::info!(
            "Frontend revision rolled back: reason={}, restored_revision={}",
            reason,
            restored_revision
        );
        Ok(json!({"status": "rolled_back", "activeRevision": restored_revision, "reason": reason}))
    }

    fn rollback_confirmed(&self) -> Result<Value, String> {
        let pending_transaction = self
            .state
            .lock()
            .map_err(lock_error)?
            .pending
            .as_ref()
            .map(|pending| pending.transaction_id.clone());
        if let Some(transaction_id) = pending_transaction {
            return self.rollback_pending(&transaction_id, "explicit");
        }

        let restored_revision = {
            let mut state = self.state.lock().map_err(lock_error)?;
            let target = state.previous_revision.clone().ok_or_else(|| {
                "No previous confirmed frontend revision is available".to_string()
            })?;
            if !self.revision_is_available(&target) {
                return Err("The previous frontend revision is unavailable".to_string());
            }
            let mut next = state.clone();
            let current = next.active_revision.replace(target.clone());
            next.previous_revision = current;
            self.save_state(&next)?;
            *state = next;
            target
        };
        if let Some(app) = self.app.get() {
            navigate_main_to_frontend(app)?;
        }
        Ok(json!({"status": "rolled_back", "activeRevision": restored_revision}))
    }

    fn status(&self) -> Result<Value, String> {
        let state = self.state.lock().map_err(lock_error)?;
        Ok(self.status_value(&state))
    }

    fn status_value(&self, state: &FrontendWorkbenchState) -> Value {
        json!({
            "status": if state.pending.is_some() { "pending_confirmation" } else { "ready" },
            "activeRevision": state.active_revision,
            "bundledRevision": state.bundled_revision,
            "previousRevision": state.previous_revision,
            "pending": state.pending,
            "confirmationTimeoutSeconds": CONFIRM_TIMEOUT.as_secs(),
        })
    }

    pub fn protocol_response(
        &self,
        request: tauri::http::Request<Vec<u8>>,
    ) -> tauri::http::Response<Vec<u8>> {
        let request_path = request.uri().path();
        match self.read_protocol_asset(request_path) {
            Ok((bytes, content_type)) => tauri::http::Response::builder()
                .status(tauri::http::StatusCode::OK)
                .header(tauri::http::header::CONTENT_TYPE, content_type)
                .header(tauri::http::header::CACHE_CONTROL, "no-store, max-age=0")
                .header(tauri::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(bytes)
                .unwrap_or_else(|_| tauri::http::Response::new(Vec::new())),
            Err(error) if matches!(request_path, "" | "/" | "/index.html") => {
                log::error!("Serving immutable frontend recovery page: error={error}");
                tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::SERVICE_UNAVAILABLE)
                    .header(
                        tauri::http::header::CONTENT_TYPE,
                        "text/html; charset=utf-8",
                    )
                    .header(tauri::http::header::CACHE_CONTROL, "no-store, max-age=0")
                    .body(RECOVERY_HTML.to_vec())
                    .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
            }
            Err(error) => tauri::http::Response::builder()
                .status(tauri::http::StatusCode::NOT_FOUND)
                .header(
                    tauri::http::header::CONTENT_TYPE,
                    "text/plain; charset=utf-8",
                )
                .header(tauri::http::header::CACHE_CONTROL, "no-store, max-age=0")
                .body(error.into_bytes())
                .unwrap_or_else(|_| tauri::http::Response::new(Vec::new())),
        }
    }

    fn read_protocol_asset(&self, request_path: &str) -> Result<(Vec<u8>, &'static str), String> {
        let decoded = urlencoding::decode(request_path)
            .map_err(|_| "Invalid frontend asset path encoding".to_string())?;
        let relative = decoded.trim_start_matches('/');
        let relative = if relative.is_empty() {
            "index.html"
        } else {
            relative
        };
        let relative_path = Path::new(relative);
        if relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("Invalid frontend asset path".to_string());
        }

        let active_revision = self
            .state
            .lock()
            .map_err(lock_error)?
            .active_revision
            .clone()
            .ok_or_else(|| "Frontend workbench is not initialized".to_string())?;
        validate_revision_id(&active_revision)?;
        let root = self.revision_dir(&active_revision);
        let canonical_root = root
            .canonicalize()
            .map_err(|error| format!("Frontend root is unavailable: {error}"))?;
        let mut candidate = root.join(relative_path);
        if candidate.is_dir() {
            candidate = candidate.join("index.html");
        }
        if !candidate.is_file() && relative_path.extension().is_none() {
            candidate = root.join("index.html");
        }
        let canonical_candidate = candidate
            .canonicalize()
            .map_err(|_| format!("Frontend asset not found: {relative}"))?;
        if !canonical_candidate.starts_with(&canonical_root) || !canonical_candidate.is_file() {
            return Err("Frontend asset path escaped the active revision".to_string());
        }
        let content_type = content_type_for(&canonical_candidate);
        fs::read(&canonical_candidate)
            .map(|bytes| (bytes, content_type))
            .map_err(|error| format!("Failed to read frontend asset: {error}"))
    }

    fn load_state(&self) -> FrontendWorkbenchState {
        let path = self.state_path();
        let Ok(bytes) = fs::read(&path) else {
            return FrontendWorkbenchState::default();
        };
        match serde_json::from_slice(&bytes) {
            Ok(state) => state,
            Err(error) => {
                let preserved = self.root.join(format!("state.invalid.{}.json", unix_ms()));
                if let Err(copy_error) = fs::copy(&path, &preserved) {
                    log::warn!(
                        "Failed to preserve unreadable frontend state: source={}, destination={}, error={}",
                        path.display(),
                        preserved.display(),
                        copy_error
                    );
                }
                log::warn!(
                    "Ignoring unreadable frontend workbench state after preserving it: path={}, error={}",
                    path.display(),
                    error
                );
                FrontendWorkbenchState::default()
            }
        }
    }

    fn save_state(&self, state: &FrontendWorkbenchState) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(io_error("create frontend workbench root"))?;
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| format!("Failed to serialize frontend workbench state: {error}"))?;
        let temporary = self.root.join(format!("state.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, bytes).map_err(io_error("write frontend workbench state"))?;
        match fs::rename(&temporary, self.state_path()) {
            Ok(()) => Ok(()),
            Err(_error) if self.state_path().exists() => {
                let backup = self.root.join("state.previous.json");
                let _ = fs::copy(self.state_path(), backup);
                fs::remove_file(self.state_path())
                    .map_err(io_error("replace frontend workbench state"))?;
                fs::rename(&temporary, self.state_path())
                    .map_err(io_error("commit frontend workbench state"))
            }
            Err(error) => Err(format!(
                "Failed to commit frontend workbench state: {error}"
            )),
        }
    }

    fn close_confirmation_window(&self) {
        if let Some(window) = self
            .app
            .get()
            .and_then(|app| app.get_webview_window(CONFIRM_WINDOW_LABEL))
        {
            let _ = window.close();
        }
    }

    fn revision_is_available(&self, revision_id: &str) -> bool {
        validate_revision_id(revision_id).is_ok()
            && self.revision_dir(revision_id).join("index.html").is_file()
    }
}

fn ensure_local_confirmation_surface(attached_peer_controllers: usize) -> Result<(), String> {
    if attached_peer_controllers > 0 {
        return Err(
            "FrontendWorkbench is unavailable while this BitFun host is controlled through Peer Device Mode; run Creative mode on the visible local desktop instead"
                .to_string(),
        );
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendUpdateDecisionRequest {
    transaction_id: String,
}

#[tauri::command]
pub async fn confirm_frontend_update(
    state: tauri::State<'_, Arc<FrontendWorkbenchManager>>,
    webview: tauri::WebviewWindow,
    request: FrontendUpdateDecisionRequest,
) -> Result<Value, String> {
    require_confirmation_window(&webview)?;
    state.confirm_pending(&request.transaction_id)
}

#[tauri::command]
pub async fn rollback_frontend_update(
    state: tauri::State<'_, Arc<FrontendWorkbenchManager>>,
    webview: tauri::WebviewWindow,
    request: FrontendUpdateDecisionRequest,
) -> Result<Value, String> {
    require_confirmation_window(&webview)?;
    state.rollback_pending(&request.transaction_id, "user")
}

fn require_confirmation_window(webview: &tauri::WebviewWindow) -> Result<(), String> {
    if webview.label() != CONFIRM_WINDOW_LABEL {
        return Err(
            "Frontend updates can only be confirmed from the immutable confirmation window"
                .to_string(),
        );
    }
    Ok(())
}

pub fn custom_frontend_url(path: &str) -> WebviewUrl {
    let suffix = if path.is_empty() {
        "index.html".to_string()
    } else if path.starts_with('?') {
        format!("index.html{path}")
    } else {
        path.trim_start_matches('/').to_string()
    };
    let url = format!("{FRONTEND_PROTOCOL_SCHEME}://localhost/{suffix}")
        .parse::<Url>()
        .expect("static frontend custom-protocol URL must parse");
    WebviewUrl::CustomProtocol(url)
}

fn show_confirmation_window(app: &tauri::AppHandle, transaction_id: &str) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(CONFIRM_WINDOW_LABEL) {
        let _ = window.close();
    }
    let url = WebviewUrl::App(
        format!(
            "frontend-update-confirm.html?transactionId={}",
            urlencoding::encode(transaction_id)
        )
        .into(),
    );
    WebviewWindowBuilder::new(app, CONFIRM_WINDOW_LABEL, url)
        .title("Confirm BitFun frontend update")
        .inner_size(420.0, 260.0)
        .resizable(false)
        .always_on_top(true)
        .center()
        .focused(true)
        .build()
        .map(|_| ())
        .map_err(|error| format!("Failed to open frontend confirmation window: {error}"))
}

fn navigate_main_to_frontend(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window is unavailable".to_string())?;
    let url = FRONTEND_URL
        .parse::<Url>()
        .map_err(|error| format!("Invalid frontend URL: {error}"))?;
    window
        .navigate(url)
        .map_err(|error| format!("Failed to reload the active frontend revision: {error}"))
}

fn bundled_revision_id(root: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    visit_tree(root, &mut |path, metadata| {
        if metadata.is_file() {
            files.push(path.to_path_buf());
        }
        Ok(())
    })?;
    files.sort();

    let mut hasher = Sha256::new();
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    hasher.update([0]);
    for path in files {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "Bundled frontend asset escaped its root".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path).map_err(io_error("read bundled frontend asset"))?;
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    let digest = format!("{:x}", hasher.finalize());
    Ok(format!("bundled-{}", &digest[..16]))
}

fn validate_frontend_tree(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!(
            "Frontend directory is unavailable: {}",
            root.display()
        ));
    }
    if !root.join("index.html").is_file() {
        return Err(format!(
            "Frontend directory has no index.html: {}",
            root.display()
        ));
    }
    visit_tree(root, &mut |path, metadata| {
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Frontend revisions cannot contain symbolic links: {}",
                path.display()
            ));
        }
        Ok(())
    })
}

fn copy_tree_transactional(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!(
            "Frontend destination already exists: {}",
            destination.display()
        ));
    }
    validate_frontend_tree(source)?;
    let parent = destination
        .parent()
        .ok_or_else(|| "Frontend destination has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(io_error("create frontend destination parent"))?;
    let staging = parent.join(format!(".copy-{}", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(io_error("create frontend copy staging directory"))?;
    let result = copy_tree_contents(source, &staging)
        .and_then(|_| fs::rename(&staging, destination).map_err(io_error("commit frontend copy")));
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn copy_tree_contents(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(io_error("read frontend directory"))? {
        let entry = entry.map_err(io_error("read frontend directory entry"))?;
        let source_path = entry.path();
        let metadata = fs::symlink_metadata(&source_path)
            .map_err(io_error("inspect frontend directory entry"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Frontend revisions cannot contain symbolic links: {}",
                source_path.display()
            ));
        }
        let destination_path = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&destination_path).map_err(io_error("create frontend subdirectory"))?;
            copy_tree_contents(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).map_err(io_error("copy frontend asset"))?;
        }
    }
    Ok(())
}

fn visit_tree(
    root: &Path,
    visitor: &mut impl FnMut(&Path, &fs::Metadata) -> Result<(), String>,
) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(io_error("read frontend tree"))? {
        let entry = entry.map_err(io_error("read frontend tree entry"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(io_error("inspect frontend tree"))?;
        visitor(&path, &metadata)?;
        if metadata.is_dir() {
            visit_tree(&path, visitor)?;
        }
    }
    Ok(())
}

fn validate_uuid(value: &str, field: &str) -> Result<(), String> {
    let parsed = Uuid::parse_str(value).map_err(|_| format!("{field} is invalid"))?;
    if parsed.to_string() != value.to_ascii_lowercase() {
        return Err(format!("{field} is invalid"));
    }
    Ok(())
}

fn validate_revision_id(value: &str) -> Result<(), String> {
    let mut components = Path::new(value).components();
    if value.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err("Frontend revision id is invalid".to_string());
    }
    Ok(())
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn io_error(operation: &'static str) -> impl FnOnce(std::io::Error) -> String {
    move |error| format!("Failed to {operation}: {error}")
}

fn lock_error<T>(error: std::sync::PoisonError<T>) -> String {
    format!("Frontend workbench state lock is unavailable: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_frontend(root: &Path, label: &str) {
        fs::create_dir_all(root.join("assets")).expect("asset directory");
        fs::write(root.join("index.html"), format!("<h1>{label}</h1>")).expect("index.html");
        fs::write(root.join("assets/app.js"), "export {};").expect("asset");
    }

    #[test]
    fn transactional_copy_preserves_a_valid_frontend() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        write_frontend(&source, "source");
        copy_tree_transactional(&source, &destination).expect("copy");
        assert_eq!(
            fs::read_to_string(destination.join("index.html")).expect("copied index"),
            "<h1>source</h1>"
        );
    }

    #[test]
    fn bundled_revision_fingerprint_covers_non_index_assets() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        write_frontend(&source, "source");
        let first = bundled_revision_id(&source).expect("first fingerprint");

        fs::write(source.join("assets/app.js"), "export const changed = true;")
            .expect("change asset");
        let second = bundled_revision_id(&source).expect("second fingerprint");

        assert_ne!(first, second);
    }

    #[cfg(unix)]
    #[test]
    fn validation_rejects_symlinked_assets() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source");
        write_frontend(&source, "source");
        symlink(source.join("index.html"), source.join("linked.html")).expect("symlink");
        assert!(validate_frontend_tree(&source)
            .expect_err("symlink should fail")
            .contains("symbolic links"));
    }

    #[test]
    fn revision_ids_cannot_escape_the_revision_root() {
        for value in ["", ".", "..", "../outside", "/outside"] {
            assert!(validate_revision_id(value).is_err(), "accepted {value}");
        }
        assert!(validate_revision_id("bundled-0123456789abcdef").is_ok());
        assert!(validate_revision_id("creative-38e14f63-30ad-4ad7-9e4e-5ad556450ba3").is_ok());
    }

    #[test]
    fn peer_control_requires_a_visible_local_confirmation_surface() {
        assert!(ensure_local_confirmation_surface(0).is_ok());
        let error = ensure_local_confirmation_surface(1)
            .expect_err("peer-controlled frontend updates must fail loudly");
        assert!(error.contains("Peer Device Mode"));
    }

    #[test]
    fn legacy_state_fields_default_without_data_loss() {
        let state: FrontendWorkbenchState =
            serde_json::from_str(r#"{"activeRevision":"bundled-old","unknownFutureField":true}"#)
                .expect("legacy state");
        assert_eq!(state.active_revision.as_deref(), Some("bundled-old"));
        assert!(state.pending.is_none());
    }

    #[test]
    fn expired_confirmation_restores_the_previous_revision() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manager = FrontendWorkbenchManager::new(temp.path());
        write_frontend(&manager.revision_dir("previous"), "previous");
        write_frontend(&manager.revision_dir("candidate"), "candidate");
        *manager.state.lock().expect("state") = FrontendWorkbenchState {
            schema_version: STATE_SCHEMA_VERSION,
            bundled_revision: Some("previous".to_string()),
            active_revision: Some("candidate".to_string()),
            previous_revision: Some("previous".to_string()),
            pending: Some(PendingFrontendRevision {
                transaction_id: "expired-transaction".to_string(),
                revision_id: "candidate".to_string(),
                previous_revision: "previous".to_string(),
                expires_at_unix_ms: unix_ms().saturating_sub(1),
            }),
        };

        let error = manager
            .confirm_pending("expired-transaction")
            .expect_err("expired confirmation must fail");
        assert!(error.contains("expired"));
        let state = manager.state.lock().expect("state");
        assert_eq!(state.active_revision.as_deref(), Some("previous"));
        assert!(state.pending.is_none());
    }

    #[test]
    fn protocol_uses_immutable_recovery_page_when_no_revision_is_ready() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manager = FrontendWorkbenchManager::new(temp.path());
        let request = tauri::http::Request::builder()
            .uri("bitfun-ui://localhost/index.html")
            .body(Vec::new())
            .expect("request");

        let response = manager.protocol_response(request);

        assert_eq!(
            response.status(),
            tauri::http::StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(String::from_utf8_lossy(response.body()).contains("BitFun frontend recovery"));
    }
}
