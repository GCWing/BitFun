use anyhow::{anyhow, bail, Context, Result};
use bitfun_acp::client::{
    AcpClientConfig, AcpClientInfo, AcpClientPermissionMode, AcpClientRequirementProbe,
};
use bitfun_acp::AcpClientService;
use clap::ValueEnum;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::config::CliConfig;

#[derive(Clone, Debug, ValueEnum)]
pub enum AcpConfigClient {
    Zed,
    Generic,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ExternalAcpClient {
    Opencode,
    ClaudeCode,
    Codex,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliAcpPermissionMode {
    Ask,
    AllowOnce,
    RejectOnce,
}

impl ExternalAcpClient {
    fn id(self) -> &'static str {
        match self {
            Self::Opencode => "opencode",
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Opencode => "opencode",
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    fn config(self) -> AcpClientConfig {
        let (command, args) = match self {
            Self::Opencode => ("opencode", vec!["acp"]),
            Self::ClaudeCode => (
                "npx",
                vec!["--yes", "@zed-industries/claude-code-acp@latest"],
            ),
            Self::Codex => ("npx", vec!["--yes", "@zed-industries/codex-acp@latest"]),
        };
        AcpClientConfig {
            name: Some(self.display_name().to_string()),
            command: command.to_string(),
            args: args.into_iter().map(ToString::to_string).collect(),
            env: HashMap::new(),
            enabled: true,
            readonly: false,
            permission_mode: AcpClientPermissionMode::Ask,
        }
    }
}

impl CliAcpPermissionMode {
    fn to_config_mode(self) -> AcpClientPermissionMode {
        match self {
            Self::Ask => AcpClientPermissionMode::Ask,
            Self::AllowOnce => AcpClientPermissionMode::AllowOnce,
            Self::RejectOnce => AcpClientPermissionMode::RejectOnce,
        }
    }
}

pub fn print_status(command: &str) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to resolve current directory")?;
    let config_dir =
        CliConfig::config_dir().context("Failed to resolve BitFun config directory")?;

    println!("BitFun ACP");
    println!("Status: available");
    println!("Protocol: Agent Client Protocol v1 over stdio");
    println!("Server command: {}", shell_command(command));
    println!("Working directory: {}", cwd.display());
    println!("Config directory: {}", config_dir.display());
    println!();
    println!("Capabilities:");
    println!("- Sessions: create, list, load");
    println!("- Prompts: text, images, embedded context");
    println!("- MCP: HTTP-capable remote server declarations");
    println!("- Session controls: mode, model, config options");
    println!();
    println!("Run `{} acp doctor` to check local readiness.", command);
    Ok(())
}

pub async fn print_doctor(command: &str) -> Result<bool> {
    let mut checks = Vec::new();

    checks.push(check_result(
        "Current working directory",
        std::env::current_dir()
            .map(|path| path.display().to_string())
            .map_err(|error| error.to_string()),
    ));

    checks.push(check_result(
        "BitFun config directory",
        CliConfig::config_dir()
            .map(|path| path.display().to_string())
            .map_err(|error| error.to_string()),
    ));

    let core_config = bitfun_core::service::config::initialize_global_config()
        .await
        .map(|_| "initialized".to_string())
        .map_err(|error| error.to_string());
    checks.push(check_result("Core config service", core_config));

    let ai_check = match bitfun_core::service::config::get_global_config_service().await {
        Ok(service) => {
            let ai_config: bitfun_core::service::config::types::AIConfig =
                service.get_config(Some("ai")).await.unwrap_or_default();
            let enabled_models = ai_config
                .models
                .iter()
                .filter(|model| model.enabled)
                .count();
            if enabled_models > 0 {
                Check::ok(
                    "Enabled AI models",
                    format!("{} enabled model(s)", enabled_models),
                )
            } else {
                Check::warning(
                    "Enabled AI models",
                    "No enabled model found; ACP can start, but prompts will fail until a model is configured.",
                )
            }
        }
        Err(error) => Check::warning(
            "Enabled AI models",
            format!("Skipped because config service is unavailable: {}", error),
        ),
    };
    checks.push(ai_check);

    println!("BitFun ACP doctor");
    println!();

    let mut has_error = false;
    let mut has_warning = false;
    for check in &checks {
        match check.level {
            CheckLevel::Ok => println!("[ok]      {}: {}", check.name, check.detail),
            CheckLevel::Warning => {
                has_warning = true;
                println!("[warning] {}: {}", check.name, check.detail);
            }
            CheckLevel::Error => {
                has_error = true;
                println!("[error]   {}: {}", check.name, check.detail);
            }
        }
    }

    println!();
    if has_error {
        println!(
            "ACP server is not ready. Fix the errors above, then run `{} acp doctor` again.",
            command
        );
    } else if has_warning {
        println!("ACP server can start, but prompts may not complete until warnings are resolved.");
    } else {
        println!(
            "ACP server is ready. Configure your ACP client to run `{}`.",
            shell_command(command)
        );
    }

    Ok(!has_error)
}

pub fn print_config(client: AcpConfigClient, command: &str) -> Result<()> {
    match client {
        AcpConfigClient::Zed => print_zed_config(command),
        AcpConfigClient::Generic => print_generic_config(command),
    }
}

pub fn acp_help_text(command: &str) -> String {
    format!(
        "\
Agent Client Protocol (ACP)\n\
─────────────────────────────────\n\
BitFun exposes its agent runtime as an ACP server over stdio.\n\
BitFun CLI can also launch external ACP agents such as opencode, Claude Code, and Codex.\n\
\n\
Use this from an ACP-compatible editor or host:\n\
  {command} acp\n\
\n\
Human-facing helper commands:\n\
  {command} acp status\n\
  {command} acp doctor\n\
  {command} acp config --client zed\n\
  {command} acp clients list\n\
  {command} acp clients doctor\n\
  {command} acp clients enable opencode\n\
  {command} acp run opencode \"review this repository\"\n\
\n\
Notes:\n\
- The plain `acp` command reserves stdout for JSON-RPC protocol traffic.\n\
- Logs for the ACP server are written to stderr.\n\
- Run the command from the project directory you want BitFun to operate on.",
        command = command
    )
}

pub async fn list_external_clients() -> Result<()> {
    let service = create_client_service().await?;
    let infos = service
        .list_clients()
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let configured = infos
        .into_iter()
        .map(|info| (info.id.clone(), info))
        .collect::<BTreeMap<_, _>>();

    println!("External ACP agents");
    println!();
    for client in [
        ExternalAcpClient::Opencode,
        ExternalAcpClient::ClaudeCode,
        ExternalAcpClient::Codex,
    ] {
        if let Some(info) = configured.get(client.id()) {
            print_client_info(info);
        } else {
            let preset = client.config();
            println!(
                "- {}: not configured ({})",
                client.id(),
                render_command(&preset.command, &preset.args)
            );
        }
    }

    for info in configured.values() {
        if !matches!(info.id.as_str(), "opencode" | "claude-code" | "codex") {
            print_client_info(info);
        }
    }

    println!();
    println!("Enable a built-in client with `bitfun-cli acp clients enable opencode`.");
    println!("Run a prompt with `bitfun-cli acp run opencode \"your task\"`.");
    Ok(())
}

pub async fn doctor_external_clients() -> Result<bool> {
    let service = create_client_service().await?;
    let probes = service
        .probe_client_requirements(None, true)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;

    println!("External ACP agent doctor");
    println!();

    let mut has_runnable = false;
    for probe in probes {
        if probe.runnable {
            has_runnable = true;
        }
        print_requirement_probe(&probe);
    }

    println!();
    if has_runnable {
        println!("At least one external ACP agent is runnable.");
    } else {
        println!(
            "No external ACP agent is runnable yet. Install opencode, Claude Code, or Codex first."
        );
    }
    Ok(has_runnable)
}

pub async fn enable_external_client(
    client: ExternalAcpClient,
    permission: CliAcpPermissionMode,
) -> Result<()> {
    let service = create_client_service().await?;
    let config_json = update_client_config_json(
        load_client_config_json(&service).await?,
        client.id(),
        Some(client.config()),
        true,
        Some(permission.to_config_mode()),
    )?;
    save_client_config_json(&service, &config_json).await?;
    println!(
        "Enabled external ACP agent '{}' with permission mode {:?}.",
        client.id(),
        permission
    );
    Ok(())
}

pub async fn disable_external_client(client_id: &str) -> Result<()> {
    let service = create_client_service().await?;
    let current = load_client_config_json(&service).await?;
    if client_entry(&current, client_id).is_none() {
        bail!("ACP client '{}' is not configured", client_id);
    }
    let config_json = update_client_config_json(current, client_id, None, false, None)?;
    save_client_config_json(&service, &config_json).await?;
    println!("Disabled external ACP agent '{}'.", client_id);
    Ok(())
}

pub async fn print_external_client_config() -> Result<()> {
    let service = create_client_service().await?;
    let json = service
        .load_json_config()
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    println!("{}", json);
    Ok(())
}

pub async fn run_external_client(
    client: ExternalAcpClient,
    prompt: String,
    workspace: Option<String>,
    timeout: u64,
    permission: CliAcpPermissionMode,
) -> Result<()> {
    if matches!(permission, CliAcpPermissionMode::Ask) {
        bail!(
            "`--permission ask` is not available for non-interactive `acp run`; use allow-once or reject-once."
        );
    }

    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        bail!("Prompt cannot be empty");
    }

    let service = create_client_service().await?;
    let client_id = client.id();
    let base_config_json = update_client_config_json(
        load_client_config_json(&service).await?,
        client_id,
        Some(client.config()),
        true,
        None,
    )?;
    save_client_config_json(&service, &base_config_json).await?;

    let run_config_json = update_client_config_json(
        base_config_json.clone(),
        client_id,
        None,
        true,
        Some(permission.to_config_mode()),
    )?;
    if run_config_json != base_config_json {
        save_client_config_json(&service, &run_config_json).await?;
    }

    let workspace_path = match workspace {
        Some(path) => path,
        None => std::env::current_dir()
            .context("Failed to resolve current directory")?
            .to_string_lossy()
            .to_string(),
    };
    let session_id = format!("cli_acp_{}_{}", client_id, uuid::Uuid::new_v4());

    eprintln!(
        "Starting external ACP agent '{}' in {}",
        client_id, workspace_path
    );
    let result = service
        .prompt_agent(
            client_id,
            prompt,
            Some(workspace_path),
            None,
            session_id,
            None,
            Some(timeout),
        )
        .await
        .map_err(|error| anyhow!(error.to_string()));

    let _ = service.stop_client(client_id).await;
    if run_config_json != base_config_json {
        let _ = save_client_config_json(&service, &base_config_json).await;
    }

    let output = result?;
    if output.trim().is_empty() {
        println!("External ACP agent completed without text output.");
    } else {
        println!("{}", output);
    }
    Ok(())
}

async fn create_client_service() -> Result<Arc<AcpClientService>> {
    bitfun_core::service::config::initialize_global_config()
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let config_service = bitfun_core::service::config::get_global_config_service()
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let path_manager = bitfun_core::infrastructure::try_get_path_manager_arc()
        .map_err(|error| anyhow!(error.to_string()))?;
    let service = AcpClientService::new(config_service, path_manager)
        .map_err(|error| anyhow!(error.to_string()))?;
    service
        .initialize_all()
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(service)
}

async fn load_client_config_json(service: &Arc<AcpClientService>) -> Result<Value> {
    let json = service
        .load_json_config()
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    serde_json::from_str(&json).context("Failed to parse ACP client config")
}

async fn save_client_config_json(service: &Arc<AcpClientService>, value: &Value) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    service
        .save_json_config(&json)
        .await
        .map_err(|error| anyhow!(error.to_string()))
}

fn update_client_config_json(
    mut value: Value,
    client_id: &str,
    default_config: Option<AcpClientConfig>,
    enabled: bool,
    permission_mode: Option<AcpClientPermissionMode>,
) -> Result<Value> {
    ensure_acp_clients_object(&mut value)?;
    let acp_clients = value
        .get_mut("acpClients")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("ACP client config must contain an acpClients object"))?;

    let entry = acp_clients.entry(client_id.to_string()).or_insert_with(|| {
        default_config
            .as_ref()
            .and_then(|config| serde_json::to_value(config).ok())
            .unwrap_or_else(|| json!({}))
    });
    if !entry.is_object() {
        *entry = json!({});
    }
    let entry_object = entry
        .as_object_mut()
        .ok_or_else(|| anyhow!("ACP client '{}' config must be an object", client_id))?;

    if let Some(default_config) = default_config {
        let default_value = serde_json::to_value(default_config)?;
        let default_object = default_value
            .as_object()
            .ok_or_else(|| anyhow!("Default ACP client config must be an object"))?;
        for (key, value) in default_object {
            entry_object
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }

    entry_object.insert("enabled".to_string(), json!(enabled));
    if let Some(permission_mode) = permission_mode {
        entry_object.insert(
            "permissionMode".to_string(),
            serde_json::to_value(permission_mode)?,
        );
    }

    Ok(value)
}

fn ensure_acp_clients_object(value: &mut Value) -> Result<()> {
    if value.get("acpClients").is_none() {
        if value.is_object() {
            let map = value.as_object_mut().expect("object checked").clone();
            *value = json!({ "acpClients": map });
        } else {
            *value = json!({ "acpClients": {} });
        }
    }
    if !value
        .get("acpClients")
        .map(Value::is_object)
        .unwrap_or(false)
    {
        bail!("ACP client config must contain an object at acpClients");
    }
    Ok(())
}

fn client_entry<'a>(value: &'a Value, client_id: &str) -> Option<&'a Value> {
    value.get("acpClients")?.as_object()?.get(client_id)
}

fn print_client_info(info: &AcpClientInfo) {
    println!(
        "- {}: {} / {:?} / {:?} ({})",
        info.id,
        if info.enabled { "enabled" } else { "disabled" },
        info.status,
        info.permission_mode,
        render_command(&info.command, &info.args)
    );
}

fn print_requirement_probe(probe: &AcpClientRequirementProbe) {
    let status = if probe.runnable { "ok" } else { "missing" };
    println!("- {}: {}", probe.id, status);
    print_requirement_item("tool", &probe.tool);
    if let Some(adapter) = &probe.adapter {
        print_requirement_item("adapter", adapter);
    }
    for note in &probe.notes {
        println!("  note: {}", note);
    }
}

fn print_requirement_item(label: &str, item: &bitfun_acp::client::AcpRequirementProbeItem) {
    let installed = if item.installed {
        "installed"
    } else {
        "missing"
    };
    let mut details = Vec::new();
    if let Some(version) = item.version.as_ref().filter(|value| !value.is_empty()) {
        details.push(format!("version {}", version));
    }
    if let Some(path) = item.path.as_ref().filter(|value| !value.is_empty()) {
        details.push(path.clone());
    }
    if let Some(error) = item.error.as_ref().filter(|value| !value.is_empty()) {
        details.push(error.clone());
    }
    if details.is_empty() {
        println!("  {} {}: {}", label, item.name, installed);
    } else {
        println!(
            "  {} {}: {} ({})",
            label,
            item.name,
            installed,
            details.join(", ")
        );
    }
}

fn render_command(command: &str, args: &[String]) -> String {
    std::iter::once(command.to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn print_zed_config(command: &str) -> Result<()> {
    println!("Zed settings JSON");
    println!();
    let snippet = json!({
        "agent_servers": {
            "BitFun": {
                "command": command,
                "args": ["acp"]
            }
        }
    });
    println!("{}", serde_json::to_string_pretty(&snippet)?);
    println!();
    println!(
        "Use an absolute command path if your editor cannot find `{}` on PATH.",
        command
    );
    Ok(())
}

fn print_generic_config(command: &str) -> Result<()> {
    println!("Generic ACP stdio configuration");
    println!();
    println!("Command: {}", command);
    println!("Arguments: acp");
    println!("Transport: stdio");
    println!("Working directory: project root");
    println!();
    println!("JSON shape:");
    let snippet = json!({
        "name": "BitFun",
        "transport": "stdio",
        "command": command,
        "args": ["acp"]
    });
    println!("{}", serde_json::to_string_pretty(&snippet)?);
    Ok(())
}

fn shell_command(command: &str) -> String {
    format!("{} acp", command)
}

fn check_result(name: &'static str, result: std::result::Result<String, String>) -> Check {
    match result {
        Ok(detail) => Check::ok(name, detail),
        Err(error) => Check::error(name, error),
    }
}

struct Check {
    name: &'static str,
    detail: String,
    level: CheckLevel,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            detail: detail.into(),
            level: CheckLevel::Ok,
        }
    }

    fn warning(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            detail: detail.into(),
            level: CheckLevel::Warning,
        }
    }

    fn error(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            detail: detail.into(),
            level: CheckLevel::Error,
        }
    }
}

enum CheckLevel {
    Ok,
    Warning,
    Error,
}
