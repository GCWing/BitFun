use bitfun_agent_runtime::native_hooks::RuntimeHookCommitToken;
use bitfun_opencode_plugin_host::{
    PluginDeclaration, PluginHost, PluginHostConfig, PluginHostShutdownPolicy,
    PluginHostShutdownReport, PluginInstanceOpenRequest, PluginPrepareRequest, RpcHandlerError,
    CONFIG_CONTRIBUTIONS_V2, CONFIG_CONTRIBUTORS_V1, GENERATION_FENCING_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
// Product-assembly bridge for the managed OpenCode Plugin Host.
//
// `PluginHost` itself remains the adapter-owned process/IPC resource. Core
// keeps only the product-level lifecycle assembly and logical instance/PTy
// ownership needed to bind adapter callbacks to BitFun owners; these maps do
// not supervise a physical process tree or make trust/configuration policy.

use terminal_core::{CloseSessionRequest, TerminalApi};
use tokio::sync::{Mutex, Notify, OnceCell};

const BUN_HOST_ENTRY_ENV: &str = "BITFUN_OPENCODE_BUN_HOST_ENTRY";
const BUN_COMMAND_ENV: &str = "BITFUN_BUN_COMMAND";
static PLUGIN_HOST: OnceCell<Mutex<Option<PluginHost>>> = OnceCell::const_new();
static PLUGIN_HOST_SHUTDOWN_REPORT: OnceCell<Mutex<Option<PluginHostShutdownReport>>> =
    OnceCell::const_new();
static PLUGIN_HOST_SHUTDOWN_NOTIFY: OnceCell<Notify> = OnceCell::const_new();
static PLUGIN_HOST_SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);
static PLUGIN_HOST_SHUTDOWN_COMPLETE: AtomicBool = AtomicBool::new(false);
static PLUGIN_HOST_INSTANCES: OnceCell<Mutex<HashMap<String, PluginHostInstance>>> =
    OnceCell::const_new();
static PLUGIN_HOST_PTY_OWNERS: OnceCell<Mutex<HashMap<String, String>>> = OnceCell::const_new();
static NEXT_INSTANCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_PLUGIN_HOST_DIAGNOSTICS: usize = 100;

#[derive(Debug, Clone)]
pub(crate) struct PluginHostInstance {
    pub(crate) canonical_directory: String,
    pub(crate) directory: PathBuf,
    pub(crate) worktree: PathBuf,
    pub(crate) project_id: String,
    pub(crate) created_at_ms: i64,
    pub(crate) instance_id: String,
    pub(crate) generation_key: String,
    pub(crate) revision: String,
    pub(crate) open_result: Value,
    pub(crate) ready: bool,
    pub(crate) hook_commit_token: Option<RuntimeHookCommitToken>,
    pub(crate) transformed_config_health_snapshot: Option<Value>,
    pub(crate) diagnostic_health_snapshot: Vec<Value>,
    pub(crate) tool_names: Vec<String>,
    pub(crate) agent_runtime_keys: Vec<String>,
    pub(crate) retirement_scheduled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginHostDiagnostic {
    severity: String,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginHostDiagnosticPublishParams {
    #[serde(rename = "instanceID")]
    instance_id: Option<String>,
    diagnostic: PluginHostDiagnostic,
}

impl PluginHostInstance {
    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }
}

#[derive(Debug, Clone, Copy)]
struct PluginHostLaunchSpec {
    runtime_name: &'static str,
    default_command: &'static str,
    command_env: &'static str,
    entry_env: &'static str,
    entry_filename: &'static str,
}

impl PluginHostLaunchSpec {
    fn bun() -> Self {
        Self {
            runtime_name: "Bun",
            default_command: "bun",
            command_env: BUN_COMMAND_ENV,
            entry_env: BUN_HOST_ENTRY_ENV,
            entry_filename: "extension-host.js",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHostStartup {
    Disabled,
    Started,
    AlreadyStarted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHostLaunchPolicy {
    Enabled,
    Disabled,
}

pub async fn configured_plugins_present() -> crate::BitFunResult<bool> {
    use crate::service::config::{get_global_config_service, GlobalConfig};

    let config_service = get_global_config_service().await?;
    let config: GlobalConfig = config_service.get_config(None).await?;
    Ok(config.has_configured_plugins())
}

pub async fn initialize_configured_plugin_host(
    launch_policy: PluginHostLaunchPolicy,
) -> crate::BitFunResult<PluginHostStartup> {
    initialize_configured_plugin_host_with_log_file(launch_policy, None).await
}

pub async fn initialize_configured_plugin_host_with_log_file(
    launch_policy: PluginHostLaunchPolicy,
    log_file: Option<PathBuf>,
) -> crate::BitFunResult<PluginHostStartup> {
    use crate::service::config::{get_global_config_service, GlobalConfig};

    if launch_policy == PluginHostLaunchPolicy::Disabled {
        return Ok(PluginHostStartup::Disabled);
    }
    let config_service = get_global_config_service().await?;
    let config: GlobalConfig = config_service.get_config(None).await?;
    if !config.has_configured_plugins() {
        return Ok(PluginHostStartup::Disabled);
    }
    if PLUGIN_HOST_SHUTDOWN_STARTED.load(Ordering::Acquire) {
        return Err(crate::BitFunError::ProcessError(
            "Plugin host is shutting down".to_string(),
        ));
    }
    let launch_spec = PluginHostLaunchSpec::bun();

    let host_state = PLUGIN_HOST.get_or_init(|| async { Mutex::new(None) }).await;
    let mut host_state = host_state.lock().await;
    if PLUGIN_HOST_SHUTDOWN_STARTED.load(Ordering::Acquire) {
        return Err(crate::BitFunError::ProcessError(
            "Plugin host is shutting down".to_string(),
        ));
    }
    if host_state.is_some() {
        return Ok(PluginHostStartup::AlreadyStarted);
    }
    let path_manager = crate::infrastructure::try_get_path_manager_arc()?;
    let log_file = log_file.unwrap_or_else(|| path_manager.logs_dir().join("plugin-host.log"));
    let entry = resolve_host_entry(launch_spec)?;
    let working_directory = entry.parent().ok_or_else(|| {
        crate::BitFunError::config(format!(
            "{} plugin host entry has no parent directory: {}",
            launch_spec.runtime_name,
            entry.display()
        ))
    })?;
    let host = PluginHost::start(PluginHostConfig {
        runtime_command: std::env::var_os(launch_spec.command_env)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(launch_spec.default_command)),
        entry: entry.clone(),
        working_directory: working_directory.to_path_buf(),
        cache_directory: path_manager.cache_root().join("opencode-plugin-host"),
        log_file,
        log_level: config.app.logging.level.trim().to_lowercase(),
    })
    .await
    .map_err(|error| crate::BitFunError::ProcessError(format!(
            "Failed to initialize {} plugin host from {}: {error}",
            launch_spec.runtime_name,
            entry.display()
        )))?;
    let client = host.client();
    crate::plugin_host_http::register_plugin_host_backend_handlers(client.clone()).await?;
    let plugins = config
        .plugin
        .iter()
        .filter_map(plugin_declaration)
        .collect::<Vec<_>>();
    let configuration_fingerprint = plugin_config_fingerprint(&config)?;
    *host_state = Some(host);
    tokio::spawn(async move {
        let plugin_count = plugins.len();
        log::info!(
            "Configured plugin host background prewarm started: generation={}, plugin_count={}",
            client.generation(),
            plugin_count
        );
        match client
            .prepare_plugins(
                PluginPrepareRequest {
                    plugins,
                    configuration_fingerprint: Some(configuration_fingerprint),
                    default_base_directory: None,
                },
                std::time::Duration::from_secs(120),
            )
            .await
        {
            Ok(result) => {
                let prepared_count = result
                    .get("prepared")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                let failed_count = result
                    .get("failed")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                log::info!(
                    "Configured plugin host background prewarm completed: generation={}, plugin_count={}, prepared_count={}, failed_count={}",
                    client.generation(),
                    plugin_count,
                    prepared_count,
                    failed_count
                );
            }
            Err(error) => {
                log::warn!(
                    "Configured plugin host background prewarm failed: generation={}, plugin_count={}, error={}",
                    client.generation(),
                    plugin_count,
                    error
                );
            }
        }
    });
    Ok(PluginHostStartup::Started)
}

pub async fn set_configured_plugin_host_log_level(level: &str) -> crate::BitFunResult<()> {
    let host_state = PLUGIN_HOST.get_or_init(|| async { Mutex::new(None) }).await;
    let client = host_state.lock().await.as_ref().map(PluginHost::client);
    let Some(client) = client else {
        return Ok(());
    };
    client.set_log_level(level).await.map_err(|error| {
        crate::BitFunError::ProcessError(format!(
            "Failed to update plugin host log level to {}: {}",
            level, error
        ))
    })
}

pub async fn ensure_configured_plugin_instance(
    launch_policy: PluginHostLaunchPolicy,
    directory: PathBuf,
    worktree: PathBuf,
    project_id: Option<String>,
) -> crate::BitFunResult<Option<Value>> {
    use crate::service::config::{get_global_config_service, GlobalConfig};

    if launch_policy == PluginHostLaunchPolicy::Disabled {
        withdraw_configured_plugin_workspace(&directory).await;
        return Ok(None);
    }
    let config_service = get_global_config_service().await?;
    let global_config: GlobalConfig = config_service.get_config(None).await?;
    if !global_config.has_configured_plugins() {
        withdraw_configured_plugin_workspace(&directory).await;
        return Ok(None);
    }
    if directory.as_os_str().is_empty() || !directory.is_dir() {
        return Err(crate::BitFunError::Validation(format!(
            "Plugin host instance directory does not exist: {}",
            directory.display()
        )));
    }

    let canonical_directory = dunce::canonicalize(&directory).map_err(|error| {
        crate::BitFunError::Io(std::io::Error::other(format!(
            "Failed to canonicalize plugin host instance directory {}: {error}",
            directory.display()
        )))
    })?;
    let canonical_directory_string = canonical_directory.to_string_lossy().into_owned();
    let comparable_directory = comparable_instance_directory(&canonical_directory_string);
    let config = serde_json::to_value(
        crate::plugin_runtime::opencode_config_snapshot(&canonical_directory).map_err(|error| {
            crate::BitFunError::Validation(format!(
                "Failed to load OpenCode config for plugin activation: {error}"
            ))
        })?,
    )
    .and_then(|value| match value {
        Value::Object(config) => Ok(config),
        _ => unreachable!("OpenCodeConfigSnapshot must serialize as an object"),
    })
    .map_err(|error| {
        crate::BitFunError::Validation(format!(
            "Failed to serialize OpenCode plugin config snapshot: {error}"
        ))
    })?;
    let initial_config = config.clone();
    let config_fingerprint = plugin_config_fingerprint(&global_config)?;
    let client = {
        let host_state = PLUGIN_HOST.get_or_init(|| async { Mutex::new(None) }).await;
        host_state
            .lock()
            .await
            .as_ref()
            .map(PluginHost::client)
            .ok_or_else(|| {
                crate::BitFunError::ProcessError(
                    "Configured plugin host is not running".to_string(),
                )
            })?
    };
    if !client.capabilities().supports(GENERATION_FENCING_V1) {
        return Err(crate::BitFunError::ProcessError(
            "Configured plugin host does not support generation-fencing-v1".to_string(),
        ));
    }
    let instances = PLUGIN_HOST_INSTANCES
        .get_or_init(|| async { Mutex::new(HashMap::new()) })
        .await;
    let instance_key = format!("{comparable_directory}\n{config_fingerprint}");
    let reusable_instance = {
        let mut state = instances.lock().await;
        state.get_mut(&instance_key).map(|instance| {
            instance.retirement_scheduled = false;
            instance.clone()
        })
    };
    if let Some(instance) = reusable_instance {
        if crate::plugin_config_projection::active_generation_key(&canonical_directory).as_deref()
            != Some(instance.generation_key.as_str())
        {
            let projection = crate::plugin_config_projection::prepare(
                &canonical_directory,
                &instance.generation_key,
                &initial_config,
                &instance.open_result,
            )?;
            crate::plugin_hook_bridge::commit_plugin_generation(
                &crate::native_hooks::plugin_hook_registry(&comparable_directory),
                &comparable_directory,
                instance.hook_commit_token.as_ref(),
            );
            projection.commit();
        }
        log::debug!(
            "Configured plugin host instance reused: generation={}, instance_id={}",
            client.generation(),
            instance.instance_id
        );
        retire_superseded_plugin_instances(
            &client,
            instances,
            &instance_key,
            &comparable_directory,
        )
        .await;
        return Ok(Some(instance.open_result.clone()));
    }

    let sequence = NEXT_INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let instance_id = format!("bitfun:host:{}:{sequence}", client.generation());
    let revision = format!("revision-{sequence}");
    let generation_key = format!(
        "host-{}:instance-{sequence}:sha256-{}",
        client.generation(),
        config_fingerprint
    );
    let project_id = project_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "bitfun-project-{}",
                hex::encode(Sha256::digest(canonical_directory_string.as_bytes()))
            )
        });
    let now_ms = chrono::Utc::now().timestamp_millis();
    let opening_context = PluginHostInstance {
        canonical_directory: comparable_directory.clone(),
        directory: canonical_directory.clone(),
        worktree: worktree.clone(),
        project_id: project_id.clone(),
        created_at_ms: now_ms,
        instance_id: instance_id.clone(),
        generation_key: generation_key.clone(),
        revision: revision.clone(),
        open_result: Value::Null,
        ready: false,
        hook_commit_token: None,
        transformed_config_health_snapshot: None,
        diagnostic_health_snapshot: Vec::new(),
        tool_names: Vec::new(),
        agent_runtime_keys: Vec::new(),
        retirement_scheduled: false,
    };
    instances
        .lock()
        .await
        .insert(instance_key.clone(), opening_context);
    let open_result = match client
        .open_instance(
            PluginInstanceOpenRequest {
                instance_id: instance_id.clone(),
                generation_key: generation_key.clone(),
                revision: revision.clone(),
                project: serde_json::json!({
                    "id": project_id,
                    "worktree": canonical_directory_string,
                    "time": {"created": now_ms},
                }),
                config,
                directory: canonical_directory.to_string_lossy().into_owned(),
                worktree: worktree.to_string_lossy().into_owned(),
                plugins: global_config
                    .plugin
                    .iter()
                    .filter_map(plugin_declaration)
                    .collect(),
                configuration_fingerprint: Some(config_fingerprint.clone()),
            },
            std::time::Duration::from_secs(30),
        )
        .await
    {
        Ok(result) => result,
        Err(error) => {
            discard_opening_plugin_instance(&client, instances, &instance_key, &instance_id).await;
            return Err(crate::BitFunError::ProcessError(format!(
                "Failed to activate plugins for workspace {}: {error}",
                canonical_directory.display()
            )));
        }
    };
    if let Err(error) =
        validate_open_generation_lease(&open_result, &instance_id, &generation_key, &revision)
    {
        discard_opening_plugin_instance(&client, instances, &instance_key, &instance_id).await;
        return Err(error);
    }
    if client.capabilities().supports(CONFIG_CONTRIBUTORS_V1)
        && !open_result
            .get("configContributors")
            .is_some_and(Value::is_array)
    {
        discard_opening_plugin_instance(&client, instances, &instance_key, &instance_id).await;
        return Err(crate::BitFunError::Validation(
            "Plugin host open result is missing configContributors".to_string(),
        ));
    }
    if client.capabilities().supports(CONFIG_CONTRIBUTIONS_V2)
        && !open_result
            .get("configContributions")
            .is_some_and(Value::is_array)
    {
        discard_opening_plugin_instance(&client, instances, &instance_key, &instance_id).await;
        return Err(crate::BitFunError::Validation(
            "Plugin host open result is missing configContributions".to_string(),
        ));
    }
    let config_projection = match crate::plugin_config_projection::prepare(
        &canonical_directory,
        &generation_key,
        &initial_config,
        &open_result,
    ) {
        Ok(projection) => projection,
        Err(error) => {
            discard_opening_plugin_instance(&client, instances, &instance_key, &instance_id).await;
            return Err(error);
        }
    };
    let plugin_agent_runtime_keys = config_projection.agent_runtime_keys();
    log::info!(
        "Configured plugin host instance prepared: generation={}, instance_id={}, plugin_count={}",
        client.generation(),
        instance_id,
        global_config.plugin.len()
    );
    let hook_commit_token = match crate::plugin_hook_bridge::register_plugin_hooks(
        &crate::native_hooks::plugin_hook_registry(&comparable_directory),
        &comparable_directory,
        client.clone(),
        &instance_id,
        &generation_key,
        &revision,
        &crate::plugin_hook_bridge::hook_names(&open_result),
    ) {
        Ok(token) => token,
        Err(error) => {
            discard_opening_plugin_instance(&client, instances, &instance_key, &instance_id).await;
            return Err(crate::BitFunError::ProcessError(format!(
                "Failed to register plugin hooks for workspace {}: {error}",
                canonical_directory.display()
            )));
        }
    };
    let tool_names = match register_plugin_tools(
        &client,
        &instance_id,
        &comparable_directory,
        &canonical_directory,
        &generation_key,
        &revision,
        &config_fingerprint,
        &open_result,
        &config_projection,
    )
    .await
    {
        Ok(names) => names,
        Err(error) => {
            if let Some(token) = hook_commit_token.clone() {
                crate::plugin_hook_bridge::unregister_plugin_hooks(
                    &crate::native_hooks::plugin_hook_registry(&comparable_directory),
                    &comparable_directory,
                    token,
                );
            }
            discard_opening_plugin_instance(&client, instances, &instance_key, &instance_id).await;
            return Err(error);
        }
    };
    // Publish readiness, Hooks, and Config routes while holding the instance
    // table lock. Hook dispatch cannot observe ready=true before its Registry
    // generation is active, and Agent routing is published last, after the
    // instance identity is available to generation-fenced dispatch.
    {
        let mut state = instances.lock().await;
        let instance = state.get_mut(&instance_key).ok_or_else(|| {
            crate::BitFunError::ProcessError(
                "Plugin instance disappeared before generation publication".to_string(),
            )
        })?;
        instance.open_result = open_result.clone();
        instance.ready = true;
        instance.hook_commit_token = hook_commit_token.clone();
        instance.transformed_config_health_snapshot = open_result.get("config").cloned();
        instance.tool_names = tool_names;
        instance.agent_runtime_keys = plugin_agent_runtime_keys.into_iter().collect();
        crate::plugin_hook_bridge::commit_plugin_generation(
            &crate::native_hooks::plugin_hook_registry(&comparable_directory),
            &comparable_directory,
            hook_commit_token.as_ref(),
        );
        config_projection.commit();
    }
    retire_superseded_plugin_instances(&client, instances, &instance_key, &comparable_directory)
        .await;
    Ok(Some(open_result))
}

async fn discard_opening_plugin_instance(
    client: &bitfun_opencode_plugin_host::PluginHostClient,
    instances: &Mutex<HashMap<String, PluginHostInstance>>,
    instance_key: &str,
    instance_id: &str,
) {
    if let Err(error) = client
        .close_instance(instance_id, std::time::Duration::from_secs(10))
        .await
    {
        log::debug!(
            "Plugin instance cleanup after failed prepare was incomplete: instance_id={}, error={}",
            instance_id,
            error
        );
    }
    close_plugin_host_ptys(instance_id).await;
    instances.lock().await.remove(instance_key);
}

async fn withdraw_configured_plugin_workspace(directory: &Path) {
    let Ok(canonical) = dunce::canonicalize(directory) else {
        return;
    };
    let workspace_scope = comparable_instance_directory(&canonical.to_string_lossy());
    let registry = crate::native_hooks::plugin_hook_registry(&workspace_scope);
    crate::plugin_hook_bridge::withdraw_plugin_workspace(&registry, &workspace_scope);
    crate::plugin_config_projection::release_workspace(&canonical);
    let Some(instances) = PLUGIN_HOST_INSTANCES.get() else {
        crate::native_hooks::clear_plugin_hook_workspace(&workspace_scope);
        return;
    };
    let owned = instances
        .lock()
        .await
        .iter()
        .filter(|(_, instance)| instance.canonical_directory == workspace_scope)
        .map(|(key, instance)| (key.clone(), instance.clone()))
        .collect::<Vec<_>>();
    let client = PLUGIN_HOST
        .get()
        .and_then(|state| state.try_lock().ok())
        .and_then(|host| host.as_ref().map(PluginHost::client));
    for (key, instance) in owned {
        if let Some(token) = instance.hook_commit_token.clone() {
            crate::plugin_hook_bridge::unregister_plugin_hooks(&registry, &workspace_scope, token);
        }
        crate::agentic::tools::plugin_host_tool::unregister_workspace_tools(
            &workspace_scope,
            &instance.directory,
            &instance.tool_names,
            &instance.generation_key,
        )
        .await;
        if let Some(bridge) = crate::plugin_host_http::plugin_host_backend_bridge() {
            bridge.cancel_instance_streams(&instance.instance_id).await;
        }
        if let Some(client) = client.as_ref() {
            let _ = client
                .close_instance(&instance.instance_id, std::time::Duration::from_secs(10))
                .await;
        }
        close_plugin_host_ptys(&instance.instance_id).await;
        instances.lock().await.remove(&key);
    }
    crate::native_hooks::clear_plugin_hook_workspace(&workspace_scope);
}

async fn retire_superseded_plugin_instances(
    client: &bitfun_opencode_plugin_host::PluginHostClient,
    instances: &Mutex<HashMap<String, PluginHostInstance>>,
    active_key: &str,
    workspace_scope: &str,
) {
    let stale = instances
        .lock()
        .await
        .iter()
        .filter(|(key, instance)| {
            key.as_str() != active_key && instance.canonical_directory == workspace_scope
        })
        .map(|(key, instance)| (key.clone(), instance.clone()))
        .collect::<Vec<_>>();
    for (key, instance) in stale {
        if instance.agent_runtime_keys.iter().any(|runtime_key| {
            crate::agentic::agents::get_agent_registry().check_agent_exists(runtime_key)
        }) {
            let should_schedule = {
                let mut state = instances.lock().await;
                state.get_mut(&key).is_some_and(|current| {
                    if current.retirement_scheduled {
                        false
                    } else {
                        current.retirement_scheduled = true;
                        true
                    }
                })
            };
            if should_schedule {
                schedule_plugin_instance_retirement(client.clone(), key.clone());
            }
            continue;
        }
        let removed = {
            let mut state = instances.lock().await;
            state
                .get(&key)
                .filter(|current| current.instance_id == instance.instance_id)
                .is_some()
                .then(|| state.remove(&key))
                .flatten()
        };
        if let Some(removed) = removed {
            retire_plugin_instance(client, removed, workspace_scope).await;
        }
    }
}

fn schedule_plugin_instance_retirement(
    client: bitfun_opencode_plugin_host::PluginHostClient,
    instance_key: String,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let Some(instances) = PLUGIN_HOST_INSTANCES.get() else {
                return;
            };
            let snapshot = {
                let state = instances.lock().await;
                let Some(instance) = state.get(&instance_key) else {
                    return;
                };
                if !instance.retirement_scheduled {
                    return;
                }
                instance.clone()
            };
            if crate::plugin_config_projection::active_generation_key(&snapshot.directory)
                .as_deref()
                == Some(snapshot.generation_key.as_str())
            {
                if let Some(instance) = instances.lock().await.get_mut(&instance_key) {
                    instance.retirement_scheduled = false;
                }
                return;
            }
            if snapshot.agent_runtime_keys.iter().any(|runtime_key| {
                crate::agentic::agents::get_agent_registry().check_agent_exists(runtime_key)
            }) {
                continue;
            }
            let removed = {
                let mut state = instances.lock().await;
                let matches = state.get(&instance_key).is_some_and(|current| {
                    current.retirement_scheduled
                        && current.instance_id == snapshot.instance_id
                        && current.generation_key == snapshot.generation_key
                });
                matches.then(|| state.remove(&instance_key)).flatten()
            };
            if let Some(instance) = removed {
                let workspace_scope = instance.canonical_directory.clone();
                retire_plugin_instance(&client, instance, &workspace_scope).await;
            }
            return;
        }
    });
}

async fn retire_plugin_instance(
    client: &bitfun_opencode_plugin_host::PluginHostClient,
    instance: PluginHostInstance,
    workspace_scope: &str,
) {
    if let Some(token) = instance.hook_commit_token.clone() {
        crate::plugin_hook_bridge::unregister_plugin_hooks(
            &crate::native_hooks::plugin_hook_registry(workspace_scope),
            workspace_scope,
            token,
        );
    }
    crate::agentic::tools::plugin_host_tool::unregister_workspace_tools(
        workspace_scope,
        &instance.directory,
        &instance.tool_names,
        &instance.generation_key,
    )
    .await;
    if let Some(bridge) = crate::plugin_host_http::plugin_host_backend_bridge() {
        bridge.cancel_instance_streams(&instance.instance_id).await;
    }
    if let Err(error) = client
        .close_instance(&instance.instance_id, std::time::Duration::from_secs(10))
        .await
    {
        log::warn!(
            "Superseded plugin instance close failed: instance_id={}, error={}",
            instance.instance_id,
            error
        );
    }
    close_plugin_host_ptys(&instance.instance_id).await;
}

pub(crate) async fn plugin_host_instance_by_id(instance_id: &str) -> Option<PluginHostInstance> {
    let instances = PLUGIN_HOST_INSTANCES.get()?;
    instances
        .lock()
        .await
        .values()
        .find(|instance| instance.instance_id == instance_id)
        .cloned()
}

pub(crate) async fn plugin_hook_generation_for_agent(
    workspace_scope: &str,
    runtime_agent_key: &str,
) -> Option<bitfun_agent_runtime::native_hooks::PluginHookGenerationIdentity> {
    let instances = PLUGIN_HOST_INSTANCES.get()?;
    instances
        .lock()
        .await
        .values()
        .find(|instance| {
            instance.ready
                && instance.canonical_directory == workspace_scope
                && instance
                    .agent_runtime_keys
                    .iter()
                    .any(|key| key == runtime_agent_key)
        })
        .map(
            |instance| bitfun_agent_runtime::native_hooks::PluginHookGenerationIdentity {
                instance_id: instance.instance_id.clone(),
                generation_key: instance.generation_key.clone(),
                revision: instance.revision.clone(),
            },
        )
}

pub(crate) async fn publish_plugin_host_diagnostic(
    params: Value,
) -> Result<Value, RpcHandlerError> {
    let params: PluginHostDiagnosticPublishParams =
        serde_json::from_value(params).map_err(|error| {
            RpcHandlerError::new(
                -32602,
                format!("invalid backend.diagnostic.publish params: {error}"),
            )
        })?;
    if !matches!(
        params.diagnostic.severity.as_str(),
        "debug" | "info" | "warning" | "error"
    ) {
        return Err(RpcHandlerError::new(
            -32602,
            "backend.diagnostic.publish severity is invalid",
        ));
    }
    let diagnostic = serde_json::to_value(params.diagnostic)
        .map_err(|error| RpcHandlerError::new(-32603, error.to_string()))?;
    if let Some(instance_id) = params.instance_id.as_deref() {
        let instances = PLUGIN_HOST_INSTANCES
            .get()
            .ok_or_else(|| RpcHandlerError::new(-32004, "plugin instance is unavailable"))?;
        let mut instances = instances.lock().await;
        let instance = instances
            .values_mut()
            .find(|instance| instance.instance_id == instance_id)
            .ok_or_else(|| RpcHandlerError::new(-32004, "plugin instance is unavailable"))?;
        push_plugin_host_diagnostic(&mut instance.diagnostic_health_snapshot, diagnostic.clone());
    }
    crate::infrastructure::events::emit_global_event(
        crate::infrastructure::events::BackendEvent::Custom {
            event_name: "plugin-host-diagnostic".to_string(),
            payload: serde_json::json!({
                "instance_id": params.instance_id,
                "diagnostic": diagnostic,
                "timestamp": chrono::Utc::now().timestamp_millis(),
            }),
        },
    )
    .await
    .map_err(|error| {
        RpcHandlerError::new(
            -32603,
            format!("failed to publish plugin host diagnostic: {error}"),
        )
    })?;
    Ok(serde_json::json!({}))
}

fn push_plugin_host_diagnostic(snapshot: &mut Vec<Value>, diagnostic: Value) {
    snapshot.push(diagnostic);
    let overflow = snapshot.len().saturating_sub(MAX_PLUGIN_HOST_DIAGNOSTICS);
    if overflow > 0 {
        snapshot.drain(..overflow);
    }
}

pub(crate) async fn register_plugin_host_pty(pty_id: &str, instance_id: &str) {
    let owners = PLUGIN_HOST_PTY_OWNERS
        .get_or_init(|| async { Mutex::new(HashMap::new()) })
        .await;
    owners
        .lock()
        .await
        .insert(pty_id.to_string(), instance_id.to_string());
}

pub(crate) async fn plugin_host_pty_owned_by(pty_id: &str, instance_id: &str) -> bool {
    let Some(owners) = PLUGIN_HOST_PTY_OWNERS.get() else {
        return false;
    };
    owners
        .lock()
        .await
        .get(pty_id)
        .is_some_and(|owner| owner == instance_id)
}

pub(crate) async fn unregister_plugin_host_pty(pty_id: &str, instance_id: &str) -> bool {
    let Some(owners) = PLUGIN_HOST_PTY_OWNERS.get() else {
        return false;
    };
    let mut owners = owners.lock().await;
    if owners.get(pty_id).is_some_and(|owner| owner == instance_id) {
        owners.remove(pty_id);
        true
    } else {
        false
    }
}

pub(crate) async fn prune_plugin_host_pty(pty_id: &str, instance_id: &str) {
    if unregister_plugin_host_pty(pty_id, instance_id).await {
        log::debug!(
            "Removed stale plugin host PTY ownership: instance_id={}, pty_id={}",
            instance_id,
            pty_id
        );
    }
}

pub(crate) async fn plugin_host_pty_ids_for_instance(instance_id: &str) -> Vec<String> {
    let Some(owners) = PLUGIN_HOST_PTY_OWNERS.get() else {
        return Vec::new();
    };
    owners
        .lock()
        .await
        .iter()
        .filter_map(|(pty_id, owner)| (owner == instance_id).then_some(pty_id.clone()))
        .collect()
}

async fn close_plugin_host_ptys(instance_id: &str) {
    let pty_ids = plugin_host_pty_ids_for_instance(instance_id).await;
    if pty_ids.is_empty() {
        return;
    }
    let api = match TerminalApi::from_singleton() {
        Ok(api) => Some(api),
        Err(error) => {
            log::warn!(
                "Plugin host PTYs could not be closed because the terminal owner is unavailable: instance_id={}, pty_count={}, error={}",
                instance_id,
                pty_ids.len(),
                error
            );
            None
        }
    };
    for pty_id in &pty_ids {
        if let Some(api) = api.as_ref() {
            if let Err(error) = api
                .close_session(CloseSessionRequest {
                    session_id: pty_id.clone(),
                    immediate: Some(false),
                })
                .await
            {
                log::warn!(
                    "Plugin host PTY close failed: instance_id={}, pty_id={}, error={}",
                    instance_id,
                    pty_id,
                    error
                );
            }
        }
        unregister_plugin_host_pty(pty_id, instance_id).await;
    }
    log::info!(
        "Plugin host PTY cleanup completed: instance_id={}, pty_count={}",
        instance_id,
        pty_ids.len()
    );
}

async fn close_all_plugin_host_ptys() {
    let instance_ids = if let Some(owners) = PLUGIN_HOST_PTY_OWNERS.get() {
        let mut instance_ids = owners.lock().await.values().cloned().collect::<Vec<_>>();
        instance_ids.sort();
        instance_ids.dedup();
        instance_ids
    } else {
        Vec::new()
    };
    for instance_id in instance_ids {
        close_plugin_host_ptys(&instance_id).await;
    }
}

pub(crate) fn instance_directories_equal(requested: &str, expected: &Path) -> bool {
    let Ok(expected) = dunce::canonicalize(expected) else {
        return false;
    };
    let expected = comparable_instance_directory(&expected.to_string_lossy());
    let matches = |candidate: &str| {
        dunce::canonicalize(candidate)
            .map(|path| comparable_instance_directory(&path.to_string_lossy()) == expected)
            .unwrap_or(false)
    };
    matches(requested)
        || urlencoding::decode(requested)
            .ok()
            .is_some_and(|decoded| decoded.as_ref() != requested && matches(decoded.as_ref()))
}

pub async fn shutdown_configured_plugin_host(
) -> crate::BitFunResult<Option<PluginHostShutdownReport>> {
    let shutdown_report = PLUGIN_HOST_SHUTDOWN_REPORT
        .get_or_init(|| async { Mutex::new(None) })
        .await;
    let shutdown_notify = PLUGIN_HOST_SHUTDOWN_NOTIFY
        .get_or_init(|| async { Notify::new() })
        .await;

    if PLUGIN_HOST_SHUTDOWN_STARTED.swap(true, Ordering::AcqRel) {
        loop {
            let notified = shutdown_notify.notified();
            if PLUGIN_HOST_SHUTDOWN_COMPLETE.load(Ordering::Acquire) {
                return Ok(shutdown_report.lock().await.clone());
            }
            notified.await;
        }
    }

    if let Some(bridge) = crate::plugin_host_http::plugin_host_backend_bridge() {
        bridge.begin_draining().await;
    }
    let host_state = PLUGIN_HOST.get_or_init(|| async { Mutex::new(None) }).await;
    let host = host_state.lock().await.take();
    if let Some(instances) = PLUGIN_HOST_INSTANCES.get() {
        let mut instances = instances.lock().await;
        for instance in instances.values() {
            if let Some(token) = instance.hook_commit_token.clone() {
                crate::plugin_hook_bridge::unregister_plugin_hooks(
                    &crate::native_hooks::plugin_hook_registry(&instance.canonical_directory),
                    &instance.canonical_directory,
                    token,
                );
            }
            crate::agentic::tools::plugin_host_tool::unregister_workspace_tools(
                &instance.canonical_directory,
                &instance.directory,
                &instance.tool_names,
                &instance.generation_key,
            )
            .await;
            crate::plugin_config_projection::release_workspace(&instance.directory);
        }
        let workspaces = instances
            .values()
            .map(|instance| instance.canonical_directory.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for workspace in workspaces {
            let registry = crate::native_hooks::plugin_hook_registry(&workspace);
            crate::plugin_hook_bridge::withdraw_plugin_workspace(&registry, &workspace);
            crate::native_hooks::clear_plugin_hook_workspace(&workspace);
        }
        instances.clear();
    }
    let report = match host {
        Some(host) => {
            log::info!("Starting configured plugin host graceful shutdown");
            Some(host.shutdown(PluginHostShutdownPolicy::default()).await)
        }
        None => {
            log::debug!("Configured plugin host graceful shutdown skipped: host not started");
            None
        }
    };
    close_all_plugin_host_ptys().await;
    if let Some(owners) = PLUGIN_HOST_PTY_OWNERS.get() {
        owners.lock().await.clear();
    }
    *shutdown_report.lock().await = report.clone();
    PLUGIN_HOST_SHUTDOWN_COMPLETE.store(true, Ordering::Release);
    shutdown_notify.notify_waiters();
    Ok(report)
}

async fn register_plugin_tools(
    client: &bitfun_opencode_plugin_host::PluginHostClient,
    instance_id: &str,
    workspace_scope: &str,
    workspace_root: &Path,
    generation_key: &str,
    revision: &str,
    config_fingerprint: &str,
    open_result: &Value,
    projection: &crate::plugin_config_projection::PluginConfigProjectionPlan,
) -> crate::BitFunResult<Vec<String>> {
    let Some(tools) = open_result.get("tools").and_then(Value::as_array) else {
        log::debug!(
            "Plugin tool registration completed with no tools: workspace={}, instance_id={}",
            workspace_scope,
            instance_id
        );
        return Ok(Vec::new());
    };
    log::debug!(
        "Plugin tool registration preparing: workspace={}, instance_id={}, tool_count={}",
        workspace_scope,
        instance_id,
        tools.len()
    );
    let mut prepared = Vec::new();
    let mut seen_ids = std::collections::BTreeSet::new();
    for tool in tools {
        let allowed_runtime_agent_keys = projection.allowed_runtime_agent_keys_for_tool(tool)?;
        if allowed_runtime_agent_keys.is_empty() {
            continue;
        }
        let registration_id = tool
            .get("registrationID")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                crate::BitFunError::Validation("Plugin tool registrationID is missing".to_string())
            })?;
        let id = tool.get("id").and_then(Value::as_str).ok_or_else(|| {
            crate::BitFunError::Validation("Plugin tool id is missing".to_string())
        })?;
        if !seen_ids.insert(id.to_string()) {
            return Err(crate::BitFunError::Validation(format!(
                "Plugin tool id is duplicated in the open result: {id}"
            )));
        }
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let parameters = tool
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"type":"object"}));
        prepared.push((
            registration_id.to_string(),
            id.to_string(),
            description.to_string(),
            parameters,
            allowed_runtime_agent_keys,
        ));
    }

    // Validate the complete generation before mutating the Tool mux. Once
    // registration starts, all remaining operations are infallible local
    // publication steps, so a malformed later entry cannot leave a partial
    // generation installed.
    let mut names = Vec::with_capacity(prepared.len());
    for (registration_id, id, description, parameters, allowed_runtime_agent_keys) in prepared {
        crate::agentic::tools::plugin_host_tool::register_workspace_tool(
            workspace_scope,
            workspace_root,
            client.clone(),
            instance_id,
            generation_key,
            revision,
            &registration_id,
            &id,
            &description,
            parameters,
            config_fingerprint,
            allowed_runtime_agent_keys,
        )
        .await;
        log::debug!(
            "Plugin tool registration committed to Rust registry: workspace={}, instance_id={}, tool_id={}, registration_id={}",
            workspace_scope,
            instance_id,
            id,
            registration_id
        );
        names.push(id);
    }
    log::info!(
        "Plugin tool registration completed: workspace={}, instance_id={}, tool_count={}",
        workspace_scope,
        instance_id,
        names.len()
    );
    Ok(names)
}

fn validate_open_generation_lease(
    result: &Value,
    instance_id: &str,
    generation_key: &str,
    revision: &str,
) -> crate::BitFunResult<()> {
    let valid = result.get("instanceID").and_then(Value::as_str) == Some(instance_id)
        && result.get("generationKey").and_then(Value::as_str) == Some(generation_key)
        && result.get("revision").and_then(Value::as_str) == Some(revision);
    if valid {
        Ok(())
    } else {
        Err(crate::BitFunError::Validation(
            "Plugin host open result generation lease does not match the request".to_string(),
        ))
    }
}

fn resolve_host_entry(spec: PluginHostLaunchSpec) -> crate::BitFunResult<PathBuf> {
    if let Some(entry) = std::env::var_os(spec.entry_env) {
        return absolutize_existing_entry(PathBuf::from(entry), spec);
    }
    let executable = std::env::current_exe().map_err(crate::BitFunError::Io)?;
    let executable_directory = executable.parent().ok_or_else(|| {
        crate::BitFunError::config(format!(
            "BitFun executable has no parent directory: {}",
            executable.display()
        ))
    })?;
    let bundled_entry = executable_directory
        .join("resources")
        .join("ext-host")
        .join(spec.entry_filename);
    if bundled_entry.is_file() {
        return Ok(bundled_entry);
    }
    let development_entry = development_host_entry(spec);
    if let Some(entry) = development_entry.filter(|entry| entry.is_file()) {
        return Ok(entry);
    }
    Err(crate::BitFunError::NotFound(format!(
        "{} plugin host entry does not exist at {}. Set {} in development.",
        spec.runtime_name,
        bundled_entry.display(),
        spec.entry_env
    )))
}

fn development_host_entry(spec: PluginHostLaunchSpec) -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .map(|repository_root| {
            repository_root
                .join("src")
                .join("apps")
                .join("extension-host")
                .join("dist")
                .join(spec.entry_filename)
        })
}

fn plugin_declaration(
    declaration: &crate::service::config::PluginDeclarationConfig,
) -> Option<PluginDeclaration> {
    use crate::service::config::PluginDeclarationConfig;

    let declaration = match declaration {
        PluginDeclarationConfig::Spec(spec) => PluginDeclaration {
            spec: spec.clone(),
            options: None,
            base_directory: None,
        },
        PluginDeclarationConfig::Detailed(details) => PluginDeclaration {
            spec: details.spec.clone(),
            options: details.options.clone(),
            base_directory: details.base_directory.clone(),
        },
    };
    if declaration.spec.trim().is_empty() {
        None
    } else {
        Some(declaration)
    }
}

fn plugin_config_fingerprint(
    config: &crate::service::config::GlobalConfig,
) -> crate::BitFunResult<String> {
    let declarations = config
        .plugin
        .iter()
        .filter_map(plugin_declaration)
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&declarations)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn comparable_instance_directory(directory: &str) -> String {
    let mut comparable = directory.replace('\\', "/");
    #[cfg(windows)]
    comparable.make_ascii_lowercase();
    comparable
}

pub(crate) fn canonical_plugin_workspace_scope(path: &Path) -> Option<String> {
    dunce::canonicalize(path)
        .ok()
        .map(|path| comparable_instance_directory(&path.to_string_lossy()))
}

fn absolutize_existing_entry(
    entry: PathBuf,
    spec: PluginHostLaunchSpec,
) -> crate::BitFunResult<PathBuf> {
    let entry = if entry.is_absolute() {
        entry
    } else {
        std::env::current_dir()
            .map_err(crate::BitFunError::Io)?
            .join(entry)
    };
    if !entry.is_file() {
        return Err(crate::BitFunError::NotFound(format!(
            "{} plugin host entry does not exist: {}. Set {} in development.",
            spec.runtime_name,
            entry.display(),
            spec.entry_env
        )));
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::{
        development_host_entry, initialize_configured_plugin_host, instance_directories_equal,
        plugin_host_pty_ids_for_instance, plugin_host_pty_owned_by, push_plugin_host_diagnostic,
        register_plugin_host_pty, unregister_plugin_host_pty, PluginHostLaunchPolicy,
        PluginHostLaunchSpec, PluginHostStartup, MAX_PLUGIN_HOST_DIAGNOSTICS,
    };
    use std::path::Path;

    #[test]
    fn bun_runtime_selects_bun_command_and_entry() {
        let spec = PluginHostLaunchSpec::bun();

        assert_eq!(spec.default_command, "bun");
        assert_eq!(spec.entry_filename, "extension-host.js");
        assert_eq!(spec.command_env, "BITFUN_BUN_COMMAND");
        assert_eq!(spec.entry_env, "BITFUN_OPENCODE_BUN_HOST_ENTRY");
    }

    #[test]
    fn development_host_entry_is_owned_by_the_bitfun_repository() {
        let spec = PluginHostLaunchSpec::bun();
        let entry = development_host_entry(spec).expect("BitFun repository root");

        assert!(entry.ends_with(
            Path::new("src")
                .join("apps")
                .join("extension-host")
                .join("dist")
                .join("extension-host.js")
        ));
    }

    #[tokio::test]
    async fn disabled_launch_policy_skips_host_initialization() {
        let status = initialize_configured_plugin_host(PluginHostLaunchPolicy::Disabled)
            .await
            .expect("disabled policy");

        assert_eq!(status, PluginHostStartup::Disabled);
    }

    #[test]
    fn instance_directory_matching_accepts_encoded_paths_and_rejects_siblings() {
        let directory = tempfile::tempdir().expect("temporary workspace");
        let workspace = directory.path().join("workspace with space");
        let sibling = directory.path().join("workspace with space-sibling");
        std::fs::create_dir_all(&workspace).expect("workspace directory");
        std::fs::create_dir_all(&sibling).expect("sibling directory");
        let encoded = urlencoding::encode(&workspace.to_string_lossy()).into_owned();

        assert!(instance_directories_equal(&encoded, &workspace));
        assert!(!instance_directories_equal(
            &sibling.to_string_lossy(),
            &workspace
        ));
    }

    #[tokio::test]
    async fn plugin_host_pty_ownership_is_instance_scoped() {
        let pty_id = format!("pty-test-{}", std::process::id());
        let first = format!("instance-first-{}", std::process::id());
        let second = format!("instance-second-{}", std::process::id());

        register_plugin_host_pty(&pty_id, &first).await;
        assert!(plugin_host_pty_owned_by(&pty_id, &first).await);
        assert!(!plugin_host_pty_owned_by(&pty_id, &second).await);
        assert_eq!(
            plugin_host_pty_ids_for_instance(&first).await,
            vec![pty_id.clone()]
        );
        assert!(unregister_plugin_host_pty(&pty_id, &first).await);
    }

    #[test]
    fn diagnostic_health_snapshot_retains_the_newest_entries() {
        let mut snapshot = Vec::new();
        for index in 0..=MAX_PLUGIN_HOST_DIAGNOSTICS {
            push_plugin_host_diagnostic(&mut snapshot, serde_json::json!({"index": index}));
        }

        assert_eq!(snapshot.len(), MAX_PLUGIN_HOST_DIAGNOSTICS);
        assert_eq!(snapshot.first().unwrap()["index"], 1);
        assert_eq!(
            snapshot.last().unwrap()["index"],
            MAX_PLUGIN_HOST_DIAGNOSTICS
        );
    }
}
