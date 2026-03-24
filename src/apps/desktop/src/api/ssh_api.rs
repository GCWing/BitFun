//! SSH Remote Connection API
//!
//! Tauri commands for SSH connection management and remote file operations.

use tauri::State;

use bitfun_core::service::remote_ssh::{
    SSHConnectionConfig, SSHConnectionResult, SavedConnection, RemoteTreeNode,
    SSHConfigLookupResult, SSHConfigEntry,
};
use crate::api::app_state::SSHServiceError;
use crate::AppState;

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
    log::info!("ssh_list_saved_connections returning {} connections", connections.len());
    for conn in &connections {
        log::info!("  - id={}, name={}, host={}:{}", conn.id, conn.name, conn.host, conn.port);
    }
    Ok(connections)
}

#[tauri::command]
pub async fn ssh_save_connection(
    state: State<'_, AppState>,
    config: SSHConnectionConfig,
) -> Result<(), String> {
    log::info!("ssh_save_connection called: id={}, host={}, port={}, username={}",
        config.id, config.host, config.port, config.username);
    let manager = state.get_ssh_manager_async().await?;
    manager.save_connection(&config).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_delete_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let manager = state.get_ssh_manager_async().await?;
    manager.delete_saved_connection(&connection_id).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_connect(
    state: State<'_, AppState>,
    config: SSHConnectionConfig,
) -> Result<SSHConnectionResult, String> {
    log::info!("ssh_connect called: id={}, host={}, port={}, username={}",
        config.id, config.host, config.port, config.username);

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

    // First save the connection config so it persists across restarts
    log::info!("ssh_connect: about to save connection config");
    if let Err(e) = manager.save_connection(&config).await {
        log::warn!("ssh_connect: Failed to save connection config before connect: {}", e);
        // Continue anyway - connection might still work
    } else {
        log::info!("ssh_connect: Connection config saved successfully");
    }

    log::info!("ssh_connect: about to establish connection");
    let result = manager.connect(config).await
        .map_err(|e| e.to_string());
    log::info!("ssh_connect result: {:?}", result);
    result
}

#[tauri::command]
pub async fn ssh_disconnect(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let manager = state.get_ssh_manager_async().await?;
    manager.disconnect(&connection_id).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ssh_disconnect_all(
    state: State<'_, AppState>,
) -> Result<(), String> {
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
    log::info!("ssh_is_connected: connection_id={}, is_connected={}", connection_id, is_connected);
    Ok(is_connected)
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
    let bytes = remote_fs.read_file(&connection_id, &path).await
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
    remote_fs.write_file(&connection_id, &path, content.as_bytes()).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_exists(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<bool, String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    remote_fs.exists(&connection_id, &path).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_read_dir(
    state: State<'_, AppState>,
    connection_id: String,
    path: String,
) -> Result<Vec<bitfun_core::service::remote_ssh::RemoteDirEntry>, String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    remote_fs.read_dir(&connection_id, &path).await
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
    remote_fs.build_tree(&connection_id, &path, depth).await
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
    remote_fs.rename(&connection_id, &old_path, &new_path).await
        .map_err(|e| e.to_string())
}

/// Read a remote file via SFTP and write it to a local path (binary-safe).
#[tauri::command]
pub async fn remote_download_to_local_path(
    state: State<'_, AppState>,
    connection_id: String,
    remote_path: String,
    local_path: String,
) -> Result<(), String> {
    let remote_fs = state.get_remote_file_service_async().await?;
    let bytes = remote_fs
        .read_file(&connection_id, &remote_path)
        .await
        .map_err(|e| e.to_string())?;

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

/// Read a local file and write it to the remote path via SFTP (binary-safe).
#[tauri::command]
pub async fn remote_upload_from_local_path(
    state: State<'_, AppState>,
    connection_id: String,
    local_path: String,
    remote_path: String,
) -> Result<(), String> {
    let bytes = tokio::task::spawn_blocking(move || {
        std::fs::read(&local_path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let remote_fs = state.get_remote_file_service_async().await?;
    remote_fs
        .write_file(&connection_id, &remote_path, &bytes)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remote_execute(
    state: State<'_, AppState>,
    connection_id: String,
    command: String,
) -> Result<(String, String, i32), String> {
    let manager = state.get_ssh_manager_async().await?;
    manager.execute_command(&connection_id, &command).await
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
    let exists = remote_fs.exists(&connection_id, &remote_path).await
        .map_err(|e| e.to_string())?;

    if !exists {
        return Err(format!("Remote path does not exist: {}", remote_path));
    }

    // Get connection info for workspace
    let connections = manager.get_saved_connections().await;
    let conn = connections.iter().find(|c| c.id == connection_id);

    let workspace = crate::api::RemoteWorkspace {
        connection_id: connection_id.clone(),
        connection_name: conn.map(|c| c.name.clone()).unwrap_or_default(),
        remote_path: remote_path.clone(),
    };

    state.set_remote_workspace(workspace).await
        .map_err(|e| e.to_string())?;

    log::info!("Opened remote workspace: {} on connection {}", remote_path, connection_id);
    Ok(())
}

#[tauri::command]
pub async fn remote_close_workspace(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.clear_remote_workspace().await;
    log::info!("Closed remote workspace");
    Ok(())
}

#[tauri::command]
pub async fn remote_get_workspace_info(
    state: State<'_, AppState>,
) -> Result<Option<crate::api::RemoteWorkspace>, String> {
    let workspace = state.get_remote_workspace_async().await;
    log::info!("remote_get_workspace_info: returning {:?}", workspace);
    Ok(workspace)
}
