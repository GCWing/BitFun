use bitfun_opencode_plugin_host::{
    PluginDeclaration, PluginHost, PluginHostConfig, PluginHostShutdownPolicy,
    PluginHostShutdownReport, PluginInstanceOpenRequest, PluginPrepareRequest,
};
use serde_json::{Map, Value};
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

#[derive(Debug, Clone)]
pub(crate) struct PluginHostInstance {
    pub(crate) canonical_directory: String,
    pub(crate) directory: PathBuf,
    pub(crate) worktree: PathBuf,
    pub(crate) project_id: String,
    pub(crate) created_at_ms: i64,
    pub(crate) instance_id: String,
    pub(crate) open_result: Value,
    pub(crate) ready: bool,
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
    .map_err(|error| match error {
        bitfun_opencode_plugin_host::PluginHostError::RuntimeNotFound(command) => {
            crate::BitFunError::ProcessError(format!(
                "{} executable was not found at {}. Install Bun or set {} to a valid Bun executable.",
                launch_spec.runtime_name,
                command.display(),
                BUN_COMMAND_ENV
            ))
        }
        error => crate::BitFunError::ProcessError(format!(
            "Failed to initialize {} plugin host from {}: {error}",
            launch_spec.runtime_name,
            entry.display()
        )),
    })?;
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
    config: Map<String, Value>,
) -> crate::BitFunResult<Option<Value>> {
    use crate::service::config::{get_global_config_service, GlobalConfig};

    if launch_policy == PluginHostLaunchPolicy::Disabled {
        return Ok(None);
    }
    let config_service = get_global_config_service().await?;
    let global_config: GlobalConfig = config_service.get_config(None).await?;
    if !global_config.has_configured_plugins() {
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
    let instances = PLUGIN_HOST_INSTANCES
        .get_or_init(|| async { Mutex::new(HashMap::new()) })
        .await;
    let instance_key = format!("{comparable_directory}\n{config_fingerprint}");
    if let Some(instance) = instances.lock().await.get(&instance_key).cloned() {
        log::debug!(
            "Configured plugin host instance reused: generation={}, instance_id={}",
            client.generation(),
            instance.instance_id
        );
        return Ok(Some(instance.open_result.clone()));
    }

    let previous_keys = instances
        .lock()
        .await
        .iter()
        .filter(|(_, instance)| instance.canonical_directory == comparable_directory)
        .map(|(key, instance)| (key.clone(), instance.instance_id.clone()))
        .collect::<Vec<_>>();
    for (key, instance_id) in previous_keys {
        if let Some(bridge) = crate::plugin_host_http::plugin_host_backend_bridge() {
            bridge.cancel_instance_streams(&instance_id).await;
        }
        client
            .close_instance(&instance_id, std::time::Duration::from_secs(10))
            .await
            .map_err(|error| {
                crate::BitFunError::ProcessError(format!(
                    "Failed to close stale plugin host instance {instance_id}: {error}"
                ))
            })?;
        close_plugin_host_ptys(&instance_id).await;
        instances.lock().await.remove(&key);
    }

    let sequence = NEXT_INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let instance_id = format!("bitfun:host:{}:{sequence}", client.generation());
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
        open_result: Value::Null,
        ready: false,
    };
    instances
        .lock()
        .await
        .insert(instance_key.clone(), opening_context);
    let open_result = match client
        .open_instance(
            PluginInstanceOpenRequest {
                instance_id: instance_id.clone(),
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
            close_plugin_host_ptys(&instance_id).await;
            instances.lock().await.remove(&instance_key);
            return Err(crate::BitFunError::ProcessError(format!(
                "Failed to activate plugins for workspace {}: {error}",
                canonical_directory.display()
            )));
        }
    };
    log::info!(
        "Configured plugin host instance activated: generation={}, instance_id={}, plugin_count={}",
        client.generation(),
        instance_id,
        global_config.plugin.len()
    );
    if let Some(instance) = instances.lock().await.get_mut(&instance_key) {
        instance.open_result = open_result.clone();
        instance.ready = true;
    }
    Ok(Some(open_result))
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
        instances.lock().await.clear();
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
    // `mut` is only consumed by the #[cfg(windows)] branch; non-Windows
    // builds would trip `-D warnings` (unused_mut) without the allow.
    #[allow(unused_mut)]
    let mut comparable = directory.replace('\\', "/");
    #[cfg(windows)]
    comparable.make_ascii_lowercase();
    comparable
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
        plugin_host_pty_ids_for_instance, plugin_host_pty_owned_by, register_plugin_host_pty,
        unregister_plugin_host_pty, PluginHostLaunchPolicy, PluginHostLaunchSpec,
        PluginHostStartup,
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
}
