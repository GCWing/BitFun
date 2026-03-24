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
    CommandCompletionReason as CoreCommandCompletionReason,
    CreateSessionRequest as CoreCreateSessionRequest,
    ExecuteCommandRequest as CoreExecuteCommandRequest,
    ExecuteCommandResponse as CoreExecuteCommandResponse,
    GetHistoryRequest as CoreGetHistoryRequest, GetHistoryResponse as CoreGetHistoryResponse,
    ResizeRequest as CoreResizeRequest, SendCommandRequest as CoreSendCommandRequest,
    SessionResponse as CoreSessionResponse, SessionSource as CoreSessionSource,
    ShellInfo as CoreShellInfo, ShellType,
    SignalRequest as CoreSignalRequest, TerminalApi, TerminalConfig,
    WriteRequest as CoreWriteRequest,
};
use bitfun_core::service::terminal::TerminalEvent;
use bitfun_core::service::remote_ssh::workspace_state::get_remote_workspace_manager;

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
    pub source: Option<String>,
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
    /// For remote terminals: the SSH connection ID that owns this session.
    /// None/null for local terminals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<String>,
    pub source: String,
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
            connection_id: None,
            source: format_session_source(&resp.source),
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
    pub completion_reason: String,
}

impl From<CoreExecuteCommandResponse> for ExecuteCommandResponse {
    fn from(resp: CoreExecuteCommandResponse) -> Self {
        Self {
            command: resp.command,
            command_id: resp.command_id,
            output: resp.output,
            exit_code: resp.exit_code,
            completion_reason: match resp.completion_reason {
                CoreCommandCompletionReason::Completed => "completed".to_string(),
                CoreCommandCompletionReason::TimedOut => "timedOut".to_string(),
            },
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
    /// PTY column count at the time history was captured.
    pub cols: u16,
    /// PTY row count at the time history was captured.
    pub rows: u16,
}

impl From<CoreGetHistoryResponse> for GetHistoryResponse {
    fn from(resp: CoreGetHistoryResponse) -> Self {
        Self {
            session_id: resp.session_id,
            data: resp.data,
            history_size: resp.history_size,
            cols: resp.cols,
            rows: resp.rows,
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

fn parse_session_source(source: &str) -> Option<CoreSessionSource> {
    match source.to_lowercase().as_str() {
        "manual" => Some(CoreSessionSource::Manual),
        "agent" => Some(CoreSessionSource::Agent),
        _ => None,
    }
}

fn format_session_source(source: &CoreSessionSource) -> String {
    match source {
        CoreSessionSource::Manual => "manual".to_string(),
        CoreSessionSource::Agent => "agent".to_string(),
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

/// Check if the given working directory belongs to any registered remote workspace.
/// Returns (connection_id, remote_cwd) if so.
async fn lookup_remote_for_terminal(working_directory: Option<&str>) -> Option<(String, String)> {
    let wd = working_directory?;
    let manager = get_remote_workspace_manager()?;
    let entry = manager.lookup_connection(wd, None).await?;
    Some((entry.connection_id, wd.to_string()))
}

/// Try to find session in remote terminal manager. Returns true if found.
async fn is_remote_session(session_id: &str) -> bool {
    if let Some(manager) = get_remote_workspace_manager() {
        if let Some(terminal_manager) = manager.get_terminal_manager().await {
            return terminal_manager.get_session(session_id).await.is_some();
        }
    }
    false
}

#[tauri::command]
pub async fn terminal_create(
    _app: AppHandle,
    request: CreateSessionRequest,
    state: State<'_, TerminalState>,
) -> Result<SessionResponse, String> {
    if let Some((connection_id, remote_cwd)) = lookup_remote_for_terminal(request.working_directory.as_deref()).await {
        if let Some(remote_manager) = get_remote_workspace_manager() {
            let terminal_manager = remote_manager
                .get_terminal_manager()
                .await
                .ok_or("Remote terminal manager not available")?;

            let result = terminal_manager
                .create_session(
                    request.session_id,
                    request.name,
                    &connection_id,
                    request.cols.unwrap_or(80),
                    request.rows.unwrap_or(24),
                    Some(remote_cwd.as_str()),
                    request.source.as_deref().and_then(parse_session_source),
                )
                .await
                .map_err(|e| format!("Failed to create remote session: {}", e))?;

            let session = result.session;
            let mut rx = result.output_rx;
            let session_id = session.id.clone();

            let response = SessionResponse {
                id: session.id,
                name: session.name,
                shell_type: "Remote".to_string(),
                cwd: session.cwd.clone(),
                pid: session.pid,
                status: format!("{:?}", session.status),
                cols: session.cols,
                rows: session.rows,
                connection_id: Some(connection_id.clone()),
                source: format_session_source(&session.source),
            };

            let app_handle = _app.clone();
            let sid = session_id.clone();
            tokio::spawn(async move {
                let _ = app_handle.emit(
                    "terminal_event",
                    &TerminalEvent::Ready {
                        session_id: sid.clone(),
                        pid: 0,
                        cwd: String::new(),
                    },
                );

                loop {
                    match rx.recv().await {
                        Ok(data) => {
                            let text = String::from_utf8_lossy(&data).to_string();
                            if let Err(e) = app_handle.emit(
                                "terminal_event",
                                &TerminalEvent::Data {
                                    session_id: sid.clone(),
                                    data: text,
                                },
                            ) {
                                warn!("Failed to emit remote terminal event: {}", e);
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Remote terminal output lagged, skipped {} messages: session_id={}", n, sid);
                            continue;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }

                let _ = app_handle.emit(
                    "terminal_event",
                    &TerminalEvent::Exit {
                        session_id: sid,
                        exit_code: Some(0),
                    },
                );
            });

            return Ok(response);
        }
    }

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
        remote_connection_id: None,
        source: request.source.as_deref().and_then(parse_session_source),
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
    // Try remote first (by session_id lookup, not global flag)
    if let Some(remote_manager) = get_remote_workspace_manager() {
        if let Some(terminal_manager) = remote_manager.get_terminal_manager().await {
            if let Some(session) = terminal_manager.get_session(&session_id).await {
                return Ok(SessionResponse {
                    id: session.id,
                    name: session.name,
                    shell_type: "Remote".to_string(),
                    cwd: session.cwd,
                    pid: session.pid,
                    status: format!("{:?}", session.status),
                    cols: session.cols,
                    rows: session.rows,
                    connection_id: Some(session.connection_id),
                    source: format_session_source(&session.source),
                });
            }
        }
    }

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
    let mut all_sessions: Vec<SessionResponse> = Vec::new();

    // Collect remote sessions
    if let Some(remote_manager) = get_remote_workspace_manager() {
        if let Some(terminal_manager) = remote_manager.get_terminal_manager().await {
            let remote_sessions = terminal_manager.list_sessions().await;
            all_sessions.extend(remote_sessions.into_iter().map(|s| SessionResponse {
                id: s.id,
                name: s.name,
                shell_type: "Remote".to_string(),
                cwd: s.cwd,
                pid: s.pid,
                status: format!("{:?}", s.status),
                cols: s.cols,
                rows: s.rows,
                connection_id: Some(s.connection_id),
                source: format_session_source(&s.source),
            }));
        }
    }

    // Collect local sessions
    let api = state.get_or_init_api().await?;
    let local_sessions = api
        .list_sessions()
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;
    all_sessions.extend(local_sessions.into_iter().map(SessionResponse::from));

    Ok(all_sessions)
}

#[tauri::command]
pub async fn terminal_close(
    request: CloseSessionRequest,
    state: State<'_, TerminalState>,
) -> Result<(), String> {
    if is_remote_session(&request.session_id).await {
        if let Some(remote_manager) = get_remote_workspace_manager() {
            let terminal_manager = remote_manager
                .get_terminal_manager()
                .await
                .ok_or("Remote terminal manager not available")?;

            terminal_manager
                .close_session(&request.session_id)
                .await
                .map_err(|e| format!("Failed to close session: {}", e))?;

            return Ok(());
        }
    }

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
    if is_remote_session(&request.session_id).await {
        if let Some(remote_manager) = get_remote_workspace_manager() {
            let terminal_manager = remote_manager
                .get_terminal_manager()
                .await
                .ok_or("Remote terminal manager not available")?;

            terminal_manager
                .write(&request.session_id, request.data.as_bytes())
                .await
                .map_err(|e| format!("Failed to write: {}", e))?;

            return Ok(());
        }
    }

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
    if is_remote_session(&request.session_id).await {
        if let Some(remote_manager) = get_remote_workspace_manager() {
            let terminal_manager = remote_manager
                .get_terminal_manager()
                .await
                .ok_or("Remote terminal manager not available")?;

            terminal_manager
                .resize(&request.session_id, request.cols, request.rows)
                .await
                .map_err(|e| format!("Failed to resize: {}", e))?;

            return Ok(());
        }
    }

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
    if is_remote_session(&request.session_id).await {
        // Remote terminals don't support signal yet
        return Ok(());
    }

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
    if is_remote_session(&request.session_id).await {
        // Remote terminals don't use flow control ack
        return Ok(());
    }

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
    if is_remote_session(&request.session_id).await {
        if let Some(remote_manager) = get_remote_workspace_manager() {
            let terminal_manager = remote_manager
                .get_terminal_manager()
                .await
                .ok_or("Remote terminal manager not available")?;
            let session = terminal_manager
                .get_session(&request.session_id)
                .await
                .ok_or("Remote session not found")?;
            let ssh_manager = remote_manager
                .get_ssh_manager()
                .await
                .ok_or("SSH manager not available")?;
            let (stdout, stderr, exit_code) = ssh_manager
                .execute_command(&session.connection_id, &request.command)
                .await
                .map_err(|e| format!("Failed to execute remote command: {}", e))?;

            return Ok(ExecuteCommandResponse {
                command: request.command,
                command_id: format!("remote-cmd-{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()),
                output: if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) },
                exit_code: Some(exit_code),
                completion_reason: "completed".to_string(),
            });
        }
    }

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
    if is_remote_session(&request.session_id).await {
        if let Some(remote_manager) = get_remote_workspace_manager() {
            let terminal_manager = remote_manager
                .get_terminal_manager()
                .await
                .ok_or("Remote terminal manager not available")?;

            terminal_manager
                .write(&request.session_id, format!("{}\n", request.command).as_bytes())
                .await
                .map_err(|e| format!("Failed to send command: {}", e))?;

            return Ok(());
        }
    }

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
    if is_remote_session(&session_id).await {
        return Ok(false);
    }

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
    if is_remote_session(&session_id).await {
        if let Some(remote_manager) = get_remote_workspace_manager() {
            if let Some(terminal_manager) = remote_manager.get_terminal_manager().await {
                if let Some(session) = terminal_manager.get_session(&session_id).await {
                    return Ok(GetHistoryResponse {
                        session_id: session.id,
                        data: String::new(),
                        history_size: 0,
                        cols: session.cols,
                        rows: session.rows,
                    });
                }
            }
        }
    }

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
        cols: response.cols,
        rows: response.rows,
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
