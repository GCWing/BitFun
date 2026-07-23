/// BitFun CLI
///
/// Command-line interface version, supports:
/// - Interactive TUI
/// - Single command execution
/// - Batch task processing
mod account;
mod account_sync;
mod acp_cli;
mod actions;
mod agent;
#[allow(dead_code)]
mod chat_state;
mod config;
mod daemon;
mod diagnostics;
mod logging;
mod management;
mod model_selection;
mod modes;
mod peer_host;
mod plugin_diagnostics;
mod product_assembly;
mod prompts;
mod root_handlers;
mod runtime;
mod ui;

use anyhow::{anyhow, Result};
use bitfun_core::service::remote_connect::DeviceIdentity;
use clap::{Parser, Subcommand, ValueEnum};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use config::CliConfig;
use modes::chat::ChatMode;
use modes::exec::{ExecApprovalMode, ExecOutputFormat};

// ======================== Global MCP Service ========================

static MCP_SERVICE: OnceLock<std::sync::Arc<bitfun_core::service::mcp::MCPService>> =
    OnceLock::new();

/// MCP initialization status: 0=not started, 1=in progress, 2=completed, 3=failed
static MCP_INIT_STATUS: OnceLock<AtomicU8> = OnceLock::new();

/// Get the MCP init status atomic
fn get_mcp_init_status() -> &'static AtomicU8 {
    MCP_INIT_STATUS.get_or_init(|| AtomicU8::new(0))
}

/// Get MCP status text for UI display
pub fn get_mcp_status_text() -> String {
    let status = get_mcp_init_status().load(Ordering::Relaxed);
    match status {
        0 => "MCP: Pending".to_string(),
        1 => "MCP: Connecting...".to_string(),
        2 => "MCP: Ready".to_string(),
        3 => "MCP: Failed".to_string(),
        _ => "MCP: Unknown".to_string(),
    }
}

/// Get the global MCP service instance (if initialized)
pub fn get_mcp_service() -> Option<&'static std::sync::Arc<bitfun_core::service::mcp::MCPService>> {
    MCP_SERVICE.get()
}

#[derive(Parser)]
#[command(name = "bitfun")]
#[command(about = "BitFun CLI - AI agent-driven command-line programming assistant", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive chat (TUI)
    Chat {
        /// Agent type
        #[arg(short, long, default_value = "agentic")]
        agent: String,
    },

    /// Execute single command
    Exec {
        /// User message. If omitted, stdin is used when piped.
        message: Option<String>,

        /// Agent type
        #[arg(short, long, default_value = "agentic")]
        agent: String,

        /// Continue the most recent session in the current workspace
        #[arg(short = 'c', long = "continue")]
        continue_last: bool,

        /// Resume a session by ID, or use "last" for the most recent session
        #[arg(short = 'r', long)]
        resume: Option<String>,

        /// Alias for --resume, compatible with opencode-style CLIs
        #[arg(short = 's', long)]
        session: Option<String>,

        /// Create a new session with a fixed session ID
        #[arg(long)]
        session_id: Option<String>,

        /// Fork the resumed session before executing the prompt
        #[arg(long = "fork-session")]
        fork_session: bool,

        /// Output format for automation
        #[arg(long, value_enum, default_value_t = ExecOutputFormat::Text)]
        output_format: ExecOutputFormat,

        /// Output git diff patch after execution (for SWE-bench evaluation)
        /// Without path outputs to terminal, with path saves to file
        /// The snapshot is captured before writing an explicit output artifact;
        /// the artifact itself is not included in the captured diff
        /// Example: --output-patch or --output-patch ./result.patch
        #[arg(long, num_args = 0..=1, default_missing_value = "-")]
        output_patch: Option<String>,

        /// Auto-approve tool permissions that are not explicitly denied
        #[arg(long, conflicts_with = "confirm")]
        auto: bool,

        /// Deprecated compatibility flag; confirmations are rejected in non-interactive mode
        #[arg(long, hide = true, conflicts_with = "auto")]
        confirm: bool,
    },

    /// Session management
    Sessions {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Agent management
    Agents,

    /// Model management
    Models {
        #[command(subcommand)]
        action: Option<ModelAction>,
    },

    /// MCP server management
    Mcp {
        #[command(subcommand)]
        action: Option<McpAction>,
    },

    /// Inspect and review BitFun-managed plugin packages
    ///
    /// Package layout: <package-root>/<package-id>/bitfun.plugin.json
    Plugins {
        #[command(subcommand)]
        action: Option<PluginAction>,
    },

    /// Usage reporting
    Usage {
        /// Session ID to inspect; defaults to the most recent session in the current workspace
        session_id: Option<String>,
    },

    /// Diagnostic check
    Doctor,

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Health check
    Health,

    /// Manage the always-on account device host daemon
    ///
    /// The daemon holds the relay device-routing connection in a headless
    /// process so this device stays reachable by account peers whenever the
    /// machine is up, even without an interactive CLI running.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Start or inspect the Agent Client Protocol (ACP) server
    Acp {
        #[command(subcommand)]
        action: Option<AcpAction>,
    },
}

#[derive(Subcommand)]
enum ModelAction {
    /// List configured models
    List,
    /// Set the default model for all modes
    SetDefault {
        /// Model id
        model_id: String,
    },
}

#[derive(Subcommand)]
enum McpAction {
    /// List configured MCP servers
    List,
    /// Show the configured MCP entries without probing readiness
    Doctor,
    /// Enable an MCP server by id
    Enable {
        /// MCP server id
        server_id: String,
    },
    /// Disable an MCP server by id
    Disable {
        /// MCP server id
        server_id: String,
    },
    /// Print the stored MCP JSON config
    Config,
}

#[derive(Subcommand)]
enum PluginAction {
    /// List discovered packages and trust status
    List,
    /// Approve the current manifest and declared files without enabling execution
    ApproveSource { package_id: String },
    /// Deny the current manifest and declared files for this workspace
    Deny { package_id: String },
    /// Revoke the current package approval for this workspace
    Revoke { package_id: String },
    /// Preview or confirm activation of one source-approved package
    Activate {
        package_id: String,
        /// Confirm the exact content hash displayed by the activation preview
        #[arg(long, value_name = "CONTENT_HASH")]
        confirm: Option<String>,
    },
    /// Deactivate one package for this workspace
    Deactivate { package_id: String },
}

#[derive(Subcommand)]
enum AcpAction {
    /// Start the ACP server over stdio
    Serve,
    /// Show ACP server status and capabilities
    Status {
        /// Command name or path to show in generated examples
        #[arg(long, default_value = "bitfun")]
        command: String,
    },
    /// Check local readiness for ACP clients
    Doctor {
        /// Command name or path to show in generated examples
        #[arg(long, default_value = "bitfun")]
        command: String,
    },
    /// Print editor/client integration snippets
    Config {
        /// ACP client/editor to generate config for
        #[arg(long, value_enum, default_value_t = acp_cli::AcpConfigClient::Zed)]
        client: acp_cli::AcpConfigClient,

        /// Command name or path your editor should execute
        #[arg(long, default_value = "bitfun")]
        command: String,
    },
    /// Manage external ACP agents that BitFun can launch
    Clients {
        #[command(subcommand)]
        action: AcpClientsAction,
    },
    /// Run a prompt through an external ACP agent
    Run {
        /// External ACP agent to launch
        #[arg(value_enum)]
        client: acp_cli::ExternalAcpClient,

        /// Prompt to send to the external ACP agent
        prompt: String,

        /// Workspace directory for the external agent
        #[arg(long)]
        workspace: Option<String>,

        /// Timeout in seconds
        #[arg(long, default_value_t = 600)]
        timeout: u64,

        /// Permission handling for ACP tool permission requests
        #[arg(long, value_enum, default_value_t = acp_cli::CliAcpPermissionMode::AllowOnce)]
        permission: acp_cli::CliAcpPermissionMode,
    },
}

#[derive(Subcommand)]
enum AcpClientsAction {
    /// List configured and built-in external ACP agents
    List,
    /// Check whether external ACP agent CLIs and adapters are available
    Doctor,
    /// Enable a built-in external ACP agent
    Enable {
        /// Built-in external ACP agent
        #[arg(value_enum)]
        client: acp_cli::ExternalAcpClient,

        /// Permission handling to store for this client
        #[arg(long, value_enum, default_value_t = acp_cli::CliAcpPermissionMode::Ask)]
        permission: acp_cli::CliAcpPermissionMode,
    },
    /// Disable an external ACP agent by id
    Disable {
        /// External ACP client id, for example opencode
        client_id: String,
    },
    /// Print the stored ACP client JSON
    Config,
}

#[derive(Subcommand)]
enum SessionAction {
    /// List all sessions
    List,
    /// Show session details
    Show {
        /// Session ID (or "last" for the most recent)
        id: String,
    },
    /// Delete session
    Delete {
        /// Session ID
        id: String,
    },
    /// Resume a session in the interactive TUI
    Resume {
        /// Session ID (or "last" for the most recent)
        id: String,
    },
    /// Continue the most recent session in the interactive TUI
    Continue,
    /// Fork a session at the latest persisted turn
    Fork {
        /// Session ID (or "last" for the most recent)
        id: String,
        /// Print only the new session ID
        #[arg(long)]
        id_only: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BootstrapProfile {
    Interactive,
    Execution,
    Management,
}

impl BootstrapProfile {
    const fn starts_peer_host(self) -> bool {
        matches!(self, Self::Interactive)
    }

    const fn starts_mcp(self) -> bool {
        matches!(self, Self::Interactive | Self::Execution)
    }
}

impl SessionAction {
    const fn bootstrap_profile(&self) -> BootstrapProfile {
        match self {
            Self::Resume { .. } | Self::Continue => BootstrapProfile::Interactive,
            Self::List | Self::Show { .. } | Self::Delete { .. } | Self::Fork { .. } => {
                BootstrapProfile::Management
            }
        }
    }
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show configuration
    Show,
    /// Edit configuration
    Edit,
    /// Reset to default configuration
    Reset,
    /// Inspect or change external AI application compatibility
    External {
        #[command(subcommand)]
        action: ExternalConfigAction,
    },
}

#[derive(Subcommand)]
enum ExternalConfigAction {
    /// Show effective global and project compatibility settings
    Status,
    /// Enable or disable external compatibility
    SetEnabled {
        enabled: bool,
        #[arg(long, value_enum, default_value = "project")]
        scope: ExternalPolicyScopeArg,
    },
    /// Select an external ecosystem compatibility mode
    SetMode {
        #[arg(value_enum)]
        mode: ExternalPolicyModeArg,
        /// Ecosystem id; optional when exactly one ecosystem is registered
        #[arg(long)]
        ecosystem: Option<String>,
        #[arg(long, value_enum, default_value = "project")]
        scope: ExternalPolicyScopeArg,
    },
    /// Customize one external ecosystem capability
    SetCapability {
        #[arg(value_enum)]
        capability: ExternalCapabilityArg,
        #[arg(value_enum)]
        access: ExternalAccessArg,
        /// Ecosystem id; optional when exactly one ecosystem is registered
        #[arg(long)]
        ecosystem: Option<String>,
        #[arg(long, value_enum, default_value = "project")]
        scope: ExternalPolicyScopeArg,
    },
    /// Remove this project's overrides and inherit global settings
    ResetProject,
    /// Back up and reset a policy written by an incompatible BitFun version
    ResetIncompatible,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExternalPolicyScopeArg {
    Global,
    Project,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExternalPolicyModeArg {
    Recommended,
    DiscoverOnly,
    Off,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExternalCapabilityArg {
    Command,
    Tool,
    Agent,
    Mcp,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ExternalAccessArg {
    Off,
    Discover,
    Ask,
    Auto,
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Run the daemon in the foreground (used by the service manager)
    Run,
    /// Install and start the auto-start service (systemd user unit / LaunchAgent)
    Install,
    /// Stop and remove the auto-start service
    Uninstall,
    /// Show daemon and auto-start service status
    Status,
}

// ======================== System Initialization ========================

/// Return the current project path. CLI session scope is intentionally cwd-only.
fn setup_workspace() -> Option<String> {
    let workspace_path = std::env::current_dir().ok();
    tracing::info!("Workspace path set: {:?}", workspace_path);
    workspace_path.map(|p| p.to_string_lossy().to_string())
}

fn terminal_scripts_dir() -> std::path::PathBuf {
    CliConfig::config_dir()
        .ok()
        .unwrap_or_else(|| std::env::temp_dir().join("bitfun-cli"))
        .join("temp")
        .join("scripts")
}

async fn initialize_terminal_service() {
    use bitfun_core::infrastructure::try_get_path_manager_arc;
    use bitfun_core::service::runtime::RuntimeManager;
    use bitfun_core::service::terminal::{TerminalApi, TerminalConfig};

    let mut terminal_config = TerminalConfig::default();
    terminal_config.shell_integration.scripts_dir = Some(terminal_scripts_dir());
    match try_get_path_manager_arc() {
        Ok(path_manager) => {
            terminal_config.transcript.root_dir =
                Some(path_manager.user_data_dir().join("terminals"));
        }
        Err(error) => {
            tracing::warn!(
                "Failed to configure terminal transcript storage; recording is disabled: {}",
                error
            );
        }
    }

    if let Ok(runtime_manager) = RuntimeManager::new() {
        let current_path = std::env::var("PATH").ok();
        if let Some(merged_path) = runtime_manager.merged_path_env(current_path.as_deref()) {
            terminal_config
                .env
                .insert("PATH".to_string(), merged_path.clone());
            #[cfg(windows)]
            {
                terminal_config.env.insert("Path".to_string(), merged_path);
            }
        }
    } else {
        tracing::warn!("Failed to initialize runtime manager for terminal PATH");
    }

    let _terminal_api = TerminalApi::new(terminal_config).await;
    tracing::info!("Terminal service initialized");
}

/// Initialize Core owners and assemble one invocation-scoped CLI runtime.
async fn initialize_core_services(
    workspace_root: &std::path::Path,
    approval_policy: runtime::approval::CliApprovalPolicy,
    bootstrap_profile: BootstrapProfile,
) -> Result<std::sync::Arc<runtime::CliRuntimeContext>> {
    use bitfun_core::infrastructure::ai::AIClientFactory;

    bitfun_core::service::config::initialize_global_config()
        .await
        .map_err(|error| anyhow!("Failed to initialize global config service: {error}"))?;
    tracing::info!("Global config service initialized");

    let config_service = bitfun_core::service::config::get_global_config_service()
        .await
        .ok();

    AIClientFactory::initialize_global()
        .await
        .map_err(|error| anyhow!("Failed to initialize global AIClientFactory: {error}"))?;
    tracing::info!("Global AI client factory initialized");

    initialize_terminal_service().await;

    let agentic_system = agent::agentic_system::init_agentic_system()
        .await
        .map_err(|error| anyhow!("Failed to initialize agentic system: {error}"))?;
    tracing::info!("Agentic system initialized");

    let runtime = std::sync::Arc::new(runtime::CliRuntimeContext::build(
        agentic_system,
        workspace_root,
        approval_policy,
    )?);
    debug_assert!(runtime
        .product()
        .service_availability()
        .iter()
        .all(|entry| {
            runtime
                .services()
                .has_capability(entry.requirement().service_capability())
        }));
    tracing::info!(
        "CLI product runtime assembled: profile={}, services={}, harnesses={}, plugin_runtime={:?}",
        runtime.product().plan().profile().id(),
        runtime.product().service_availability().len(),
        runtime.product().harness_provider_ids().len(),
        runtime.product().plugin_runtime(),
    );

    if bootstrap_profile.starts_peer_host() {
        if let Err(e) = peer_host::ensure_peer_host_ready(runtime.as_ref()).await {
            tracing::warn!("Failed to initialize CLI peer host services: {e}");
        } else {
            tracing::info!("CLI peer host services initialized");
        }
    }

    // Initialize MCP service in background (non-blocking)
    if bootstrap_profile.starts_mcp() {
        if let Some(ref cfg_svc) = config_service {
            match bitfun_core::service::mcp::MCPService::new(cfg_svc.clone()) {
                Ok(mcp_service) => {
                    let mcp_service = std::sync::Arc::new(mcp_service);
                    MCP_SERVICE.set(mcp_service.clone()).ok();
                    bitfun_core::service::mcp::set_global_mcp_service(mcp_service.clone());

                    // Mark as in progress
                    get_mcp_init_status().store(1, Ordering::Relaxed);

                    // Background async initialization
                    tokio::spawn(async move {
                        let result = mcp_service.server_manager().initialize_all().await;
                        match result {
                            Ok(_) => {
                                tracing::info!("MCP servers initialized successfully");
                                get_mcp_init_status().store(2, Ordering::Relaxed);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to initialize MCP servers: {}", e);
                                get_mcp_init_status().store(3, Ordering::Relaxed);
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to create MCP service: {}", e);
                    get_mcp_init_status().store(3, Ordering::Relaxed);
                }
            }
        }
    }

    Ok(runtime)
}

/// Shutdown MCP servers gracefully
async fn shutdown_mcp_servers() {
    if let Some(mcp_service) = get_mcp_service() {
        if let Err(e) = mcp_service.server_manager().shutdown().await {
            tracing::warn!("Failed to shutdown MCP servers: {}", e);
        } else {
            tracing::info!("MCP servers shut down successfully");
        }
    }
}

// ======================== Interactive TUI Flow ========================

/// Run the full interactive TUI flow: loading screen → startup page → chat
async fn run_interactive(
    config: CliConfig,
    default_agent: String,
    _workspace_str: String,
) -> Result<()> {
    use ui::startup::{StartupPage, StartupResult};

    // 1. Initialize terminal and show loading screen
    let mut terminal = ui::init_terminal()?;
    ui::render_loading(&mut terminal, "Initializing system, please wait...")?;

    // 2. Set workspace path
    let workspace = setup_workspace();
    let workspace_path = workspace
        .as_deref()
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // 3. Initialize core services
    let runtime = initialize_core_services(
        &workspace_path,
        runtime::approval::CliApprovalPolicy::Ask,
        BootstrapProfile::Interactive,
    )
    .await?;
    // 3.5 Restore persisted account session (if any)
    if let Some(user_id) = account::try_restore_session().await {
        tracing::info!("Restored account session for user {user_id}");
        // Re-establish device routing so the CLI becomes RPC-controllable.
        // The daemon owns device routing when it is running: same-machine
        // processes share one device_id and last AuthConnect wins.
        if daemon::is_daemon_running() {
            tracing::info!(
                "CLI daemon is running; skipping in-process device routing (daemon owns it)"
            );
        } else {
            let device = DeviceIdentity::from_current_machine()
                .map_err(|e| anyhow!("detect device: {e}"))?;
            if let Err(e) = account::restore_device_routing(&device.device_name).await {
                tracing::warn!("Failed to restore device routing: {e}");
            }
        }
    }

    // 3.6 Continuous account settings sync (30s pull + debounced push).
    // Safe to start before login: cycles skip while logged out.
    account_sync::start_settings_sync_loop();

    // 4. Show startup page (with full command support)
    let mut startup_page = StartupPage::new(
        config,
        runtime.agent_runtime().clone(),
        runtime.compatibility().clone(),
        default_agent,
        workspace.clone(),
    );
    let startup_result = startup_page.run(&mut terminal)?;

    if let StartupResult::Exit = startup_result {
        shutdown_mcp_servers().await;
        ui::restore_terminal(terminal)?;
        println!("Goodbye!");
        return Ok(());
    }

    // 5. Parse startup result and enter chat
    let (restore_session_id, initial_prompt) = match &startup_result {
        StartupResult::NewSession { prompt } => (None, prompt.clone()),
        StartupResult::ContinueSession(id) => (Some(id.clone()), None),
        StartupResult::Exit => unreachable!(),
    };

    let agent_type = startup_page.agent_type().to_string();
    // Use the current project workspace selected at process start.
    let workspace = startup_page.workspace();
    let config = startup_page.config().clone();
    let mut chat_mode = ChatMode::new(config, agent_type, workspace, runtime.clone());
    if let Some(session_id) = restore_session_id {
        chat_mode = chat_mode.with_restore_session(session_id);
    }
    if let Some(prompt) = initial_prompt {
        chat_mode = chat_mode.with_initial_prompt(prompt);
    }
    let chat_result = chat_mode.run(Some(terminal));

    // 6. Cleanup, including fatal event-stream exits.
    shutdown_mcp_servers().await;
    let _exit_reason = chat_result?;
    println!("Goodbye!");

    Ok(())
}

// ======================== Main ========================

#[derive(Debug)]
struct ReportedCliError {
    exit_code: i32,
}

impl std::fmt::Display for ReportedCliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CLI error was already reported")
    }
}

impl std::error::Error for ReportedCliError {}

async fn run_cli() -> Result<()> {
    let raw_args = std::env::args_os().collect::<Vec<_>>();
    let cli = match Cli::try_parse_from(&raw_args) {
        Ok(cli) => cli,
        Err(error)
            if exec_requests_json_output(&raw_args)
                && matches!(
                    error.kind(),
                    clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
                ) =>
        {
            error.print()?;
            return Ok(());
        }
        Err(error) if exec_requests_json_output(&raw_args) => {
            let exit_code = error.exit_code();
            let error = anyhow!(error.to_string());
            modes::exec::emit_preflight_json_error(ExecOutputFormat::Json, &error)?;
            return Err(anyhow::Error::new(ReportedCliError { exit_code }));
        }
        Err(error) => error.exit(),
    };

    let is_tui_mode = matches!(cli.command, None | Some(Commands::Chat { .. }));
    let is_exec_mode = matches!(cli.command, Some(Commands::Exec { .. }));
    let is_daemon_run = matches!(
        cli.command,
        Some(Commands::Daemon {
            action: DaemonAction::Run,
        })
    );
    let file_log_level = logging::default_log_level(cli.verbose);
    let stderr_log_level = if cli.verbose {
        tracing::Level::TRACE
    } else {
        tracing::Level::ERROR
    };

    if is_tui_mode || is_exec_mode || is_daemon_run {
        logging::init_file_logging(file_log_level);
    } else {
        tracing_subscriber::fmt()
            .with_max_level(stderr_log_level)
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .with_target(false)
            .init();
    }

    let config = CliConfig::load().unwrap_or_else(|e| {
        if !is_tui_mode {
            eprintln!("Warning: Failed to load config: {}", e);
            eprintln!("Using default configuration");
        }
        CliConfig::default()
    });

    match cli.command {
        Some(Commands::Chat { agent }) => {
            // Interactive mode with startup page, scoped to the current directory.
            run_interactive(config, agent, ".".to_string()).await?;
        }

        Some(Commands::Exec {
            message,
            agent,
            continue_last,
            resume,
            session,
            session_id,
            fork_session,
            output_format,
            output_patch,
            auto,
            confirm,
        }) => {
            let approval_mode = if auto {
                ExecApprovalMode::Auto
            } else {
                if confirm {
                    eprintln!(
                        "Warning: --confirm is deprecated; non-interactive confirmations are rejected by default"
                    );
                }
                ExecApprovalMode::Reject
            };
            root_handlers::handle_exec_command(
                config,
                root_handlers::ExecCommandArgs {
                    message,
                    agent,
                    continue_last,
                    resume,
                    session,
                    session_id,
                    fork_session,
                    output_format,
                    output_patch,
                    approval_mode,
                },
            )
            .await?;
        }

        Some(Commands::Sessions { action }) => {
            if let Some((session_id, runtime)) =
                root_handlers::handle_session_action(action).await?
            {
                run_interactive_with_session(config, session_id, runtime).await?;
            }
        }

        Some(Commands::Agents) => {
            let workspace = std::env::current_dir()?;
            management::print_agents(Some(workspace.as_path())).await?;
        }

        Some(Commands::Models { action }) => match action {
            None | Some(ModelAction::List) => management::print_models().await?,
            Some(ModelAction::SetDefault { model_id }) => {
                management::set_default_model(&model_id).await?;
            }
        },

        Some(Commands::Mcp { action }) => match action {
            None | Some(McpAction::List) => management::print_mcp_servers().await?,
            Some(McpAction::Doctor) => management::print_mcp_config_summary().await?,
            Some(McpAction::Enable { server_id }) => {
                management::set_mcp_server_enabled(&server_id, true).await?;
            }
            Some(McpAction::Disable { server_id }) => {
                management::set_mcp_server_enabled(&server_id, false).await?;
            }
            Some(McpAction::Config) => {
                management::print_mcp_json_config().await?;
            }
        },

        Some(Commands::Plugins { action }) => match action {
            None | Some(PluginAction::List) => management::print_plugins().await?,
            Some(PluginAction::ApproveSource { package_id }) => {
                management::set_plugin_trust(
                    &package_id,
                    bitfun_core::plugin_source::ManagedPluginTrustDecision::ApproveSource,
                )
                .await?;
            }
            Some(PluginAction::Deny { package_id }) => {
                management::set_plugin_trust(
                    &package_id,
                    bitfun_core::plugin_source::ManagedPluginTrustDecision::Denied,
                )
                .await?;
            }
            Some(PluginAction::Revoke { package_id }) => {
                management::set_plugin_trust(
                    &package_id,
                    bitfun_core::plugin_source::ManagedPluginTrustDecision::Revoked,
                )
                .await?;
            }
            Some(PluginAction::Activate {
                package_id,
                confirm,
            }) => {
                management::activate_plugin(&package_id, confirm.as_deref()).await?;
            }
            Some(PluginAction::Deactivate { package_id }) => {
                management::deactivate_plugin(&package_id).await?;
            }
        },

        Some(Commands::Usage { session_id }) => {
            management::print_usage_report(session_id.as_deref()).await?;
        }

        Some(Commands::Doctor) => {
            use std::sync::Arc;

            use runtime::services::{CliClock, CliRuntimeEventSink, CliRuntimeServicesProvider};

            let workspace = std::env::current_dir()?;
            let services = CliRuntimeServicesProvider::new(
                &workspace,
                Arc::new(CliRuntimeEventSink::new(16)),
                Arc::new(CliClock),
            )?
            .build()?;
            let product_runtime = product_assembly::assemble_cli_runtime_parts(services)?;
            if !management::print_doctor(&product_runtime).await? {
                std::process::exit(1);
            }
        }

        Some(Commands::Config { action }) => {
            root_handlers::handle_config_action(action, &config).await?;
        }

        Some(Commands::Health) => {
            root_handlers::handle_health_command()?;
        }

        Some(Commands::Daemon { action }) => match action {
            DaemonAction::Run => daemon::run_daemon().await?,
            DaemonAction::Install => daemon::install_service()?,
            DaemonAction::Uninstall => daemon::uninstall_service()?,
            DaemonAction::Status => daemon::print_status()?,
        },

        Some(Commands::Acp {
            action: None | Some(AcpAction::Serve),
        }) => {
            root_handlers::serve_acp_stdio().await?;
        }

        Some(Commands::Acp {
            action: Some(AcpAction::Status { command }),
        }) => {
            acp_cli::print_status(&command)?;
        }

        Some(Commands::Acp {
            action: Some(AcpAction::Doctor { command }),
        }) => {
            if !acp_cli::print_doctor(&command).await? {
                std::process::exit(1);
            }
        }

        Some(Commands::Acp {
            action: Some(AcpAction::Config { client, command }),
        }) => {
            acp_cli::print_config(client, &command)?;
        }

        Some(Commands::Acp {
            action: Some(AcpAction::Clients { action }),
        }) => match action {
            AcpClientsAction::List => acp_cli::list_external_clients().await?,
            AcpClientsAction::Doctor => {
                if !acp_cli::doctor_external_clients().await? {
                    std::process::exit(1);
                }
            }
            AcpClientsAction::Enable { client, permission } => {
                acp_cli::enable_external_client(client, permission).await?;
            }
            AcpClientsAction::Disable { client_id } => {
                acp_cli::disable_external_client(&client_id).await?;
            }
            AcpClientsAction::Config => acp_cli::print_external_client_config().await?,
        },

        Some(Commands::Acp {
            action:
                Some(AcpAction::Run {
                    client,
                    prompt,
                    workspace,
                    timeout,
                    permission,
                }),
        }) => {
            acp_cli::run_external_client(client, prompt, workspace, timeout, permission).await?;
        }

        None => {
            // Default: interactive TUI with startup page
            let workspace_str = ".".to_string();

            let default_agent = config.behavior.default_agent.clone();
            run_interactive(config, default_agent, workspace_str).await?;
        }
    }

    Ok(())
}

fn exec_requests_json_output(args: &[std::ffi::OsString]) -> bool {
    let values = args
        .iter()
        .skip(1)
        .map(|value| value.to_string_lossy())
        .collect::<Vec<_>>();
    if !values.iter().any(|value| value == "exec") {
        return false;
    }

    values.iter().enumerate().any(|(index, value)| {
        value == "--output-format=json"
            || (value == "--output-format"
                && values.get(index + 1).is_some_and(|format| format == "json"))
    })
}

async fn run_interactive_with_session(
    config: CliConfig,
    session_id: String,
    runtime: std::sync::Arc<runtime::CliRuntimeContext>,
) -> Result<()> {
    let mut terminal = ui::init_terminal()?;
    ui::render_loading(&mut terminal, "Initializing system, please wait...")?;

    let workspace = Some(runtime.workspace_root().to_string_lossy().to_string());
    let sessions = runtime
        .agent_runtime()
        .list_sessions(bitfun_runtime_ports::AgentSessionListRequest {
            workspace_path: runtime.workspace_root().to_string_lossy().to_string(),
            remote_connection_id: None,
            remote_ssh_host: None,
        })
        .await
        .map_err(|error| anyhow::anyhow!(error.into_message()))?;
    let agent_type = sessions
        .iter()
        .find(|session| session.session_id == session_id)
        .map(|session| session.agent_type.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Session {session_id} was not found in the current workspace: {}",
                runtime.workspace_root().display()
            )
        })?;

    let mut chat_mode = ChatMode::new(config, agent_type, workspace, runtime.clone())
        .with_restore_session(session_id);
    let run_result = chat_mode.run(Some(terminal));

    shutdown_mcp_servers().await;
    println!("Goodbye!");

    run_result?;
    Ok(())
}

fn main() {
    // Install rustls CryptoProvider before any TLS-capable work (relay WS,
    // reqwest rustls paths, Feishu wss). Required when both ring and aws-lc-rs
    // are linked: rustls cannot auto-select a provider.
    bitfun_core::service::remote_connect::ensure_rustls_crypto_provider();

    let worker = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime");
            runtime.block_on(run_cli())
        })
        .expect("failed to spawn bitfun worker thread");

    match worker.join() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            if let Some(reported) = err.downcast_ref::<ReportedCliError>() {
                std::process::exit(reported.exit_code);
            }
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("Error: bitfun worker thread panicked");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod plugin_command_tests {
    use super::{Cli, Commands, PluginAction};
    use clap::Parser;

    #[test]
    fn plugin_commands_parse_list_and_source_review_actions() {
        let list = Cli::try_parse_from(["bitfun", "plugins"]).expect("parse plugin list");
        assert!(matches!(
            list.command,
            Some(Commands::Plugins { action: None })
        ));

        let approval = Cli::try_parse_from(["bitfun", "plugins", "approve-source", "acme.demo"])
            .expect("parse plugin source approval");
        assert!(matches!(
            approval.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::ApproveSource { package_id })
            }) if package_id == "acme.demo"
        ));

        let deny = Cli::try_parse_from(["bitfun", "plugins", "deny", "acme.demo"])
            .expect("parse plugin deny");
        assert!(matches!(
            deny.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::Deny { package_id })
            }) if package_id == "acme.demo"
        ));

        let revoke = Cli::try_parse_from(["bitfun", "plugins", "revoke", "acme.demo"])
            .expect("parse plugin revoke");
        assert!(matches!(
            revoke.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::Revoke { package_id })
            }) if package_id == "acme.demo"
        ));

        let preview = Cli::try_parse_from(["bitfun", "plugins", "activate", "acme.demo"])
            .expect("parse plugin activation preview");
        assert!(matches!(
            preview.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::Activate {
                    package_id,
                    confirm: None,
                })
            }) if package_id == "acme.demo"
        ));

        let confirm = Cli::try_parse_from([
            "bitfun",
            "plugins",
            "activate",
            "acme.demo",
            "--confirm",
            "sha256:previewed",
        ])
        .expect("parse confirmed plugin activation");
        assert!(matches!(
            confirm.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::Activate {
                    package_id,
                    confirm: Some(content_hash),
                })
            }) if package_id == "acme.demo" && content_hash == "sha256:previewed"
        ));

        let deactivate = Cli::try_parse_from(["bitfun", "plugins", "deactivate", "acme.demo"])
            .expect("parse plugin deactivation");
        assert!(matches!(
            deactivate.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::Deactivate { package_id })
            }) if package_id == "acme.demo"
        ));
    }
}

#[cfg(test)]
mod external_config_command_tests {
    use super::{
        Cli, Commands, ConfigAction, ExternalAccessArg, ExternalCapabilityArg,
        ExternalConfigAction, ExternalPolicyModeArg, ExternalPolicyScopeArg,
    };
    use clap::Parser;

    #[test]
    fn external_config_commands_keep_scope_and_capability_explicit() {
        let status = Cli::try_parse_from(["bitfun", "config", "external", "status"])
            .expect("parse external status");
        assert!(matches!(
            status.command,
            Some(Commands::Config {
                action: ConfigAction::External {
                    action: ExternalConfigAction::Status
                }
            })
        ));

        let mode = Cli::try_parse_from([
            "bitfun",
            "config",
            "external",
            "set-mode",
            "discover-only",
            "--scope",
            "global",
        ])
        .expect("parse external mode");
        assert!(matches!(
            mode.command,
            Some(Commands::Config {
                action: ConfigAction::External {
                    action: ExternalConfigAction::SetMode {
                        mode: ExternalPolicyModeArg::DiscoverOnly,
                        ecosystem: None,
                        scope: ExternalPolicyScopeArg::Global,
                    }
                }
            })
        ));

        let capability = Cli::try_parse_from([
            "bitfun",
            "config",
            "external",
            "set-capability",
            "mcp",
            "ask",
            "--ecosystem",
            "opencode",
        ])
        .expect("parse external capability");
        assert!(matches!(
            capability.command,
            Some(Commands::Config {
                action: ConfigAction::External {
                    action: ExternalConfigAction::SetCapability {
                        capability: ExternalCapabilityArg::Mcp,
                        access: ExternalAccessArg::Ask,
                        ecosystem: Some(ref ecosystem),
                        scope: ExternalPolicyScopeArg::Project,
                    }
                }
            }) if ecosystem == "opencode"
        ));

        let reset = Cli::try_parse_from(["bitfun", "config", "external", "reset-incompatible"])
            .expect("parse incompatible policy reset");
        assert!(matches!(
            reset.command,
            Some(Commands::Config {
                action: ConfigAction::External {
                    action: ExternalConfigAction::ResetIncompatible
                }
            })
        ));
    }
}

#[cfg(test)]
mod bootstrap_profile_tests {
    use super::{exec_requests_json_output, BootstrapProfile, SessionAction};

    #[test]
    fn profiles_start_only_their_requested_background_services() {
        let cases = [
            (BootstrapProfile::Interactive, true, true),
            (BootstrapProfile::Execution, false, true),
            (BootstrapProfile::Management, false, false),
        ];

        for (profile, starts_peer_host, starts_mcp) in cases {
            assert_eq!(profile.starts_peer_host(), starts_peer_host);
            assert_eq!(profile.starts_mcp(), starts_mcp);
        }
    }

    #[test]
    fn session_resume_and_continue_use_interactive_bootstrap() {
        let resume = SessionAction::Resume {
            id: "session-1".to_string(),
        };

        assert_eq!(resume.bootstrap_profile(), BootstrapProfile::Interactive);
        assert_eq!(
            SessionAction::Continue.bootstrap_profile(),
            BootstrapProfile::Interactive
        );
    }

    #[test]
    fn session_management_actions_use_management_bootstrap() {
        let actions = [
            SessionAction::List,
            SessionAction::Show {
                id: "session-1".to_string(),
            },
            SessionAction::Delete {
                id: "session-1".to_string(),
            },
            SessionAction::Fork {
                id: "session-1".to_string(),
                id_only: false,
            },
        ];

        for action in actions {
            assert_eq!(action.bootstrap_profile(), BootstrapProfile::Management);
        }
    }

    #[test]
    fn json_exec_parse_failures_are_detected_before_clap_exits() {
        let args = [
            "bitfun",
            "exec",
            "task",
            "--output-format",
            "json",
            "--unknown-option",
        ]
        .map(std::ffi::OsString::from);

        assert!(exec_requests_json_output(&args));
    }
}
