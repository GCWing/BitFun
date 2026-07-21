//! CLI account auto-sync (settings + session upload), matching Desktop semantics.

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use bitfun_core::product_runtime::CoreAgentRuntimeCompatibility;
use bitfun_core::service::config::get_global_config_service;
use bitfun_core::service::remote_connect::settings_sync;
use bitfun_core::service::remote_connect::{sync_state, AccountClient};

use crate::account::read_account_context;

const UPLOAD_CONCURRENCY_CHUNK: usize = 5;

/// Start the continuous settings sync loop (debounced push + 30s pull).
/// Started once per process (interactive TUI and daemon); every cycle
/// silently skips while logged out and converges as soon as an account
/// session exists. Peer Mode controllers are notified via DeviceEvent when
/// this host's effective settings change.
pub(crate) fn start_settings_sync_loop() {
    let hooks = settings_sync::SettingsSyncHooks {
        account_context: Some(Arc::new(|| {
            Box::pin(async { read_account_context().await })
        })),
        on_settings_applied: Some(Arc::new(|| {
            crate::peer_host::notify_controllers_settings_changed();
        })),
        on_settings_pushed: Some(Arc::new(|| {
            crate::peer_host::notify_controllers_settings_changed();
        })),
        on_token_expired: Some(Arc::new(|| crate::account::mark_token_expired())),
        ..Default::default()
    };
    settings_sync::start_settings_sync_engine(hooks);
}

/// Notify the sync loop that local settings changed (TUI edits, peer
/// `set_config`). Upload is debounced and content-hash deduped.
pub(crate) fn notify_local_settings_changed() {
    settings_sync::notify_settings_changed();
}

/// Best-effort one-shot settings push for short-lived management commands
/// (e.g. `bitfun models set-default`) where the sync loop never starts.
/// Silently no-ops when logged out; failures are logged, not fatal.
pub(crate) async fn push_settings_after_local_change() {
    // Management commands never restore the persisted account session into
    // memory — do it on demand so the push can authenticate.
    if read_account_context().await.is_err() {
        crate::account::try_restore_session().await;
    }
    let Ok((account, relay_url)) = read_account_context().await else {
        return;
    };
    match settings_sync::push_settings_now(&account, &relay_url).await {
        Ok(true) => tracing::info!("Settings pushed to account cloud"),
        Ok(false) => {}
        Err(e) => tracing::warn!("Settings push failed: {e}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncStatus {
    Idle,
    Syncing,
    Done,
    Failed,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SyncProgress {
    pub status: SyncStatus,
    pub phase: String,
    pub percent: u8,
    pub current: Option<usize>,
    pub total: Option<usize>,
    pub detail: Option<String>,
    pub error: Option<String>,
    pub settings_synced: bool,
    pub sessions_exported: usize,
}

impl Default for SyncProgress {
    fn default() -> Self {
        Self {
            status: SyncStatus::Idle,
            phase: String::new(),
            percent: 0,
            current: None,
            total: None,
            detail: None,
            error: None,
            settings_synced: false,
            sessions_exported: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AutoSyncResult {
    pub settings_synced: bool,
    pub sessions_exported: usize,
    #[allow(dead_code)]
    pub sessions_imported: usize,
}

#[derive(Serialize, Deserialize)]
struct SessionBundle {
    session_id: String,
    metadata: serde_json::Value,
    turns: Vec<serde_json::Value>,
    source_device_id: Option<String>,
    source_device_name: Option<String>,
}

static SYNC_PROGRESS: OnceLock<Arc<RwLock<SyncProgress>>> = OnceLock::new();
static AUTO_SYNC_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

fn sync_progress_store() -> &'static Arc<RwLock<SyncProgress>> {
    SYNC_PROGRESS.get_or_init(|| Arc::new(RwLock::new(SyncProgress::default())))
}

pub(crate) async fn current_sync_progress() -> SyncProgress {
    sync_progress_store().read().await.clone()
}

pub(crate) fn sync_in_flight() -> bool {
    AUTO_SYNC_IN_FLIGHT.load(Ordering::SeqCst)
}

async fn set_progress(mut update: impl FnMut(&mut SyncProgress)) {
    let mut guard = sync_progress_store().write().await;
    update(&mut guard);
}

async fn emit_progress(
    phase: &str,
    percent: u8,
    current: Option<usize>,
    total: Option<usize>,
    detail: Option<&str>,
) {
    set_progress(|p| {
        p.status = SyncStatus::Syncing;
        p.phase = phase.to_string();
        p.percent = percent;
        p.current = current;
        p.total = total;
        p.detail = detail.map(|s| s.to_string());
        p.error = None;
    })
    .await;
}

/// Start auto-sync in the background. Returns immediately; progress is in
/// [`current_sync_progress`].
pub(crate) fn start_auto_sync_background(
    compatibility: CoreAgentRuntimeCompatibility,
    is_first_login: bool,
    workspace_path: PathBuf,
) {
    if AUTO_SYNC_IN_FLIGHT.swap(true, Ordering::SeqCst) {
        tracing::warn!("Account auto-sync already in flight; skipping duplicate start");
        return;
    }
    tokio::spawn(async move {
        let result = run_auto_sync(&compatibility, is_first_login, &workspace_path).await;
        AUTO_SYNC_IN_FLIGHT.store(false, Ordering::SeqCst);
        match result {
            Ok(r) => {
                set_progress(|p| {
                    p.status = SyncStatus::Done;
                    p.phase = "done".into();
                    p.percent = 100;
                    p.settings_synced = r.settings_synced;
                    p.sessions_exported = r.sessions_exported;
                    p.error = None;
                })
                .await;
            }
            Err(e) => {
                set_progress(|p| {
                    p.status = SyncStatus::Failed;
                    p.error = Some(e.to_string());
                })
                .await;
                tracing::warn!("Account auto-sync failed: {e}");
            }
        }
    });
}

pub(crate) async fn run_auto_sync(
    compatibility: &CoreAgentRuntimeCompatibility,
    is_first_login: bool,
    workspace_path: &Path,
) -> Result<AutoSyncResult> {
    set_progress(|p| {
        *p = SyncProgress {
            status: SyncStatus::Syncing,
            phase: "starting".into(),
            percent: 1,
            ..SyncProgress::default()
        };
    })
    .await;

    let (acct_session, relay_url) = read_account_context().await?;
    let client = AccountClient::new();

    let settings_synced = if is_first_login {
        emit_progress("uploading_settings", 5, None, None, None).await;
        let config_service = get_global_config_service()
            .await
            .map_err(|e| anyhow!("config service: {e}"))?;
        let exported = config_service
            .export_config()
            .await
            .map_err(|e| anyhow!("export config: {e}"))?;
        let config_json =
            serde_json::to_string(&exported).map_err(|e| anyhow!("serialize config: {e}"))?;
        settings_sync::upload_settings_payload(&acct_session, &relay_url, &config_json)
            .await
            .map_err(|e| anyhow!("upload settings: {e}"))?;
        emit_progress("settings_done", 15, None, None, None).await;
        true
    } else {
        emit_progress("downloading_settings", 5, None, None, None).await;
        let cloud = client
            .fetch_settings_with_version(&relay_url, &acct_session)
            .await
            .map_err(|e| anyhow!("fetch settings: {e}"))?;
        if let Some(blob) = cloud {
            emit_progress("applying_settings", 10, None, None, None).await;
            // Explicit user choice ("use cloud") — always apply, even when the
            // cursor says this device already has this version.
            settings_sync::apply_settings_blob(&acct_session, &blob, true)
                .await
                .map_err(|e| anyhow!("apply cloud config: {e}"))?;
            emit_progress("settings_done", 15, None, None, None).await;
            true
        } else {
            emit_progress("settings_done", 15, None, None, None).await;
            false
        }
    };

    emit_progress("listing_sessions", 18, None, None, None).await;
    let storage_path = workspace_path.to_path_buf();

    let local_sessions = compatibility
        .list_persisted_sessions(&storage_path)
        .await
        .map_err(|e| anyhow!("list sessions: {e}"))?;

    emit_progress(
        "exporting_sessions",
        20,
        Some(0),
        Some(local_sessions.len()),
        None,
    )
    .await;

    let mut sync_state_local = sync_state::load(&acct_session.user_id);
    let mut pending_uploads: Vec<(String, String, String)> = Vec::new();
    for meta in local_sessions.iter() {
        let turns = compatibility
            .load_persisted_session_turns(&storage_path, &meta.session_id, None)
            .await
            .map_err(|e| anyhow!("load turns: {e}"))?;
        let metadata_json =
            serde_json::to_value(meta).map_err(|e| anyhow!("serialize metadata: {e}"))?;
        let turns_json: Vec<serde_json::Value> = turns
            .iter()
            .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null))
            .collect();
        let bundle = SessionBundle {
            session_id: meta.session_id.clone(),
            metadata: metadata_json,
            turns: turns_json,
            source_device_id: None,
            source_device_name: None,
        };
        let bundle_json =
            serde_json::to_string(&bundle).map_err(|e| anyhow!("serialize bundle: {e}"))?;
        let hash = sync_state::content_hash(&bundle_json);
        if sync_state_local.uploaded_hash(&meta.session_id) == Some(hash.as_str()) {
            continue;
        }
        pending_uploads.push((meta.session_id.clone(), bundle_json, hash));
    }

    let upload_total = pending_uploads.len();
    emit_progress("exporting_sessions", 20, Some(0), Some(upload_total), None).await;

    let mut uploaded: Vec<(String, String, i64)> = Vec::new();
    for (chunk_idx, chunk) in pending_uploads.chunks(UPLOAD_CONCURRENCY_CHUNK).enumerate() {
        let mut handles = Vec::new();
        for (session_id, bundle_json, hash) in chunk {
            let client = AccountClient::new();
            let relay_url = relay_url.clone();
            let acct_session = acct_session.clone();
            let session_id = session_id.clone();
            let bundle_json = bundle_json.clone();
            let hash = hash.clone();
            handles.push(tokio::spawn(async move {
                let result = client
                    .upload_session(&relay_url, &acct_session, &session_id, &bundle_json)
                    .await;
                (session_id, hash, result)
            }));
        }
        for handle in handles {
            let done_base = chunk_idx * UPLOAD_CONCURRENCY_CHUNK;
            match handle.await {
                Ok((session_id, hash, Ok(version))) => {
                    uploaded.push((session_id.clone(), hash, version));
                    let done = uploaded.len();
                    let percent = if upload_total == 0 {
                        95u8
                    } else {
                        20 + ((75 * done) / upload_total) as u8
                    };
                    emit_progress(
                        "exporting_sessions",
                        percent.min(95),
                        Some(done),
                        Some(upload_total),
                        Some(&session_id),
                    )
                    .await;
                }
                Ok((session_id, _, Err(e))) => {
                    tracing::warn!("Auto-sync upload {session_id} failed: {e}");
                    let _ = done_base;
                }
                Err(e) => {
                    tracing::warn!("Auto-sync upload task join failed: {e}");
                }
            }
        }
    }

    let exported = uploaded.len();
    let mut max_uploaded_version = sync_state_local.last_session_since;
    for (session_id, hash, version) in uploaded {
        sync_state_local.set_uploaded_hash(&session_id, hash);
        if version > max_uploaded_version {
            max_uploaded_version = version;
        }
    }
    if max_uploaded_version > sync_state_local.last_session_since {
        sync_state_local.last_session_since = max_uploaded_version;
    }
    let _ = sync_state::save(&acct_session.user_id, &sync_state_local);

    tracing::info!("Auto-sync: settings={settings_synced} exported={exported} imported=0");
    emit_progress("done", 100, Some(exported), Some(0), None).await;

    Ok(AutoSyncResult {
        settings_synced,
        sessions_exported: exported,
        sessions_imported: 0,
    })
}

pub(crate) fn sync_phase_label(progress: &SyncProgress) -> String {
    match progress.phase.as_str() {
        "uploading_settings" => "Uploading settings…".into(),
        "downloading_settings" => "Downloading settings…".into(),
        "applying_settings" => "Applying cloud settings…".into(),
        "settings_done" => "Settings sync done".into(),
        "listing_sessions" => "Listing local sessions…".into(),
        "exporting_sessions" => {
            if let (Some(c), Some(t)) = (progress.current, progress.total) {
                format!("Uploading sessions ({c}/{t})…")
            } else {
                "Uploading sessions…".into()
            }
        }
        "done" => format!("Sync complete (exported {})", progress.sessions_exported),
        "starting" => "Starting sync…".into(),
        other if other.is_empty() => "Sync".into(),
        other => other.to_string(),
    }
}
