//! SSH Connection Manager using russh
//!
//! This module manages SSH connections using the pure-Russ SSH implementation

use crate::service::remote_ssh::password_vault::SSHPasswordVault;
use crate::service::remote_ssh::types::{
    SSHAuthMethod, SSHCommandOptions, SSHCommandResult, SSHConfigEntry, SSHConfigLookupResult,
    SSHConnectionConfig, SSHConnectionResult, SavedConnection, ServerInfo,
};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use russh::client::{DisconnectReason, Handle, Handler, Msg};
use russh::Sig;
use russh_keys::key::PublicKey;
use russh_keys::PublicKeyBase64;
use russh_sftp::client::fs::ReadDir;
use russh_sftp::client::SftpSession;
#[cfg(feature = "ssh_config")]
use ssh_config::SSHConfig;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::time::{Duration, Instant};

const SSH_COMMAND_WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SSH_COMMAND_INTERRUPT_DRAIN_GRACE: Duration = Duration::from_millis(500);

/// OpenSSH keyword matching is case-insensitive, but `ssh_config` stores keys as written in the file
/// (e.g. `HostName` vs `Hostname`). Resolve by ASCII case-insensitive compare.
#[cfg(feature = "ssh_config")]
fn ssh_cfg_get<'a>(
    settings: &std::collections::HashMap<&'a str, &'a str>,
    canonical_key: &str,
) -> Option<&'a str> {
    settings
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(canonical_key))
        .map(|(_, v)| *v)
}

#[cfg(feature = "ssh_config")]
fn ssh_cfg_has(settings: &std::collections::HashMap<&str, &str>, canonical_key: &str) -> bool {
    settings
        .keys()
        .any(|k| k.eq_ignore_ascii_case(canonical_key))
}

/// Known hosts entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnownHostEntry {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
    pub public_key: String,
}

/// Active SSH connection
struct ActiveConnection {
    handle: Arc<Handle<SSHHandler>>,
    config: SSHConnectionConfig,
    server_info: Option<ServerInfo>,
    sftp_session: Arc<tokio::sync::RwLock<Option<Arc<SftpSession>>>>,
    #[allow(dead_code)]
    server_key: Option<PublicKey>,
    /// Liveness flag; flipped to false from `SSHHandler::disconnected`.
    /// Allows `is_connected` and SFTP/exec entry points to detect a dead session
    /// without waiting for the next failed I/O.
    alive: Arc<AtomicBool>,
    /// Per-connection lock to serialize transparent reconnect attempts and
    /// avoid stampedes when multiple SFTP/exec calls hit a dead session at once.
    reconnect_lock: Arc<tokio::sync::Mutex<()>>,
}

/// SSH client handler with host key verification
struct SSHHandler {
    /// Expected host key (if connecting to known host)
    expected_key: Option<(String, u16, PublicKey)>,
    /// Callback for new host key verification
    verify_callback: Option<Box<HostKeyVerifyCallback>>,
    /// Known hosts storage for verification
    known_hosts: Option<Arc<tokio::sync::RwLock<HashMap<String, KnownHostEntry>>>>,
    /// Host info for known hosts lookup
    host: Option<String>,
    port: Option<u16>,
    /// Stores the real disconnect reason so callers get a useful error message.
    /// russh's run() absorbs errors internally; we capture them here and
    /// surface them after connect_stream() returns.
    /// Uses std::sync::Mutex so it can be read from sync map_err closures.
    disconnect_reason: Arc<std::sync::Mutex<Option<String>>>,
    /// Shared liveness flag, flipped to false on disconnect so the manager
    /// can detect dead sessions and trigger transparent reconnect.
    alive: Arc<AtomicBool>,
}

type HostKeyVerifyCallback = dyn Fn(String, u16, &PublicKey) -> bool + Send + Sync;

impl SSHHandler {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            expected_key: None,
            verify_callback: None,
            known_hosts: None,
            host: None,
            port: None,
            disconnect_reason: Arc::new(std::sync::Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
        }
    }

    #[allow(dead_code)]
    fn with_expected_key(host: String, port: u16, key: PublicKey) -> Self {
        Self {
            expected_key: Some((host, port, key)),
            verify_callback: None,
            known_hosts: None,
            host: None,
            port: None,
            disconnect_reason: Arc::new(std::sync::Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
        }
    }

    #[allow(dead_code)]
    fn with_verify_callback<F>(callback: F) -> Self
    where
        F: Fn(String, u16, &PublicKey) -> bool + Send + Sync + 'static,
    {
        Self {
            expected_key: None,
            verify_callback: Some(Box::new(callback)),
            known_hosts: None,
            host: None,
            port: None,
            disconnect_reason: Arc::new(std::sync::Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
        }
    }

    fn with_known_hosts(
        host: String,
        port: u16,
        known_hosts: Arc<tokio::sync::RwLock<HashMap<String, KnownHostEntry>>>,
    ) -> (Self, Arc<std::sync::Mutex<Option<String>>>, Arc<AtomicBool>) {
        let disconnect_reason = Arc::new(std::sync::Mutex::new(None));
        let alive = Arc::new(AtomicBool::new(true));
        let handler = Self {
            expected_key: None,
            verify_callback: None,
            known_hosts: Some(known_hosts),
            host: Some(host),
            port: Some(port),
            disconnect_reason: disconnect_reason.clone(),
            alive: alive.clone(),
        };
        (handler, disconnect_reason, alive)
    }
}

#[derive(Debug)]
struct HandlerError(String);

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for HandlerError {}

impl From<russh::Error> for HandlerError {
    fn from(e: russh::Error) -> Self {
        HandlerError(format!("{:?}", e))
    }
}

impl From<String> for HandlerError {
    fn from(s: String) -> Self {
        HandlerError(s)
    }
}

#[async_trait]
impl Handler for SSHHandler {
    type Error = HandlerError;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let server_fingerprint = server_public_key.fingerprint();

        // 1. If we have an expected key, verify it matches
        if let Some((ref host, port, ref expected)) = self.expected_key {
            if expected.fingerprint() == server_fingerprint {
                log::debug!("Server key matches expected key for {}:{}", host, port);
                return Ok(true);
            }
            log::warn!(
                "Server key mismatch for {}:{}. Expected fingerprint: {}, got: {}",
                host,
                port,
                expected.fingerprint(),
                server_fingerprint
            );
            return Err(HandlerError(format!(
                "Host key mismatch for {}:{}: expected {}, got {}",
                host,
                port,
                expected.fingerprint(),
                server_fingerprint
            )));
        }

        // 2. Check known_hosts for this host
        if let (Some(host), Some(port)) = (self.host.as_ref(), self.port) {
            if let Some(known_hosts) = self.known_hosts.as_ref() {
                let key = format!("{}:{}", host, port);
                let known_guard = known_hosts.read().await;
                if let Some(known) = known_guard.get(&key) {
                    let stored_fingerprint = known.fingerprint.clone();
                    drop(known_guard);

                    if stored_fingerprint == server_fingerprint {
                        log::debug!("Server key verified from known_hosts for {}:{}", host, port);
                        return Ok(true);
                    } else {
                        log::warn!(
                            "Host key changed for {}:{}. Expected: {}, got: {}",
                            host,
                            port,
                            stored_fingerprint,
                            server_fingerprint
                        );
                        return Err(HandlerError(format!(
                            "Host key changed for {}:{} — stored fingerprint {} does not match server fingerprint {}. \
                             If the server key was legitimately updated, clear the known host entry and reconnect.",
                            host, port, stored_fingerprint, server_fingerprint
                        )));
                    }
                }
            }
        }

        // 3. If we have a verify callback, use it
        if let Some(ref callback) = self.verify_callback {
            let host = self.host.as_deref().unwrap_or("");
            let port = self.port.unwrap_or(22);
            if callback(host.to_string(), port, server_public_key) {
                log::debug!("Server key verified via callback for {}:{}", host, port);
                return Ok(true);
            }
            return Err(HandlerError(
                "Host key rejected by verify callback".to_string(),
            ));
        }

        // 4. First time connection - accept the key (like standard SSH client's StrictHostKeyChecking=accept-new)
        // This is safe for development and matches user expectations
        log::info!(
            "First time connection - accepting server key. Host: {}, Port: {}, Fingerprint: {}",
            self.host.as_deref().unwrap_or("unknown"),
            self.port.unwrap_or(22),
            server_fingerprint
        );
        Ok(true)
    }

    async fn disconnected(
        &mut self,
        reason: DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        let msg = match &reason {
            DisconnectReason::ReceivedDisconnect(info) => {
                format!(
                    "Server sent disconnect: {:?} — {}",
                    info.reason_code, info.message
                )
            }
            DisconnectReason::Error(e) => {
                format!("Connection closed with error: {}", e)
            }
        };
        log::warn!(
            "SSH disconnected ({}:{}): {}",
            self.host.as_deref().unwrap_or("?"),
            self.port.unwrap_or(22),
            msg
        );
        if let Ok(mut guard) = self.disconnect_reason.lock() {
            *guard = Some(msg);
        }
        // Flip the shared liveness flag so the manager can detect the dead
        // session and trigger transparent reconnect on the next SFTP/exec call.
        self.alive.store(false, Ordering::SeqCst);
        // Propagate errors so russh surfaces them; swallow clean server disconnect.
        match reason {
            DisconnectReason::ReceivedDisconnect(_) => Ok(()),
            DisconnectReason::Error(e) => Err(e),
        }
    }
}

/// SSH Connection Manager
#[derive(Clone)]
pub struct SSHConnectionManager {
    connections: Arc<tokio::sync::RwLock<HashMap<String, ActiveConnection>>>,
    saved_connections: Arc<tokio::sync::RwLock<Vec<SavedConnection>>>,
    config_path: std::path::PathBuf,
    /// Known hosts storage
    known_hosts: Arc<tokio::sync::RwLock<HashMap<String, KnownHostEntry>>>,
    known_hosts_path: std::path::PathBuf,
    /// Remote workspace persistence (multiple workspaces)
    remote_workspaces:
        Arc<tokio::sync::RwLock<Vec<crate::service::remote_ssh::types::RemoteWorkspace>>>,
    remote_workspace_path: std::path::PathBuf,
    password_vault: std::sync::Arc<SSHPasswordVault>,
}

impl SSHConnectionManager {
    /// Create a new SSH connection manager
    pub fn new(data_dir: std::path::PathBuf) -> Self {
        let config_path = data_dir.join("ssh_connections.json");
        let known_hosts_path = data_dir.join("known_hosts");
        let remote_workspace_path = data_dir.join("remote_workspace.json");
        let password_vault = std::sync::Arc::new(SSHPasswordVault::new(data_dir));
        Self {
            connections: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            saved_connections: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            config_path,
            known_hosts: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            known_hosts_path,
            remote_workspaces: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            remote_workspace_path,
            password_vault,
        }
    }

    /// Load known hosts from disk
    pub async fn load_known_hosts(&self) -> anyhow::Result<()> {
        if !self.known_hosts_path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.known_hosts_path).await?;
        let entries: Vec<KnownHostEntry> =
            serde_json::from_str(&content).context("Failed to parse known hosts")?;

        let mut guard = self.known_hosts.write().await;
        for entry in entries {
            let key = format!("{}:{}", entry.host, entry.port);
            guard.insert(key, entry);
        }

        Ok(())
    }

    /// Save known hosts to disk
    async fn save_known_hosts(&self) -> anyhow::Result<()> {
        let guard = self.known_hosts.read().await;
        let entries: Vec<_> = guard.values().cloned().collect();

        if let Some(parent) = self.known_hosts_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(&entries)?;
        tokio::fs::write(&self.known_hosts_path, content).await?;
        Ok(())
    }

    /// Add a known host
    pub async fn add_known_host(
        &self,
        host: String,
        port: u16,
        key: &PublicKey,
    ) -> anyhow::Result<()> {
        let entry = KnownHostEntry {
            host: host.clone(),
            port,
            key_type: format!("{:?}", key.name()),
            fingerprint: key.fingerprint(),
            public_key: key
                .public_key_bytes()
                .to_vec()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect(),
        };

        let key = format!("{}:{}", host, port);
        {
            let mut guard = self.known_hosts.write().await;
            guard.insert(key, entry);
        }

        self.save_known_hosts().await
    }

    /// Check if host is in known hosts
    pub async fn is_known_host(&self, host: &str, port: u16) -> bool {
        let key = format!("{}:{}", host, port);
        let guard = self.known_hosts.read().await;
        guard.contains_key(&key)
    }

    /// Get known host entry
    pub async fn get_known_host(&self, host: &str, port: u16) -> Option<KnownHostEntry> {
        let key = format!("{}:{}", host, port);
        let guard = self.known_hosts.read().await;
        guard.get(&key).cloned()
    }

    /// Remove a known host
    pub async fn remove_known_host(&self, host: &str, port: u16) -> anyhow::Result<()> {
        let key = format!("{}:{}", host, port);
        {
            let mut guard = self.known_hosts.write().await;
            guard.remove(&key);
        }
        self.save_known_hosts().await
    }

    /// List all known hosts
    pub async fn list_known_hosts(&self) -> Vec<KnownHostEntry> {
        let guard = self.known_hosts.read().await;
        guard.values().cloned().collect()
    }

    // ── Remote Workspace Persistence ─────────────────────────────────────────────

    /// Load remote workspaces from disk
    pub async fn load_remote_workspace(&self) -> anyhow::Result<()> {
        if !self.remote_workspace_path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.remote_workspace_path).await?;
        // Try array format first, fall back to single-object for backward compat
        let mut workspaces: Vec<crate::service::remote_ssh::types::RemoteWorkspace> =
            serde_json::from_str(&content)
                .or_else(|_| {
                    // Legacy: single workspace object
                    serde_json::from_str::<crate::service::remote_ssh::types::RemoteWorkspace>(
                        &content,
                    )
                    .map(|ws| vec![ws])
                })
                .context("Failed to parse remote workspace(s)")?;

        let before = workspaces.len();
        workspaces.retain(|w| !w.connection_id.is_empty() && !w.remote_path.is_empty());
        if workspaces.len() < before {
            log::warn!(
                "Dropped {} persisted remote workspace(s) with empty connectionId or remotePath",
                before - workspaces.len()
            );
        }

        let mut guard = self.remote_workspaces.write().await;
        *guard = workspaces;

        Ok(())
    }

    /// Save remote workspaces to disk
    async fn save_remote_workspaces(&self) -> anyhow::Result<()> {
        let guard = self.remote_workspaces.read().await;

        if let Some(parent) = self.remote_workspace_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(&*guard)?;
        tokio::fs::write(&self.remote_workspace_path, content).await?;
        Ok(())
    }

    /// Add/update a persisted remote workspace (key = `connection_id` + `remote_path`).
    pub async fn set_remote_workspace(
        &self,
        mut workspace: crate::service::remote_ssh::types::RemoteWorkspace,
    ) -> anyhow::Result<()> {
        workspace.remote_path =
            crate::service::remote_ssh::workspace_state::normalize_remote_workspace_path(
                &workspace.remote_path,
            );
        {
            let mut guard = self.remote_workspaces.write().await;
            let rp = workspace.remote_path.clone();
            let cid = workspace.connection_id.clone();
            guard.retain(|w| {
                !(w.connection_id == cid
                    && crate::service::remote_ssh::workspace_state::normalize_remote_workspace_path(
                        &w.remote_path,
                    ) == rp)
            });
            guard.push(workspace);
        }
        self.save_remote_workspaces().await
    }

    /// Get all persisted remote workspaces
    pub async fn get_remote_workspaces(
        &self,
    ) -> Vec<crate::service::remote_ssh::types::RemoteWorkspace> {
        self.remote_workspaces.read().await.clone()
    }

    /// Get first persisted remote workspace (legacy compat)
    pub async fn get_remote_workspace(
        &self,
    ) -> Option<crate::service::remote_ssh::types::RemoteWorkspace> {
        self.remote_workspaces.read().await.first().cloned()
    }

    /// Remove a specific remote workspace by **connection** + **remote path** (not path alone).
    pub async fn remove_remote_workspace(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> anyhow::Result<()> {
        let rp = crate::service::remote_ssh::workspace_state::normalize_remote_workspace_path(
            remote_path,
        );
        {
            let mut guard = self.remote_workspaces.write().await;
            guard.retain(|w| {
                !(w.connection_id == connection_id
                    && crate::service::remote_ssh::workspace_state::normalize_remote_workspace_path(
                        &w.remote_path,
                    ) == rp)
            });
        }
        self.save_remote_workspaces().await
    }

    /// Clear all remote workspaces
    pub async fn clear_remote_workspace(&self) -> anyhow::Result<()> {
        {
            let mut guard = self.remote_workspaces.write().await;
            guard.clear();
        }
        if self.remote_workspace_path.exists() {
            tokio::fs::remove_file(&self.remote_workspace_path).await?;
        }
        Ok(())
    }

    /// Look up SSH config for a given host alias or hostname
    ///
    /// This parses ~/.ssh/config to find connection parameters for the given host.
    /// The host parameter can be either an alias defined in SSH config or an actual hostname.
    #[cfg(feature = "ssh_config")]
    pub async fn get_ssh_config(&self, host: &str) -> SSHConfigLookupResult {
        let ssh_config_path = dirs::home_dir()
            .map(|p| p.join(".ssh").join("config"))
            .unwrap_or_default();

        if !ssh_config_path.exists() {
            log::debug!("SSH config not found at {:?}", ssh_config_path);
            return SSHConfigLookupResult {
                found: false,
                config: None,
            };
        }

        let config_content = match tokio::fs::read_to_string(&ssh_config_path).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read SSH config: {:?}", e);
                return SSHConfigLookupResult {
                    found: false,
                    config: None,
                };
            }
        };

        let config = match SSHConfig::parse_str(&config_content) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to parse SSH config: {:?}", e);
                return SSHConfigLookupResult {
                    found: false,
                    config: None,
                };
            }
        };

        // Use query() to get host configuration - this handles Host pattern matching
        let host_settings = config.query(host);

        if host_settings.is_empty() {
            log::debug!("No SSH config found for host: {}", host);
            return SSHConfigLookupResult {
                found: false,
                config: None,
            };
        }

        log::debug!(
            "Found SSH config for host: {} with {} settings",
            host,
            host_settings.len()
        );

        // Canonical OpenSSH names; lookup is case-insensitive (see ssh_cfg_get).
        let hostname = ssh_cfg_get(&host_settings, "HostName").map(|s| s.to_string());
        let user = ssh_cfg_get(&host_settings, "User").map(|s| s.to_string());
        let port = ssh_cfg_get(&host_settings, "Port").and_then(|s| s.parse::<u16>().ok());
        let identity_file =
            ssh_cfg_get(&host_settings, "IdentityFile").map(|f| shellexpand::tilde(f).to_string());

        let has_proxy_command = ssh_cfg_has(&host_settings, "ProxyCommand");

        SSHConfigLookupResult {
            found: true,
            config: Some(SSHConfigEntry {
                host: host.to_string(),
                hostname,
                port,
                user,
                identity_file,
                agent: if has_proxy_command { None } else { Some(true) },
            }),
        }
    }

    #[cfg(not(feature = "ssh_config"))]
    pub async fn get_ssh_config(&self, _host: &str) -> SSHConfigLookupResult {
        SSHConfigLookupResult {
            found: false,
            config: None,
        }
    }

    /// List all hosts defined in ~/.ssh/config
    #[cfg(feature = "ssh_config")]
    pub async fn list_ssh_config_hosts(&self) -> Vec<SSHConfigEntry> {
        let ssh_config_path = dirs::home_dir()
            .map(|p| p.join(".ssh").join("config"))
            .unwrap_or_default();

        if !ssh_config_path.exists() {
            log::debug!("SSH config not found at {:?}", ssh_config_path);
            return Vec::new();
        }

        let config_content = match tokio::fs::read_to_string(&ssh_config_path).await {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read SSH config: {:?}", e);
                return Vec::new();
            }
        };

        let config = match SSHConfig::parse_str(&config_content) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to parse SSH config: {:?}", e);
                return Vec::new();
            }
        };

        let mut hosts = Vec::new();

        // SSHConfig library doesn't expose listing all hosts, so we parse the raw config
        // to extract Host entries. This is a simple but effective approach.
        for line in config_content.lines() {
            let line = line.trim();
            // Match "Host alias1 alias2 ..." lines (but not "HostName")
            if line.starts_with("Host ") && !line.starts_with("HostName") {
                // Extract everything after "Host "
                let host_part = line.strip_prefix("Host ").unwrap_or("").trim();
                if host_part.is_empty() {
                    continue;
                }
                // Host can be "alias1 alias2 ..." - we want the first one (main alias)
                let aliases: Vec<&str> = host_part.split_whitespace().collect();
                if aliases.is_empty() {
                    continue;
                }

                let alias = aliases[0];
                // Query config for this host to get details
                let settings = config.query(alias);

                let identity_file = ssh_cfg_get(&settings, "IdentityFile")
                    .map(|f| shellexpand::tilde(f).to_string());

                let hostname = ssh_cfg_get(&settings, "HostName").map(|s| s.to_string());
                let user = ssh_cfg_get(&settings, "User").map(|s| s.to_string());
                let port = ssh_cfg_get(&settings, "Port").and_then(|s| s.parse::<u16>().ok());

                hosts.push(SSHConfigEntry {
                    host: alias.to_string(),
                    hostname,
                    port,
                    user,
                    identity_file,
                    agent: None, // Can't easily determine agent setting from raw parsing
                });
            }
        }

        log::debug!("Found {} hosts in SSH config", hosts.len());
        hosts
    }

    #[cfg(not(feature = "ssh_config"))]
    pub async fn list_ssh_config_hosts(&self) -> Vec<SSHConfigEntry> {
        Vec::new()
    }

    /// Load saved connections from disk
    pub async fn load_saved_connections(&self) -> anyhow::Result<()> {
        log::info!(
            "load_saved_connections: config_path={:?}, exists={}",
            self.config_path,
            self.config_path.exists()
        );

        if !self.config_path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.config_path).await?;
        log::info!("load_saved_connections: content={}", content);
        let saved: Vec<SavedConnection> =
            serde_json::from_str(&content).context("Failed to parse saved SSH connections")?;

        let mut guard = self.saved_connections.write().await;
        *guard = saved;

        log::info!("load_saved_connections: loaded {} connections", guard.len());
        Ok(())
    }

    /// Save connections to disk
    async fn save_connections(&self) -> anyhow::Result<()> {
        log::info!("save_connections: saving to {:?}", self.config_path);
        let guard = self.saved_connections.read().await;
        let content = serde_json::to_string_pretty(&*guard)?;
        log::info!("save_connections: content={}", content);

        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&self.config_path, content).await?;
        log::info!(
            "save_connections: saved {} connections to {:?}",
            guard.len(),
            self.config_path
        );
        Ok(())
    }

    /// Get list of saved connections
    pub async fn get_saved_connections(&self) -> Vec<SavedConnection> {
        self.saved_connections.read().await.clone()
    }

    /// SSH `host` field from the saved profile with this `connection_id` (works when not connected).
    /// Used to resolve session mirror paths when workspace metadata omitted `sshHost`.
    pub async fn get_saved_host_for_connection_id(&self, connection_id: &str) -> Option<String> {
        let cid = connection_id.trim();
        if cid.is_empty() {
            return None;
        }
        let guard = self.saved_connections.read().await;
        guard
            .iter()
            .find(|c| c.id == cid)
            .map(|c| c.host.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Save a connection configuration
    pub async fn save_connection(&self, config: &SSHConnectionConfig) -> anyhow::Result<()> {
        let mut guard = self.saved_connections.write().await;

        // Remove existing entry with same id OR same host+port+username (dedup)
        guard.retain(|c| {
            c.id != config.id
                && !(c.host == config.host
                    && c.port == config.port
                    && c.username == config.username)
        });

        // Add new entry
        guard.push(SavedConnection {
            id: config.id.clone(),
            name: config.name.clone(),
            host: config.host.clone(),
            port: config.port,
            username: config.username.clone(),
            auth_type: match &config.auth {
                SSHAuthMethod::Password { .. } => {
                    crate::service::remote_ssh::types::SavedAuthType::Password
                }
                SSHAuthMethod::PrivateKey { key_path, .. } => {
                    crate::service::remote_ssh::types::SavedAuthType::PrivateKey {
                        key_path: key_path.clone(),
                    }
                }
            },
            default_workspace: config.default_workspace.clone(),
            last_connected: Some(chrono::Utc::now().timestamp() as u64),
        });

        drop(guard);

        match &config.auth {
            SSHAuthMethod::Password { password } => {
                if !password.is_empty() {
                    self.password_vault
                        .store(&config.id, password)
                        .await
                        .with_context(|| format!("store ssh password vault for {}", config.id))?;
                }
            }
            SSHAuthMethod::PrivateKey { .. } => {
                self.password_vault.remove(&config.id).await?;
            }
        }

        self.save_connections().await
    }

    /// Decrypt stored password for password-based saved connections (auto-reconnect).
    pub async fn load_stored_password(
        &self,
        connection_id: &str,
    ) -> anyhow::Result<Option<String>> {
        self.password_vault.load(connection_id).await
    }

    /// Whether the vault has a stored password for this connection (skip auto-reconnect when false).
    pub async fn has_stored_password(&self, connection_id: &str) -> bool {
        match self.load_stored_password(connection_id).await {
            Ok(opt) => opt.is_some(),
            Err(e) => {
                log::warn!("has_stored_password failed for {}: {}", connection_id, e);
                false
            }
        }
    }

    /// Delete a saved connection
    pub async fn delete_saved_connection(&self, connection_id: &str) -> anyhow::Result<()> {
        let mut guard = self.saved_connections.write().await;
        guard.retain(|c| c.id != connection_id);
        drop(guard);
        self.password_vault.remove(connection_id).await?;
        self.save_connections().await
    }

    /// Connect to a remote SSH server
    ///
    /// # Arguments
    /// * `config` - SSH connection configuration
    /// * `timeout_secs` - Connection timeout in seconds (default: 30)
    pub async fn connect(
        &self,
        config: SSHConnectionConfig,
    ) -> anyhow::Result<SSHConnectionResult> {
        self.connect_with_timeout(config, 30).await
    }

    /// Connect with custom timeout
    pub async fn connect_with_timeout(
        &self,
        config: SSHConnectionConfig,
        timeout_secs: u64,
    ) -> anyhow::Result<SSHConnectionResult> {
        let (handle, alive, server_info) = self.establish_session(&config, timeout_secs).await?;

        let connection_id = config.id.clone();

        let mut guard = self.connections.write().await;
        guard.insert(
            connection_id.clone(),
            ActiveConnection {
                handle: Arc::new(handle),
                config,
                server_info: server_info.clone(),
                sftp_session: Arc::new(tokio::sync::RwLock::new(None)),
                server_key: None,
                alive,
                reconnect_lock: Arc::new(tokio::sync::Mutex::new(())),
            },
        );

        Ok(SSHConnectionResult {
            success: true,
            connection_id: Some(connection_id),
            error: None,
            server_info,
        })
    }

    /// Build a fresh SSH session (handshake + auth + server info probe) without
    /// touching the connection map. Reused by both [`Self::connect_with_timeout`]
    /// and the transparent reconnect path in [`Self::ensure_alive_or_reconnect`].
    async fn establish_session(
        &self,
        config: &SSHConnectionConfig,
        timeout_secs: u64,
    ) -> anyhow::Result<(Handle<SSHHandler>, Arc<AtomicBool>, Option<ServerInfo>)> {
        let addr = format!("{}:{}", config.host, config.port);

        // Connect to the server with timeout
        let stream = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| anyhow!("Connection timeout after {} seconds", timeout_secs))?
        .map_err(|e| anyhow!("Failed to connect to {}: {}", addr, e))?;

        // Create SSH transport config
        let key_pair = match &config.auth {
            SSHAuthMethod::Password { .. } => None,
            SSHAuthMethod::PrivateKey {
                key_path,
                passphrase,
            } => {
                log::info!(
                    "Attempting private key auth with key_path: {}, passphrase provided: {}",
                    key_path,
                    passphrase.is_some()
                );
                // Try to read the specified key file
                let expanded = shellexpand::tilde(key_path);
                log::info!("Expanded key path: {}", expanded);
                let key_content = match std::fs::read_to_string(expanded.as_ref()) {
                    Ok(content) => {
                        log::info!("Successfully read {} bytes from key file", content.len());
                        content
                    }
                    Err(e) => {
                        // If specified key fails, try default ~/.ssh/id_rsa
                        log::warn!(
                            "Failed to read private key at '{}': {}, trying default ~/.ssh/id_rsa",
                            expanded,
                            e
                        );
                        if let Ok(home) = std::env::var("HOME") {
                            let default_key = format!("{}/.ssh/id_rsa", home);
                            log::info!("Trying default key at: {}", default_key);
                            std::fs::read_to_string(&default_key).map_err(|e| {
                                anyhow!(
                                    "Failed to read private key '{}' and default key '{}': {}",
                                    key_path,
                                    default_key,
                                    e
                                )
                            })?
                        } else {
                            return Err(anyhow!("Failed to read private key '{}': {}, and could not determine home directory", key_path, e));
                        }
                    }
                };
                log::info!("Decoding private key...");
                let key_pair = russh_keys::decode_secret_key(
                    &key_content,
                    passphrase.as_ref().map(|s| s.as_str()),
                )
                .map_err(|e| anyhow!("Failed to decode private key: {}", e))?;
                log::info!("Successfully decoded private key");
                Some(key_pair)
            }
        };

        let ssh_config = Arc::new(russh::client::Config {
            // Tolerate brief network blips (NAT timeouts, Wi-Fi roaming) by
            // widening the inactivity window and allowing more missed keepalives
            // before declaring the session dead. Combined with transparent
            // reconnect, this prevents the user-visible "early eof" cascade
            // while idly browsing the remote file picker.
            inactivity_timeout: Some(std::time::Duration::from_secs(180)),
            keepalive_interval: Some(std::time::Duration::from_secs(30)),
            keepalive_max: 6,
            // Broad algorithm list for compatibility with both modern and legacy SSH servers.
            // Modern algorithms first (preferred), legacy ones appended as fallback.
            preferred: russh::Preferred {
                // KEX: modern curve25519 first, then older DH groups for legacy servers
                kex: std::borrow::Cow::Owned(vec![
                    russh::kex::CURVE25519,
                    russh::kex::CURVE25519_PRE_RFC_8731,
                    russh::kex::DH_G16_SHA512,
                    russh::kex::DH_G14_SHA256,
                    russh::kex::DH_G14_SHA1, // legacy servers
                    russh::kex::DH_G1_SHA1,  // very old servers
                    russh::kex::EXTENSION_SUPPORT_AS_CLIENT,
                    russh::kex::EXTENSION_OPENSSH_STRICT_KEX_AS_CLIENT,
                ]),
                // Host key algorithms: include ssh-rsa for older servers
                key: std::borrow::Cow::Owned(vec![
                    russh_keys::key::ED25519,
                    russh_keys::key::ECDSA_SHA2_NISTP256,
                    russh_keys::key::ECDSA_SHA2_NISTP521,
                    russh_keys::key::RSA_SHA2_256,
                    russh_keys::key::RSA_SHA2_512,
                    russh_keys::key::SSH_RSA, // legacy servers that only advertise ssh-rsa
                ]),
                ..russh::Preferred::DEFAULT
            },
            ..Default::default()
        });

        // Create handler with known_hosts for verification
        let (handler, disconnect_reason, alive) = SSHHandler::with_known_hosts(
            config.host.clone(),
            config.port,
            self.known_hosts.clone(),
        );

        // SSH handshake with timeout
        log::info!("Starting SSH handshake to {}", addr);
        let connect_result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            russh::client::connect_stream(ssh_config, stream, handler),
        )
        .await
        .map_err(|_| anyhow!("SSH handshake timeout after {} seconds", timeout_secs))?;

        let mut handle = connect_result.map_err(|e| {
            // Try to surface the real disconnect reason captured in the handler.
            // russh's run() absorbs errors; our disconnected() callback stores them.
            let real_reason = disconnect_reason
                .lock()
                .ok()
                .and_then(|g| g.clone());
            if let Some(reason) = real_reason {
                anyhow!("SSH handshake failed: {}", reason)
            } else {
                // HandlerError("Disconnect") with no stored reason means the server
                // closed the TCP connection before sending any SSH banner.
                // This typically means: sshd is not running, max connections reached,
                // or a firewall/IP ban is in effect.
                let e_dbg = format!("{:?}", e);
                if e_dbg.contains("Disconnect") {
                    anyhow!(
                        "SSH connection refused: server {}:{} closed the connection without sending an SSH banner. \
                         Check that sshd is running and accepting connections.",
                        config.host, config.port
                    )
                } else {
                    anyhow!("Failed to establish SSH connection: {:?}", e)
                }
            }
        })?;
        log::info!("SSH handshake completed successfully");

        // Authenticate based on auth method
        log::info!("Starting authentication for user {}", config.username);
        let auth_success: bool = match &config.auth {
            SSHAuthMethod::Password { password } => {
                log::debug!("Using password authentication");
                handle
                    .authenticate_password(&config.username, password.clone())
                    .await
                    .map_err(|e| anyhow!("Password authentication failed: {:?}", e))?
            }
            SSHAuthMethod::PrivateKey {
                key_path,
                passphrase: _,
            } => {
                log::info!("Using public key authentication with key: {}", key_path);
                if let Some(ref key) = key_pair {
                    log::info!(
                        "Attempting to authenticate user '{}' with public key",
                        config.username
                    );
                    let result = handle
                        .authenticate_publickey(&config.username, Arc::new(key.clone()))
                        .await;
                    log::info!("Public key auth result: {:?}", result);
                    match result {
                        Ok(true) => {
                            log::info!("Public key authentication successful");
                            true
                        }
                        Ok(false) => {
                            log::warn!(
                                "Public key authentication rejected by server for user '{}'",
                                config.username
                            );
                            false
                        }
                        Err(e) => {
                            log::error!("Public key authentication error: {:?}", e);
                            return Err(anyhow!("Public key authentication failed: {:?}", e));
                        }
                    }
                } else {
                    return Err(anyhow!("Failed to load private key"));
                }
            }
        };

        if !auth_success {
            log::warn!("Authentication returned false for user {}", config.username);
            return Err(anyhow!(
                "Authentication failed for user {}",
                config.username
            ));
        }
        log::info!("Authentication successful for user {}", config.username);

        // Resolve remote home to an absolute path (SFTP does not expand `~`; never rely on literal `~` in UI).
        let mut server_info = Self::get_server_info_internal(&handle).await;
        if server_info
            .as_ref()
            .map(|s| s.home_dir.trim().is_empty())
            .unwrap_or(true)
        {
            if let Some(home) = Self::probe_remote_home_dir(&handle).await {
                match &mut server_info {
                    Some(si) => si.home_dir = home,
                    None => {
                        server_info = Some(ServerInfo {
                            os_type: "unknown".to_string(),
                            hostname: "unknown".to_string(),
                            home_dir: home,
                        });
                    }
                }
            }
        }

        Ok((handle, alive, server_info))
    }

    /// Get server information (partial lines allowed so we can still fill `home_dir` via [`Self::probe_remote_home_dir`]).
    async fn get_server_info_internal(handle: &Handle<SSHHandler>) -> Option<ServerInfo> {
        let result = Self::execute_command_internal(
            handle,
            "uname -s && hostname && echo $HOME",
            SSHCommandOptions::default(),
        )
        .await
        .ok()?;

        if result.exit_code != 0 {
            return None;
        }

        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        if lines.is_empty() {
            return None;
        }

        Some(ServerInfo {
            os_type: lines[0].to_string(),
            hostname: lines.get(1).unwrap_or(&"").to_string(),
            home_dir: lines.get(2).unwrap_or(&"").to_string(),
        })
    }

    /// Resolve remote home directory via SSH `exec` (tilde and `$HOME` are expanded by the remote shell).
    async fn probe_remote_home_dir(handle: &Handle<SSHHandler>) -> Option<String> {
        const PROBES: &[&str] = &[
            "sh -c 'echo ~'",
            "echo $HOME",
            "bash -lc 'echo ~'",
            "bash -c 'echo ~'",
            "sh -c 'getent passwd \"$(id -un)\" 2>/dev/null | cut -d: -f6'",
        ];
        for cmd in PROBES {
            let Ok(result) =
                Self::execute_command_internal(handle, cmd, SSHCommandOptions::default()).await
            else {
                continue;
            };
            if result.exit_code != 0 {
                continue;
            }
            let first = result.stdout.trim().lines().next().unwrap_or("").trim();
            if first.is_empty() || first == "~" {
                continue;
            }
            return Some(first.to_string());
        }
        None
    }

    /// Execute a command on the remote server
    async fn interrupt_exec_channel(
        session: &russh::Channel<Msg>,
        signal: Sig,
    ) -> anyhow::Result<()> {
        session.signal(signal).await?;
        let _ = session.eof().await;
        Ok(())
    }

    async fn execute_command_internal(
        handle: &Handle<SSHHandler>,
        command: &str,
        options: SSHCommandOptions,
    ) -> std::result::Result<SSHCommandResult, anyhow::Error> {
        let mut session = handle.channel_open_session().await?;
        session.exec(true, command).await?;

        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_status: Option<i32> = None;
        let mut interrupted = false;
        let mut timed_out = false;
        let timeout_deadline = options
            .timeout_ms
            .map(|ms| Instant::now() + Duration::from_millis(ms));
        let mut interrupt_drain_deadline: Option<Instant> = None;

        loop {
            let now = Instant::now();

            if !interrupted
                && options
                    .cancellation_token
                    .as_ref()
                    .is_some_and(|token| token.is_cancelled())
            {
                interrupted = true;
                interrupt_drain_deadline = Some(now + SSH_COMMAND_INTERRUPT_DRAIN_GRACE);
                if let Err(e) = Self::interrupt_exec_channel(&session, Sig::INT).await {
                    log::debug!("Failed to interrupt remote exec channel via SIGINT: {}", e);
                }
            }

            if !timed_out && timeout_deadline.is_some_and(|deadline| now >= deadline) {
                timed_out = true;
                interrupt_drain_deadline = Some(now + SSH_COMMAND_INTERRUPT_DRAIN_GRACE);
                if let Err(e) = Self::interrupt_exec_channel(&session, Sig::INT).await {
                    log::debug!("Failed to interrupt timed out remote exec channel: {}", e);
                }
            }

            let wait_budget = if let Some(deadline) = interrupt_drain_deadline {
                if now >= deadline {
                    let _ = session.close().await;
                    break;
                }
                (deadline - now).min(SSH_COMMAND_WAIT_POLL_INTERVAL)
            } else if let Some(deadline) = timeout_deadline {
                if now >= deadline {
                    SSH_COMMAND_WAIT_POLL_INTERVAL
                } else {
                    (deadline - now).min(SSH_COMMAND_WAIT_POLL_INTERVAL)
                }
            } else {
                SSH_COMMAND_WAIT_POLL_INTERVAL
            };

            let next_msg = match tokio::time::timeout(wait_budget, session.wait()).await {
                Ok(msg) => msg,
                Err(_) => continue,
            };

            match next_msg {
                Some(russh::ChannelMsg::Data { ref data }) => {
                    stdout.push_str(&String::from_utf8_lossy(data));
                }
                Some(russh::ChannelMsg::ExtendedData { ref data, .. }) => {
                    stderr.push_str(&String::from_utf8_lossy(data));
                }
                Some(russh::ChannelMsg::ExitStatus {
                    exit_status: status,
                }) => {
                    exit_status = Some(status as i32);
                }
                Some(russh::ChannelMsg::ExitSignal { signal_name, .. }) => {
                    interrupted = interrupted || matches!(signal_name, Sig::INT | Sig::TERM);
                }
                Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) => {
                    break;
                }
                None => {
                    break;
                }
                _ => {}
            }
        }

        Ok(SSHCommandResult {
            stdout,
            stderr,
            exit_code: exit_status.unwrap_or_else(|| {
                if timed_out {
                    124
                } else if interrupted {
                    130
                } else {
                    -1
                }
            }),
            interrupted,
            timed_out,
        })
    }

    /// Disconnect from a server
    pub async fn disconnect(&self, connection_id: &str) -> anyhow::Result<()> {
        let mut guard = self.connections.write().await;
        guard.remove(connection_id);
        Ok(())
    }

    /// Disconnect all connections
    pub async fn disconnect_all(&self) {
        let mut guard = self.connections.write().await;
        guard.clear();
    }

    /// Check if connected.
    ///
    /// Returns true only when there is an entry in the connections map AND its
    /// liveness flag is still set. A previously-connected session that the
    /// server (or network) tore down is considered NOT connected even though
    /// the entry has not yet been pruned, so the UI cannot mistakenly believe
    /// the session is healthy.
    pub async fn is_connected(&self, connection_id: &str) -> bool {
        let guard = self.connections.read().await;
        guard
            .get(connection_id)
            .map(|c| c.alive.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Ensure the connection is alive; if it was torn down (network blip,
    /// server-side timeout), transparently reconnect using the saved config
    /// and (for password auth) the encrypted password vault.
    ///
    /// Uses a per-connection mutex to prevent reconnect stampedes when many
    /// concurrent SFTP/exec calls hit a dead session at the same time.
    /// Idempotent: returns Ok(()) immediately when the session is already alive.
    async fn ensure_alive_or_reconnect(&self, connection_id: &str) -> anyhow::Result<()> {
        let (alive_flag, reconnect_lock, mut config) = {
            let guard = self.connections.read().await;
            let conn = guard
                .get(connection_id)
                .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;
            (
                conn.alive.clone(),
                conn.reconnect_lock.clone(),
                conn.config.clone(),
            )
        };

        if alive_flag.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Serialize concurrent reconnect attempts for the same connection.
        let _guard = reconnect_lock.lock().await;
        // Re-check under lock; another task may have already restored the session.
        if alive_flag.load(Ordering::SeqCst) {
            return Ok(());
        }

        log::warn!(
            "SSH session {} is dead; attempting transparent reconnect",
            connection_id
        );

        // Refresh the password from the encrypted vault if password auth was
        // configured but the in-memory copy is empty (defensive — covers cases
        // where callers cleared it intentionally).
        if let SSHAuthMethod::Password { ref password } = config.auth {
            if password.is_empty() {
                match self.password_vault.load(connection_id).await {
                    Ok(Some(pwd)) => {
                        config.auth = SSHAuthMethod::Password { password: pwd };
                    }
                    Ok(None) => {
                        return Err(anyhow!(
                            "SSH session {} is dead and no stored password is available for reconnect",
                            connection_id
                        ));
                    }
                    Err(e) => {
                        return Err(anyhow!("Failed to load stored SSH password: {}", e));
                    }
                }
            }
        }

        let (handle, alive, server_info) = self.establish_session(&config, 30).await?;

        // Replace the handle and clear the cached SFTP session so subsequent
        // operations open a fresh channel on the new transport.
        {
            let mut guard = self.connections.write().await;
            if let Some(conn) = guard.get_mut(connection_id) {
                conn.handle = Arc::new(handle);
                conn.alive = alive;
                if let Some(si) = server_info {
                    conn.server_info = Some(si);
                }
                let mut sftp_guard = conn.sftp_session.write().await;
                *sftp_guard = None;
            } else {
                // Entry was removed concurrently (e.g. user-triggered disconnect);
                // nothing to restore.
                return Err(anyhow!(
                    "Connection {} was removed during reconnect",
                    connection_id
                ));
            }
        }

        log::info!("SSH session {} reconnected successfully", connection_id);
        Ok(())
    }

    /// Execute a command on the remote server
    pub async fn execute_command(
        &self,
        connection_id: &str,
        command: &str,
    ) -> anyhow::Result<(String, String, i32)> {
        let result = self
            .execute_command_with_options(connection_id, command, SSHCommandOptions::default())
            .await?;

        if result.timed_out {
            return Err(anyhow!("Command timed out"));
        }
        if result.interrupted {
            return Err(anyhow!("Command was cancelled"));
        }

        Ok((result.stdout, result.stderr, result.exit_code))
    }

    /// Execute a command on the remote server with structured timeout/cancellation handling.
    pub async fn execute_command_with_options(
        &self,
        connection_id: &str,
        command: &str,
        options: SSHCommandOptions,
    ) -> anyhow::Result<SSHCommandResult> {
        self.ensure_alive_or_reconnect(connection_id).await?;
        let handle = {
            let guard = self.connections.read().await;
            guard
                .get(connection_id)
                .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?
                .handle
                .clone()
        };

        Self::execute_command_internal(&handle, command, options)
            .await
            .map_err(|e| anyhow!("Command execution failed: {}", e))
    }

    /// Get server info for a connection
    pub async fn get_server_info(&self, connection_id: &str) -> Option<ServerInfo> {
        let guard = self.connections.read().await;
        guard.get(connection_id).and_then(|c| c.server_info.clone())
    }

    /// If `home_dir` is missing, run [`Self::probe_remote_home_dir`] and persist it on the connection.
    pub async fn resolve_remote_home_if_missing(&self, connection_id: &str) -> Option<ServerInfo> {
        let need_probe = {
            let guard = self.connections.read().await;
            match guard.get(connection_id) {
                None => return None,
                Some(conn) => conn
                    .server_info
                    .as_ref()
                    .map(|s| s.home_dir.trim().is_empty())
                    .unwrap_or(true),
            }
        };
        if !need_probe {
            return self.get_server_info(connection_id).await;
        }
        let handle = {
            let guard = self.connections.read().await;
            guard.get(connection_id)?.handle.clone()
        };
        let Some(home) = Self::probe_remote_home_dir(&handle).await else {
            return self.get_server_info(connection_id).await;
        };
        {
            let mut guard = self.connections.write().await;
            if let Some(conn) = guard.get_mut(connection_id) {
                match conn.server_info.as_mut() {
                    Some(si) => si.home_dir = home.clone(),
                    None => {
                        conn.server_info = Some(ServerInfo {
                            os_type: "unknown".to_string(),
                            hostname: "unknown".to_string(),
                            home_dir: home,
                        });
                    }
                }
            }
        }
        self.get_server_info(connection_id).await
    }

    /// Get connection configuration
    pub async fn get_connection_config(&self, connection_id: &str) -> Option<SSHConnectionConfig> {
        let guard = self.connections.read().await;
        guard.get(connection_id).map(|c| c.config.clone())
    }

    // ============================================================================
    // SFTP Operations
    // ============================================================================

    /// Expand leading `~` using the remote user's home from [`ServerInfo`] (SFTP paths are not shell-expanded).
    pub async fn resolve_sftp_path(
        &self,
        connection_id: &str,
        path: &str,
    ) -> anyhow::Result<String> {
        let path = path.trim();
        if path.is_empty() {
            return Err(anyhow!("Empty remote path"));
        }
        if path == "~" || path.starts_with("~/") {
            let guard = self.connections.read().await;
            let home = guard
                .get(connection_id)
                .and_then(|c| c.server_info.as_ref())
                .map(|s| s.home_dir.trim())
                .filter(|h| !h.is_empty());
            let home = match home {
                Some(h) => h.to_string(),
                None => {
                    return Err(anyhow!(
                        "Cannot use '~' in remote path: home directory is not available for this connection"
                    ));
                }
            };
            if path == "~" || path == "~/" {
                return Ok(home);
            }
            let rest = path[2..].trim_start_matches('/');
            if rest.is_empty() {
                return Ok(home);
            }
            Ok(format!("{}/{}", home.trim_end_matches('/'), rest))
        } else {
            Ok(path.to_string())
        }
    }

    /// Get or create SFTP session for a connection.
    ///
    /// Detects dead transports up-front via [`Self::ensure_alive_or_reconnect`]
    /// so a transient SSH disconnect (e.g. NAT timeout while the user is idly
    /// browsing the remote folder picker) is recovered transparently instead
    /// of cascading into a stale cached SFTP handle that fails forever.
    pub async fn get_sftp(&self, connection_id: &str) -> anyhow::Result<Arc<SftpSession>> {
        self.ensure_alive_or_reconnect(connection_id).await?;

        // First check if we have an existing SFTP session
        {
            let guard = self.connections.read().await;
            if let Some(conn) = guard.get(connection_id) {
                let sftp_guard = conn.sftp_session.read().await;
                if let Some(ref sftp) = *sftp_guard {
                    return Ok(sftp.clone());
                }
            }
        }

        // Get handle (clone the Arc)
        let handle: Arc<Handle<SSHHandler>> = {
            let guard = self.connections.read().await;
            let conn = guard
                .get(connection_id)
                .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;
            conn.handle.clone()
        };

        // Open a channel and request SFTP subsystem
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| anyhow!("Failed to open channel for SFTP: {}", e))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| anyhow!("Failed to request SFTP subsystem: {}", e))?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| anyhow!("Failed to create SFTP session: {}", e))?;

        let sftp = Arc::new(sftp);

        // Store the SFTP session
        {
            let mut guard = self.connections.write().await;
            if let Some(conn) = guard.get_mut(connection_id) {
                let mut sftp_guard = conn.sftp_session.write().await;
                *sftp_guard = Some(sftp.clone());
            }
        }

        Ok(sftp)
    }

    /// Read a file via SFTP
    pub async fn sftp_read(&self, connection_id: &str, path: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        let mut file = sftp
            .open(&path)
            .await
            .map_err(|e| anyhow!("Failed to open remote file '{}': {}", path, e))?;

        let mut buffer = Vec::new();
        use tokio::io::AsyncReadExt;
        file.read_to_end(&mut buffer)
            .await
            .map_err(|e| anyhow!("Failed to read remote file '{}': {}", path, e))?;

        Ok(buffer)
    }

    /// Write a file via SFTP
    pub async fn sftp_write(
        &self,
        connection_id: &str,
        path: &str,
        content: &[u8],
    ) -> anyhow::Result<()> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        let mut file = sftp
            .create(&path)
            .await
            .map_err(|e| anyhow!("Failed to create remote file '{}': {}", path, e))?;

        use tokio::io::AsyncWriteExt;
        file.write_all(content)
            .await
            .map_err(|e| anyhow!("Failed to write remote file '{}': {}", path, e))?;

        file.flush()
            .await
            .map_err(|e| anyhow!("Failed to flush remote file '{}': {}", path, e))?;

        Ok(())
    }

    /// Read directory via SFTP.
    ///
    /// Retries once after dropping the cached SFTP session and forcing a
    /// reconnect attempt, so a stale SFTP channel left over from a prior
    /// network blip does not permanently break the remote folder picker.
    pub async fn sftp_read_dir(&self, connection_id: &str, path: &str) -> anyhow::Result<ReadDir> {
        let resolved = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        match sftp.read_dir(&resolved).await {
            Ok(entries) => Ok(entries),
            Err(first_err) => {
                log::warn!(
                    "SFTP read_dir '{}' failed (will retry once after refreshing session): {}",
                    resolved,
                    first_err
                );
                self.invalidate_sftp_session(connection_id).await;
                // Force the alive flag to false so ensure_alive_or_reconnect rebuilds
                // the underlying SSH transport too — the previous failure may indicate
                // the channel was torn down even though the keepalive callback has not
                // fired yet.
                self.mark_dead(connection_id).await;
                let sftp = self.get_sftp(connection_id).await?;
                sftp.read_dir(&resolved)
                    .await
                    .map_err(|e| anyhow!("Failed to read directory '{}': {}", resolved, e))
            }
        }
    }

    /// Drop the cached SFTP session for a connection so the next call opens a
    /// fresh channel. Safe to call when no session is cached.
    async fn invalidate_sftp_session(&self, connection_id: &str) {
        let guard = self.connections.read().await;
        if let Some(conn) = guard.get(connection_id) {
            let mut sftp_guard = conn.sftp_session.write().await;
            *sftp_guard = None;
        }
    }

    /// Force the liveness flag to false. Triggers a transparent reconnect on
    /// the next call to [`Self::ensure_alive_or_reconnect`].
    async fn mark_dead(&self, connection_id: &str) {
        let guard = self.connections.read().await;
        if let Some(conn) = guard.get(connection_id) {
            conn.alive.store(false, Ordering::SeqCst);
        }
    }

    /// Create directory via SFTP
    pub async fn sftp_mkdir(&self, connection_id: &str, path: &str) -> anyhow::Result<()> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        sftp.create_dir(&path)
            .await
            .map_err(|e| anyhow!("Failed to create directory '{}': {}", path, e))?;
        Ok(())
    }

    /// Create directory and all parents via SFTP
    pub async fn sftp_mkdir_all(&self, connection_id: &str, path: &str) -> anyhow::Result<()> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;

        // Check if path exists
        match sftp.as_ref().try_exists(&path).await {
            Ok(true) => return Ok(()), // Already exists
            Ok(false) => {}
            Err(_) => {}
        }

        // Try to create
        sftp.as_ref()
            .create_dir(&path)
            .await
            .map_err(|e| anyhow!("Failed to create directory '{}': {}", path, e))?;
        Ok(())
    }

    /// Remove file via SFTP
    pub async fn sftp_remove(&self, connection_id: &str, path: &str) -> anyhow::Result<()> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        sftp.remove_file(&path)
            .await
            .map_err(|e| anyhow!("Failed to remove file '{}': {}", path, e))?;
        Ok(())
    }

    /// Remove directory via SFTP
    pub async fn sftp_rmdir(&self, connection_id: &str, path: &str) -> anyhow::Result<()> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        sftp.remove_dir(&path)
            .await
            .map_err(|e| anyhow!("Failed to remove directory '{}': {}", path, e))?;
        Ok(())
    }

    /// Rename/move via SFTP
    pub async fn sftp_rename(
        &self,
        connection_id: &str,
        old_path: &str,
        new_path: &str,
    ) -> anyhow::Result<()> {
        let old_path = self.resolve_sftp_path(connection_id, old_path).await?;
        let new_path = self.resolve_sftp_path(connection_id, new_path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        sftp.rename(&old_path, &new_path)
            .await
            .map_err(|e| anyhow!("Failed to rename '{}' to '{}': {}", old_path, new_path, e))?;
        Ok(())
    }

    /// Check if path exists via SFTP
    pub async fn sftp_exists(&self, connection_id: &str, path: &str) -> anyhow::Result<bool> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        sftp.as_ref()
            .try_exists(&path)
            .await
            .map_err(|e| anyhow!("Failed to check if '{}' exists: {}", path, e))
    }

    /// Get file metadata via SFTP
    pub async fn sftp_stat(
        &self,
        connection_id: &str,
        path: &str,
    ) -> anyhow::Result<russh_sftp::client::fs::Metadata> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        sftp.as_ref()
            .metadata(&path)
            .await
            .map_err(|e| anyhow!("Failed to stat '{}': {}", path, e))
    }

    // ============================================================================
    // PTY (Interactive Terminal) Operations
    // ============================================================================

    /// Open a PTY session and start a shell
    pub async fn open_pty(
        &self,
        connection_id: &str,
        cols: u32,
        rows: u32,
    ) -> anyhow::Result<PTYSession> {
        let guard = self.connections.read().await;
        let conn = guard
            .get(connection_id)
            .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;

        // Open a session channel
        let channel = conn
            .handle
            .channel_open_session()
            .await
            .map_err(|e| anyhow!("Failed to open channel: {}", e))?;

        // Request PTY — `false` = don't wait for reply (reply handled in reader loop)
        channel
            .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
            .await
            .map_err(|e| anyhow!("Failed to request PTY: {}", e))?;

        // Start shell — `false` = don't wait for reply
        channel
            .request_shell(false)
            .await
            .map_err(|e| anyhow!("Failed to start shell: {}", e))?;

        Ok(PTYSession {
            channel: Arc::new(tokio::sync::Mutex::new(channel)),
            connection_id: connection_id.to_string(),
        })
    }

    /// Get server key fingerprint for verification
    pub async fn get_server_key_fingerprint(&self, connection_id: &str) -> anyhow::Result<String> {
        let guard = self.connections.read().await;
        let conn = guard
            .get(connection_id)
            .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;

        // Return a fingerprint based on connection info
        // Note: Actual server key fingerprint requires access to the SSH transport layer
        // For security verification, the server key is verified during connection via SSHHandler
        let fingerprint = format!(
            "{}:{}:{}",
            conn.config.host, conn.config.port, conn.config.username
        );
        Ok(fingerprint)
    }
}

/// PTY session for interactive terminal
#[derive(Clone)]
pub struct PTYSession {
    channel: Arc<tokio::sync::Mutex<russh::Channel<Msg>>>,
    connection_id: String,
}

impl PTYSession {
    /// Extract the inner Channel, consuming the Mutex wrapper.
    /// Only works if this is the sole Arc reference.
    /// Intended for use by RemoteTerminalManager to hand ownership to the owner task.
    pub async fn into_channel(self) -> Option<russh::Channel<Msg>> {
        match Arc::try_unwrap(self.channel) {
            Ok(mutex) => Some(mutex.into_inner()),
            Err(_) => None,
        }
    }
}

impl PTYSession {
    /// Write data to PTY
    pub async fn write(&self, data: &[u8]) -> anyhow::Result<()> {
        let channel = self.channel.lock().await;
        channel
            .data(data)
            .await
            .map_err(|e| anyhow!("Failed to write to PTY: {}", e))?;
        Ok(())
    }

    /// Resize PTY
    pub async fn resize(&self, cols: u32, rows: u32) -> anyhow::Result<()> {
        let channel = self.channel.lock().await;
        // Use default pixel dimensions (80x24 characters)
        channel
            .window_change(cols, rows, 0, 0)
            .await
            .map_err(|e| anyhow!("Failed to resize PTY: {}", e))?;
        Ok(())
    }

    /// Read data from PTY.
    /// Blocks until data is available, PTY closes, or an error occurs.
    /// Returns Ok(Some(bytes)) for data, Ok(None) for clean close, Err for errors.
    pub async fn read(&self) -> anyhow::Result<Option<Vec<u8>>> {
        let mut channel = self.channel.lock().await;
        loop {
            match channel.wait().await {
                Some(russh::ChannelMsg::Data { data }) => return Ok(Some(data.to_vec())),
                Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                    return Ok(Some(data.to_vec()))
                }
                Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) => return Ok(None),
                Some(russh::ChannelMsg::ExitStatus { .. }) => return Ok(None),
                Some(_) => {
                    // WindowAdjust, Success, RequestSuccess, etc. — skip and keep reading
                    continue;
                }
                None => return Ok(None),
            }
        }
    }

    /// Close PTY session
    pub async fn close(self) -> anyhow::Result<()> {
        let channel = self.channel.lock().await;
        channel
            .eof()
            .await
            .map_err(|e| anyhow!("Failed to close PTY: {}", e))?;
        channel
            .close()
            .await
            .map_err(|e| anyhow!("Failed to close channel: {}", e))?;
        Ok(())
    }

    /// Get connection ID
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }
}

// ============================================================================
// Port Forwarding
// ============================================================================

/// Port forwarding entry
#[derive(Debug, Clone)]
pub struct PortForward {
    pub id: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub direction: PortForwardDirection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortForwardDirection {
    Local,   // -L: forward local port to remote
    Remote,  // -R: forward remote port to local
    Dynamic, // -D: dynamic SOCKS proxy
}

/// Port forwarding manager
pub struct PortForwardManager {
    forwards: Arc<tokio::sync::RwLock<HashMap<String, PortForward>>>,
    ssh_manager: Arc<tokio::sync::RwLock<Option<SSHConnectionManager>>>,
}

impl PortForwardManager {
    pub fn new() -> Self {
        Self {
            forwards: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            ssh_manager: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    pub fn with_ssh_manager(ssh_manager: SSHConnectionManager) -> Self {
        Self {
            forwards: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            ssh_manager: Arc::new(tokio::sync::RwLock::new(Some(ssh_manager))),
        }
    }

    pub async fn set_ssh_manager(&self, manager: SSHConnectionManager) {
        let mut guard = self.ssh_manager.write().await;
        *guard = Some(manager);
    }

    /// Start local port forwarding (-L)
    ///
    /// TODO: Full implementation requires:
    /// - TCP listener to accept local connections
    /// - SSH channel for each forwarded connection
    /// - Proper cleanup when stopping the forward
    ///
    /// Currently this is a placeholder that only tracks the forward configuration.
    pub async fn start_local_forward(
        &self,
        _connection_id: &str,
        local_port: u16,
        remote_host: String,
        remote_port: u16,
    ) -> anyhow::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        let forward = PortForward {
            id: id.clone(),
            local_port,
            remote_host: remote_host.clone(),
            remote_port,
            direction: PortForwardDirection::Local,
        };

        // Store forward entry
        let mut guard = self.forwards.write().await;
        guard.insert(id.clone(), forward);

        log::info!(
            "[TODO] Local port forward registered: localhost:{} -> {}:{}",
            local_port,
            remote_host,
            remote_port
        );
        log::warn!("Port forwarding is not fully implemented - connections will not be forwarded");

        Ok(id)
    }

    /// Start remote port forwarding (-R)
    ///
    /// TODO: Full implementation requires SSH reverse port forwarding channel.
    /// This is more complex as it needs to bind to a remote port.
    pub async fn start_remote_forward(
        &self,
        _connection_id: &str,
        remote_port: u16,
        local_host: String,
        local_port: u16,
    ) -> anyhow::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();

        let forward = PortForward {
            id: id.clone(),
            local_port: remote_port,
            remote_host: local_host.clone(),
            remote_port: local_port,
            direction: PortForwardDirection::Remote,
        };

        // Remote port forwarding requires SSH channel forwarding
        // This is a placeholder - full implementation would need:
        // 1. Open a "reverse" channel on SSH connection
        // 2. Bind to remote port
        // 3. Forward connections back through the channel

        let mut guard = self.forwards.write().await;
        guard.insert(id.clone(), forward);

        log::info!(
            "Started remote port forward (placeholder): *:{} -> {}:{}",
            remote_port,
            local_host,
            local_port
        );

        // TODO: Implement actual SSH reverse port forwarding
        log::warn!("Remote port forwarding is not fully implemented - data will not be forwarded");

        Ok(id)
    }

    /// Stop a port forward
    pub async fn stop_forward(&self, forward_id: &str) -> anyhow::Result<()> {
        let mut guard = self.forwards.write().await;
        if let Some(forward) = guard.remove(forward_id) {
            log::info!(
                "Stopped port forward: {} ({}:{} -> {}:{})",
                forward.id,
                match forward.direction {
                    PortForwardDirection::Local => "local",
                    PortForwardDirection::Remote => "remote",
                    PortForwardDirection::Dynamic => "dynamic",
                },
                forward.local_port,
                forward.remote_host,
                forward.remote_port
            );
        }
        Ok(())
    }

    /// Stop all port forwards
    pub async fn stop_all(&self) {
        let mut guard = self.forwards.write().await;
        let count = guard.len();
        guard.drain();
        log::info!("All {} port forwards stopped", count);
    }

    /// List all active forwards
    pub async fn list_forwards(&self) -> Vec<PortForward> {
        let guard = self.forwards.read().await;
        guard.values().cloned().collect()
    }

    /// Check if a port is already forwarded
    pub async fn is_port_forwarded(&self, port: u16) -> bool {
        let guard = self.forwards.read().await;
        guard.values().any(|f| f.local_port == port)
    }
}

impl Default for PortForwardManager {
    fn default() -> Self {
        Self::new()
    }
}
