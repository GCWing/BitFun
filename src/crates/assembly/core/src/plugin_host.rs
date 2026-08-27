use crate::external_sources::{
    ExternalExecutableActivationReview, ExternalExecutableSourceIdentity,
};
use bitfun_agent_runtime::native_hooks::RuntimeHookCommitToken;
use bitfun_opencode_plugin_host::{
    invocation_port, PluginDeclaration, PluginHost, PluginHostConfig, PluginHostShutdownPolicy,
    PluginHostShutdownReport, PluginInstanceOpenRequest, PluginPrepareRequest, RpcHandlerError,
    CONFIG_CONTRIBUTIONS_V2, CONFIG_CONTRIBUTORS_V1, GENERATION_FENCING_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
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
static PLUGIN_HOST_ENSURE_LOCKS: OnceCell<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    OnceCell::const_new();
static PLUGIN_HOST_PTY_OWNERS: OnceCell<Mutex<HashMap<String, String>>> = OnceCell::const_new();
static NEXT_INSTANCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_PLUGIN_HOST_DIAGNOSTICS: usize = 100;

async fn plugin_host_workspace_lock(scope: &str) -> Arc<Mutex<()>> {
    let locks = PLUGIN_HOST_ENSURE_LOCKS
        .get_or_init(|| async { Mutex::new(HashMap::new()) })
        .await;
    let mut locks = locks.lock().await;
    locks
        .entry(scope.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn opencode_execution_authorized(
    safe_mode: bool,
    policy: &bitfun_product_domains::external_integration_policy::ExternalIntegrationPolicySnapshot,
) -> bool {
    if safe_mode || !policy.status.is_compatible() || !policy.effective.enabled {
        return false;
    }
    policy
        .effective
        .ecosystems
        .iter()
        .find(|(ecosystem, _)| ecosystem.as_str() == "opencode")
        .is_some_and(|(_, policy)| {
            matches!(
                policy.mode,
                bitfun_product_domains::external_integration_policy::ExternalIntegrationMode::Recommended
                    | bitfun_product_domains::external_integration_policy::ExternalIntegrationMode::Custom
            )
        })
}

fn digest_json(value: &Value) -> String {
    hex::encode(Sha256::digest(
        serde_json::to_vec(value).unwrap_or_default(),
    ))
}

fn stable_prepared_digest(prepared: &Value) -> String {
    prepared
        .get("reviewDigest")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn opencode_activation_review(
    workspace_scope: &str,
    execution_domain_id: &str,
    _declarations: &[PluginDeclaration],
    prepared: &Value,
    policy: &bitfun_product_domains::external_integration_policy::ExternalIntegrationPolicySnapshot,
    policy_revision: u64,
) -> ExternalExecutableActivationReview {
    let prepared_entries = prepared
        .get("prepared")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let reviewed_entries = prepared
        .get("reviewed")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let failed_entries = prepared
        .get("failed")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let identities = reviewed_entries
        .iter()
        .map(|reviewed_entry| {
            let spec = reviewed_entry
                .get("spec")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let identity = reviewed_entry
                .get("identity")
                .and_then(Value::as_str)
                .unwrap_or(spec);
            let match_entry = prepared_entries
                .iter()
                .find(|entry| entry.get("identity").and_then(Value::as_str) == Some(identity));
            ExternalExecutableSourceIdentity {
                plugin_id: identity.to_string(),
                source_kind: reviewed_entry
                    .get("source")
                    .or_else(|| match_entry.and_then(|entry| entry.get("source")))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                canonical_source: reviewed_entry
                    .get("canonicalSource")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        match_entry
                            .and_then(|entry| entry.get("target").or_else(|| entry.get("entry")))
                            .and_then(Value::as_str)
                    })
                    .unwrap_or(spec)
                    .to_string(),
                declaration_digest: reviewed_entry
                    .get("optionsDigest")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                content_digest: match_entry
                    .and_then(|entry| entry.get("contentHash"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }
        })
        .collect::<Vec<_>>();
    let mut review = ExternalExecutableActivationReview {
        schema_version: 1,
        ecosystem_id: "opencode".to_string(),
        execution_domain_id: execution_domain_id.to_string(),
        workspace_scope: workspace_scope.to_string(),
        phase: "opencode-plugin-host".to_string(),
        source_identities: identities,
        prepared_digest: stable_prepared_digest(prepared),
        permission_summary_digest: digest_json(
            &serde_json::to_value(policy).unwrap_or_else(|_| Value::Null),
        ),
        policy_revision,
        requires_install: failed_entries
            .iter()
            .any(|entry| entry.get("stage").and_then(Value::as_str) == Some("install")),
        approval_fingerprint: String::new(),
    };
    let material = serde_json::to_vec(&review).unwrap_or_default();
    review.approval_fingerprint = hex::encode(Sha256::digest(material));
    review
}

/// Produce the exact, non-importing activation envelope for a workspace. This
/// is used by product surfaces to display a fingerprint before persisting a
/// grant. npm cache misses are represented as an install-plan review and do
/// not run `bun add`.
pub async fn review_configured_plugin_activation(
    launch_policy: PluginHostLaunchPolicy,
    directory: PathBuf,
) -> crate::BitFunResult<Option<ExternalExecutableActivationReview>> {
    use crate::service::config::{get_global_config_service, GlobalConfig};

    if launch_policy == PluginHostLaunchPolicy::Disabled {
        return Ok(None);
    }
    if !matches!(
        initialize_configured_plugin_host(launch_policy).await?,
        PluginHostStartup::Disabled
            | PluginHostStartup::Started
            | PluginHostStartup::AlreadyStarted
    ) {
        return Ok(None);
    }
    let canonical_directory = dunce::canonicalize(&directory).map_err(|error| {
        crate::BitFunError::Io(std::io::Error::other(format!(
            "Failed to canonicalize plugin activation workspace {}: {error}",
            directory.display()
        )))
    })?;
    let workspace_scope = comparable_instance_directory(&canonical_directory.to_string_lossy());
    let surface = crate::external_sources::get_external_source_control_snapshot(
        Some(&canonical_directory),
        true,
        crate::external_sources::ExternalSourceHostCapabilities::read_write(),
    )
    .await
    .map_err(|error| crate::BitFunError::Validation(error.to_string()))?;
    if !opencode_execution_authorized(
        surface.control.safe_mode,
        &surface.catalog.integration_policy,
    ) {
        return Err(crate::BitFunError::NotImplemented(
            "Configured OpenCode Plugin Host execution is not eligible for activation review"
                .to_string(),
        ));
    }
    let config_service = get_global_config_service().await?;
    let global_config: GlobalConfig = config_service.get_config(None).await?;
    let declarations = global_config
        .plugin
        .iter()
        .filter_map(plugin_declaration)
        .collect::<Vec<_>>();
    if declarations.is_empty() {
        return Ok(None);
    }
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
    let prepared = client
        .prepare_plugins(
            PluginPrepareRequest {
                plugins: declarations.clone(),
                configuration_fingerprint: Some(plugin_config_fingerprint(&global_config)?),
                default_base_directory: Some(canonical_directory.to_string_lossy().into_owned()),
                allow_install: Some(false),
            },
            std::time::Duration::from_secs(30),
        )
        .await
        .map_err(|error| crate::BitFunError::ProcessError(error.to_string()))?;
    Ok(Some(opencode_activation_review(
        &workspace_scope,
        surface.control.execution_domain_id.as_str(),
        &declarations,
        &prepared,
        &surface.catalog.integration_policy,
        surface.control.preference_revision,
    )))
}

pub async fn approve_configured_plugin_activation(
    review: ExternalExecutableActivationReview,
    expected_fingerprint: &str,
) -> crate::BitFunResult<()> {
    if review.approval_fingerprint != expected_fingerprint {
        return Err(crate::BitFunError::Validation(
            "Plugin activation confirmation does not match the current review fingerprint"
                .to_string(),
        ));
    }
    crate::external_sources::set_executable_activation_approval(
        review.clone(),
        true,
        review.policy_revision,
    )
    .await
    .map_err(crate::BitFunError::Validation)?;
    Ok(())
}

pub async fn revoke_configured_plugin_activation(directory: PathBuf) -> crate::BitFunResult<()> {
    let canonical_directory = dunce::canonicalize(&directory).map_err(|error| {
        crate::BitFunError::Io(std::io::Error::other(format!(
            "Failed to canonicalize plugin activation workspace {}: {error}",
            directory.display()
        )))
    })?;
    let workspace_scope = comparable_instance_directory(&canonical_directory.to_string_lossy());
    let surface = crate::external_sources::get_external_source_control_snapshot(
        Some(&canonical_directory),
        false,
        crate::external_sources::ExternalSourceHostCapabilities::read_write(),
    )
    .await
    .map_err(|error| crate::BitFunError::Validation(error.to_string()))?;
    crate::external_sources::revoke_executable_activation_approval(
        "opencode",
        surface.control.execution_domain_id.as_str(),
        &workspace_scope,
        surface.control.preference_revision,
    )
    .await
    .map_err(crate::BitFunError::Validation)?;
    withdraw_configured_plugin_workspace(&canonical_directory).await;
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct PluginHostInstance {
    pub(crate) canonical_directory: String,
    pub(crate) directory: PathBuf,
    pub(crate) worktree: PathBuf,
    pub(crate) project_id: String,
    pub(crate) created_at_ms: i64,
    pub(crate) instance_id: String,
    pub(crate) host_generation: u64,
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
    let stale_host = {
        let mut host_state = host_state.lock().await;
        if PLUGIN_HOST_SHUTDOWN_STARTED.load(Ordering::Acquire) {
            return Err(crate::BitFunError::ProcessError(
                "Plugin host is shutting down".to_string(),
            ));
        }
        if let Some(host) = host_state.as_mut() {
            if host
                .is_connected()
                .map_err(|error| crate::BitFunError::ProcessError(error.to_string()))?
            {
                return Ok(PluginHostStartup::AlreadyStarted);
            }
            log::warn!("Configured plugin host slot contained a disconnected host; retiring it before restart");
            host_state.take()
        } else {
            None
        }
    };
    if let Some(stale) = stale_host {
        let _ = stale.shutdown(PluginHostShutdownPolicy::default()).await;
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
    .map_err(|error| {
        crate::BitFunError::ProcessError(format!(
            "Failed to initialize {} plugin host from {}: {error}",
            launch_spec.runtime_name,
            entry.display()
        ))
    })?;
    let client = host.client();
    crate::plugin_host_http::register_plugin_host_backend_handlers(client.clone()).await?;
    *host_state.lock().await = Some(host);
    start_plugin_host_health_monitor(client.clone());
    // Do not prepare or import configured packages during host startup. Package
    // resolution may install dependencies and execute lifecycle code; it must
    // happen only after the workspace-specific source authority has approved
    // the exact plugin activation.
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

/// Fault the current Host connection and reap its complete process tree. This
/// is used after a side-effecting invocation whose cancellation could not be
/// confirmed; closing the RPC socket alone is not a stop guarantee.
pub(crate) async fn fault_configured_plugin_host(reason: &str) {
    let host = {
        let Some(state) = PLUGIN_HOST.get() else {
            return;
        };
        state.lock().await.take()
    };
    let Some(host) = host else {
        return;
    };
    log::error!(
        "Faulting configured plugin host and terminating its process tree: generation={}, reason={}",
        host.client().generation(),
        reason
    );
    let report = host.shutdown(PluginHostShutdownPolicy::default()).await;
    log::error!(
        "Configured plugin host fault cleanup completed: generation={}, disposition={:?}, exit_code={:?}",
        report.generation,
        report.disposition,
        report.exit_code
    );
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
    // All ensure/withdraw operations for a workspace use this same lock. The
    // lock is acquired before reading policy/config so a concurrent revoke or
    // replacement cannot race a later publish.
    let workspace_lock = plugin_host_workspace_lock(&comparable_directory).await;
    let _workspace_guard = workspace_lock.lock().await;

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
    let declarations = global_config
        .plugin
        .iter()
        .filter_map(plugin_declaration)
        .collect::<Vec<_>>();
    // Resolve the configured declarations directly. Plugin activation is an
    // explicit BitFun configuration choice; no separate external-integration
    // policy, safe-mode switch, or activation approval is required before the
    // host can load the configured plugins.
    let prepared = client
        .prepare_plugins(
            PluginPrepareRequest {
                plugins: declarations.clone(),
                configuration_fingerprint: Some(config_fingerprint.clone()),
                default_base_directory: Some(canonical_directory_string.clone()),
                allow_install: Some(true),
            },
            std::time::Duration::from_secs(30),
        )
        .await
        .map_err(|error| {
            crate::BitFunError::ProcessError(format!(
                "Failed to prepare plugins for workspace {}: {error}",
                canonical_directory.display()
            ))
        })?;
    let prepared_count = prepared
        .get("prepared")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let failed_count = prepared
        .get("failed")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let reviewed_count = prepared
        .get("reviewed")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if failed_count != 0 || prepared_count != reviewed_count {
        return Err(crate::BitFunError::Validation(format!(
            "Configured OpenCode plugin preparation did not resolve the complete approved graph: prepared={prepared_count}, failed={failed_count}, reviewed={reviewed_count}"
        )));
    }
    // Use the adapter's stable digest for generation identity. Cache state and
    // diagnostic prose are operational details and must not churn a generation.
    let prepared_fingerprint = stable_prepared_digest(&prepared);
    let expected_content_digests = prepared
        .get("prepared")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    Some((
                        entry.get("identity")?.as_str()?.to_string(),
                        entry.get("contentHash")?.as_str()?.to_string(),
                    ))
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        })
        .filter(|digests| !digests.is_empty());

    let workspace_config_fingerprint = serde_json::to_vec(&initial_config)
        .map(|bytes| hex::encode(Sha256::digest(bytes)))
        .map_err(|error| {
            crate::BitFunError::Validation(format!(
                "Failed to fingerprint workspace plugin config: {error}"
            ))
        })?;
    let instance_key = format!(
        "{comparable_directory}\n{config_fingerprint}\n{workspace_config_fingerprint}\n{prepared_fingerprint}"
    );
    let reusable_instance = {
        let mut state = instances.lock().await;
        state
            .get_mut(&instance_key)
            .filter(|instance| instance.ready && instance.host_generation == client.generation())
            .map(|instance| {
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

    // The extension host owns one canonical directory at a time. Stop and
    // remove the old logical generation before opening the replacement so the
    // host cannot reject the new instance with directory_exists or run old and
    // new plugin code concurrently.
    if !retire_workspace_instances_before_open(&client, instances, &comparable_directory).await {
        return Err(crate::BitFunError::ProcessError(
            "Configured plugin host could not confirm closure of the previous workspace generation"
                .to_string(),
        ));
    }

    let sequence = NEXT_INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let instance_id = format!("bitfun:host:{}:{sequence}", client.generation());
    let revision = format!("revision-{sequence}");
    let generation_material =
        format!(
            "{config_fingerprint}\n{workspace_config_fingerprint}\n{prepared_fingerprint}"
        );
    let generation_key = format!(
        "host-{}:instance-{sequence}:sha256-{}",
        client.generation(),
        hex::encode(Sha256::digest(generation_material.as_bytes()))
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
        host_generation: client.generation(),
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
                plugins: declarations,
                configuration_fingerprint: Some(config_fingerprint.clone()),
                expected_content_digests,
                expected_review_digest: Some(prepared_fingerprint.clone()),
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
    let invoker = invocation_port(
        client.clone(),
        instance_id.clone(),
        generation_key.clone(),
        revision.clone(),
    );
    log::info!(
        "Configured plugin host instance prepared: generation={}, instance_id={}, plugin_count={}",
        client.generation(),
        instance_id,
        global_config.plugin.len()
    );
    let hook_commit_token = match crate::plugin_hook_bridge::register_plugin_hooks_with_invoker(
        &crate::native_hooks::plugin_hook_registry(&comparable_directory),
        &comparable_directory,
        invoker.clone(),
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
        invoker,
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
    let mut state = instances.lock().await;
    if state
        .get(instance_key)
        .is_some_and(|current| current.instance_id == instance_id)
    {
        state.remove(instance_key);
    }
}

async fn retire_workspace_instances_before_open(
    client: &bitfun_opencode_plugin_host::PluginHostClient,
    instances: &Mutex<HashMap<String, PluginHostInstance>>,
    workspace_scope: &str,
) -> bool {
    let stale = {
        let state = instances.lock().await;
        let keys = state
            .iter()
            .filter(|(_, instance)| instance.canonical_directory == workspace_scope)
            .map(|(key, instance)| (key.clone(), instance.instance_id.clone()))
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|(key, instance_id)| {
                state
                    .get(&key)
                    .filter(|current| current.instance_id == instance_id)
                    .cloned()
                    .map(|instance| (key, instance))
            })
            .collect::<Vec<_>>()
    };
    let mut all_closed = true;
    for (key, instance) in stale {
        if retire_plugin_instance(client, instance.clone(), workspace_scope).await {
            let mut state = instances.lock().await;
            if state
                .get(&key)
                .is_some_and(|current| current.instance_id == instance.instance_id)
            {
                state.remove(&key);
            }
        } else if let Some(current) = instances.lock().await.get_mut(&key) {
            if current.instance_id == instance.instance_id {
                current.ready = false;
            }
            all_closed = false;
        }
    }
    all_closed
}

async fn withdraw_configured_plugin_workspace(directory: &Path) {
    let Ok(canonical) = dunce::canonicalize(directory) else {
        return;
    };
    let workspace_scope = comparable_instance_directory(&canonical.to_string_lossy());
    let workspace_lock = plugin_host_workspace_lock(&workspace_scope).await;
    let _workspace_guard = workspace_lock.lock().await;
    withdraw_configured_plugin_workspace_locked(&canonical, &workspace_scope).await;
}

async fn withdraw_configured_plugin_workspace_locked(canonical: &Path, workspace_scope: &str) {
    let registry = crate::native_hooks::plugin_hook_registry(&workspace_scope);
    crate::plugin_hook_bridge::withdraw_plugin_workspace(&registry, &workspace_scope);
    crate::plugin_config_projection::release_workspace(canonical);
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
        let close_result = if let Some(client) = client.as_ref() {
            client
                .close_instance(&instance.instance_id, std::time::Duration::from_secs(10))
                .await
                .map(|_| ())
        } else {
            Err(
                bitfun_opencode_plugin_host::PluginHostError::ConnectionClosed(
                    "plugin host is unavailable during workspace withdrawal".to_string(),
                ),
            )
        };
        close_plugin_host_ptys(&instance.instance_id).await;
        let mut state = instances.lock().await;
        if let Some(current) = state.get_mut(&key) {
            if current.instance_id != instance.instance_id {
                continue;
            }
            if let Err(error) = close_result {
                current.ready = false;
                log::error!(
                    "Plugin instance withdrawal could not confirm Host close; retaining fault state: instance_id={}, error={}",
                    current.instance_id,
                    error
                );
            } else {
                state.remove(&key);
            }
        }
    }
    crate::native_hooks::clear_plugin_hook_workspace(&workspace_scope);
}

fn start_plugin_host_health_monitor(client: bitfun_opencode_plugin_host::PluginHostClient) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let connected = {
                let Some(host_state) = PLUGIN_HOST.get() else {
                    return;
                };
                let mut host_state = host_state.lock().await;
                match host_state.as_mut() {
                    Some(host) if host.client().generation() != client.generation() => {
                        return;
                    }
                    Some(host) => match host.is_connected() {
                        Ok(connected) => connected,
                        Err(error) => {
                            log::error!(
                                "Configured plugin host health check failed: generation={}, error={}",
                                client.generation(),
                                error
                            );
                            false
                        }
                    },
                    None => false,
                }
            };
            if connected && !client.is_closed() {
                continue;
            }
            log::error!(
                "Configured plugin host process or connection closed; withdrawing all plugin contributions: generation={}",
                client.generation()
            );
            fault_configured_plugin_host("host connection or process lost").await;
            let workspaces = PLUGIN_HOST_INSTANCES.get().map(|instances| async {
                instances
                    .lock()
                    .await
                    .values()
                    .map(|instance| instance.directory.clone())
                    .collect::<Vec<_>>()
            });
            let Some(workspaces) = workspaces else { return };
            let workspaces = workspaces.await;
            for workspace in workspaces {
                withdraw_configured_plugin_workspace(&workspace).await;
            }
            return;
        }
    });
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
                let _ = retire_plugin_instance(&client, instance, &workspace_scope).await;
            }
            return;
        }
    });
}

async fn retire_plugin_instance(
    client: &bitfun_opencode_plugin_host::PluginHostClient,
    instance: PluginHostInstance,
    workspace_scope: &str,
) -> bool {
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
    let closed = if instance.host_generation != client.generation() {
        // A connection-generation change fences every instance owned by the
        // dead Host. There is no valid RPC target left to close; treating this
        // as closed allows the next Host generation to recover the workspace.
        true
    } else {
        match client
            .close_instance(&instance.instance_id, std::time::Duration::from_secs(10))
            .await
        {
            Ok(true) => true,
            Ok(false) => {
                log::warn!(
                    "Superseded plugin instance close was not confirmed: instance_id={}",
                    instance.instance_id
                );
                false
            }
            Err(error) => {
                log::warn!(
                    "Superseded plugin instance close failed: instance_id={}, error={}",
                    instance.instance_id,
                    error
                );
                false
            }
        }
    };
    close_plugin_host_ptys(&instance.instance_id).await;
    closed
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
    invoker: std::sync::Arc<dyn bitfun_runtime_ports::PluginRuntimeInvocationPort>,
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
            invoker.clone(),
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
