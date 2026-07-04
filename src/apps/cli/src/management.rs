use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::time::Duration;

use bitfun_core::agentic::get_agent_registry;
use bitfun_core::agentic::persistence::PersistenceManager;
use bitfun_core::infrastructure::try_get_path_manager_arc;
use bitfun_core::service::config::initialize_global_config;
use bitfun_core::service::session_usage::{
    generate_session_usage_report, render_usage_report_markdown, SessionUsageReportRequest,
};

async fn ensure_global_config_service(
) -> Result<std::sync::Arc<bitfun_core::service::config::ConfigService>> {
    initialize_global_config()
        .await
        .context("Failed to initialize global config service")?;
    bitfun_core::service::config::get_global_config_service()
        .await
        .context("Failed to get global config service")
}

pub async fn print_agents(workspace: Option<&Path>) -> Result<()> {
    let registry = get_agent_registry();
    let modes = registry.get_modes_info().await;
    let subagents = registry.get_subagents_info(workspace).await;

    println!("Agent modes");
    println!();
    if modes.is_empty() {
        println!("No agent modes found.");
    } else {
        for agent in modes {
            println!(
                "- {}: {} (tools: {}, readonly: {}, review: {})",
                agent.id, agent.name, agent.tool_count, agent.is_readonly, agent.is_review
            );
            if !agent.description.is_empty() {
                println!("  {}", agent.description);
            }
        }
    }

    println!();
    println!("Subagents");
    println!();
    if subagents.is_empty() {
        println!("No subagents found for the current workspace.");
    } else {
        for agent in subagents {
            println!(
                "- {}: {} (tools: {}, enabled: {}, readonly: {}, review: {})",
                agent.id,
                agent.name,
                agent.tool_count,
                agent.effective_enabled,
                agent.is_readonly,
                agent.is_review
            );
            if !agent.description.is_empty() {
                println!("  {}", agent.description);
            }
        }
    }

    Ok(())
}

pub async fn print_models() -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let models = config_service.get_ai_models().await?;
    let global_config: bitfun_core::service::config::GlobalConfig =
        config_service.get_config(None).await?;

    let primary_model_id = global_config.ai.default_models.primary.clone();

    println!("AI models");
    println!();
    if models.is_empty() {
        println!("No AI models configured.");
        return Ok(());
    }

    for model in models {
        let is_primary = primary_model_id.as_deref() == Some(model.id.as_str());
        let current_modes: Vec<String> = global_config
            .ai
            .agent_models
            .iter()
            .filter_map(|(mode, model_id)| (model_id == &model.id).then_some(mode.clone()))
            .collect();

        println!(
            "- {}{} ({})",
            if is_primary { "* " } else { "  " },
            model.id,
            if model.enabled { "enabled" } else { "disabled" }
        );
        println!("  Name: {}", model.name);
        println!("  Provider: {}", model.provider);
        println!("  Model: {}", model.model_name);
        if !current_modes.is_empty() {
            println!("  Used by modes: {}", current_modes.join(", "));
        }
    }

    Ok(())
}

pub async fn print_mcp_servers() -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    let configs = mcp_service.config_service().load_all_configs().await?;

    println!("MCP servers");
    println!();
    if configs.is_empty() {
        println!("No MCP servers configured.");
        return Ok(());
    }

    for config in configs {
        let status = if config.enabled {
            match tokio::time::timeout(
                Duration::from_millis(30),
                mcp_service.server_manager().get_server_status(&config.id),
            )
            .await
            {
                Ok(Ok(status)) => format!("{:?}", status),
                Ok(Err(_)) => "Unknown".to_string(),
                Err(_) => "Starting".to_string(),
            }
        } else {
            "Disabled".to_string()
        };

        let endpoint = match config.server_type {
            bitfun_core::service::mcp::server::MCPServerType::Local => config
                .command
                .as_ref()
                .map(|cmd| format!("{} {}", cmd, config.args.join(" ")))
                .unwrap_or_else(|| "<missing command>".to_string()),
            bitfun_core::service::mcp::server::MCPServerType::Remote => config
                .url
                .clone()
                .unwrap_or_else(|| "<missing url>".to_string()),
        };

        println!("- {} ({:?})", config.id, config.server_type);
        println!("  Name: {}", config.name);
        println!("  Status: {}", status);
        println!("  Enabled: {}", config.enabled);
        println!("  Endpoint: {}", endpoint);
    }

    Ok(())
}

pub async fn set_default_model(model_id: &str) -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let agent_registry = get_agent_registry();
    let modes = agent_registry.get_modes_info().await;

    config_service
        .set_config("ai.default_models.primary", model_id)
        .await?;
    for mode in modes {
        let path = format!("ai.agent_models.{}", mode.id);
        config_service.set_config(&path, model_id).await?;
    }

    println!("Default model set to: {}", model_id);
    Ok(())
}

pub async fn set_mcp_server_enabled(server_id: &str, enabled: bool) -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    let mut config = mcp_service
        .config_service()
        .get_server_config(server_id)
        .await?
        .ok_or_else(|| anyhow!("MCP server not found: {}", server_id))?;
    config.enabled = enabled;
    mcp_service
        .config_service()
        .save_server_config(&config)
        .await?;

    println!(
        "MCP server {} {}.",
        server_id,
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

pub async fn print_mcp_json_config() -> Result<()> {
    let config_service = ensure_global_config_service().await?;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    let json = mcp_service.config_service().load_mcp_json_config().await?;
    println!("{}", json);
    Ok(())
}

pub async fn print_usage_report(session_id: Option<&str>) -> Result<()> {
    let agentic_system = crate::agent::agentic_system::init_agentic_system_for_cli().await?;
    let path_manager = try_get_path_manager_arc().map_err(|error| anyhow!(error.to_string()))?;
    let persistence_manager =
        PersistenceManager::new(path_manager).map_err(|error| anyhow!(error.to_string()))?;
    let workspace_path = std::env::current_dir().context("Failed to resolve current directory")?;
    let coordinator = agentic_system.coordinator.clone();
    let resolved_session_id = match session_id {
        Some(session_id) if !session_id.trim().is_empty() => session_id.to_string(),
        _ => coordinator
            .list_sessions(&workspace_path)
            .await?
            .first()
            .map(|session| session.session_id.clone())
            .ok_or_else(|| anyhow!("No history sessions for current project"))?,
    };

    let report = generate_session_usage_report(
        &persistence_manager,
        Some(agentic_system.token_usage_service.as_ref()),
        SessionUsageReportRequest {
            session_id: resolved_session_id,
            workspace_path: Some(workspace_path.to_string_lossy().to_string()),
            remote_connection_id: None,
            remote_ssh_host: None,
            include_hidden_subagents: true,
        },
    )
    .await
    .map_err(|error| anyhow!(error.to_string()))?;

    println!("{}", render_usage_report_markdown(&report));
    Ok(())
}

pub async fn print_doctor() -> Result<bool> {
    let workspace = std::env::current_dir().context("Failed to resolve current directory")?;
    let config_dir = crate::config::CliConfig::config_dir()?;
    let config_service = ensure_global_config_service().await?;
    let models = config_service.get_ai_models().await?;
    let agent_registry = get_agent_registry();
    let modes = agent_registry.get_modes_info().await;
    let subagents = agent_registry
        .get_subagents_info(Some(workspace.as_path()))
        .await;
    let mcp_service = bitfun_core::service::mcp::MCPService::new(config_service.clone())
        .map_err(|error| anyhow!(error.to_string()))?;
    let mcp_configs = mcp_service.config_service().load_all_configs().await?;

    println!("BitFun CLI doctor");
    println!();
    println!("[ok] Workspace: {}", workspace.display());
    println!("[ok] Config directory: {}", config_dir.display());
    println!("[ok] Agent modes: {}", modes.len());
    println!("[ok] Subagents: {}", subagents.len());
    println!(
        "[ok] AI models: {} total, {} enabled",
        models.len(),
        models.iter().filter(|m| m.enabled).count()
    );
    println!("[ok] MCP servers: {}", mcp_configs.len());
    println!();
    println!("Doctor checks passed.");
    Ok(true)
}
