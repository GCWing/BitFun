use anyhow::{Context, Result};

use std::io::IsTerminal;
use std::path::Path;

use crate::{
    config::CliConfig,
    diagnostics::{emit_exit_diagnostic, ExitContext, ExitKind},
    modes::exec::{ExecMode, ExecOutputFormat, ExecSessionOptions},
    ui::string_utils::truncate_str,
    ConfigAction, SessionAction,
};

pub struct ExecCommandArgs {
    pub message: Option<String>,
    pub agent: String,
    pub continue_last: bool,
    pub resume: Option<String>,
    pub session: Option<String>,
    pub session_id: Option<String>,
    pub fork_session: bool,
    pub output_format: ExecOutputFormat,
    pub output_patch: Option<String>,
    pub confirm: bool,
}

pub async fn handle_exec_command(config: CliConfig, args: ExecCommandArgs) -> Result<()> {
    let workspace_path_resolved = std::env::current_dir().ok();

    if let Some(ref ws_path) = workspace_path_resolved {
        tracing::info!("Workspace path set: {:?}", ws_path);
    }

    let message = resolve_exec_message(args.message)?;
    let resume = match (args.resume, args.session) {
        (Some(_), Some(_)) => {
            anyhow::bail!("Use only one of --resume or --session");
        }
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
    if args.session_id.is_some() && (args.continue_last || resume.is_some()) {
        anyhow::bail!("--session-id cannot be combined with --continue, --resume, or --session");
    }
    if args.fork_session && args.session_id.is_some() {
        anyhow::bail!("--fork-session cannot be combined with --session-id");
    }

    let skip_confirmation = !args.confirm;
    let (agentic_system, original_skip_confirmation) =
        crate::initialize_core_services(skip_confirmation)
            .await
            .map_err(|error| {
                emit_exit_diagnostic(
                    ExitKind::ExecError,
                    &error.to_string(),
                    &ExitContext {
                        agent_type: Some(args.agent.as_str()),
                        workspace: workspace_path_resolved.as_deref(),
                        ..Default::default()
                    },
                );
                error
            })?;

    let mut exec_mode = ExecMode::new(
        config,
        message,
        args.agent,
        &agentic_system,
        workspace_path_resolved,
        args.output_patch,
        args.output_format,
        ExecSessionOptions {
            resume,
            continue_last: args.continue_last,
            session_id: args.session_id,
            fork_session: args.fork_session,
        },
    );
    let run_result = exec_mode.run().await;

    crate::shutdown_mcp_servers().await;
    crate::restore_tool_confirmation(original_skip_confirmation).await;

    run_result
}

fn resolve_exec_message(message: Option<String>) -> Result<String> {
    let mut combined = message.unwrap_or_default();
    if !std::io::stdin().is_terminal() {
        use std::io::Read;
        let mut stdin_content = String::new();
        std::io::stdin().read_to_string(&mut stdin_content)?;
        let stdin_content = stdin_content.trim_end().to_string();
        if !stdin_content.is_empty() {
            if combined.is_empty() {
                combined = stdin_content;
            } else {
                combined.push('\n');
                combined.push_str(&stdin_content);
            }
        }
    }

    let message = combined.trim().to_string();
    if message.is_empty() {
        anyhow::bail!("Prompt cannot be empty");
    }

    Ok(message)
}

pub async fn handle_session_action(action: SessionAction) -> Result<Option<String>> {
    let agentic_system = crate::agent::agentic_system::init_agentic_system_for_cli().await?;

    let coordinator = agentic_system.coordinator.clone();
    let workspace_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    match action {
        SessionAction::List => {
            let sessions = coordinator.list_sessions(&workspace_path).await?;

            if sessions.is_empty() {
                println!(
                    "No history sessions for current project: {}",
                    workspace_path.display()
                );
                return Ok(None);
            }

            println!(
                "History sessions for current project (total {})",
                sessions.len()
            );
            println!("Project: {}\n", workspace_path.display());

            for (i, info) in sessions.iter().enumerate() {
                let last_updated = {
                    let duration = info
                        .last_activity_at
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    let secs = duration.as_secs() as i64;
                    chrono::DateTime::from_timestamp(secs, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                };

                println!("{}. {} (ID: {})", i + 1, info.session_name, info.session_id);
                println!(
                    "   Agent: {} | Turns: {} | Updated: {}",
                    info.agent_type, info.turn_count, last_updated
                );
                println!();
            }
        }

        SessionAction::Show { id } => {
            let sessions = coordinator.list_sessions(&workspace_path).await?;

            let session_id = if id == "last" {
                sessions
                    .first()
                    .map(|s| s.session_id.clone())
                    .ok_or_else(|| anyhow::anyhow!("No history sessions"))?
            } else {
                id
            };

            let session = coordinator
                .restore_session(&workspace_path, &session_id)
                .await?;
            let messages = coordinator.get_messages(&session_id).await?;

            println!("Session Details\n");
            println!("Name: {}", session.session_name);
            println!("ID: {}", session.session_id);
            println!("Agent: {}", session.agent_type);
            println!("State: {:?}", session.state);
            println!("Messages: {}", messages.len());
            println!();

            if !messages.is_empty() {
                println!("Recent messages:");
                let recent: Vec<_> = messages.iter().rev().take(5).collect();
                for msg in recent.iter().rev() {
                    let role = format!("{:?}", msg.role);
                    let content_preview = match &msg.content {
                        bitfun_core::agentic::core::message::MessageContent::Text(text) => {
                            text.lines().next().unwrap_or("").to_string()
                        }
                        bitfun_core::agentic::core::message::MessageContent::Multimodal {
                            text,
                            images,
                        } => {
                            if text.is_empty() {
                                format!("[{} images]", images.len())
                            } else {
                                text.lines().next().unwrap_or("").to_string()
                            }
                        }
                        bitfun_core::agentic::core::message::MessageContent::Mixed {
                            text,
                            tool_calls,
                            ..
                        } => {
                            if text.is_empty() {
                                format!("[{} tool calls]", tool_calls.len())
                            } else {
                                text.lines().next().unwrap_or("").to_string()
                            }
                        }
                        bitfun_core::agentic::core::message::MessageContent::ToolResult {
                            tool_name,
                            ..
                        } => format!("[Tool result: {}]", tool_name),
                    };
                    let preview = if content_preview.len() > 80 {
                        truncate_str(&content_preview, 77)
                    } else {
                        content_preview
                    };
                    println!("  [{}] {}", role, preview);
                }
            }
        }

        SessionAction::Delete { id } => {
            coordinator.delete_session(&workspace_path, &id).await?;
            println!("Deleted session from current project: {}", id);
        }

        SessionAction::Resume { id } => {
            let session_id = resolve_cli_session_id(&coordinator, &workspace_path, &id).await?;
            return Ok(Some(session_id));
        }

        SessionAction::Continue => {
            let session_id = resolve_cli_session_id(&coordinator, &workspace_path, "last").await?;
            return Ok(Some(session_id));
        }

        SessionAction::Fork { id, id_only } => {
            let session_id = resolve_cli_session_id(&coordinator, &workspace_path, &id).await?;
            let (_session, turns) = coordinator
                .restore_session_view(&workspace_path, &session_id)
                .await?;
            let source_turn_id = turns
                .last()
                .map(|turn| turn.turn_id.clone())
                .ok_or_else(|| anyhow::anyhow!("Session has no persisted turns to fork"))?;
            let path_manager = bitfun_core::infrastructure::try_get_path_manager_arc()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let persistence_manager =
                bitfun_core::agentic::persistence::PersistenceManager::new(path_manager)
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let result = persistence_manager
                .branch_session(
                    &workspace_path,
                    &bitfun_core::agentic::persistence::session_branch::SessionBranchRequest {
                        source_session_id: session_id.clone(),
                        source_turn_id,
                    },
                )
                .await?;

            if id_only {
                println!("{}", result.session_id);
            } else {
                println!("Forked session");
                println!("Source ID: {}", session_id);
                println!("New ID: {}", result.session_id);
                println!("Name: {}", result.session_name);
                println!("Agent: {}", result.agent_type);
            }
        }
    }

    Ok(None)
}

async fn resolve_cli_session_id(
    coordinator: &std::sync::Arc<bitfun_core::agentic::coordination::ConversationCoordinator>,
    workspace_path: &Path,
    id: &str,
) -> Result<String> {
    if id == "last" {
        let sessions = coordinator.list_sessions(workspace_path).await?;
        return sessions
            .first()
            .map(|session| session.session_id.clone())
            .ok_or_else(|| anyhow::anyhow!("No history sessions"));
    }

    Ok(id.to_string())
}

pub fn handle_config_action(action: ConfigAction, config: &CliConfig) -> Result<()> {
    match action {
        ConfigAction::Show => {
            println!("Current Configuration\n");
            println!("Note: AI model configuration is managed via GlobalConfig");
            println!();
            println!("UI Configuration:");
            println!("  Appearance: {}", config.ui.theme);
            println!("  Theme ID: {}", config.ui.theme_id);
            println!("  Color scheme: {}", config.ui.color_scheme);
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

pub fn handle_health_command() -> Result<()> {
    println!("BitFun CLI is running normally");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Config directory: {:?}", CliConfig::config_dir()?);
    Ok(())
}

pub async fn serve_acp_stdio() -> Result<()> {
    crate::setup_workspace();

    bitfun_core::service::config::initialize_global_config()
        .await
        .context("Failed to initialize global config service")?;
    tracing::info!("Global config service initialized");

    use bitfun_core::infrastructure::ai::AIClientFactory;
    AIClientFactory::initialize_global()
        .await
        .context("Failed to initialize global AIClientFactory")?;
    tracing::info!("Global AI client factory initialized");

    crate::initialize_terminal_service().await;

    let agentic_system = crate::agent::agentic_system::init_agentic_system()
        .await
        .context("Failed to initialize agentic system")?;
    tracing::info!("Agentic system initialized");

    bitfun_acp::BitfunAcpRuntime::serve_stdio(agentic_system).await?;
    Ok(())
}
