//! SSH Remote Connection API
//!
//! Tauri commands for SSH connection management and remote file operations.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{Emitter, State};

use crate::api::app_state::SSHServiceError;
use crate::startup_trace::DesktopStartupTrace;
use crate::AppState;
use bitfun_core::service::remote_ssh::{
    RemoteTreeNode, SSHAuthMethod, SSHConfigEntry, SSHConfigLookupResult, SSHConnectionConfig,
    SSHConnectionResult, SavedConnection, ServerInfo,
};

impl From<SSHServiceError> for String {
    fn from(e: SSHServiceError) -> Self {
        e.to_string()
    }
}

// === SSH Connection Management ===

#[tauri::command]
pub async fn ssh_list_saved_connections(
    state: State<'_, AppState>,
) -> Result<Vec<SavedConnection>, String> {
    let manager = state.get_ssh_manager_async().await?;
    let connections = manager.get_saved_connections().await;
    log::info!(
        "ssh_list_saved_connections returning {} connections",
        connections.len()
    );
    for conn in &connections {
        log::info!(
            "  - id={}, name={}, host={}:{}",
            conn.id,
            conn.name,
            conn.host,
            conn.port
        );
    }
    Ok(connections)
}

#[tauri::command]
pub async fn ssh_save_connection(
    state: State<'_, AppState>,
    config: SSHConnectionConfig,
) -> Result<(), String> {
    log::info!(
        "ssh_save_connection called: id={}, host={}, port={}, username={}",
        config.id,
        config.host,
        config.port,
        config.username
    );
    let manager = state.get_ssh_manager_async().await?;
    manager
        .save_connection(&config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_delete_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let manager = state.get_ssh_manager_async().await?;
    manager
        .delete_saved_connection(&connection_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_has_stored_password(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<bool, String> {
    let manager = state.get_ssh_manager_async().await?;
    Ok(manager.has_stored_password(&connection_id).await)
}

#[tauri::command]
pub async fn ssh_connect(
    state: State<'_, AppState>,
    mut config: SSHConnectionConfig,
) -> Result<SSHConnectionResult, String> {
    log::info!(
        "ssh_connect called: id={}, host={}, port={}, username={}",
        config.id,
        config.host,
        config.port,
        config.username
    );

    let manager = match state.get_ssh_manager_async().await {
        Ok(m) => {
            log::info!("ssh_connect: got SSH manager OK");
            m
        }
        Err(e) => {
            log::error!("ssh_connect: failed to get SSH manager: {}", e);
            return Err(e.to_string());
        }
    };

    if let SSHAuthMethod::Password { ref password } = config.auth {
        if password.is_empty() {
            match manager.load_stored_password(&config.id).await {
                Ok(Some(pwd)) => {
                    config.auth = SSHAuthMethod::Password { password: pwd };
                }
                Ok(None) => {
                    return Err(
                        "SSH password is required (no saved password for this connection)"
                            .to_string(),
                    );
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }

    log::info!("ssh_connect: about to establish connection");
    let config_to_save = config.clone();
    let result = manager.connect(config).await.map_err(|e| e.to_string());
    if result.is_ok() {
        log::info!("ssh_connect: about to save successful connection config");
        if let Err(e) = manager.save_connection(&config_to_save).await {
            log::warn!(
                "ssh_connect: Failed to save successful connection config: {}",
                e
            );
        } else {
            log::info!("ssh_connect: Connection config saved successfully");
        }
    }
    log::info!("ssh_connect result: {:?}", result);
    result
}

#[tauri::command]
pub async fn ssh_disconnect(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let manager = state.get_ssh_manager_async().await?;
    manager
        .disconnect(&connection_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_disconnect_all(state: State<'_, AppState>) -> Result<(), String> {
    let manager = state.get_ssh_manager_async().await?;
    manager.disconnect_all().await;
    Ok(())
}

#[tauri::command]
pub async fn ssh_is_connected(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<bool, String> {
    let manager = state.get_ssh_manager_async().await?;
    let is_connected = manager.is_connected(&connection_id).await;
    log::info!(
        "ssh_is_connected: connection_id={}, is_connected={}",
        connection_id,
        is_connected
    );
    Ok(is_connected)
}

#[tauri::command]
pub async fn ssh_get_server_info(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Option<ServerInfo>, String> {
    let manager = state.get_ssh_manager_async().await?;
    Ok(manager.resolve_remote_home_if_missing(&connection_id).await)
}

#[tauri::command]
pub async fn ssh_get_config(
    state: State<'_, AppState>,
    host: String,
) -> Result<SSHConfigLookupResult, String> {
    let manager = state.get_ssh_manager_async().await?;
    Ok(manager.get_ssh_config(&host).await)
}

#[tauri::command]
pub async fn ssh_list_config_hosts(
    state: State<'_, AppState>,
) -> Result<Vec<SSHConfigEntry>, String> {
    let manager = state.get_ssh_manager_async().await?;
    Ok(manager.list_ssh_config_hosts().await)
}

// === Remote File System Operations ===

#[tauri::command]
pub async fn remote_read_file(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<String, String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    let bytes = remote_fs
        .read_file(&connection_id, &path)
        .await
        .map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_write_file(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
    content: String,
) -> Result<(), String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    remote_fs
        .write_file(&connection_id, &path, content.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_exists(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<bool, String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    remote_fs
        .exists(&connection_id, &path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_read_dir(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<Vec<bitfun_core::service::remote_ssh::RemoteDirEntry>, String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    remote_fs
        .read_dir(&connection_id, &path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_get_tree(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
    depth: Option<u32>,
) -> Result<RemoteTreeNode, String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    remote_fs
        .build_tree(&connection_id, &path, depth)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_create_dir(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
    recursive: bool,
) -> Result<(), String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    if recursive {
        remote_fs.create_dir_all(&connection_id, &path).await
    } else {
        remote_fs.create_dir(&connection_id, &path).await
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_remove(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
    recursive: bool,
) -> Result<(), String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    if recursive {
        remote_fs.remove_dir_all(&connection_id, &path).await
    } else {
        // Check if it's a directory by trying to read it
        let entries = remote_fs.read_dir(&connection_id, &path).await;
        match entries {
            Ok(_) => {
                // It's a directory, but non-recursive remove of non-empty dir
                // Try to remove it anyway (will fail if not empty)
                remote_fs.remove_dir_all(&connection_id, &path).await
            }
            Err(_) => {
                // Not a directory or empty, remove as file
                remote_fs.remove_file(&connection_id, &path).await
            }
        }
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_rename(
    state: State<'_, AppState>,
    connection_id: String,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    remote_fs
        .rename(&connection_id, &old_path, &new_path)
        .await
        .map_err(|e| e.to_string())
}

/// Payload emitted via `download_progress` events during a remote download.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressPayload {
    pub transfer_id: String,
    pub downloaded: u64,
    pub total: u64,
}

/// Read a remote file or directory via SFTP and write it to a local path.
///
/// If `remote_path` is a file, its bytes are read via SFTP and written to
/// `local_path` (binary-safe). If it is a directory, the directory tree is
/// recreated locally: subdirectories are created with `create_dir_all`, and
/// each file is read via SFTP and written to disk.
///
/// Emits `download_progress` events with `{ transferId, downloaded, total }`
/// (bytes) during the SFTP read so the frontend can render a determinate
/// progress bar with speed display. The `transfer_id` lets the frontend
/// distinguish concurrent downloads and cancel individual transfers. Events
/// are throttled to at most one per 100 ms (plus a guaranteed final event) to
/// avoid flooding the webview.
#[tauri::command]
pub async fn remote_download_to_local_path(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    remote_path: String,
    local_path: String,
    transfer_id: String,
) -> Result<(), String> {
    let remote_fs = state.get_remote_file_service_async().await?;

    // Register a cancellation flag for this transfer.
    let cancel_flag = std::sync::Arc::new(AtomicBool::new(false));
    {
        let mut map = state.active_transfers.lock().map_err(|e| e.to_string())?;
        map.insert(transfer_id.clone(), cancel_flag.clone());
    }

    // Check if the remote path is a directory.
    let is_dir = remote_fs
        .is_dir(&connection_id, &remote_path)
        .await
        .map_err(|e| e.to_string())?;

    if is_dir {
        let dir_result = download_directory_from_remote(
            &app_handle,
            &state,
            &connection_id,
            &remote_path,
            &local_path,
            &transfer_id,
            &cancel_flag,
        )
        .await;
        // Clean up the cancellation flag.
        {
            let mut map = state.active_transfers.lock().map_err(|e| e.to_string())?;
            map.remove(&transfer_id);
        }
        return dir_result;
    }

    // Regular file: read via SFTP with progress.
    let mut last_emit = Instant::now();
    let result = remote_fs
        .read_file_with_progress(&connection_id, &remote_path, &mut |downloaded, total| {
            // Throttle: emit at most every 100 ms, plus the final 100% event.
            let now = Instant::now();
            if downloaded >= total || now.duration_since(last_emit).as_millis() >= 100 {
                let _ = app_handle.emit(
                    "download_progress",
                    DownloadProgressPayload {
                        transfer_id: transfer_id.clone(),
                        downloaded,
                        total,
                    },
                );
                last_emit = now;
            }
            // Return false to abort the read if cancelled.
            !cancel_flag.load(Ordering::Relaxed)
        })
        .await;

    // Clean up the cancellation flag.
    {
        let mut map = state.active_transfers.lock().map_err(|e| e.to_string())?;
        map.remove(&transfer_id);
    }

    let bytes = result.map_err(|e| e.to_string())?;

    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&local_path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(path, &bytes).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Recursively download a remote directory to a local path.
///
/// Pre-scans the remote tree to determine total file size, then walks the tree
/// and downloads each file with chunked progress reporting. Emits cumulative
/// `download_progress` events so the frontend can show overall directory
/// download progress.
async fn download_directory_from_remote(
    app_handle: &tauri::AppHandle,
    state: &State<'_, AppState>,
    connection_id: &str,
    remote_dir: &str,
    local_dir: &str,
    transfer_id: &str,
    cancel_flag: &std::sync::Arc<AtomicBool>,
) -> Result<(), String> {
    let remote_fs = state.get_remote_file_service_async().await?;

    // Create the top-level local directory.
    let local_dir_path = std::path::PathBuf::from(local_dir);
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&local_dir_path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    // Pre-scan the remote directory to determine total bytes for progress reporting.
    let mut total_bytes: u64 = 0;
    let mut scan_stack = vec![remote_dir.to_string()];
    while let Some(current) = scan_stack.pop() {
        let entries = remote_fs
            .read_dir(connection_id, &current)
            .await
            .map_err(|e| e.to_string())?;
        for entry in entries {
            if entry.is_dir {
                scan_stack.push(entry.path);
            } else if let Some(size) = entry.size {
                total_bytes += size;
            }
        }
    }

    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now();

    // Walk the remote directory tree.
    let mut stack: Vec<(String, std::path::PathBuf)> =
        vec![(remote_dir.to_string(), std::path::PathBuf::from(local_dir))];

    while let Some((remote_current, local_current)) = stack.pop() {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Transfer cancelled".to_string());
        }

        let entries = remote_fs
            .read_dir(connection_id, &remote_current)
            .await
            .map_err(|e| e.to_string())?;

        for entry in entries {
            let remote_child = entry.path;
            let local_child = local_current.join(&entry.name);

            if entry.is_dir {
                tokio::task::spawn_blocking({
                    let local_child = local_child.clone();
                    move || std::fs::create_dir_all(&local_child).map_err(|e| e.to_string())
                })
                .await
                .map_err(|e| e.to_string())??;
                stack.push((remote_child, local_child));
            } else {
                let base_downloaded = downloaded;
                let bytes = remote_fs
                    .read_file_with_progress(connection_id, &remote_child, &mut |read_bytes, _| {
                        let cumulative = base_downloaded + read_bytes;
                        let now = Instant::now();
                        if cumulative >= total_bytes
                            || now.duration_since(last_emit).as_millis() >= 100
                        {
                            let _ = app_handle.emit(
                                "download_progress",
                                DownloadProgressPayload {
                                    transfer_id: transfer_id.to_string(),
                                    downloaded: cumulative,
                                    total: total_bytes,
                                },
                            );
                            last_emit = now;
                        }
                        !cancel_flag.load(Ordering::Relaxed)
                    })
                    .await
                    .map_err(|e| e.to_string())?;

                let file_size = bytes.len() as u64;
                let local_child_write = local_child.clone();
                tokio::task::spawn_blocking(move || {
                    if let Some(parent) = local_child_write.parent() {
                        if !parent.as_os_str().is_empty() {
                            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                    }
                    std::fs::write(&local_child_write, &bytes).map_err(|e| e.to_string())
                })
                .await
                .map_err(|e| e.to_string())??;

                downloaded += file_size;
            }
        }
    }

    // Final progress event.
    let _ = app_handle.emit(
        "download_progress",
        DownloadProgressPayload {
            transfer_id: transfer_id.to_string(),
            downloaded: total_bytes,
            total: total_bytes,
        },
    );

    Ok(())
}

/// Result of uploading a local path to a remote server.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteUploadResult {
    pub was_directory: bool,
}

/// Payload emitted via `upload_progress` events during a remote upload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadProgressPayload {
    pub transfer_id: String,
    pub uploaded: u64,
    pub total: u64,
}

/// Recursively scan a local directory and return the total size of all
/// regular files in bytes. Used for pre-scanning before directory upload
/// so that overall progress can be reported.
fn scan_directory_total_size(dir: &std::path::Path) -> u64 {
    let mut total: u64 = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(meta) = path.metadata() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

/// Upload a local file or directory tree to a remote path via SFTP.
///
/// If `local_path` is a file, its bytes are written to `remote_path`. If it is
/// a directory, the directory tree is recreated on the remote side:
/// subdirectories are created with `create_dir_all`, and each file is read
/// locally and written via SFTP.
///
/// Emits `upload_progress` events with `{ transferId, uploaded, total }`
/// (bytes) during the SFTP write so the frontend can render a determinate
/// progress bar with speed display. The `transfer_id` lets the frontend
/// distinguish concurrent uploads and cancel individual transfers. Events
/// are throttled to at most one per 100 ms (plus a guaranteed final event).
#[tauri::command]
pub async fn remote_upload_from_local_path(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    local_path: String,
    remote_path: String,
    transfer_id: String,
) -> Result<RemoteUploadResult, String> {
    let local_path = std::path::Path::new(&local_path);

    // Register a cancellation flag for this transfer.
    let cancel_flag = std::sync::Arc::new(AtomicBool::new(false));
    {
        let mut map = state.active_transfers.lock().map_err(|e| e.to_string())?;
        map.insert(transfer_id.clone(), cancel_flag.clone());
    }

    // A directory needs to be walked locally and recreated on the remote side.
    if local_path.is_dir() {
        let dir_result = upload_directory_to_remote(
            &app_handle,
            &state,
            &connection_id,
            local_path,
            &remote_path,
            &transfer_id,
            &cancel_flag,
        )
        .await;
        // Clean up the cancellation flag.
        {
            let mut map = state.active_transfers.lock().map_err(|e| e.to_string())?;
            map.remove(&transfer_id);
        }
        dir_result?;
        return Ok(RemoteUploadResult {
            was_directory: true,
        });
    }

    // Regular file: read locally, write via SFTP with progress.
    let local_path_owned = local_path.to_path_buf();
    let bytes = tokio::task::spawn_blocking(move || {
        std::fs::read(&local_path_owned).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let remote_fs = state.get_remote_file_service_async().await?;
    let mut last_emit = Instant::now();
    let write_result = remote_fs
        .write_file_with_progress(
            &connection_id,
            &remote_path,
            &bytes,
            &mut |written, total| {
                let now = Instant::now();
                if written >= total || now.duration_since(last_emit).as_millis() >= 100 {
                    let _ = app_handle.emit(
                        "upload_progress",
                        UploadProgressPayload {
                            transfer_id: transfer_id.clone(),
                            uploaded: written,
                            total,
                        },
                    );
                    last_emit = now;
                }
                !cancel_flag.load(Ordering::Relaxed)
            },
        )
        .await;

    // Clean up the cancellation flag.
    {
        let mut map = state.active_transfers.lock().map_err(|e| e.to_string())?;
        map.remove(&transfer_id);
    }

    write_result.map_err(|e| e.to_string())?;

    Ok(RemoteUploadResult {
        was_directory: false,
    })
}

/// Recursively upload a local directory to a remote path.
///
/// Pre-scans the directory to determine total file size, then walks the tree
/// and uploads each file with chunked progress reporting. Emits cumulative
/// `upload_progress` events so the frontend can show overall directory upload
/// progress.
async fn upload_directory_to_remote(
    app_handle: &tauri::AppHandle,
    state: &State<'_, AppState>,
    connection_id: &str,
    local_dir: &std::path::Path,
    remote_dir: &str,
    transfer_id: &str,
    cancel_flag: &std::sync::Arc<AtomicBool>,
) -> Result<(), String> {
    let remote_fs = state.get_remote_file_service_async().await?;

    // Create the top-level remote directory.
    remote_fs
        .create_dir_all(connection_id, remote_dir)
        .await
        .map_err(|e| e.to_string())?;

    // Pre-scan the directory to determine total bytes for progress reporting.
    let local_dir_owned = local_dir.to_path_buf();
    let total_bytes =
        tokio::task::spawn_blocking(move || scan_directory_total_size(&local_dir_owned))
            .await
            .map_err(|e| e.to_string())?;

    let mut uploaded: u64 = 0;
    let mut last_emit = Instant::now();

    // Walk the local directory tree.
    let mut stack: Vec<(std::path::PathBuf, String)> =
        vec![(local_dir.to_path_buf(), remote_dir.to_string())];

    while let Some((local_current, remote_current)) = stack.pop() {
        let entries = tokio::task::spawn_blocking(move || {
            std::fs::read_dir(&local_current)
                .map_err(|e| e.to_string())
                .and_then(|dir| {
                    dir.collect::<Result<Vec<_>, _>>()
                        .map_err(|e| e.to_string())
                })
        })
        .await
        .map_err(|e| e.to_string())??;

        for entry in entries {
            let entry_path = entry.path();
            let file_name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue,
            };
            let remote_child = if remote_current.ends_with('/') {
                format!("{}{}", remote_current, file_name)
            } else {
                format!("{}/{}", remote_current, file_name)
            };

            if entry_path.is_dir() {
                remote_fs
                    .create_dir_all(connection_id, &remote_child)
                    .await
                    .map_err(|e| e.to_string())?;
                stack.push((entry_path, remote_child));
            } else {
                let local_file = entry_path.clone();
                let bytes = tokio::task::spawn_blocking(move || {
                    std::fs::read(&local_file).map_err(|e| e.to_string())
                })
                .await
                .map_err(|e| e.to_string())??;

                let file_size = bytes.len() as u64;
                let base_uploaded = uploaded;
                remote_fs
                    .write_file_with_progress(
                        connection_id,
                        &remote_child,
                        &bytes,
                        &mut |written, _| {
                            let cumulative = base_uploaded + written;
                            let now = Instant::now();
                            if cumulative >= total_bytes
                                || now.duration_since(last_emit).as_millis() >= 100
                            {
                                let _ = app_handle.emit(
                                    "upload_progress",
                                    UploadProgressPayload {
                                        transfer_id: transfer_id.to_string(),
                                        uploaded: cumulative,
                                        total: total_bytes,
                                    },
                                );
                                last_emit = now;
                            }
                            !cancel_flag.load(Ordering::Relaxed)
                        },
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                uploaded += file_size;
            }
        }
    }

    Ok(())
}

/// Cancel an in-progress file transfer by setting its cancellation flag.
///
/// The transfer will abort at the next chunk boundary and the original
/// download/upload command will return an error.
#[tauri::command]
pub async fn cancel_transfer(
    state: State<'_, AppState>,
    transfer_id: String,
) -> Result<(), String> {
    let map = state.active_transfers.lock().map_err(|e| e.to_string())?;
    if let Some(flag) = map.get(&transfer_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn remote_execute(
    state: State<'_, AppState>,
    connection_id: String,
    command: String,
) -> Result<(String, String, i32), String> {
    let manager = state.get_ssh_manager_async().await?;
    manager
        .execute_command(&connection_id, &command)
        .await
        .map_err(|e| e.to_string())
}

// === Remote Workspace Management ===

#[tauri::command]
pub async fn remote_open_workspace(
    state: State<'_, AppState>,
    connection_id: String,
    remote_path: String,
) -> Result<(), String> {
    let remote_path =
        bitfun_core::service::remote_ssh::normalize_remote_workspace_path(&remote_path);
    let manager = state.get_ssh_manager_async().await?;

    // Verify connection exists
    if !manager.is_connected(&connection_id).await {
        return Err("Not connected to remote server".to_string());
    }

    // Verify remote path exists
    let remote_fs = state.get_remote_file_service_async().await?;
    let exists = remote_fs
        .exists(&connection_id, &remote_path)
        .await
        .map_err(|e| e.to_string())?;

    if !exists {
        return Err(format!("Remote path does not exist: {}", remote_path));
    }

    // Get connection info for workspace
    let connections = manager.get_saved_connections().await;
    let conn = connections.iter().find(|c| c.id == connection_id);

    let ssh_host = manager
        .get_connection_config(&connection_id)
        .await
        .map(|c| c.host)
        .unwrap_or_default();

    let workspace = crate::api::RemoteWorkspace {
        connection_id: connection_id.clone(),
        connection_name: conn.map(|c| c.name.clone()).unwrap_or_default(),
        remote_path: remote_path.clone(),
        ssh_host,
    };

    state
        .set_remote_workspace(workspace)
        .await
        .map_err(|e| e.to_string())?;

    log::info!(
        "Opened remote workspace: {} on connection {}",
        remote_path,
        connection_id
    );
    Ok(())
}

#[tauri::command]
pub async fn remote_close_workspace(state: State<'_, AppState>) -> Result<(), String> {
    state.clear_remote_workspace().await;
    log::info!("Closed remote workspace");
    Ok(())
}

#[tauri::command]
pub async fn remote_remove_workspace(
    state: State<'_, AppState>,
    connection_id: String,
    remote_path: String,
) -> Result<(), String> {
    state
        .unregister_remote_workspace_entry(&connection_id, &remote_path)
        .await;
    log::info!(
        "Removed remote workspace restore entry: connection_id={}, remote_path={}",
        connection_id,
        remote_path
    );
    Ok(())
}

#[tauri::command]
pub async fn remote_get_workspace_info(
    state: State<'_, AppState>,
    startup_trace: State<'_, DesktopStartupTrace>,
) -> Result<Option<crate::api::RemoteWorkspace>, String> {
    let trace_started = Instant::now();
    let workspace = state.get_remote_workspace_async().await;
    log::info!("remote_get_workspace_info: returning {:?}", workspace);
    startup_trace.record_tauri_command_elapsed("remote_get_workspace_info", None, trace_started);
    Ok(workspace)
}
