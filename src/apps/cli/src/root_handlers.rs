use anyhow::{Context, Result};

use std::io::IsTerminal;
use std::path::Path;

use bitfun_agent_runtime::sdk::{AgentSessionRestoreRequest, SessionTranscriptRequest};

use crate::{
    chat_state::{transcript_message_preview, transcript_role_label},
    config::CliConfig,
    diagnostics::{emit_exit_diagnostic, ExitContext, ExitKind},
    modes::exec::{
        emit_preflight_json_error, ExecApprovalMode, ExecMode, ExecOutputFormat, ExecSessionOptions,
    },
    ui::string_utils::truncate_str,
    ConfigAction, SessionAction,
};

pub(crate) struct ExecCommandArgs {
    pub message: Option<String>,
    pub agent: String,
    pub continue_last: bool,
    pub resume: Option<String>,
    pub session: Option<String>,
    pub session_id: Option<String>,
    pub fork_session: bool,
    pub output_format: ExecOutputFormat,
    pub output_patch: Option<String>,
    pub approval_mode: ExecApprovalMode,
}

pub(crate) async fn handle_exec_command(config: CliConfig, args: ExecCommandArgs) -> Result<()> {
    let workspace_path_resolved = std::env::current_dir().ok();

    if let Some(ref ws_path) = workspace_path_resolved {
        tracing::info!("Workspace path set: {:?}", ws_path);
    }

    let message = match resolve_exec_message(args.message) {
        Ok(message) => message,
        Err(error) => return exec_preflight_error(args.output_format, error),
    };
    let resume = match (args.resume, args.session) {
        (Some(_), Some(_)) => {
            return exec_preflight_error(
                args.output_format,
                anyhow::anyhow!("Use only one of --resume or --session"),
            );
        }
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
    if args.continue_last && resume.is_some() {
        return exec_preflight_error(
            args.output_format,
            anyhow::anyhow!("--continue cannot be combined with --resume or --session"),
        );
    }
    if let Some(session_id) = resume.as_deref().filter(|session_id| *session_id != "last") {
        if let Err(error) = bitfun_agent_runtime::session_control::validate_session_id(session_id) {
            return exec_preflight_error(args.output_format, anyhow::anyhow!(error));
        }
    }
    if let Some(session_id) = args.session_id.as_deref() {
        if let Err(error) = bitfun_agent_runtime::session_control::validate_session_id(session_id) {
            return exec_preflight_error(args.output_format, anyhow::anyhow!(error));
        }
    }
    if args.session_id.is_some() && (args.continue_last || resume.is_some()) {
        return exec_preflight_error(
            args.output_format,
            anyhow::anyhow!(
                "--session-id cannot be combined with --continue, --resume, or --session"
            ),
        );
    }
    if args.fork_session && args.session_id.is_some() {
        return exec_preflight_error(
            args.output_format,
            anyhow::anyhow!("--fork-session cannot be combined with --session-id"),
        );
    }
    if args.output_format == ExecOutputFormat::StreamJson
        && args.output_patch.as_deref() == Some("-")
    {
        return exec_preflight_error(
            args.output_format,
            anyhow::anyhow!(
                "--output-patch with --output-format stream-json requires an explicit file path"
            ),
        );
    }

    let approval_policy = match args.approval_mode {
        ExecApprovalMode::Reject => crate::runtime::approval::CliApprovalPolicy::Reject,
        ExecApprovalMode::Auto => crate::runtime::approval::CliApprovalPolicy::Auto,
    };
    let runtime = match crate::initialize_core_services(
        workspace_path_resolved
            .as_deref()
            .unwrap_or_else(|| Path::new(".")),
        approval_policy,
        crate::BootstrapProfile::Execution,
    )
    .await
    {
        Ok(runtime) => runtime,
        Err(error) => {
            emit_exit_diagnostic(
                ExitKind::ExecError,
                &error.to_string(),
                &ExitContext {
                    agent_type: Some(args.agent.as_str()),
                    workspace: workspace_path_resolved.as_deref(),
                    ..Default::default()
                },
            );
            return exec_preflight_error(args.output_format, error);
        }
    };

    let mut exec_mode = ExecMode::new(
        config,
        message,
        args.agent,
        runtime.clone(),
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

    run_result
}

fn exec_preflight_error<T>(output_format: ExecOutputFormat, error: anyhow::Error) -> Result<T> {
    emit_preflight_json_error(output_format, &error)?;
    Err(error)
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

pub(crate) async fn handle_session_action(
    action: SessionAction,
) -> Result<Option<(String, std::sync::Arc<crate::runtime::CliRuntimeContext>)>> {
    let workspace_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let approval_policy = match &action {
        SessionAction::Resume { .. } | SessionAction::Continue => {
            crate::runtime::approval::CliApprovalPolicy::Ask
        }
        _ => crate::runtime::approval::CliApprovalPolicy::Reject,
    };
    let bootstrap_profile = action.bootstrap_profile();
    let runtime =
        crate::initialize_core_services(&workspace_path, approval_policy, bootstrap_profile)
            .await?;

    match action {
        SessionAction::List => {
            let sessions = list_cli_sessions(runtime.agent_runtime(), &workspace_path).await?;

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
                    i64::try_from(info.last_active_at_ms)
                        .ok()
                        .and_then(chrono::DateTime::from_timestamp_millis)
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
            let session_id =
                resolve_cli_session_id(runtime.agent_runtime(), &workspace_path, &id).await?;

            let restored = runtime
                .agent_runtime()
                .restore_session(AgentSessionRestoreRequest {
                    workspace_path: workspace_path.to_string_lossy().to_string(),
                    session_id: session_id.clone(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?;
            let transcript = runtime
                .agent_runtime()
                .read_session_transcript(SessionTranscriptRequest {
                    session_id: session_id.clone(),
                    turn_id: None,
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?;

            println!("Session Details\n");
            println!("Name: {}", restored.session.session_name);
            println!("ID: {}", restored.session.session_id);
            println!("Agent: {}", restored.session.agent_type);
            println!("State: {:?}", restored.state);
            println!("Messages: {}", transcript.messages.len());
            println!();

            if !transcript.messages.is_empty() {
                println!("Recent messages:");
                let recent: Vec<_> = transcript.messages.iter().rev().take(5).collect();
                for msg in recent.iter().rev() {
                    let role = transcript_role_label(&msg.role);
                    let content_preview = transcript_message_preview(msg);
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
            bitfun_agent_runtime::session_control::validate_session_id(&id)
                .map_err(anyhow::Error::msg)?;
            runtime
                .agent_runtime()
                .delete_session(bitfun_runtime_ports::AgentSessionDeleteRequest {
                    workspace_path: workspace_path.to_string_lossy().to_string(),
                    session_id: id.clone(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?;
            println!("Deleted session from current project: {}", id);
        }

        SessionAction::Resume { id } => {
            let session_id =
                resolve_cli_session_id(runtime.agent_runtime(), &workspace_path, &id).await?;
            return Ok(Some((session_id, runtime)));
        }

        SessionAction::Continue => {
            let session_id =
                resolve_cli_session_id(runtime.agent_runtime(), &workspace_path, "last").await?;
            return Ok(Some((session_id, runtime)));
        }

        SessionAction::Fork { id, id_only } => {
            let session_id =
                resolve_cli_session_id(runtime.agent_runtime(), &workspace_path, &id).await?;
            let result = runtime
                .agent_runtime()
                .fork_session(bitfun_agent_runtime::sdk::AgentSessionForkRequest {
                    workspace_path: workspace_path.to_string_lossy().to_string(),
                    source_session_id: session_id.clone(),
                    remote_connection_id: None,
                    remote_ssh_host: None,
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.into_message()))?;

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
    runtime: &bitfun_agent_runtime::sdk::AgentRuntime,
    workspace_path: &Path,
    id: &str,
) -> Result<String> {
    if id == "last" {
        let sessions = list_cli_sessions(runtime, workspace_path).await?;
        return sessions
            .first()
            .map(|session| session.session_id.clone())
            .ok_or_else(|| anyhow::anyhow!("No history sessions"));
    }

    bitfun_agent_runtime::session_control::validate_session_id(id).map_err(anyhow::Error::msg)?;
    Ok(id.to_string())
}

async fn list_cli_sessions(
    runtime: &bitfun_agent_runtime::sdk::AgentRuntime,
    workspace_path: &Path,
) -> Result<Vec<bitfun_runtime_ports::AgentSessionSummary>> {
    runtime
        .list_sessions(bitfun_runtime_ports::AgentSessionListRequest {
            workspace_path: workspace_path.to_string_lossy().to_string(),
            remote_connection_id: None,
            remote_ssh_host: None,
        })
        .await
        .map_err(|error| anyhow::anyhow!(error.into_message()))
}

pub(crate) fn handle_config_action(action: ConfigAction, config: &CliConfig) -> Result<()> {
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

pub(crate) fn handle_health_command() -> Result<()> {
    use std::sync::Arc;

    use bitfun_core::runtime_ports::PluginRuntimeAvailability;

    use crate::runtime::approval::{CliApprovalPolicy, CliPermissionService};
    use crate::runtime::services::{CliClock, CliRuntimeEventSink, CliRuntimeServicesProvider};

    let workspace = std::env::current_dir().context("Failed to resolve current directory")?;
    let services = CliRuntimeServicesProvider::new(
        &workspace,
        Arc::new(CliPermissionService::new(CliApprovalPolicy::Reject)),
        Arc::new(CliRuntimeEventSink::new(16)),
        Arc::new(CliClock),
    )?
    .build()?;
    let product_runtime = crate::product_assembly::assemble_cli_runtime_parts(services)?;

    println!("BitFun CLI health");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!(
        "Product runtime: {} assembly-ready",
        product_runtime.plan().profile().id()
    );
    println!("Runtime capability registrations: complete");
    println!("Execution owner: bitfun-core compatibility");
    match product_runtime.plugin_runtime().availability() {
        PluginRuntimeAvailability::Disabled { reason } => {
            println!("Plugin runtime: disabled ({reason})");
        }
        PluginRuntimeAvailability::ProjectionOnly { reason } => {
            println!("Plugin runtime: projection-only ({reason})");
        }
        PluginRuntimeAvailability::Unavailable { reason } => {
            println!("Plugin runtime: unavailable ({reason})");
        }
        PluginRuntimeAvailability::Available => println!("Plugin runtime: available"),
        _ => println!("Plugin runtime: unknown"),
    }
    println!("Config directory: {:?}", CliConfig::config_dir()?);
    Ok(())
}

pub(crate) async fn serve_acp_stdio() -> Result<()> {
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

    let workspace_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let runtime = crate::runtime::AcpRuntimeContext::build(agentic_system, workspace_root)?;
    let (agent_runtime, compatibility) = runtime.parts();
    bitfun_acp::BitfunAcpRuntime::serve_stdio(agent_runtime, compatibility).await?;
    Ok(())
}
