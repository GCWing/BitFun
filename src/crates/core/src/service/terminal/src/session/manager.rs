//! Session Manager - Manages terminal sessions lifecycle

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use futures::Stream;
use log::warn;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

use crate::config::{ShellConfig, TerminalConfig};
use crate::events::{TerminalEvent, TerminalEventEmitter};
use crate::pty::{ProcessProperty, PtyService, PtyServiceEvent};
use crate::shell::{
    CommandState, ScriptsManager, ShellDetector, ShellIntegration, ShellIntegrationEvent,
    ShellIntegrationManager, ShellType,
};
use crate::{TerminalError, TerminalResult};

use super::{SessionStatus, TerminalSession};

/// Result of executing a command
#[derive(Debug, Clone)]
pub struct CommandExecuteResult {
    /// The command that was executed
    pub command: String,
    /// Unique command ID
    pub command_id: String,
    /// Command output
    pub output: String,
    /// Exit code (if available)
    pub exit_code: Option<i32>,
}

/// Options for command execution
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    /// Timeout for command execution (None = no timeout)
    pub timeout: Option<Duration>,
    /// Whether to prevent the command from being added to shell history
    pub prevent_history: bool,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        Self {
            timeout: None,
            prevent_history: true,
        }
    }
}

/// Events emitted during streaming command execution
#[derive(Debug, Clone)]
pub enum CommandStreamEvent {
    /// Command has started executing
    Started { command_id: String },
    /// Output data received
    Output { data: String },
    /// Command completed successfully
    Completed {
        exit_code: Option<i32>,
        total_output: String,
    },
    /// Command execution failed
    Error { message: String },
}

/// A stream of command execution events
pub type CommandStream = Pin<Box<dyn Stream<Item = CommandStreamEvent> + Send>>;

/// Session manager for terminal sessions
pub struct SessionManager {
    /// Configuration
    config: TerminalConfig,

    /// Active sessions
    sessions: Arc<RwLock<HashMap<String, TerminalSession>>>,

    /// PTY service
    pty_service: Arc<PtyService>,

    /// Event emitter
    event_emitter: Arc<TerminalEventEmitter>,

    /// Mapping from PTY ID to session ID
    pty_to_session: Arc<RwLock<HashMap<u32, String>>>,

    /// Shell integration manager
    integration_manager: Arc<ShellIntegrationManager>,

    /// Per-session shell integration instances
    session_integrations: Arc<RwLock<HashMap<String, ShellIntegration>>>,

    /// Session binding manager for external entity bindings
    binding: Arc<super::TerminalSessionBinding>,

    /// Shell integration scripts manager
    scripts_manager: ScriptsManager,

    /// Per-session output taps for real-time output streaming
    output_taps: Arc<DashMap<String, Vec<mpsc::Sender<String>>>>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(config: TerminalConfig) -> Self {
        // Initialize scripts manager and ensure scripts are up-to-date
        let scripts_manager = ScriptsManager::new(config.shell_integration.scripts_dir.clone());
        if let Err(e) = scripts_manager.ensure_scripts() {
            warn!("Failed to ensure shell integration scripts: {}", e);
        }

        let pty_service = Arc::new(PtyService::new(config.clone()));
        let event_emitter = Arc::new(TerminalEventEmitter::new(1024));
        let integration_manager = Arc::new(ShellIntegrationManager::new());
        let binding = Arc::new(super::TerminalSessionBinding::new());
        let output_taps = Arc::new(DashMap::new());

        let manager = Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            pty_service,
            event_emitter,
            pty_to_session: Arc::new(RwLock::new(HashMap::new())),
            integration_manager,
            session_integrations: Arc::new(RwLock::new(HashMap::new())),
            binding,
            scripts_manager,
            output_taps,
        };

        // Start event forwarding
        manager.start_event_forwarding();

        manager
    }

    /// Get the session binding manager
    ///
    /// Use this to manage bindings between external entities (e.g., chat sessions)
    /// and terminal sessions.
    pub fn binding(&self) -> Arc<super::TerminalSessionBinding> {
        self.binding.clone()
    }

    /// Start forwarding PTY service events to terminal events
    fn start_event_forwarding(&self) {
        let pty_service = self.pty_service.clone();
        let event_emitter = self.event_emitter.clone();
        let sessions = self.sessions.clone();
        let pty_to_session = self.pty_to_session.clone();
        let session_integrations = self.session_integrations.clone();
        let output_taps = self.output_taps.clone();

        tokio::spawn(async move {
            loop {
                if let Some(event) = pty_service.recv_event().await {
                    let pty_id = match &event {
                        PtyServiceEvent::ProcessData { id, .. } => *id,
                        PtyServiceEvent::ProcessReady { id, .. } => *id,
                        PtyServiceEvent::ProcessExit { id, .. } => *id,
                        PtyServiceEvent::ProcessProperty { id, .. } => *id,
                        PtyServiceEvent::ResizeCompleted { id, .. } => *id,
                    };

                    let session_id = {
                        let mapping = pty_to_session.read().await;
                        mapping.get(&pty_id).cloned()
                    };

                    if let Some(session_id) = session_id {
                        let terminal_event = match event {
                            PtyServiceEvent::ProcessData { data, .. } => {
                                // Update last activity and record to history
                                if let Some(session) = sessions.write().await.get_mut(&session_id) {
                                    session.touch();
                                    // Record output to history for frontend recovery
                                    let data_str = String::from_utf8_lossy(&data).to_string();
                                    session.add_output(&data_str);
                                }

                                // Convert to string (lossy for now)
                                let data_str = String::from_utf8_lossy(&data).to_string();

                                // Process through shell integration
                                {
                                    let mut integrations = session_integrations.write().await;
                                    if let Some(integration) = integrations.get_mut(&session_id) {
                                        let si_events = integration.process_data(&data_str);

                                        // Emit shell integration events as terminal events
                                        for si_event in si_events {
                                            match si_event {
                                                ShellIntegrationEvent::CommandStarted {
                                                    command,
                                                    command_id,
                                                } => {
                                                    let _ = event_emitter
                                                        .emit(TerminalEvent::CommandStarted {
                                                            session_id: session_id.clone(),
                                                            command,
                                                            command_id,
                                                        })
                                                        .await;
                                                }
                                                ShellIntegrationEvent::CommandFinished {
                                                    command_id,
                                                    exit_code,
                                                } => {
                                                    let _ = event_emitter
                                                        .emit(TerminalEvent::CommandFinished {
                                                            session_id: session_id.clone(),
                                                            command_id,
                                                            exit_code: exit_code.unwrap_or(0),
                                                        })
                                                        .await;
                                                }
                                                ShellIntegrationEvent::CwdChanged { cwd } => {
                                                    if let Some(session) =
                                                        sessions.write().await.get_mut(&session_id)
                                                    {
                                                        session.update_cwd(cwd.clone());
                                                    }
                                                    let _ = event_emitter
                                                        .emit(TerminalEvent::CwdChanged {
                                                            session_id: session_id.clone(),
                                                            cwd,
                                                        })
                                                        .await;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }

                                // Fan out raw data to output taps (e.g. background session file loggers)
                                if let Some(mut senders) = output_taps.get_mut(&session_id) {
                                    senders.retain(|tx| tx.try_send(data_str.clone()).is_ok());
                                }

                                TerminalEvent::Data {
                                    session_id,
                                    data: data_str,
                                }
                            }
                            PtyServiceEvent::ProcessReady { pid, cwd, .. } => {
                                // Update session
                                if let Some(session) = sessions.write().await.get_mut(&session_id) {
                                    session.pid = Some(pid);
                                    session.cwd = cwd.clone();
                                    session.status = SessionStatus::Active;
                                    session.touch();
                                }

                                TerminalEvent::Ready {
                                    session_id,
                                    pid,
                                    cwd,
                                }
                            }
                            PtyServiceEvent::ProcessExit { exit_code, .. } => {
                                // Update session
                                if let Some(session) = sessions.write().await.get_mut(&session_id) {
                                    session.set_exited(exit_code.map(|c| c as i32));
                                }

                                TerminalEvent::Exit {
                                    session_id,
                                    exit_code: exit_code.map(|c| c as i32),
                                }
                            }
                            PtyServiceEvent::ProcessProperty { property, .. } => match property {
                                ProcessProperty::Title(title) => {
                                    TerminalEvent::TitleChanged { session_id, title }
                                }
                                ProcessProperty::Cwd(cwd) => {
                                    if let Some(session) =
                                        sessions.write().await.get_mut(&session_id)
                                    {
                                        session.update_cwd(cwd.clone());
                                    }
                                    TerminalEvent::CwdChanged { session_id, cwd }
                                }
                                ProcessProperty::ShellType(shell_type) => {
                                    TerminalEvent::ShellTypeChanged {
                                        session_id,
                                        shell_type,
                                    }
                                }
                                _ => continue,
                            },
                            PtyServiceEvent::ResizeCompleted { cols, rows, .. } => {
                                // Update session dimensions
                                if let Some(session) = sessions.write().await.get_mut(&session_id) {
                                    session.cols = cols;
                                    session.rows = rows;
                                }
                                TerminalEvent::Resized {
                                    session_id,
                                    cols,
                                    rows,
                                }
                            }
                        };

                        let _ = event_emitter.emit(terminal_event).await;
                    }
                }
            }
        });
    }

    /// Create a new terminal session with shell integration
    pub async fn create_session(
        &self,
        session_id: Option<String>,
        name: Option<String>,
        shell_type: Option<ShellType>,
        cwd: Option<String>,
        env: Option<HashMap<String, String>>,
        cols: Option<u16>,
        rows: Option<u16>,
    ) -> TerminalResult<TerminalSession> {
        self.create_session_with_options(session_id, name, shell_type, cwd, env, cols, rows, true)
            .await
    }

    /// Create a new terminal session with optional shell integration
    pub async fn create_session_with_options(
        &self,
        session_id: Option<String>,
        name: Option<String>,
        shell_type: Option<ShellType>,
        cwd: Option<String>,
        env: Option<HashMap<String, String>>,
        cols: Option<u16>,
        rows: Option<u16>,
        enable_integration: bool,
    ) -> TerminalResult<TerminalSession> {
        // Use provided session ID or generate a new one
        let session_id = session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Check if session ID already exists
        {
            let sessions = self.sessions.read().await;
            if sessions.contains_key(&session_id) {
                return Err(TerminalError::Session(format!(
                    "Session with ID '{}' already exists",
                    session_id
                )));
            }
        }

        // Determine shell type
        let shell_type = shell_type.unwrap_or_else(|| {
            let detected = ShellDetector::get_default_shell();
            detected.shell_type
        });

        // Determine working directory
        let cwd = cwd.unwrap_or_else(|| {
            self.config.default_cwd.clone().unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| ".".to_string())
            })
        });

        // Generate name
        let name = name.unwrap_or_else(|| format!("Terminal {}", &session_id[..8]));

        // Generate nonce for shell integration
        let nonce = uuid::Uuid::new_v4().to_string();

        // Create shell config
        // On Windows, when shell_type is Bash, we need to use the detected Git Bash path
        // instead of just "bash" which might resolve to WSL bash in System32
        #[cfg(windows)]
        let shell_config_base = if matches!(shell_type, ShellType::Bash) {
            // Try to get Git Bash path from detection
            if let Some(detected) = ShellDetector::detect_git_bash() {
                detected.to_config()
            } else {
                // Fallback to default if Git Bash not found
                ShellConfig {
                    executable: shell_type.default_executable().to_string(),
                    args: Vec::new(),
                    env: HashMap::new(),
                    cwd: None,
                    login: false,
                }
            }
        } else {
            ShellConfig {
                executable: shell_type.default_executable().to_string(),
                args: Vec::new(),
                env: HashMap::new(),
                cwd: None,
                login: false,
            }
        };

        #[cfg(not(windows))]
        let shell_config_base = ShellConfig {
            executable: shell_type.default_executable().to_string(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
            login: false,
        };

        let mut shell_config = ShellConfig {
            executable: shell_config_base.executable,
            args: shell_config_base.args,
            env: self.config.env.clone(),
            cwd: Some(cwd.clone()),
            login: shell_config_base.login,
        };

        // Add custom environment
        if let Some(custom_env) = env {
            shell_config.env.extend(custom_env);
        }

        // Inject shell integration if enabled and supported
        if enable_integration && shell_type.supports_integration() {
            self.inject_shell_integration(&mut shell_config, &shell_type, &nonce);
        }

        // Use provided dimensions or fall back to config defaults
        let cols = cols.unwrap_or(self.config.default_cols);
        let rows = rows.unwrap_or(self.config.default_rows);

        // Create the session record
        let session = TerminalSession::new(
            session_id.clone(),
            name,
            shell_type.clone(),
            cwd,
            cols,
            rows,
        );

        // Store the session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session.clone());
        }

        // Create shell integration instance
        if enable_integration && shell_type.supports_integration() {
            let mut integration = ShellIntegration::new();
            integration.set_nonce(nonce.clone());

            let mut integrations = self.session_integrations.write().await;
            integrations.insert(session_id.clone(), integration);

            self.integration_manager
                .register_session(&session_id, Some(nonce))
                .await;
        }

        // Create the PTY process
        let pty_id = self
            .pty_service
            .create_process(shell_config, shell_type, cols, rows)
            .await?;

        // Update session with PTY ID
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.pty_id = Some(pty_id);
            }
        }

        // Store PTY to session mapping
        {
            let mut mapping = self.pty_to_session.write().await;
            mapping.insert(pty_id, session_id.clone());
        }

        // Emit creation event
        let _ = self
            .event_emitter
            .emit(TerminalEvent::SessionCreated {
                session_id: session_id.clone(),
                pid: None,
                cwd: session.cwd.clone(),
            })
            .await;

        // Return the session
        let sessions = self.sessions.read().await;
        sessions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| TerminalError::Session("Session was removed".to_string()))
    }

    /// Inject shell integration scripts and environment variables
    fn inject_shell_integration(
        &self,
        shell_config: &mut ShellConfig,
        shell_type: &ShellType,
        nonce: &str,
    ) {
        // Set environment variables for shell integration
        // NOTE: Do NOT set TERMINAL_SHELL_INTEGRATION here! The script checks this
        // variable and returns early if it's set. The script sets it itself.
        shell_config
            .env
            .insert("TERM_PROGRAM".to_string(), "terminal".to_string());
        shell_config
            .env
            .insert("TERMINAL_INJECTION".to_string(), "1".to_string());
        shell_config
            .env
            .insert("TERMINAL_NONCE".to_string(), nonce.to_string());

        // Get the script path from scripts manager
        let script_path = match self.scripts_manager.get_script_path(shell_type) {
            Some(p) => p,
            None => return,
        };

        match shell_type {
            ShellType::Bash => {
                // Check if original args had --login
                let had_login = shell_config
                    .args
                    .iter()
                    .any(|arg| arg == "--login" || arg == "-l");
                if had_login {
                    // Set env var for login shell handling (script will source profiles)
                    shell_config
                        .env
                        .insert("TERMINAL_SHELL_LOGIN".to_string(), "1".to_string());
                }
                // Clear all args and use --init-file with -i (interactive mode)
                // --init-file only works for interactive shells, so -i is required!
                shell_config.args.clear();
                shell_config.args.push("--init-file".to_string());
                // Convert path: use forward slashes but keep Windows format (C:/...)
                let path_str = script_path.to_string_lossy().to_string();
                #[cfg(windows)]
                let path_str = path_str.replace('\\', "/");
                shell_config.args.push(path_str);
                // IMPORTANT: Add -i to ensure bash runs in interactive mode
                // Without -i, --init-file won't be executed!
                shell_config.args.push("-i".to_string());
            }
            ShellType::Zsh => {
                // script_path is the ZDOTDIR (directory containing .zshrc)
                // Store original ZDOTDIR
                if let Ok(home) = std::env::var("HOME") {
                    shell_config.env.insert("USER_ZDOTDIR".to_string(), home);
                }
                shell_config.env.insert(
                    "ZDOTDIR".to_string(),
                    script_path.to_string_lossy().to_string(),
                );
            }
            ShellType::Fish => {
                // For fish, use source command to load the script file
                shell_config.args.push("--init-command".to_string());
                shell_config
                    .args
                    .push(format!("source '{}'", script_path.display()));
            }
            ShellType::PowerShell | ShellType::PowerShellCore => {
                // For PowerShell, use -ExecutionPolicy Bypass to avoid security errors
                // and -NoExit to keep the shell running after script execution
                shell_config.args.push("-ExecutionPolicy".to_string());
                shell_config.args.push("Bypass".to_string());
                shell_config.args.push("-NoLogo".to_string());
                shell_config.args.push("-NoExit".to_string());
                shell_config.args.push("-File".to_string());
                shell_config
                    .args
                    .push(script_path.to_string_lossy().to_string());
            }
            _ => {}
        }
    }

    /// Wait for a session to be ready for command execution
    ///
    /// This ensures both the session is active and shell integration is initialized.
    /// For new sessions, it waits for the shell integration to transition from Idle
    /// to Prompt/Input state, indicating the shell is ready to accept commands.
    #[allow(dead_code)]
    async fn wait_for_session_ready(&self, session_id: &str) -> TerminalResult<()> {
        Self::wait_for_session_ready_static(&self.sessions, &self.session_integrations, session_id)
            .await
    }

    /// Static version of wait_for_session_ready that takes explicit parameters
    async fn wait_for_session_ready_static(
        sessions: &Arc<RwLock<HashMap<String, TerminalSession>>>,
        session_integrations: &Arc<RwLock<HashMap<String, ShellIntegration>>>,
        session_id: &str,
    ) -> TerminalResult<()> {
        let ready_timeout = Duration::from_secs(30);
        let ready_start = std::time::Instant::now();
        let mut initial_integration_state = None;

        while ready_start.elapsed() < ready_timeout {
            // Check session status
            let session_status = {
                let sessions_guard = sessions.read().await;
                sessions_guard.get(session_id).map(|s| s.status.clone())
            };

            // Check shell integration state
            let integration_state = {
                let integrations = session_integrations.read().await;
                integrations.get(session_id).map(|i| i.state().clone())
            };

            // Remember the initial integration state
            if initial_integration_state.is_none() {
                initial_integration_state = integration_state.clone();
            }

            match (session_status, integration_state) {
                // Session must be Active
                (Some(SessionStatus::Active), Some(int_state)) => {
                    // For NEW sessions: wait for state to transition from Idle to Prompt/Input
                    // This indicates shell integration has loaded
                    if initial_integration_state == Some(CommandState::Idle) {
                        // This is a newly created session, wait for prompt
                        match int_state {
                            CommandState::Prompt | CommandState::Input => {
                                return Ok(());
                            }
                            CommandState::Idle => {
                                // Still at initial Idle, wait for transition to Prompt
                                // But don't wait forever - use a shorter timeout for new sessions
                                if ready_start.elapsed() >= ready_timeout {
                                    // Give up after ready_timeout and try anyway
                                    return Ok(());
                                }
                                // Wait before next check to avoid busy loop
                                tokio::time::sleep(Duration::from_millis(500)).await;
                            }
                            _ => {
                                // Executing or Finished - shell is working, can send command
                                return Ok(());
                            }
                        }
                    } else {
                        // Not a new session (initial state was not Idle), can proceed immediately
                        return Ok(());
                    }
                }
                (Some(SessionStatus::Terminating), _) | (Some(SessionStatus::Exited { .. }), _) => {
                    return Err(TerminalError::Session(format!(
                        "Session {} is terminated",
                        session_id
                    )));
                }
                _ => {
                    // Still starting, wait
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        Err(TerminalError::Session(format!(
            "Session {} did not become ready in {:?}. \
            Shell integration may have failed. This can happen if your shell config \
            (~/.bashrc, ~/.bash_profile, etc.) contains 'exec', 'exit', or 'return' statements \
            that interrupt the shell integration script. Please check your shell configuration.",
            session_id, ready_timeout
        )))
    }

    /// Execute a command in a session and wait for completion
    ///
    /// This function sends a command to the terminal, waits for it to complete
    /// using shell integration, and returns the output and exit code.
    pub async fn execute_command(
        &self,
        session_id: &str,
        command: &str,
    ) -> TerminalResult<CommandExecuteResult> {
        self.execute_command_with_options(session_id, command, ExecuteOptions::default())
            .await
    }

    /// Execute a command with custom options
    pub async fn execute_command_with_options(
        &self,
        session_id: &str,
        command: &str,
        options: ExecuteOptions,
    ) -> TerminalResult<CommandExecuteResult> {
        // Check if session exists
        let _session = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_id)
                .cloned()
                .ok_or_else(|| TerminalError::SessionNotFound(session_id.to_string()))?
        };

        // Check if shell integration is available
        let has_integration = {
            let integrations = self.session_integrations.read().await;
            integrations.contains_key(session_id)
        };

        if !has_integration {
            return Err(TerminalError::Session(
                "Shell integration is not enabled for this session".to_string(),
            ));
        }

        // Wait for session to be ready before executing command
        Self::wait_for_session_ready_static(&self.sessions, &self.session_integrations, session_id)
            .await?;

        // Generate command ID
        let command_id = uuid::Uuid::new_v4().to_string();

        // Clear any previous output
        {
            let mut integrations = self.session_integrations.write().await;
            if let Some(integration) = integrations.get_mut(session_id) {
                integration.clear_output();
            }
        }

        // Prepare the command (optionally with leading space to prevent history)
        let cmd_to_send = if options.prevent_history {
            format!(" {}\r", command) // Leading space prevents bash history
        } else {
            format!("{}\r", command)
        };

        // Send the command
        self.write(session_id, cmd_to_send.as_bytes()).await?;

        // Wait for command completion (with optional timeout)
        match options.timeout {
            Some(timeout_duration) => {
                let result = timeout(
                    timeout_duration,
                    self.wait_for_command_completion(session_id),
                )
                .await;

                match result {
                    Ok(Ok((output, exit_code))) => Ok(CommandExecuteResult {
                        command: command.to_string(),
                        command_id,
                        output,
                        exit_code,
                    }),
                    Ok(Err(e)) => Err(e),
                    Err(_) => {
                        // Timeout - get whatever output we have
                        let output = {
                            let integrations = self.session_integrations.read().await;
                            integrations
                                .get(session_id)
                                .map(|i| i.get_output().to_string())
                                .unwrap_or_default()
                        };

                        Err(TerminalError::Timeout(format!(
                            "Command timed out after {:?}. Partial output: {}",
                            timeout_duration,
                            if output.len() > 200 {
                                &output[..200]
                            } else {
                                &output
                            }
                        )))
                    }
                }
            }
            None => {
                // No timeout - wait indefinitely
                let (output, exit_code) = self.wait_for_command_completion(session_id).await?;
                Ok(CommandExecuteResult {
                    command: command.to_string(),
                    command_id,
                    output,
                    exit_code,
                })
            }
        }
    }

    /// Wait for command completion using shell integration
    async fn wait_for_command_completion(
        &self,
        session_id: &str,
    ) -> TerminalResult<(String, Option<i32>)> {
        let poll_interval = Duration::from_millis(50);
        let max_idle_checks = 20; // After 1 second of idle, check for prompt
        let mut idle_count = 0;
        let mut last_output_len = 0;
        let mut finished_exit_code: Option<Option<i32>> = None;
        let mut post_finish_idle_count = 0;
        // Wait for output to stabilize after CommandFinished
        let post_finish_idle_required = 4; // 200ms of idle after finish

        loop {
            tokio::time::sleep(poll_interval).await;

            // Check current state
            let (state, output, output_len, cmd_finished, last_exit) = {
                let integrations = self.session_integrations.read().await;
                if let Some(integration) = integrations.get(session_id) {
                    let output = integration.get_output().to_string();
                    let len = output.len();
                    let cmd_finished = integration.command_just_finished();
                    let last_exit = integration.last_exit_code();
                    (
                        integration.state().clone(),
                        output,
                        len,
                        cmd_finished,
                        last_exit,
                    )
                } else {
                    return Err(TerminalError::Session("Integration not found".to_string()));
                }
            };

            // If command just finished, record it even if state already changed
            if cmd_finished && finished_exit_code.is_none() {
                finished_exit_code = Some(last_exit);
                post_finish_idle_count = 0;
                last_output_len = output_len;
                // Clear the flag
                let mut integrations = self.session_integrations.write().await;
                if let Some(integration) = integrations.get_mut(session_id) {
                    integration.clear_command_finished();
                }
            }

            // Check if command finished
            match state {
                CommandState::Finished { exit_code } => {
                    // First time seeing Finished state - record it
                    if finished_exit_code.is_none() {
                        finished_exit_code = Some(exit_code);
                        post_finish_idle_count = 0;
                        last_output_len = output_len;
                    } else {
                        // Already in finished state - wait for output to stabilize
                        if output_len == last_output_len {
                            post_finish_idle_count += 1;
                            if post_finish_idle_count >= post_finish_idle_required {
                                // Output has been stable, return result
                                return Ok((output, finished_exit_code.flatten()));
                            }
                        } else {
                            // New output arrived, reset counter
                            post_finish_idle_count = 0;
                            last_output_len = output_len;
                        }
                    }
                }
                CommandState::Idle | CommandState::Prompt | CommandState::Input => {
                    // If we previously detected Finished, wait for output to stabilize then return
                    if finished_exit_code.is_some() {
                        if output_len == last_output_len {
                            post_finish_idle_count += 1;
                            if post_finish_idle_count >= post_finish_idle_required {
                                return Ok((output, finished_exit_code.flatten()));
                            }
                        } else {
                            post_finish_idle_count = 0;
                            last_output_len = output_len;
                        }
                    } else {
                        // Command might have completed without proper shell integration sequence
                        // Use idle detection as fallback
                        if output_len == last_output_len {
                            idle_count += 1;
                            if idle_count >= max_idle_checks {
                                // Assume command completed
                                return Ok((output, None));
                            }
                        } else {
                            idle_count = 0;
                            last_output_len = output_len;
                        }
                    }
                }
                CommandState::Executing => {
                    // Still executing, reset idle count
                    idle_count = 0;
                    finished_exit_code = None;
                    last_output_len = output_len;
                }
            }
        }
    }

    /// Execute a command and return a stream of events
    ///
    /// This function provides real-time streaming of command output,
    /// allowing callers to process output as it arrives.
    pub fn execute_command_stream(&self, session_id: String, command: String) -> CommandStream {
        self.execute_command_stream_with_options(session_id, command, ExecuteOptions::default())
    }

    /// Execute a command with options and return a stream of events
    pub fn execute_command_stream_with_options(
        &self,
        session_id: String,
        command: String,
        options: ExecuteOptions,
    ) -> CommandStream {
        let sessions = self.sessions.clone();
        let session_integrations = self.session_integrations.clone();
        let pty_service = self.pty_service.clone();
        let timeout_duration = options.timeout; // None means no timeout
        let prevent_history = options.prevent_history;

        let (tx, rx) = mpsc::channel::<CommandStreamEvent>(256);

        // Spawn the execution task
        tokio::spawn(async move {
            // Helper to send events
            let send = |event: CommandStreamEvent| {
                let tx = tx.clone();
                async move {
                    let _ = tx.send(event).await;
                }
            };

            // Wait for session to be ready before executing command
            if let Err(e) =
                Self::wait_for_session_ready_static(&sessions, &session_integrations, &session_id)
                    .await
            {
                send(CommandStreamEvent::Error {
                    message: format!("Session not ready: {}", e),
                })
                .await;
                return;
            }

            // Check if session exists
            let pty_id = {
                let sessions_guard = sessions.read().await;
                match sessions_guard.get(&session_id) {
                    Some(session) => session.pty_id,
                    None => {
                        send(CommandStreamEvent::Error {
                            message: format!("Session not found: {}", session_id),
                        })
                        .await;
                        return;
                    }
                }
            };

            let pty_id = match pty_id {
                Some(id) => id,
                None => {
                    send(CommandStreamEvent::Error {
                        message: "Session has no PTY".to_string(),
                    })
                    .await;
                    return;
                }
            };

            // Generate command ID
            let command_id = uuid::Uuid::new_v4().to_string();

            // Clear any previous output
            {
                let mut integrations = session_integrations.write().await;
                if let Some(integration) = integrations.get_mut(&session_id) {
                    integration.clear_output();
                }
            }

            // Send started event
            send(CommandStreamEvent::Started {
                command_id: command_id.clone(),
            })
            .await;

            // Prepare the command
            let cmd_to_send = if prevent_history {
                format!(" {}\r", command)
            } else {
                format!("{}\r", command)
            };

            // Send the command
            if let Err(e) = pty_service.write(pty_id, cmd_to_send.as_bytes()).await {
                send(CommandStreamEvent::Error {
                    message: format!("Failed to send command: {}", e),
                })
                .await;
                return;
            }

            // Poll for output and completion
            let poll_interval = Duration::from_millis(50);
            let max_idle_checks = 20;
            let mut idle_count = 0;
            let mut last_output_len = 0;
            let mut last_sent_len = 0;
            let start_time = std::time::Instant::now();
            let mut finished_exit_code: Option<Option<i32>> = None;
            let mut post_finish_idle_count = 0;
            let post_finish_idle_required = 4; // 200ms of idle after finish

            loop {
                // Check timeout (only if timeout is configured)
                if let Some(timeout_dur) = timeout_duration {
                    if start_time.elapsed() > timeout_dur {
                        let output = {
                            let integrations = session_integrations.read().await;
                            integrations
                                .get(&session_id)
                                .map(|i| i.get_output().to_string())
                                .unwrap_or_default()
                        };
                        send(CommandStreamEvent::Error {
                            message: format!("Command timed out after {:?}", timeout_dur),
                        })
                        .await;
                        send(CommandStreamEvent::Completed {
                            exit_code: None,
                            total_output: output,
                        })
                        .await;
                        return;
                    }
                }

                tokio::time::sleep(poll_interval).await;

                // Get current state, output, and command finished flag
                let (state, output, cmd_finished, last_exit) = {
                    let integrations = session_integrations.read().await;
                    if let Some(integration) = integrations.get(&session_id) {
                        let output = integration.get_output().to_string();
                        let cmd_finished = integration.command_just_finished();
                        let last_exit = integration.last_exit_code();
                        (integration.state().clone(), output, cmd_finished, last_exit)
                    } else {
                        send(CommandStreamEvent::Error {
                            message: "Integration not found".to_string(),
                        })
                        .await;
                        return;
                    }
                };

                // If command just finished, record it even if state already changed
                if cmd_finished && finished_exit_code.is_none() {
                    finished_exit_code = Some(last_exit);
                    post_finish_idle_count = 0;
                    last_output_len = output.len();
                    // Clear the flag
                    let mut integrations = session_integrations.write().await;
                    if let Some(integration) = integrations.get_mut(&session_id) {
                        integration.clear_command_finished();
                    }
                }

                let output_len = output.len();

                // Send any new output
                if output_len > last_sent_len {
                    let new_data = output[last_sent_len..].to_string();
                    send(CommandStreamEvent::Output { data: new_data }).await;
                    last_sent_len = output_len;
                }

                // Check if command finished
                match state {
                    CommandState::Finished { exit_code } => {
                        // First time seeing Finished state - record it
                        if finished_exit_code.is_none() {
                            finished_exit_code = Some(exit_code);
                            post_finish_idle_count = 0;
                            last_output_len = output_len;
                        } else {
                            // Wait for output to stabilize after finish
                            if output_len == last_output_len {
                                post_finish_idle_count += 1;
                                if post_finish_idle_count >= post_finish_idle_required {
                                    send(CommandStreamEvent::Completed {
                                        exit_code: finished_exit_code.flatten(),
                                        total_output: output,
                                    })
                                    .await;
                                    return;
                                }
                            } else {
                                post_finish_idle_count = 0;
                                last_output_len = output_len;
                            }
                        }
                    }
                    CommandState::Idle | CommandState::Prompt | CommandState::Input => {
                        // If we previously saw Finished and now see Prompt, we're done
                        // But wait for output to stabilize first (fix for intermittent output loss)
                        if finished_exit_code.is_some() {
                            if output_len == last_output_len {
                                post_finish_idle_count += 1;
                                // Wait at least 10 poll cycles (500ms) after seeing Prompt to ensure all output arrived
                                if post_finish_idle_count >= 10 {
                                    send(CommandStreamEvent::Completed {
                                        exit_code: finished_exit_code.flatten(),
                                        total_output: output,
                                    })
                                    .await;
                                    return;
                                }
                            } else {
                                // New output arrived, reset counter
                                post_finish_idle_count = 0;
                                last_output_len = output_len;
                            }
                        } else {
                            // No finished_exit_code yet, use idle detection as fallback
                            if output_len == last_output_len {
                                idle_count += 1;
                                if idle_count >= max_idle_checks {
                                    send(CommandStreamEvent::Completed {
                                        exit_code: None,
                                        total_output: output,
                                    })
                                    .await;
                                    return;
                                }
                            } else {
                                idle_count = 0;
                                last_output_len = output_len;
                            }
                        }
                    }

                    CommandState::Executing => {
                        idle_count = 0;
                        finished_exit_code = None;
                        last_output_len = output_len;
                    }
                }
            }
        });

        // Convert receiver to stream
        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    /// Send a command to a session without waiting for completion
    ///
    /// This function waits for the session to be active, then sends a command
    /// to the terminal. Unlike `execute_command`, it does NOT require shell
    /// integration and does NOT wait for command completion or capture output.
    ///
    /// This is useful for:
    /// - Shells that don't support shell integration (e.g., cmd)
    /// - Startup commands where you don't need the result
    /// - Fire-and-forget command execution
    pub async fn send_command(&self, session_id: &str, command: &str) -> TerminalResult<()> {
        // Wait for session to be active
        self.wait_for_session_active(session_id).await?;

        // Format the command with carriage return
        let cmd_to_send = format!("{}\r", command);

        // Send the command
        self.write(session_id, cmd_to_send.as_bytes()).await
    }

    /// Wait for a session to become active (simpler than wait_for_session_ready)
    ///
    /// This only checks that the session exists and is in Active status.
    /// It does NOT require shell integration.
    async fn wait_for_session_active(&self, session_id: &str) -> TerminalResult<()> {
        let ready_timeout = Duration::from_secs(30);
        let ready_start = std::time::Instant::now();

        while ready_start.elapsed() < ready_timeout {
            let session_status = {
                let sessions = self.sessions.read().await;
                sessions.get(session_id).map(|s| s.status.clone())
            };

            match session_status {
                Some(SessionStatus::Active) => {
                    return Ok(());
                }
                Some(SessionStatus::Terminating) | Some(SessionStatus::Exited { .. }) => {
                    return Err(TerminalError::Session(format!(
                        "Session {} is terminated",
                        session_id
                    )));
                }
                Some(SessionStatus::Starting)
                | Some(SessionStatus::Orphaned)
                | Some(SessionStatus::Restoring)
                | None => {
                    // Still starting, restoring, or not found yet, wait
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Err(TerminalError::Session(format!(
            "Session {} did not become active in {:?}",
            session_id, ready_timeout
        )))
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<TerminalSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<TerminalSession> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// Write data to a session
    pub async fn write(&self, session_id: &str, data: &[u8]) -> TerminalResult<()> {
        let pty_id = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_id)
                .and_then(|s| s.pty_id)
                .ok_or_else(|| TerminalError::SessionNotFound(session_id.to_string()))?
        };

        self.pty_service.write(pty_id, data).await
    }

    /// Resize a session
    ///
    /// This method:
    /// 1. Updates session dimensions
    /// 2. Flushes any buffered data in PTY service
    /// 3. Resizes the PTY
    /// 4. Emits a Resized event for frontend confirmation
    pub async fn resize(&self, session_id: &str, cols: u16, rows: u16) -> TerminalResult<()> {
        // Update session dimensions
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.resize(cols, rows);
            }
        }

        let pty_id = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_id)
                .and_then(|s| s.pty_id)
                .ok_or_else(|| TerminalError::SessionNotFound(session_id.to_string()))?
        };

        // Resize PTY (this also flushes buffered data)
        // Do not send Resized event here because Windows ConPTY has a delay
        // PTY sends ResizeCompleted event after resize is completed,
        // This event is forwarded to TerminalEvent::Resized in start_event_forwarding()
        self.pty_service.resize(pty_id, cols, rows).await?;

        Ok(())
    }

    /// Send a signal to a session
    pub async fn signal(&self, session_id: &str, signal: &str) -> TerminalResult<()> {
        let pty_id = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_id)
                .and_then(|s| s.pty_id)
                .ok_or_else(|| TerminalError::SessionNotFound(session_id.to_string()))?
        };

        self.pty_service.signal(pty_id, signal).await
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str, immediate: bool) -> TerminalResult<()> {
        let pty_id = {
            let mut sessions = self.sessions.write().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| TerminalError::SessionNotFound(session_id.to_string()))?;

            session.status = SessionStatus::Terminating;
            session.pty_id
        };

        // Shutdown PTY if exists
        if let Some(pty_id) = pty_id {
            // Remove mapping
            {
                let mut mapping = self.pty_to_session.write().await;
                mapping.remove(&pty_id);
            }

            self.pty_service.shutdown(pty_id, immediate).await?;
        }

        // Remove shell integration
        {
            let mut integrations = self.session_integrations.write().await;
            integrations.remove(session_id);
        }
        self.integration_manager
            .unregister_session(session_id)
            .await;

        // Drop output taps so file-writing tasks can detect session end
        self.output_taps.remove(session_id);

        // Remove session
        {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id);
        }

        // Remove any binding pointing to this session so the next get_or_create
        // creates a fresh session rather than returning a stale ID.
        // For primary sessions owner_id == session_id, so unbind(session_id) is sufficient.
        self.binding.unbind(session_id);

        // Emit session destroyed event for frontend
        let _ = self
            .event_emitter
            .emit(TerminalEvent::SessionDestroyed {
                session_id: session_id.to_string(),
            })
            .await;

        Ok(())
    }

    /// Acknowledge data received by frontend
    pub async fn acknowledge_data(
        &self,
        session_id: &str,
        char_count: usize,
    ) -> TerminalResult<()> {
        let pty_id = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_id)
                .and_then(|s| s.pty_id)
                .ok_or_else(|| TerminalError::SessionNotFound(session_id.to_string()))?
        };

        self.pty_service.acknowledge_data(pty_id, char_count).await
    }

    /// Get the event emitter for subscribing to events
    pub fn event_emitter(&self) -> Arc<TerminalEventEmitter> {
        self.event_emitter.clone()
    }

    /// Get the shell integration manager
    pub fn integration_manager(&self) -> Arc<ShellIntegrationManager> {
        self.integration_manager.clone()
    }

    /// Check if a session has shell integration enabled
    pub async fn has_shell_integration(&self, session_id: &str) -> bool {
        let integrations = self.session_integrations.read().await;
        integrations.contains_key(session_id)
    }

    /// Get the current command state for a session
    pub async fn get_command_state(&self, session_id: &str) -> Option<CommandState> {
        let integrations = self.session_integrations.read().await;
        integrations.get(session_id).map(|i| i.state().clone())
    }

    /// Shutdown all sessions
    pub async fn shutdown_all(&self) {
        let session_ids: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };

        for session_id in session_ids {
            if let Err(e) = self.close_session(&session_id, true).await {
                warn!("Failed to close session {}: {}", session_id, e);
            }
        }

        self.pty_service.shutdown_all().await;
    }

    /// Subscribe to the raw PTY output of a specific session.
    ///
    /// Returns a receiver that yields raw output strings as they arrive from the PTY.
    /// The receiver will return `None` (channel closed) when the session is destroyed.
    /// Multiple subscriptions to the same session are supported.
    pub fn subscribe_session_output(&self, session_id: &str) -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::channel(256);
        self.output_taps
            .entry(session_id.to_string())
            .or_default()
            .push(tx);
        rx
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {}
}
