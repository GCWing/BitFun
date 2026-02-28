//! Terminal API

use log::{error, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use bitfun_core::service::runtime::RuntimeManager;
use bitfun_core::service::terminal::{
    AcknowledgeRequest as CoreAcknowledgeRequest, CloseSessionRequest as CoreCloseSessionRequest,
    CreateSessionRequest as CoreCreateSessionRequest,
    ExecuteCommandRequest as CoreExecuteCommandRequest,
    ExecuteCommandResponse as CoreExecuteCommandResponse,
    GetHistoryRequest as CoreGetHistoryRequest, GetHistoryResponse as CoreGetHistoryResponse,
    ResizeRequest as CoreResizeRequest, SendCommandRequest as CoreSendCommandRequest,
    SessionResponse as CoreSessionResponse, ShellInfo as CoreShellInfo, ShellType,
    SignalRequest as CoreSignalRequest, TerminalApi, TerminalConfig,
    WriteRequest as CoreWriteRequest,
};

pub struct TerminalState {
    api: Arc<Mutex<Option<TerminalApi>>>,
    initialized: Arc<Mutex<bool>>,
}

impl TerminalState {
    pub fn new() -> Self {
        Self {
            api: Arc::new(Mutex::new(None)),
            initialized: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn get_or_init_api(&self) -> Result<TerminalApi, String> {
        let mut initialized = self.initialized.lock().await;
        let mut api_guard = self.api.lock().await;

        if !*initialized {
            let mut config = TerminalConfig::default();

            // Set scripts directory to app data dir: {config_dir}/bitfun/temp/scripts
            let scripts_dir = Self::get_scripts_dir();
            config.shell_integration.scripts_dir = Some(scripts_dir);

            // Prepend BitFun-managed runtime dirs to PATH so Bash/Skill commands can
            // run on machines without preinstalled dev tools.
            if let Ok(runtime_manager) = RuntimeManager::new() {
                let current_path = std::env::var("PATH").ok();
                if let Some(merged_path) = runtime_manager.merged_path_env(current_path.as_deref())
                {
                    config.env.insert("PATH".to_string(), merged_path.clone());
                    #[cfg(windows)]
                    {
                        config.env.insert("Path".to_string(), merged_path);
                    }
                }
            }

            let api = TerminalApi::new(config).await;
            *api_guard = Some(api);
            *initialized = true;
        }

        Ok(TerminalApi::from_singleton()
            .map_err(|e| format!("Terminal API not initialized: {}", e))?)
    }

    /// Get the scripts directory path for shell integration
    /// Uses the same path structure as PathManager
    fn get_scripts_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bitfun")
            .join("temp")
            .join("scripts")
    }
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub session_id: Option<String>,
    pub name: Option<String>,
    pub shell_type: Option<String>,
    pub working_directory: Option<String>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub id: String,
    pub name: String,
    pub shell_type: String,
    pub cwd: String,
    pub pid: Option<u32>,
    pub status: String,
    pub cols: u16,
    pub rows: u16,
}

impl From<CoreSessionResponse> for SessionResponse {
    fn from(resp: CoreSessionResponse) -> Self {
        Self {
            id: resp.id,
            name: resp.name,
            shell_type: format!("{:?}", resp.shell_type),
            cwd: resp.cwd,
            pid: resp.pid,
            status: resp.status,
            cols: resp.cols,
            rows: resp.rows,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellInfo {
    pub shell_type: String,
    pub name: String,
    pub path: String,
    pub version: Option<String>,
    pub available: bool,
}

impl From<CoreShellInfo> for ShellInfo {
    fn from(info: CoreShellInfo) -> Self {
        Self {
            shell_type: format!("{:?}", info.shell_type),
            name: info.name,
            path: info.path,
            version: info.version,
            available: info.available,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteRequest {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizeRequest {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseSessionRequest {
    pub session_id: String,
    pub immediate: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalRequest {
    pub session_id: String,
    pub signal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcknowledgeRequest {
    pub session_id: String,
    pub char_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteCommandRequest {
    pub session_id: String,
    pub command: String,
    pub timeout_ms: Option<u64>,
    pub prevent_history: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteCommandResponse {
    pub command: String,
    pub command_id: String,
    pub output: String,
    pub exit_code: Option<i32>,
}

impl From<CoreExecuteCommandResponse> for ExecuteCommandResponse {
    fn from(resp: CoreExecuteCommandResponse) -> Self {
        Self {
            command: resp.command,
            command_id: resp.command_id,
            output: resp.output,
            exit_code: resp.exit_code,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendCommandRequest {
    pub session_id: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetHistoryRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetHistoryResponse {
    pub session_id: String,
    pub data: String,
    pub history_size: usize,
}

impl From<CoreGetHistoryResponse> for GetHistoryResponse {
    fn from(resp: CoreGetHistoryResponse) -> Self {
        Self {
            session_id: resp.session_id,
            data: resp.data,
            history_size: resp.history_size,
        }
    }
}

fn parse_shell_type(s: &str) -> Option<ShellType> {
    match s.to_lowercase().as_str() {
        "powershell" => Some(ShellType::PowerShell),
        "powershellcore" | "pwsh" => Some(ShellType::PowerShellCore),
        "cmd" => Some(ShellType::Cmd),
        "bash" => Some(ShellType::Bash),
        "zsh" => Some(ShellType::Zsh),
        "fish" => Some(ShellType::Fish),
        "sh" => Some(ShellType::Sh),
        "ksh" => Some(ShellType::Ksh),
        "csh" | "tcsh" => Some(ShellType::Csh),
        _ => None,
    }
}

#[tauri::command]
pub async fn terminal_get_shells(
    state: State<'_, TerminalState>,
) -> Result<Vec<ShellInfo>, String> {
    let api = state.get_or_init_api().await?;
    let shells = api.get_available_shells();

    Ok(shells.into_iter().map(ShellInfo::from).collect())
}

#[tauri::command]
pub async fn terminal_create(
    request: CreateSessionRequest,
    state: State<'_, TerminalState>,
) -> Result<SessionResponse, String> {
    let api = state.get_or_init_api().await?;

    let parsed_shell_type = request.shell_type.and_then(|s| parse_shell_type(&s));
    let core_request = CoreCreateSessionRequest {
        session_id: request.session_id,
        name: request.name,
        shell_type: parsed_shell_type,
        working_directory: request.working_directory,
        env: request.env,
        cols: request.cols,
        rows: request.rows,
    };

    let session = api
        .create_session(core_request)
        .await
        .map_err(|e| format!("Failed to create session: {}", e))?;

    Ok(SessionResponse::from(session))
}

#[tauri::command]
pub async fn terminal_get(
    session_id: String,
    state: State<'_, TerminalState>,
) -> Result<SessionResponse, String> {
    let api = state.get_or_init_api().await?;

    let session = api
        .get_session(&session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))?;

    Ok(SessionResponse::from(session))
}

#[tauri::command]
pub async fn terminal_list(
    state: State<'_, TerminalState>,
) -> Result<Vec<SessionResponse>, String> {
    let api = state.get_or_init_api().await?;

    let sessions = api
        .list_sessions()
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;

    Ok(sessions.into_iter().map(SessionResponse::from).collect())
}

#[tauri::command]
pub async fn terminal_close(
    request: CloseSessionRequest,
    state: State<'_, TerminalState>,
) -> Result<(), String> {
    let api = state.get_or_init_api().await?;

    let core_request = CoreCloseSessionRequest {
        session_id: request.session_id.clone(),
        immediate: request.immediate,
    };

    api.close_session(core_request)
        .await
        .map_err(|e| format!("Failed to close session: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn terminal_write(
    request: WriteRequest,
    state: State<'_, TerminalState>,
) -> Result<(), String> {
    let api = state.get_or_init_api().await?;

    let core_request = CoreWriteRequest {
        session_id: request.session_id,
        data: request.data,
    };

    api.write(core_request)
        .await
        .map_err(|e| format!("Failed to write: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn terminal_resize(
    request: ResizeRequest,
    state: State<'_, TerminalState>,
) -> Result<(), String> {
    let api = state.get_or_init_api().await?;

    let core_request = CoreResizeRequest {
        session_id: request.session_id,
        cols: request.cols,
        rows: request.rows,
    };

    api.resize(core_request)
        .await
        .map_err(|e| format!("Failed to resize: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn terminal_signal(
    request: SignalRequest,
    state: State<'_, TerminalState>,
) -> Result<(), String> {
    let api = state.get_or_init_api().await?;

    let core_request = CoreSignalRequest {
        session_id: request.session_id,
        signal: request.signal,
    };

    api.signal(core_request)
        .await
        .map_err(|e| format!("Failed to send signal: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn terminal_ack(
    request: AcknowledgeRequest,
    state: State<'_, TerminalState>,
) -> Result<(), String> {
    let api = state.get_or_init_api().await?;

    let core_request = CoreAcknowledgeRequest {
        session_id: request.session_id,
        char_count: request.char_count,
    };

    api.acknowledge_data(core_request)
        .await
        .map_err(|e| format!("Failed to acknowledge data: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn terminal_execute(
    request: ExecuteCommandRequest,
    state: State<'_, TerminalState>,
) -> Result<ExecuteCommandResponse, String> {
    let api = state.get_or_init_api().await?;

    let core_request = CoreExecuteCommandRequest {
        session_id: request.session_id,
        command: request.command,
        timeout_ms: request.timeout_ms,
        prevent_history: request.prevent_history,
    };

    let result = api
        .execute_command(core_request)
        .await
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    Ok(ExecuteCommandResponse::from(result))
}

#[tauri::command]
pub async fn terminal_send_command(
    request: SendCommandRequest,
    state: State<'_, TerminalState>,
) -> Result<(), String> {
    let api = state.get_or_init_api().await?;

    let core_request = CoreSendCommandRequest {
        session_id: request.session_id,
        command: request.command,
    };

    api.send_command(core_request)
        .await
        .map_err(|e| format!("Failed to send command: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn terminal_has_shell_integration(
    session_id: String,
    state: State<'_, TerminalState>,
) -> Result<bool, String> {
    let api = state.get_or_init_api().await?;
    Ok(api.has_shell_integration(&session_id).await)
}

#[tauri::command]
pub async fn terminal_shutdown_all(state: State<'_, TerminalState>) -> Result<(), String> {
    let api = state.get_or_init_api().await?;
    api.shutdown_all().await;

    Ok(())
}

#[tauri::command]
pub async fn terminal_get_history(
    session_id: String,
    state: State<'_, TerminalState>,
) -> Result<GetHistoryResponse, String> {
    let api = state.get_or_init_api().await?;

    let core_request = CoreGetHistoryRequest { session_id };

    let response = api
        .get_history(core_request)
        .await
        .map_err(|e| format!("Failed to get history: {}", e))?;

    Ok(GetHistoryResponse {
        session_id: response.session_id,
        data: response.data,
        history_size: response.history_size,
    })
}

pub fn start_terminal_event_loop(terminal_state: TerminalState, app_handle: AppHandle) {
    tokio::spawn(async move {
        let api = match terminal_state.get_or_init_api().await {
            Ok(api) => api,
            Err(e) => {
                error!("Failed to start terminal event loop: {}", e);
                return;
            }
        };

        let mut rx = api.subscribe_events();

        while let Some(event) = rx.recv().await {
            let event_name = "terminal_event";
            if let Err(e) = app_handle.emit(event_name, &event) {
                warn!("Failed to emit terminal event: {}", e);
            }
        }
    });
}
