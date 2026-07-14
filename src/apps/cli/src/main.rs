/// BitFun CLI
///
/// Command-line interface version, supports:
/// - Interactive TUI
/// - Single command execution
/// - Batch task processing
mod acp_cli;
mod agent;
#[allow(dead_code)]
mod chat_state;
mod commands;
mod config;
mod diagnostics;
mod logging;
mod management;
mod modes;
mod plugin_diagnostics;
mod prompts;
mod root_handlers;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use config::CliConfig;
use modes::chat::ChatMode;
use modes::exec::ExecOutputFormat;

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
        /// Example: --output-patch or --output-patch ./result.patch
        #[arg(long, num_args = 0..=1, default_missing_value = "-")]
        output_patch: Option<String>,

        /// Tool execution requires confirmation (default: no confirmation to avoid blocking non-interactive mode)
        #[arg(long)]
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
        #[arg(long, default_value = "bitfun-cli")]
        command: String,
    },
    /// Check local readiness for ACP clients
    Doctor {
        /// Command name or path to show in generated examples
        #[arg(long, default_value = "bitfun-cli")]
        command: String,
    },
    /// Print editor/client integration snippets
    Config {
        /// ACP client/editor to generate config for
        #[arg(long, value_enum, default_value_t = acp_cli::AcpConfigClient::Zed)]
        client: acp_cli::AcpConfigClient,

        /// Command name or path your editor should execute
        #[arg(long, default_value = "bitfun-cli")]
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

#[derive(Subcommand)]
enum ConfigAction {
    /// Show configuration
    Show,
    /// Edit configuration
    Edit,
    /// Reset to default configuration
    Reset,
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

/// Initialize all core services (config, AI client, agentic system).
/// Returns (agentic_system, original_skip_confirmation).
async fn initialize_core_services(
    skip_tool_confirmation: bool,
) -> Result<(agent::agentic_system::AgenticSystem, bool)> {
    use bitfun_core::infrastructure::ai::AIClientFactory;

    bitfun_core::service::config::initialize_global_config()
        .await
        .expect("Failed to initialize global config service");
    tracing::info!("Global config service initialized");

    // Save and override tool confirmation setting
    let config_service = bitfun_core::service::config::get_global_config_service()
        .await
        .ok();
    let original_skip_confirmation = if let Some(ref svc) = config_service {
        let ai_config: bitfun_core::service::config::types::AIConfig =
            svc.get_config(Some("ai")).await.unwrap_or_default();
        ai_config.skip_tool_confirmation
    } else {
        false
    };
    if let Some(ref svc) = config_service {
        let _ = svc
            .set_config("ai.skip_tool_confirmation", skip_tool_confirmation)
            .await;
    }

    AIClientFactory::initialize_global()
        .await
        .expect("Failed to initialize global AIClientFactory");
    tracing::info!("Global AI client factory initialized");

    initialize_terminal_service().await;

    let agentic_system = agent::agentic_system::init_agentic_system()
        .await
        .expect("Failed to initialize agentic system");
    tracing::info!("Agentic system initialized");

    // Initialize MCP service in background (non-blocking)
    if let Some(ref cfg_svc) = config_service {
        match bitfun_core::service::mcp::MCPService::new(cfg_svc.clone()) {
            Ok(mcp_service) => {
                let mcp_service = std::sync::Arc::new(mcp_service);
                MCP_SERVICE.set(mcp_service.clone()).ok();

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

    Ok((agentic_system, original_skip_confirmation))
}

/// Restore original tool confirmation setting
async fn restore_tool_confirmation(original: bool) {
    if let Ok(svc) = bitfun_core::service::config::get_global_config_service().await {
        let _ = svc.set_config("ai.skip_tool_confirmation", original).await;
    }
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
    _config: CliConfig,
    default_agent: String,
    _workspace_str: String,
) -> Result<()> {
    use ui::startup::{StartupPage, StartupResult};

    // 1. Initialize terminal and show loading screen
    let mut terminal = ui::init_terminal()?;
    ui::render_loading(&mut terminal, "Initializing system, please wait...")?;

    // 2. Set workspace path
    let workspace = setup_workspace();

    // 3. Initialize core services
    let (agentic_system, original_skip_confirmation) = initialize_core_services(true).await?;

    // 4. Show startup page (with full command support)
    let mut startup_page = StartupPage::new(
        agentic_system.coordinator.clone(),
        default_agent,
        workspace.clone(),
    );
    let startup_result = startup_page.run(&mut terminal)?;

    if let StartupResult::Exit = startup_result {
        shutdown_mcp_servers().await;
        restore_tool_confirmation(original_skip_confirmation).await;
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
    let mut chat_mode = ChatMode::new(config, agent_type, workspace, &agentic_system);
    if let Some(session_id) = restore_session_id {
        chat_mode = chat_mode.with_restore_session(session_id);
    }
    if let Some(prompt) = initial_prompt {
        chat_mode = chat_mode.with_initial_prompt(prompt);
    }
    let _exit_reason = chat_mode.run(Some(terminal))?;

    // 6. Cleanup
    shutdown_mcp_servers().await;
    restore_tool_confirmation(original_skip_confirmation).await;
    println!("Goodbye!");

    Ok(())
}

// ======================== Main ========================

async fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    let is_tui_mode = matches!(cli.command, None | Some(Commands::Chat { .. }));
    let is_exec_mode = matches!(cli.command, Some(Commands::Exec { .. }));
    let file_log_level = logging::default_log_level(cli.verbose);
    let stderr_log_level = if cli.verbose {
        tracing::Level::TRACE
    } else {
        tracing::Level::ERROR
    };

    if is_tui_mode || is_exec_mode {
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
            confirm,
        }) => {
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
                    confirm,
                },
            )
            .await?;
        }

        Some(Commands::Sessions { action }) => {
            if let Some(session_id) = root_handlers::handle_session_action(action).await? {
                run_interactive_with_session(config, session_id).await?;
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
            if !management::print_doctor().await? {
                std::process::exit(1);
            }
        }

        Some(Commands::Config { action }) => {
            root_handlers::handle_config_action(action, &config)?;
        }

        Some(Commands::Health) => {
            root_handlers::handle_health_command()?;
        }

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

async fn run_interactive_with_session(config: CliConfig, session_id: String) -> Result<()> {
    let mut terminal = ui::init_terminal()?;
    ui::render_loading(&mut terminal, "Initializing system, please wait...")?;

    let workspace = setup_workspace();
    let (agentic_system, original_skip_confirmation) = initialize_core_services(true).await?;
    let workspace_path = workspace
        .clone()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let session = agentic_system
        .coordinator
        .restore_session(&workspace_path, &session_id)
        .await?;

    let mut chat_mode = ChatMode::new(config, session.agent_type, workspace, &agentic_system)
        .with_restore_session(session_id);
    let run_result = chat_mode.run(Some(terminal));

    shutdown_mcp_servers().await;
    restore_tool_confirmation(original_skip_confirmation).await;
    println!("Goodbye!");

    run_result?;
    Ok(())
}

fn main() {
    let worker = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime");
            runtime.block_on(run_cli())
        })
        .expect("failed to spawn bitfun-cli worker thread");

    match worker.join() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("Error: bitfun-cli worker thread panicked");
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
        let list = Cli::try_parse_from(["bitfun-cli", "plugins"]).expect("parse plugin list");
        assert!(matches!(
            list.command,
            Some(Commands::Plugins { action: None })
        ));

        let approval =
            Cli::try_parse_from(["bitfun-cli", "plugins", "approve-source", "acme.demo"])
                .expect("parse plugin source approval");
        assert!(matches!(
            approval.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::ApproveSource { package_id })
            }) if package_id == "acme.demo"
        ));

        let deny = Cli::try_parse_from(["bitfun-cli", "plugins", "deny", "acme.demo"])
            .expect("parse plugin deny");
        assert!(matches!(
            deny.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::Deny { package_id })
            }) if package_id == "acme.demo"
        ));

        let revoke = Cli::try_parse_from(["bitfun-cli", "plugins", "revoke", "acme.demo"])
            .expect("parse plugin revoke");
        assert!(matches!(
            revoke.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::Revoke { package_id })
            }) if package_id == "acme.demo"
        ));

        let preview = Cli::try_parse_from(["bitfun-cli", "plugins", "activate", "acme.demo"])
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
            "bitfun-cli",
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

        let deactivate = Cli::try_parse_from(["bitfun-cli", "plugins", "deactivate", "acme.demo"])
            .expect("parse plugin deactivation");
        assert!(matches!(
            deactivate.command,
            Some(Commands::Plugins {
                action: Some(PluginAction::Deactivate { package_id })
            }) if package_id == "acme.demo"
        ));
    }
}
