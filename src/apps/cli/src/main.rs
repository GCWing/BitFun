mod agent;
/// BitFun CLI
///
/// Command-line interface version, supports:
/// - Interactive TUI
/// - Single command execution
/// - Batch task processing
/// - Agent Client Protocol (ACP) server
mod config;
mod modes;
mod session;
mod ui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use config::CliConfig;
use modes::chat::ChatMode;
use modes::exec::ExecMode;

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

        /// Workspace path
        #[arg(short, long)]
        workspace: Option<String>,
    },

    /// Execute single command
    Exec {
        /// User message
        message: String,

        /// Agent type
        #[arg(short, long, default_value = "agentic")]
        agent: String,

        /// Workspace path
        #[arg(short, long)]
        workspace: Option<String>,

        /// Output in JSON format (script-friendly)
        #[arg(long)]
        json: bool,

        /// Output git diff patch after execution (for SWE-bench evaluation)
        /// Without path outputs to terminal, with path saves to file
        /// Example: --output-patch or --output-patch ./result.patch
        #[arg(long, num_args = 0..=1, default_missing_value = "-")]
        output_patch: Option<String>,

        /// Tool execution requires confirmation (default: no confirmation to avoid blocking non-interactive mode)
        #[arg(long)]
        confirm: bool,
    },

    /// Execute batch tasks
    Batch {
        /// Task configuration file path
        #[arg(short, long)]
        tasks: String,
    },

    /// Session management
    Sessions {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Invoke tool directly
    Tool {
        /// Tool name
        name: String,

        /// Tool parameters (JSON)
        #[arg(short, long)]
        params: Option<String>,
    },

    /// Health check
    Health,

    /// Start Agent Client Protocol (ACP) server
    ///
    /// Runs a JSON-RPC 2.0 server over stdio for integration with
    /// ACP-compatible editors and IDEs.
    ///
    /// Usage: bitfun acp
    /// The server reads JSON-RPC requests from stdin and writes responses to stdout.
    Acp {
        /// Working directory for the ACP session
        #[arg(short, long)]
        workspace: Option<String>,
    },
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

fn resolve_workspace_path(workspace: Option<&str>) -> Option<std::path::PathBuf> {
    match workspace {
        Some(".") => std::env::current_dir().ok(),
        Some(path) => Some(std::path::PathBuf::from(path)),
        None => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    let is_tui_mode = matches!(cli.command, None | Some(Commands::Chat { .. }));
    let is_acp_mode = matches!(cli.command, Some(Commands::Acp { .. }));

    if is_tui_mode {
        use std::fs::OpenOptions;

        let log_dir = CliConfig::config_dir()
            .ok()
            .map(|d| d.join("logs"))
            .unwrap_or_else(|| std::env::temp_dir().join("bitfun-cli"));

        std::fs::create_dir_all(&log_dir).ok();
        let log_file = log_dir.join("bitfun-cli.log");

        if let Ok(file) = OpenOptions::new().create(true).append(true).open(log_file) {
            tracing_subscriber::fmt()
                .with_max_level(log_level)
                .with_writer(move || -> Box<dyn std::io::Write + Send> {
                    match file.try_clone() {
                        Ok(cloned) => Box::new(cloned),
                        Err(e) => {
                            eprintln!("Warning: Failed to clone log file handle: {}", e);
                            Box::new(std::io::sink())
                        }
                    }
                })
                .with_ansi(false)
                .with_target(false)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_max_level(log_level)
                .with_target(false)
                .init();
        }
    } else if is_acp_mode {
        tracing_subscriber::fmt()
            .with_max_level(log_level)
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .with_target(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(log_level)
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
        Some(Commands::Chat { agent, workspace }) => {
            let (workspace, mut startup_terminal) = if workspace.is_none() {
                use ui::startup::StartupPage;

                let mut terminal = ui::init_terminal()?;
                let mut startup_page = StartupPage::new();
                let selected_workspace = startup_page.run(&mut terminal)?;

                if selected_workspace.is_none() {
                    ui::restore_terminal(terminal)?;
                    println!("Goodbye!");
                    return Ok(());
                }

                (selected_workspace, Some(terminal))
            } else {
                (workspace, None)
            };

            if let Some(ref mut term) = startup_terminal {
                ui::render_loading(term, "Initializing system, please wait...")?;
            } else {
                println!("Initializing system, please wait...");
            }

            let workspace_path = resolve_workspace_path(workspace.as_deref());
            tracing::info!("CLI workspace: {:?}", workspace_path);

            bitfun_core::service::config::initialize_global_config()
                .await
                .context("Failed to initialize global config service")?;
            tracing::info!("Global config service initialized");

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
                if let Err(e) = svc.set_config("ai.skip_tool_confirmation", true).await {
                    tracing::warn!(
                        "Failed to temporarily disable tool confirmation, continuing: {}",
                        e
                    );
                }
            }

            use bitfun_core::infrastructure::ai::AIClientFactory;
            AIClientFactory::initialize_global()
                .await
                .context("Failed to initialize global AIClientFactory")?;
            tracing::info!("Global AI client factory initialized");

            let agentic_system = bitfun_core::agentic::system::init_agentic_system()
                .await
                .context("Failed to initialize agentic system")?;
            tracing::info!("Agentic system initialized");

            if let Some(ref mut term) = startup_terminal {
                ui::render_loading(term, "System initialized, starting chat interface...")?;
            } else {
                println!("System initialized, starting chat interface...\n");
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            let mut chat_mode = ChatMode::new(config, agent, workspace_path, &agentic_system);
            let chat_result = chat_mode.run(startup_terminal);

            if let Some(ref svc) = config_service {
                let _ = svc
                    .set_config("ai.skip_tool_confirmation", original_skip_confirmation)
                    .await;
            }

            chat_result?;
        }

        Some(Commands::Exec {
            message,
            agent,
            workspace,
            json: _,
            output_patch,
            confirm,
        }) => {
            let workspace_path_resolved = resolve_workspace_path(workspace.as_deref())
                .or_else(|| std::env::current_dir().ok());
            tracing::info!("CLI workspace: {:?}", workspace_path_resolved);

            bitfun_core::service::config::initialize_global_config()
                .await
                .context("Failed to initialize global config service")?;
            tracing::info!("Global config service initialized");

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
                let desired_skip = !confirm;
                if let Err(e) = svc
                    .set_config("ai.skip_tool_confirmation", desired_skip)
                    .await
                {
                    tracing::warn!("Failed to set tool confirmation toggle, continuing: {}", e);
                }
            }

            use bitfun_core::infrastructure::ai::AIClientFactory;
            AIClientFactory::initialize_global()
                .await
                .context("Failed to initialize global AIClientFactory")?;
            tracing::info!("Global AI client factory initialized");

            let agentic_system = bitfun_core::agentic::system::init_agentic_system()
                .await
                .context("Failed to initialize agentic system")?;
            tracing::info!("Agentic system initialized");

            let mut exec_mode = ExecMode::new(
                config,
                message,
                agent,
                &agentic_system,
                workspace_path_resolved,
                output_patch,
            );
            let run_result = exec_mode.run().await;

            if let Some(ref svc) = config_service {
                let _ = svc
                    .set_config("ai.skip_tool_confirmation", original_skip_confirmation)
                    .await;
            }

            run_result?;
        }

        Some(Commands::Batch { tasks }) => {
            println!("Executing batch tasks...");
            println!("Tasks file: {}", tasks);
            println!("\nWarning: Batch execution feature coming soon");
        }

        Some(Commands::Sessions { action }) => {
            handle_session_action(action)?;
        }

        Some(Commands::Config { action }) => {
            handle_config_action(action, &config)?;
        }

        Some(Commands::Tool { name, params }) => {
            println!("Invoking tool: {}", name);
            if let Some(p) = params {
                println!("Parameters: {}", p);
            }
            println!("\nWarning: Tool invocation feature coming soon");
        }

        Some(Commands::Health) => {
            println!("BitFun CLI is running normally");
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
            println!("Config directory: {:?}", CliConfig::config_dir()?);
        }

        Some(Commands::Acp { workspace }) => {
            let workspace_path = resolve_workspace_path(workspace.as_deref())
                .or_else(|| std::env::current_dir().ok());
            tracing::info!("ACP server workspace: {:?}", workspace_path);

            // Initialize core services
            bitfun_core::service::config::initialize_global_config()
                .await
                .context("Failed to initialize global config service")?;
            tracing::info!("Global config service initialized");

            use bitfun_core::infrastructure::ai::AIClientFactory;
            AIClientFactory::initialize_global()
                .await
                .context("Failed to initialize global AIClientFactory")?;
            tracing::info!("Global AI client factory initialized");

            let agentic_system = bitfun_core::agentic::system::init_agentic_system()
                .await
                .context("Failed to initialize agentic system")?;
            tracing::info!("Agentic system initialized");

            // Start ACP server
            tracing::info!("Starting ACP server...");
            bitfun_acp::BitfunAcpRuntime::serve_stdio(agentic_system).await?;
        }

        None => {
            use modes::chat::ChatExitReason;
            use ui::startup::StartupPage;

            loop {
                let mut terminal = ui::init_terminal()?;
                let mut startup_page = StartupPage::new();
                let workspace = startup_page.run(&mut terminal)?;

                if workspace.is_none() {
                    ui::restore_terminal(terminal)?;
                    println!("Goodbye!");
                    break;
                }

                ui::render_loading(&mut terminal, "Initializing system, please wait...")?;

                let workspace_path = resolve_workspace_path(workspace.as_deref());
                tracing::info!("CLI workspace: {:?}", workspace_path);

                bitfun_core::service::config::initialize_global_config()
                    .await
                    .context("Failed to initialize global config service")?;
                tracing::info!("Global config service initialized");

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
                    let _ = svc.set_config("ai.skip_tool_confirmation", true).await;
                }

                use bitfun_core::infrastructure::ai::AIClientFactory;
                AIClientFactory::initialize_global()
                    .await
                    .context("Failed to initialize global AIClientFactory")?;
                tracing::info!("Global AI client factory initialized");

                let agentic_system = bitfun_core::agentic::system::init_agentic_system()
                    .await
                    .context("Failed to initialize agentic system")?;
                tracing::info!("Agentic system initialized");

                ui::render_loading(
                    &mut terminal,
                    "System initialized, starting chat interface...",
                )?;

                let agent = config.behavior.default_agent.clone();
                let mut chat_mode =
                    ChatMode::new(config.clone(), agent, workspace_path, &agentic_system);
                let exit_reason = chat_mode.run(Some(terminal));

                if let Some(ref svc) = config_service {
                    let _ = svc
                        .set_config("ai.skip_tool_confirmation", original_skip_confirmation)
                        .await;
                }
                let exit_reason = exit_reason?;

                match exit_reason {
                    ChatExitReason::Quit => {
                        println!("Goodbye!");
                        break;
                    }
                    ChatExitReason::BackToMenu => {
                        continue;
                    }
                }
            }
        }
    }

    Ok(())
}

fn handle_session_action(action: SessionAction) -> Result<()> {
    match action {
        SessionAction::List => {
            use session::Session;
            let sessions = Session::list_all()?;

            if sessions.is_empty() {
                println!("No history sessions");
                return Ok(());
            }

            println!("History sessions (total {})\n", sessions.len());

            for (i, info) in sessions.iter().enumerate() {
                println!("{}. {} (ID: {})", i + 1, info.title, info.id);
                println!(
                    "   Agent: {} | Messages: {} | Updated: {}",
                    info.agent,
                    info.message_count,
                    info.updated_at.format("%Y-%m-%d %H:%M")
                );
                if let Some(ws) = &info.workspace {
                    println!("   Workspace: {}", ws);
                }
                println!();
            }
        }

        SessionAction::Show { id } => {
            use session::Session;

            let session = if id == "last" {
                Session::get_last()?.ok_or_else(|| anyhow::anyhow!("No history sessions"))?
            } else {
                Session::load(&id)?
            };

            println!("Session Details\n");
            println!("Title: {}", session.title);
            println!("ID: {}", session.id);
            println!("Agent: {}", session.agent);
            println!(
                "Created: {}",
                session.created_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!(
                "Updated: {}",
                session.updated_at.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(ws) = &session.workspace {
                println!("Workspace: {}", ws);
            }
            println!();
            println!("Statistics:");
            println!("  Messages: {}", session.metadata.message_count);
            println!("  Tool calls: {}", session.metadata.tool_calls);
            println!("  Files modified: {}", session.metadata.files_modified);
            println!();

            if !session.messages.is_empty() {
                println!("Recent messages:");
                let recent = session.messages.iter().rev().take(3);
                for msg in recent {
                    println!(
                        "  [{}] {}: {}",
                        msg.timestamp.format("%H:%M:%S"),
                        msg.role,
                        msg.content.lines().next().unwrap_or("")
                    );
                }
            }
        }

        SessionAction::Delete { id } => {
            use session::Session;
            Session::delete(&id)?;
            println!("Deleted session: {}", id);
        }
    }

    Ok(())
}

fn handle_config_action(action: ConfigAction, config: &CliConfig) -> Result<()> {
    match action {
        ConfigAction::Show => {
            println!("Current Configuration\n");
            println!("Note: AI model configuration is now managed via GlobalConfig");
            println!("View and manage at: Main Menu -> Settings -> AI Model Configuration");
            println!();
            println!("UI Configuration:");
            println!("  Theme: {}", config.ui.theme);
            println!("  Show tips: {}", config.ui.show_tips);
            println!("  Animation: {}", config.ui.animation);
            println!();
            println!("Behavior Configuration:");
            println!("  Auto save: {}", config.behavior.auto_save);
            println!("  Confirm dangerous: {}", config.behavior.confirm_dangerous);
            println!("  Default Agent: {}", config.behavior.default_agent);
            println!();
            println!("Config file: {:?}", CliConfig::config_path()?);
        }

        ConfigAction::Edit => {
            let config_path = CliConfig::config_path()?;
            println!("Config file location: {:?}", config_path);
            println!();
            println!("Please use a text editor to edit the config file:");
            println!("  vi {:?}", config_path);
            println!("  or");
            println!("  code {:?}", config_path);
        }

        ConfigAction::Reset => {
            let default_config = CliConfig::default();
            default_config.save()?;
            println!("Reset to default configuration");
        }
    }

    Ok(())
}
