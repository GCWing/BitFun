//! LSP global manager
//!
//! Uses a global singleton to avoid adding dependencies to `AppState`.

use crate::infrastructure::try_get_path_manager_arc;
use log::{info, warn};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::file_sync::{FileSyncConfig, LspFileSync};
use super::{LspManager, WorkspaceLspManager};

/// Global LSP manager instance.
static GLOBAL_LSP_MANAGER: OnceCell<Arc<RwLock<LspManager>>> = OnceCell::new();

/// Global workspace manager mapping.
static WORKSPACE_MANAGERS: OnceCell<Arc<RwLock<HashMap<String, Arc<WorkspaceLspManager>>>>> =
    OnceCell::new();

/// Global file sync manager.
static GLOBAL_FILE_SYNC: OnceCell<Arc<LspFileSync>> = OnceCell::new();

/// Initializes the global LSP manager.
pub async fn initialize_global_lsp_manager() -> anyhow::Result<()> {
    if GLOBAL_LSP_MANAGER.get().is_some() {
        warn!("LSP Manager already initialized");
        return Ok(());
    }

    let user_data_dir = try_get_path_manager_arc()
        .map_err(|e| anyhow::anyhow!("Failed to create PathManager: {}", e))?
        .user_data_dir();

    let plugins_dir = user_data_dir.join("lsp-plugins");

    let manager = LspManager::new(plugins_dir);

    manager.initialize().await?;

    GLOBAL_LSP_MANAGER
        .set(Arc::new(RwLock::new(manager)))
        .map_err(|_| anyhow::anyhow!("Failed to set global LSP manager"))?;

    WORKSPACE_MANAGERS
        .set(Arc::new(RwLock::new(HashMap::new())))
        .map_err(|_| anyhow::anyhow!("Failed to set workspace managers"))?;

    let file_sync = LspFileSync::new(FileSyncConfig::default());
    GLOBAL_FILE_SYNC
        .set(file_sync)
        .map_err(|_| anyhow::anyhow!("Failed to set file sync"))?;

    info!("Global LSP Manager initialized");

    Ok(())
}

/// Returns the global LSP manager.
pub fn get_global_lsp_manager() -> anyhow::Result<Arc<RwLock<LspManager>>> {
    GLOBAL_LSP_MANAGER
        .get()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("LSP Manager not initialized"))
}

/// Returns whether the LSP manager has been initialized.
pub fn is_lsp_manager_initialized() -> bool {
    GLOBAL_LSP_MANAGER.get().is_some()
}

/// Opens a workspace (creates a workspace manager).
pub async fn open_workspace(workspace_path: PathBuf) -> anyhow::Result<Arc<WorkspaceLspManager>> {
    open_workspace_with_emitter(workspace_path, None).await
}

/// Opens a workspace and sets an `EventEmitter` (used for emitting events).
pub async fn open_workspace_with_emitter(
    workspace_path: PathBuf,
    emitter: Option<Arc<dyn crate::infrastructure::events::EventEmitter>>,
) -> anyhow::Result<Arc<WorkspaceLspManager>> {
    info!("Opening workspace: {:?}", workspace_path);

    if !is_lsp_manager_initialized() {
        warn!("LSP Manager not initialized yet, initializing now...");
        initialize_global_lsp_manager().await?;
    }

    let lsp_manager = get_global_lsp_manager()?;
    let workspace_key = workspace_path.to_string_lossy().to_string();

    let managers = WORKSPACE_MANAGERS
        .get()
        .ok_or_else(|| anyhow::anyhow!("Workspace managers not initialized"))?;

    {
        let managers_read = managers.read().await;
        if let Some(manager) = managers_read.get(&workspace_key) {
            if let Some(e) = emitter {
                manager.set_emitter(e).await;
            }
            return Ok(manager.clone());
        }
    }

    let workspace_manager = WorkspaceLspManager::new(workspace_path.clone(), lsp_manager).await;

    if let Some(e) = emitter {
        workspace_manager.set_emitter(e).await;
    }

    {
        let mut managers_write = managers.write().await;
        managers_write.insert(workspace_key.clone(), workspace_manager.clone());
    }

    if let Some(file_sync) = GLOBAL_FILE_SYNC.get() {
        if let Err(e) = file_sync
            .watch_workspace(workspace_path.clone(), workspace_manager.clone())
            .await
        {
            warn!("Failed to start file sync for workspace: {}", e);
        }
    }

    info!("Workspace opened: {:?}", workspace_path);
    Ok(workspace_manager)
}

/// Closes a workspace (cleans up the workspace manager).
pub async fn close_workspace(workspace_path: PathBuf) -> anyhow::Result<()> {
    info!("Closing workspace: {:?}", workspace_path);

    let workspace_key = workspace_path.to_string_lossy().to_string();

    let managers = WORKSPACE_MANAGERS
        .get()
        .ok_or_else(|| anyhow::anyhow!("Workspace managers not initialized"))?;

    let manager = {
        let mut managers_write = managers.write().await;
        managers_write.remove(&workspace_key)
    };

    if let Some(manager) = manager {
        if let Some(file_sync) = GLOBAL_FILE_SYNC.get() {
            if let Err(e) = file_sync.unwatch_workspace(&workspace_path).await {
                warn!("Failed to stop file sync: {}", e);
            }
        }

        manager.dispose().await?;
        info!("Workspace closed: {:?}", workspace_path);
    } else {
        warn!("Workspace not found: {:?}", workspace_path);
    }

    Ok(())
}

/// Returns the workspace manager.
pub async fn get_workspace_manager(
    workspace_path: PathBuf,
) -> anyhow::Result<Arc<WorkspaceLspManager>> {
    let workspace_key = workspace_path.to_string_lossy().to_string();

    let managers = WORKSPACE_MANAGERS
        .get()
        .ok_or_else(|| anyhow::anyhow!("Workspace managers not initialized"))?;

    let managers_read = managers.read().await;
    managers_read
        .get(&workspace_key)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Workspace not found: {:?}", workspace_path))
}

/// Returns all opened workspace paths.
pub async fn get_all_workspace_paths() -> anyhow::Result<Vec<String>> {
    let managers = WORKSPACE_MANAGERS
        .get()
        .ok_or_else(|| anyhow::anyhow!("Workspace managers not initialized"))?;

    let managers_read = managers.read().await;
    Ok(managers_read.keys().cloned().collect())
}
