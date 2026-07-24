//! SSH Connection Manager using russh
//!
//! This module manages SSH connections using the pure-Russ SSH implementation

use crate::remote_ssh::password_vault::SSHPasswordVault;
use crate::remote_ssh::types::{
    ConnectionTestReport, ConnectionTestStage, ContainerAccess, ContainerWorkspaceConfig,
    DockerContainerInfo, SSHAuthMethod, SSHCommandOptions, SSHCommandResult, SSHConfigEntry,
    SSHConfigLookupResult, SSHConnectionConfig, SSHConnectionResult, SavedConnection, ServerInfo,
};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use russh::client::{DisconnectReason, Handle, Handler, Msg};
use russh::Sig;
use russh_keys::key::{KeyPair, PublicKey};
use russh_keys::PublicKeyBase64;
use russh_sftp::client::fs::ReadDir;
use russh_sftp::client::SftpSession;
#[cfg(feature = "ssh_config")]
use ssh_config::SSHConfig;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Once;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{Duration, Instant};

const SSH_COMMAND_WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SSH_COMMAND_INTERRUPT_DRAIN_GRACE: Duration = Duration::from_millis(500);

fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

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

/// Extract the first value from an SSH config directive line, handling double-quoted strings.
///
/// SSH config values may be enclosed in double quotes when they contain whitespace
/// (e.g. `IdentityFile "~/.ssh/my key"`). This function correctly extracts the full
/// quoted value or the first whitespace-delimited token for unquoted values.
fn parse_ssh_config_value(value: &str) -> Option<&str> {
    let value = value.trim();
    if let Some(quoted) = value.strip_prefix('"') {
        if let Some(end) = quoted.find('"') {
            let inner = &quoted[..end];
            return if inner.is_empty() { None } else { Some(inner) };
        }
    }
    value.split_whitespace().next()
}

#[cfg(feature = "ssh_config")]
fn strip_utf8_bom(content: String) -> String {
    content
        .strip_prefix('\u{feff}')
        .unwrap_or(&content)
        .to_string()
}

/// Manually parse `~/.ssh/config` content into Host blocks with their direct settings.
///
/// This is a fallback for when `SSHConfig::parse_str` fails — which happens when the
/// config contains directives (e.g. `Include`, `Match`) before the first `Host` block,
/// because `ssh_config` 0.1's `EntryParser` returns `InvalidHostEntry` for any key-value
/// pair without a preceding `Host` directive, and `parse_str` fails the entire operation
/// on the first parser error.
///
/// Only direct settings within each `Host` block are captured; pattern-matched settings
/// from wildcard blocks (e.g. `Host *`) are not applied. Use `SSHConfig::query()` as a
/// supplementary source when `parse_str` succeeds.
#[cfg(feature = "ssh_config")]
fn parse_ssh_config_manually(content: &str) -> Vec<SSHConfigEntry> {
    let mut hosts = Vec::new();
    let mut current_host: Option<String> = None;
    let mut block_hostname: Option<String> = None;
    let mut block_port: Option<u16> = None;
    let mut block_user: Option<String> = None;
    let mut block_identity_file: Option<String> = None;
    let mut block_certificate_file: Option<String> = None;
    let mut block_proxy_jump: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (keyword, value) = match line.split_once(char::is_whitespace) {
            Some((k, v)) => (k, v.trim()),
            None => continue,
        };

        if keyword.eq_ignore_ascii_case("Host") {
            // Save previous host block
            if let Some(host) = current_host.take() {
                hosts.push(SSHConfigEntry {
                    host,
                    hostname: block_hostname.take(),
                    port: block_port.take(),
                    user: block_user.take(),
                    identity_file: block_identity_file.take(),
                    agent: Some(true),
                    certificate_file: block_certificate_file.take(),
                    proxy_jump: block_proxy_jump.take(),
                });
            }

            // Start new host block — take the first alias
            current_host = value.split_whitespace().next().map(|s| s.to_string());
            block_hostname = None;
            block_port = None;
            block_user = None;
            block_identity_file = None;
            block_certificate_file = None;
            block_proxy_jump = None;
        } else if current_host.is_some() {
            // Track details within the current Host block
            if keyword.eq_ignore_ascii_case("HostName") {
                block_hostname = parse_ssh_config_value(value).map(|s| s.to_string());
            } else if keyword.eq_ignore_ascii_case("Port") {
                block_port = parse_ssh_config_value(value).and_then(|s| s.parse().ok());
            } else if keyword.eq_ignore_ascii_case("User") {
                block_user = parse_ssh_config_value(value).map(|s| s.to_string());
            } else if keyword.eq_ignore_ascii_case("IdentityFile") {
                block_identity_file =
                    parse_ssh_config_value(value).map(|s| shellexpand::tilde(s).to_string());
            } else if keyword.eq_ignore_ascii_case("CertificateFile") {
                block_certificate_file =
                    parse_ssh_config_value(value).map(|s| shellexpand::tilde(s).to_string());
            } else if keyword.eq_ignore_ascii_case("ProxyJump") {
                block_proxy_jump = parse_ssh_config_value(value).map(ToOwned::to_owned);
            }
        }
    }

    // Save last host block
    if let Some(host) = current_host {
        hosts.push(SSHConfigEntry {
            host,
            hostname: block_hostname,
            port: block_port,
            user: block_user,
            identity_file: block_identity_file,
            agent: Some(true),
            certificate_file: block_certificate_file,
            proxy_jump: block_proxy_jump,
        });
    }

    hosts
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
    /// Absent only for a local Docker container workspace.
    handle: Option<Arc<Handle<SSHHandler>>>,
    /// Keep every preceding hop alive while the final transport is using its
    /// direct-tcpip channel as the underlying stream.
    jump_handles: Vec<Arc<Handle<SSHHandler>>>,
    /// User-authored/persisted configuration used for drift detection.
    config: SSHConnectionConfig,
    /// Runtime target after resolving `containerAccess: auto`.
    effective_config: SSHConnectionConfig,
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

struct EstablishedSession {
    handle: Option<Handle<SSHHandler>>,
    jump_handles: Vec<Handle<SSHHandler>>,
    alive: Arc<AtomicBool>,
    server_info: Option<ServerInfo>,
    effective_config: SSHConnectionConfig,
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

fn connection_label(config: &SSHConnectionConfig) -> String {
    format!("{}@{}:{}", config.username, config.host, config.port)
}

fn local_username() -> Option<String> {
    std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_proxy_jump_token(value: &str) -> anyhow::Result<(Option<String>, String, Option<u16>)> {
    let (user, address) = match value.rsplit_once('@') {
        Some((user, address)) if !user.trim().is_empty() => {
            (Some(user.trim().to_string()), address.trim())
        }
        _ => (None, value.trim()),
    };
    if address.is_empty() {
        anyhow::bail!("Invalid ProxyJump entry '{}': host is empty", value);
    }

    if let Some(rest) = address.strip_prefix('[') {
        let close = rest
            .find(']')
            .ok_or_else(|| anyhow!("Invalid ProxyJump entry '{}': missing closing ']'", value))?;
        let host = &rest[..close];
        let suffix = &rest[close + 1..];
        let port = if suffix.is_empty() {
            None
        } else {
            let port = suffix.strip_prefix(':').ok_or_else(|| {
                anyhow!(
                    "Invalid ProxyJump entry '{}': expected :port after ']'",
                    value
                )
            })?;
            Some(
                port.parse::<u16>()
                    .with_context(|| format!("Invalid ProxyJump port in '{}'", value))?,
            )
        };
        return Ok((user, host.to_string(), port));
    }

    let colon_count = address.bytes().filter(|byte| *byte == b':').count();
    let (host, port) = if colon_count == 1 {
        let (host, port) = address
            .rsplit_once(':')
            .expect("one colon must be splittable");
        match port.parse::<u16>() {
            Ok(port) => (host.to_string(), Some(port)),
            Err(_) => (address.to_string(), None),
        }
    } else {
        (address.to_string(), None)
    };
    if host.trim().is_empty() {
        anyhow::bail!("Invalid ProxyJump entry '{}': host is empty", value);
    }
    Ok((user, host, port))
}

fn load_key_pair(auth: &SSHAuthMethod) -> anyhow::Result<Option<KeyPair>> {
    let SSHAuthMethod::PrivateKey {
        key_path,
        passphrase,
        ..
    } = auth
    else {
        return Ok(None);
    };

    let expanded = shellexpand::tilde(key_path);
    let key_content = match std::fs::read_to_string(expanded.as_ref()) {
        Ok(content) => content,
        Err(primary_error) => {
            let default_key = dirs::home_dir()
                .map(|home| home.join(".ssh").join("id_rsa"))
                .ok_or_else(|| {
                    anyhow!(
                        "Failed to read private key '{}': {}, and could not determine the home directory",
                        key_path,
                        primary_error
                    )
                })?;
            std::fs::read_to_string(&default_key).map_err(|fallback_error| {
                anyhow!(
                    "Failed to read private key '{}' ({}) and fallback key '{}' ({})",
                    key_path,
                    primary_error,
                    default_key.display(),
                    fallback_error
                )
            })?
        }
    };
    russh_keys::decode_secret_key(&key_content, passphrase.as_deref())
        .map(Some)
        .map_err(|error| anyhow!("Failed to decode private key '{}': {}", key_path, error))
}

fn build_ssh_client_config() -> Arc<russh::client::Config> {
    Arc::new(russh::client::Config {
        inactivity_timeout: Some(Duration::from_secs(180)),
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 6,
        preferred: russh::Preferred {
            kex: std::borrow::Cow::Owned(vec![
                russh::kex::CURVE25519,
                russh::kex::CURVE25519_PRE_RFC_8731,
                russh::kex::DH_G16_SHA512,
                russh::kex::DH_G14_SHA256,
                russh::kex::DH_G14_SHA1,
                russh::kex::DH_G1_SHA1,
                russh::kex::EXTENSION_SUPPORT_AS_CLIENT,
                russh::kex::EXTENSION_OPENSSH_STRICT_KEX_AS_CLIENT,
            ]),
            key: std::borrow::Cow::Owned(vec![
                russh_keys::key::ED25519,
                russh_keys::key::ECDSA_SHA2_NISTP256,
                russh_keys::key::ECDSA_SHA2_NISTP521,
                russh_keys::key::RSA_SHA2_256,
                russh_keys::key::RSA_SHA2_512,
                russh_keys::key::SSH_RSA,
            ]),
            ..russh::Preferred::DEFAULT
        },
        ..Default::default()
    })
}

async fn authenticate_agent_client<R>(
    handle: &mut Handle<SSHHandler>,
    config: &SSHConnectionConfig,
    stage: &str,
    fingerprint: Option<&str>,
    mut agent: russh_keys::agent::client::AgentClient<R>,
) -> anyhow::Result<bool>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let keys = agent
        .request_identities()
        .await
        .with_context(|| format!("{} could not list SSH agent identities", stage))?;
    let mut matched = 0usize;
    for key in keys {
        if fingerprint.is_some_and(|expected| expected != key.fingerprint()) {
            continue;
        }
        matched += 1;
        let (returned_agent, result) = handle
            .authenticate_future(&config.username, key, agent)
            .await;
        agent = returned_agent;
        if result.with_context(|| format!("{} SSH agent signing failed", stage))? {
            return Ok(true);
        }
    }
    if matched == 0 {
        if let Some(fingerprint) = fingerprint {
            anyhow::bail!(
                "{} SSH agent has no identity with fingerprint '{}'",
                stage,
                fingerprint
            );
        }
    }
    Ok(false)
}

#[cfg(unix)]
async fn authenticate_with_agent(
    handle: &mut Handle<SSHHandler>,
    config: &SSHConnectionConfig,
    stage: &str,
    fingerprint: Option<&str>,
) -> anyhow::Result<bool> {
    let agent = russh_keys::agent::client::AgentClient::connect_env()
        .await
        .with_context(|| format!("{} could not connect to SSH_AUTH_SOCK", stage))?;
    authenticate_agent_client(handle, config, stage, fingerprint, agent).await
}

#[cfg(windows)]
async fn authenticate_with_agent(
    handle: &mut Handle<SSHHandler>,
    config: &SSHConnectionConfig,
    stage: &str,
    fingerprint: Option<&str>,
) -> anyhow::Result<bool> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let pipe = std::env::var("SSH_AUTH_SOCK")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| r"\\.\pipe\openssh-ssh-agent".to_string());
    let stream = ClientOptions::new()
        .open(&pipe)
        .with_context(|| format!("{} could not connect to SSH agent pipe '{}'", stage, pipe))?;
    let agent = russh_keys::agent::client::AgentClient::connect(stream);
    authenticate_agent_client(handle, config, stage, fingerprint, agent).await
}

async fn authenticate_keyboard_interactive(
    handle: &mut Handle<SSHHandler>,
    config: &SSHConnectionConfig,
    stage: &str,
    responses: &[String],
) -> anyhow::Result<bool> {
    use russh::client::KeyboardInteractiveAuthResponse;

    let max_rounds = usize::from(config.options.auth_attempts.max(1));
    let future = async {
        let mut response = handle
            .authenticate_keyboard_interactive_start(&config.username, None)
            .await
            .with_context(|| {
                format!(
                    "{} could not start keyboard-interactive authentication",
                    stage
                )
            })?;
        let mut cursor = 0usize;
        for round in 0..max_rounds {
            match response {
                KeyboardInteractiveAuthResponse::Success => return Ok(true),
                KeyboardInteractiveAuthResponse::Failure => return Ok(false),
                KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                    let count = prompts.len();
                    if cursor + count > responses.len() {
                        let labels = prompts
                            .iter()
                            .map(|prompt| prompt.prompt.trim())
                            .filter(|prompt| !prompt.is_empty())
                            .collect::<Vec<_>>()
                            .join(", ");
                        anyhow::bail!(
                            "{} keyboard-interactive challenge requires {} more response(s){}",
                            stage,
                            count,
                            if labels.is_empty() {
                                String::new()
                            } else {
                                format!(" for: {}", labels)
                            }
                        );
                    }
                    let answers = responses[cursor..cursor + count].to_vec();
                    cursor += count;
                    response = handle
                        .authenticate_keyboard_interactive_respond(answers)
                        .await
                        .with_context(|| {
                            format!("{} keyboard-interactive round {} failed", stage, round + 1)
                        })?;
                }
            }
        }
        anyhow::bail!(
            "{} keyboard-interactive authentication exceeded {} challenge round(s)",
            stage,
            max_rounds
        )
    };
    tokio::time::timeout(
        Duration::from_secs(config.options.auth_timeout_secs.max(1)),
        future,
    )
    .await
    .map_err(|_| {
        anyhow!(
            "{} authentication timed out after {} seconds",
            stage,
            config.options.auth_timeout_secs.max(1)
        )
    })?
}

async fn authenticate_handle(
    handle: &mut Handle<SSHHandler>,
    config: &SSHConnectionConfig,
    stage: &str,
) -> anyhow::Result<()> {
    let authenticate = async {
        Ok::<bool, anyhow::Error>(match &config.auth {
            SSHAuthMethod::Password { password } => handle
                .authenticate_password(&config.username, password.clone())
                .await
                .map_err(|error| {
                    anyhow!("{} password authentication failed: {:?}", stage, error)
                })?,
            SSHAuthMethod::PrivateKey {
                certificate_path, ..
            } => {
                let key_pair = load_key_pair(&config.auth)?
                    .ok_or_else(|| anyhow!("{} private key was not loaded", stage))?;
                if let Some(certificate_path) = certificate_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    let expanded = shellexpand::tilde(certificate_path);
                    let certificate = russh_keys::load_openssh_certificate(expanded.as_ref())
                        .with_context(|| {
                            format!(
                                "{} failed to load OpenSSH certificate '{}'",
                                stage, certificate_path
                            )
                        })?;
                    handle
                        .authenticate_openssh_cert(
                            &config.username,
                            Arc::new(key_pair),
                            certificate,
                        )
                        .await
                        .map_err(|error| {
                            anyhow!("{} certificate authentication failed: {:?}", stage, error)
                        })?
                } else {
                    handle
                        .authenticate_publickey(&config.username, Arc::new(key_pair))
                        .await
                        .map_err(|error| {
                            anyhow!("{} public key authentication failed: {:?}", stage, error)
                        })?
                }
            }
            SSHAuthMethod::Agent {
                key_fingerprint,
                fallback_key_path,
            } => match authenticate_with_agent(handle, config, stage, key_fingerprint.as_deref())
                .await
            {
                Ok(true) => true,
                Ok(false) | Err(_) if fallback_key_path.is_some() => {
                    let fallback = SSHAuthMethod::PrivateKey {
                        key_path: fallback_key_path
                            .clone()
                            .expect("guarded by fallback_key_path.is_some"),
                        passphrase: None,
                        certificate_path: None,
                    };
                    let key_pair = load_key_pair(&fallback)?
                        .ok_or_else(|| anyhow!("{} fallback private key was not loaded", stage))?;
                    handle
                        .authenticate_publickey(&config.username, Arc::new(key_pair))
                        .await
                        .map_err(|error| {
                            anyhow!(
                                "{} SSH agent and fallback key authentication failed: {:?}",
                                stage,
                                error
                            )
                        })?
                }
                Ok(false) => false,
                Err(error) => return Err(error),
            },
            SSHAuthMethod::KeyboardInteractive { responses } => {
                authenticate_keyboard_interactive(handle, config, stage, responses).await?
            }
        })
    };
    let authenticated = tokio::time::timeout(
        Duration::from_secs(config.options.auth_timeout_secs.max(1)),
        authenticate,
    )
    .await
    .map_err(|_| {
        anyhow!(
            "{} authentication timed out after {} seconds",
            stage,
            config.options.auth_timeout_secs.max(1)
        )
    })??;
    if !authenticated {
        anyhow::bail!(
            "{} authentication was rejected for user '{}'",
            stage,
            config.username
        );
    }
    Ok(())
}

fn validate_container_config(container: &ContainerWorkspaceConfig) -> anyhow::Result<()> {
    if container.name.trim().is_empty() {
        anyhow::bail!("Container name or ID is required");
    }
    if container.docker_path.trim().is_empty() {
        anyhow::bail!("Docker executable path is required");
    }
    if container.shell.trim().is_empty() {
        anyhow::bail!("Container shell is required");
    }
    if container.local && matches!(container.access, ContainerAccess::Sshd) {
        anyhow::bail!(
            "A local container with sshd must be configured as an SSH endpoint, not local Docker"
        );
    }
    Ok(())
}

fn docker_exec_args(container: &ContainerWorkspaceConfig, command: &str, tty: bool) -> Vec<String> {
    let mut args = vec!["exec".to_string()];
    if container.interactive || tty {
        args.push("-i".to_string());
    }
    if tty {
        args.push("-t".to_string());
    }
    if let Some(user) = container
        .user
        .as_deref()
        .map(str::trim)
        .filter(|user| !user.is_empty())
    {
        args.push("--user".to_string());
        args.push(user.to_string());
    }
    args.push(container.name.clone());
    args.push(container.shell.clone());
    args.push("-lc".to_string());
    args.push(command.to_string());
    args
}

fn docker_exec_host_command(
    container: &ContainerWorkspaceConfig,
    command: &str,
    tty: bool,
) -> String {
    let mut parts = vec![container.docker_path.clone()];
    parts.extend(docker_exec_args(container, command, tty));
    parts
        .iter()
        .map(String::as_str)
        .map(crate::remote_ssh::shell::quote_arg)
        .collect::<Vec<_>>()
        .join(" ")
}

fn server_info_from_container_probe(
    container: &ContainerWorkspaceConfig,
    result: SSHCommandResult,
) -> anyhow::Result<ServerInfo> {
    if result.exit_code != 0 {
        anyhow::bail!(
            "Docker container '{}' is unavailable or could not start shell '{}': {}",
            container.name,
            container.shell,
            result.stderr.trim()
        );
    }
    let mut lines = result.stdout.lines();
    let os_type = lines.next().unwrap_or("unknown").trim().to_string();
    let hostname = lines.next().unwrap_or(&container.name).trim().to_string();
    let home_dir = lines.next().unwrap_or("/").trim().to_string();
    Ok(ServerInfo {
        os_type,
        hostname: if hostname.is_empty() {
            container.name.clone()
        } else {
            hostname
        },
        home_dir: if home_dir.is_empty() {
            "/".to_string()
        } else {
            home_dir
        },
    })
}

fn parse_docker_published_endpoint(output: &str) -> Option<(String, u16)> {
    output.lines().find_map(|line| {
        let line = line.trim();
        let (host, port) = if let Some(rest) = line.strip_prefix('[') {
            let close = rest.find(']')?;
            let host = &rest[..close];
            let port = rest[close + 1..].strip_prefix(':')?;
            (host, port)
        } else {
            line.rsplit_once(':')?
        };
        let port = port.trim().parse::<u16>().ok()?;
        let host = match host.trim() {
            "" | "0.0.0.0" | "::" => "127.0.0.1",
            host => host,
        };
        Some((host.to_string(), port))
    })
}

fn parse_docker_container_list(output: &str) -> anyhow::Result<Vec<DockerContainerInfo>> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut fields = line.split('\t');
            let id = fields.next().unwrap_or_default().to_string();
            let name = fields.next().unwrap_or_default().to_string();
            let image = fields.next().unwrap_or_default().to_string();
            let status = fields.next().unwrap_or_default().to_string();
            let state = fields.next().unwrap_or_default().to_string();
            if id.is_empty() || name.is_empty() || fields.next().is_some() {
                anyhow::bail!("Docker returned a malformed container list entry");
            }
            Ok(DockerContainerInfo {
                id,
                name,
                image,
                status,
                state,
            })
        })
        .collect()
}

fn resolved_container_config(
    config: &SSHConnectionConfig,
    access: ContainerAccess,
) -> SSHConnectionConfig {
    let mut resolved = config.clone();
    if let Some(container) = resolved.container.as_mut() {
        container.access = access;
        if matches!(container.access, ContainerAccess::Sshd) {
            container.local = false;
        }
    }
    resolved
}

fn workspace_command(config: &SSHConnectionConfig, command: &str, tty: bool) -> String {
    match config.container.as_ref() {
        Some(container)
            if matches!(
                container.access,
                ContainerAccess::DockerExec | ContainerAccess::Auto
            ) =>
        {
            docker_exec_host_command(container, command, tty)
        }
        _ => command.to_string(),
    }
}

fn supervised_container_command(
    container: &ContainerWorkspaceConfig,
    command: &str,
) -> (String, String) {
    let pid_file = format!("/tmp/.bitfun-exec-{}.pid", uuid::Uuid::new_v4());
    let wrapped = supervised_container_command_with_pid_file(container, command, &pid_file);
    (wrapped, pid_file)
}

fn supervised_container_command_with_pid_file(
    container: &ContainerWorkspaceConfig,
    command: &str,
    pid_file: &str,
) -> String {
    let quoted_pid_file = crate::remote_ssh::shell::quote_arg(&pid_file);
    let quoted_command = crate::remote_ssh::shell::quote_arg(command);
    let quoted_shell = crate::remote_ssh::shell::quote_arg(&container.shell);
    format!(
        "pid_file={quoted_pid_file}; \
         child=; \
         tracking=1; \
         (umask 077; : > \"$pid_file\") 2>/dev/null || tracking=0; \
         remove_pid_file() {{ rm -f -- \"$pid_file\" 2>/dev/null || true; }}; \
         terminate_child() {{ \
           [ -n \"$child\" ] || return 0; \
           kill -TERM -- \"-$child\" 2>/dev/null \
             || kill -TERM \"$child\" 2>/dev/null \
             || true; \
         }}; \
         trap remove_pid_file EXIT; \
         trap 'terminate_child; exit 143' HUP TERM; \
         if command -v setsid >/dev/null 2>&1; then \
           setsid {quoted_shell} -lc {quoted_command} <&0 & \
         else \
           {quoted_shell} -lc {quoted_command} <&0 & \
         fi; \
         child=$!; \
         if [ \"$tracking\" -eq 1 ]; then \
           printf '%s' \"$child\" > \"$pid_file\" || tracking=0; \
         fi; \
         wait \"$child\"; status=$?; \
         child=; trap - EXIT HUP TERM; remove_pid_file; exit \"$status\""
    )
}

fn container_signal_command(
    pid_file: &str,
    signal: crate::remote_ssh::WorkspaceProcessSignal,
) -> String {
    let signal_name = match signal {
        crate::remote_ssh::WorkspaceProcessSignal::Interrupt => "INT",
        crate::remote_ssh::WorkspaceProcessSignal::Kill => "KILL",
    };
    let quoted_pid_file = crate::remote_ssh::shell::quote_arg(pid_file);
    format!(
        "pid_file={quoted_pid_file}; \
         attempt=0; \
         while [ ! -s \"$pid_file\" ] && [ \"$attempt\" -lt 20 ]; do \
           attempt=$((attempt + 1)); sleep 0.05; \
         done; \
         [ -s \"$pid_file\" ] || exit 75; \
         pid=$(cat \"$pid_file\" 2>/dev/null) || exit 75; \
         case \"$pid\" in ''|*[!0-9]*) exit 75;; esac; \
         kill -{signal_name} -- \"-$pid\" 2>/dev/null \
           || kill -{signal_name} \"$pid\" 2>/dev/null \
           || true"
    )
}

fn local_container_signal_hook(
    container: ContainerWorkspaceConfig,
    pid_file: String,
) -> crate::remote_ssh::transport::WorkspaceSignalHook {
    Arc::new(move |signal| {
        let container = container.clone();
        let pid_file = pid_file.clone();
        Box::pin(async move {
            let command = container_signal_command(&pid_file, signal);
            let output = tokio::time::timeout(
                Duration::from_secs(3),
                Command::new(&container.docker_path)
                    .args(docker_exec_args(&container, &command, false))
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .output(),
            )
            .await
            .map_err(|_| anyhow!("Timed out signalling the Docker container process"))?
            .with_context(|| {
                format!(
                    "Failed to start Docker executable '{}' for process control",
                    container.docker_path
                )
            })?;
            if !output.status.success() {
                anyhow::bail!(
                    "Docker container process control failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            Ok(())
        })
    })
}

fn remote_container_signal_hook(
    handle: Arc<Handle<SSHHandler>>,
    container: ContainerWorkspaceConfig,
    pid_file: String,
) -> crate::remote_ssh::transport::WorkspaceSignalHook {
    Arc::new(move |signal| {
        let handle = handle.clone();
        let container = container.clone();
        let pid_file = pid_file.clone();
        Box::pin(async move {
            let signal_command = container_signal_command(&pid_file, signal);
            let host_command = docker_exec_host_command(&container, &signal_command, false);
            let result = SSHConnectionManager::execute_command_internal(
                &handle,
                &host_command,
                SSHCommandOptions {
                    timeout_ms: Some(3_000),
                    cancellation_token: None,
                },
            )
            .await?;
            if result.exit_code != 0 {
                anyhow::bail!(
                    "Remote Docker container process control failed: {}",
                    result.stderr.trim()
                );
            }
            Ok(())
        })
    })
}

async fn collect_workspace_command_result(
    transport: crate::remote_ssh::WorkspaceStdio,
    options: SSHCommandOptions,
) -> anyhow::Result<SSHCommandResult> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (mut stdin, mut stdout, mut stderr, control, completion) = transport.into_parts();
    let _ = stdin.shutdown().await;
    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let mut completion_task = tokio::spawn(completion.wait());

    let cancellation = options.cancellation_token.clone();
    let cancelled = async move {
        match cancellation {
            Some(token) => token.cancelled().await,
            None => std::future::pending::<()>().await,
        }
    };
    tokio::pin!(cancelled);
    let timeout = async move {
        match options.timeout_ms {
            Some(timeout_ms) => tokio::time::sleep(Duration::from_millis(timeout_ms)).await,
            None => std::future::pending::<()>().await,
        }
    };
    tokio::pin!(timeout);

    let mut interrupted = false;
    let mut timed_out = false;
    let mut fallback_exit_code = -1;
    let exit = tokio::select! {
        result = &mut completion_task => result.ok(),
        _ = &mut cancelled => {
            interrupted = true;
            fallback_exit_code = 130;
            let _ = control.interrupt().await;
            match tokio::time::timeout(SSH_COMMAND_INTERRUPT_DRAIN_GRACE, &mut completion_task).await {
                Ok(result) => result.ok(),
                Err(_) => {
                    let _ = control.kill().await;
                    match tokio::time::timeout(Duration::from_secs(3), &mut completion_task).await {
                        Ok(result) => result.ok(),
                        Err(_) => {
                            completion_task.abort();
                            None
                        }
                    }
                }
            }
        }
        _ = &mut timeout => {
            timed_out = true;
            fallback_exit_code = 124;
            let _ = control.interrupt().await;
            match tokio::time::timeout(SSH_COMMAND_INTERRUPT_DRAIN_GRACE, &mut completion_task).await {
                Ok(result) => result.ok(),
                Err(_) => {
                    let _ = control.kill().await;
                    match tokio::time::timeout(Duration::from_secs(3), &mut completion_task).await {
                        Ok(result) => result.ok(),
                        Err(_) => {
                            completion_task.abort();
                            None
                        }
                    }
                }
            }
        }
    };
    let stdout = collect_workspace_reader(
        stdout_task,
        "Workspace command stdout reader task failed",
        interrupted || timed_out,
    )
    .await?;
    let stderr = collect_workspace_reader(
        stderr_task,
        "Workspace command stderr reader task failed",
        interrupted || timed_out,
    )
    .await?;
    Ok(SSHCommandResult {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        exit_code: exit
            .and_then(|exit| exit.exit_code)
            .unwrap_or(fallback_exit_code),
        interrupted,
        timed_out,
    })
}

async fn collect_workspace_reader(
    mut task: tokio::task::JoinHandle<std::io::Result<Vec<u8>>>,
    task_error: &'static str,
    allow_incomplete: bool,
) -> anyhow::Result<Vec<u8>> {
    match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
        Ok(result) => Ok(result.context(task_error)??),
        Err(_) if allow_incomplete => {
            task.abort();
            Ok(Vec::new())
        }
        Err(_) => {
            task.abort();
            anyhow::bail!("Workspace command output stream did not close")
        }
    }
}

type ContainerEntryFields = (
    String,
    String,
    String,
    Option<u64>,
    Option<u64>,
    Option<String>,
);

fn split_container_entry_fields<'a>(
    mut fields: impl Iterator<Item = &'a str>,
) -> anyhow::Result<ContainerEntryFields> {
    let name = fields.next().unwrap_or_default().to_string();
    let path = fields.next().unwrap_or_default().to_string();
    let kind = fields.next().unwrap_or_default().to_string();
    let size = fields
        .next()
        .filter(|value| !value.is_empty())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let modified = fields
        .next()
        .filter(|value| !value.is_empty())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| seconds.saturating_mul(1000));
    let permissions = fields
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| value.trim().to_string());
    if name.is_empty() || path.is_empty() || !matches!(kind.as_str(), "d" | "f" | "l") {
        anyhow::bail!("Container directory listing returned a malformed entry");
    }
    Ok((name, path, kind, size, modified, permissions))
}

fn parse_container_dir_output(
    output: &str,
) -> anyhow::Result<Vec<crate::remote_ssh::types::RemoteDirEntry>> {
    let mut fields = output.split('\0').collect::<Vec<_>>();
    while fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    if fields.len() % 6 != 0 {
        anyhow::bail!("Container directory listing returned a malformed record");
    }
    fields
        .chunks(6)
        .map(|record| {
            let fields = split_container_entry_fields(record.iter().copied())?;
            Ok(crate::remote_ssh::types::RemoteDirEntry {
                name: fields.0,
                path: fields.1,
                is_dir: fields.2 == "d",
                is_file: fields.2 == "f",
                is_symlink: fields.2 == "l",
                size: fields.3,
                modified: fields.4,
                permissions: fields.5,
            })
        })
        .collect()
}

fn parse_container_file_output(
    output: &str,
) -> anyhow::Result<Option<crate::remote_ssh::types::RemoteFileEntry>> {
    let mut fields = output.split('\0').collect::<Vec<_>>();
    while fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    if fields.is_empty() {
        return Ok(None);
    }
    if fields.len() != 6 {
        anyhow::bail!("Container stat returned a malformed record");
    }
    let fields = split_container_entry_fields(fields.into_iter())?;
    Ok(Some(crate::remote_ssh::types::RemoteFileEntry {
        name: fields.0,
        path: fields.1,
        is_dir: fields.2 == "d",
        is_file: fields.2 == "f",
        is_symlink: fields.2 == "l",
        size: fields.3,
        modified: fields.4,
        permissions: fields.5,
    }))
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
    remote_workspaces: Arc<tokio::sync::RwLock<Vec<crate::remote_ssh::types::RemoteWorkspace>>>,
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
        let mut workspaces: Vec<crate::remote_ssh::types::RemoteWorkspace> =
            serde_json::from_str(&content)
                .or_else(|_| {
                    // Legacy: single workspace object
                    serde_json::from_str::<crate::remote_ssh::types::RemoteWorkspace>(&content)
                        .map(|ws| vec![ws])
                })
                .context("Failed to parse remote workspace(s)")?;

        // Saved connection IDs used to include the SSH port. The saved-profile
        // loader migrates those IDs before workspace records are loaded, so
        // migrate the corresponding workspace references here as well. Without
        // this, startup pruning would treat a valid legacy workspace as orphaned.
        let saved_ids: Vec<String> = self
            .saved_connections
            .read()
            .await
            .iter()
            .map(|connection| connection.id.clone())
            .collect();
        let mut migrated_connection_ids = 0;
        for workspace in &mut workspaces {
            let Some(stable_id) = Self::migrate_connection_id(&workspace.connection_id) else {
                continue;
            };
            if saved_ids.iter().any(|saved_id| saved_id == &stable_id) {
                log::info!(
                    "Migrating remote workspace connection ID: {} -> {}",
                    workspace.connection_id,
                    stable_id
                );
                workspace.connection_id = stable_id;
                migrated_connection_ids += 1;
            }
        }

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
        drop(guard);

        if migrated_connection_ids > 0 {
            self.save_remote_workspaces().await?;
        }

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
        mut workspace: crate::remote_ssh::types::RemoteWorkspace,
    ) -> anyhow::Result<()> {
        workspace.remote_path =
            crate::remote_ssh::normalize_remote_workspace_path(&workspace.remote_path);
        {
            let mut guard = self.remote_workspaces.write().await;
            let rp = workspace.remote_path.clone();
            let cid = workspace.connection_id.clone();
            guard.retain(|w| {
                !(w.connection_id == cid
                    && crate::remote_ssh::normalize_remote_workspace_path(&w.remote_path) == rp)
            });
            guard.push(workspace);
        }
        self.save_remote_workspaces().await
    }

    /// Get all persisted remote workspaces
    pub async fn get_remote_workspaces(&self) -> Vec<crate::remote_ssh::types::RemoteWorkspace> {
        self.remote_workspaces.read().await.clone()
    }

    /// Drop persisted remote workspace restore entries whose saved SSH profile is gone.
    pub async fn prune_remote_workspaces_without_saved_connections(
        &self,
    ) -> anyhow::Result<Vec<crate::remote_ssh::types::RemoteWorkspace>> {
        let saved_ids: Vec<String> = self
            .saved_connections
            .read()
            .await
            .iter()
            .map(|c| c.id.clone())
            .collect();

        let removed = {
            let mut guard = self.remote_workspaces.write().await;
            let mut removed = Vec::new();
            guard.retain(|w| {
                let keep = saved_ids.iter().any(|id| id == &w.connection_id);
                if !keep {
                    removed.push(w.clone());
                }
                keep
            });
            removed
        };

        if !removed.is_empty() {
            log::warn!(
                "Removed {} persisted remote workspace(s) without saved SSH connection",
                removed.len()
            );
            self.save_remote_workspaces().await?;
        }

        Ok(removed)
    }

    /// Get first persisted remote workspace (legacy compat)
    pub async fn get_remote_workspace(&self) -> Option<crate::remote_ssh::types::RemoteWorkspace> {
        self.remote_workspaces.read().await.first().cloned()
    }

    /// Remove a specific remote workspace by **connection** + **remote path** (not path alone).
    pub async fn remove_remote_workspace(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> anyhow::Result<()> {
        let rp = crate::remote_ssh::normalize_remote_workspace_path(remote_path);
        {
            let mut guard = self.remote_workspaces.write().await;
            guard.retain(|w| {
                !(w.connection_id == connection_id
                    && crate::remote_ssh::normalize_remote_workspace_path(&w.remote_path) == rp)
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
            Ok(c) => strip_utf8_bom(c),
            Err(e) => {
                log::warn!("Failed to read SSH config: {:?}", e);
                return SSHConfigLookupResult {
                    found: false,
                    config: None,
                };
            }
        };

        // Try to parse with the ssh_config crate for pattern-matching query support.
        // If parsing fails, fall back to manual parsing.
        let parsed_config = SSHConfig::parse_str(&config_content).ok();
        if parsed_config.is_none() {
            log::debug!("SSHConfig::parse_str failed, using manual fallback");
        }

        // Try pattern-matched query first (handles Host wildcards like `Host *.example.com`)
        if let Some(ref config) = parsed_config {
            let host_settings = config.query(host);
            if !host_settings.is_empty() {
                log::debug!(
                    "Found SSH config for host: {} with {} settings",
                    host,
                    host_settings.len()
                );

                let hostname = ssh_cfg_get(&host_settings, "HostName").map(|s| s.to_string());
                let user = ssh_cfg_get(&host_settings, "User").map(|s| s.to_string());
                let port = ssh_cfg_get(&host_settings, "Port").and_then(|s| s.parse::<u16>().ok());
                let identity_file = ssh_cfg_get(&host_settings, "IdentityFile")
                    .map(|f| shellexpand::tilde(f).to_string());
                let certificate_file = ssh_cfg_get(&host_settings, "CertificateFile")
                    .map(|f| shellexpand::tilde(f).to_string());
                let has_proxy_command = ssh_cfg_has(&host_settings, "ProxyCommand");
                let proxy_jump = ssh_cfg_get(&host_settings, "ProxyJump").map(ToOwned::to_owned);

                return SSHConfigLookupResult {
                    found: true,
                    config: Some(SSHConfigEntry {
                        host: host.to_string(),
                        hostname,
                        port,
                        user,
                        identity_file,
                        agent: if has_proxy_command { None } else { Some(true) },
                        certificate_file,
                        proxy_jump,
                    }),
                };
            }
        }

        // Fallback: manual lookup for exact host name match
        let hosts = parse_ssh_config_manually(&config_content);
        if let Some(entry) = hosts.into_iter().find(|e| e.host == host) {
            log::debug!("Found SSH config for host: {} (manual fallback)", host);
            return SSHConfigLookupResult {
                found: true,
                config: Some(entry),
            };
        }

        log::debug!("No SSH config found for host: {}", host);
        SSHConfigLookupResult {
            found: false,
            config: None,
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
            Ok(c) => strip_utf8_bom(c),
            Err(e) => {
                log::warn!("Failed to read SSH config: {:?}", e);
                return Vec::new();
            }
        };

        // Try to parse with the ssh_config crate for pattern-matching query support.
        // If parsing fails (e.g. `Include`/`Match` directives before the first `Host`
        // block cause ssh_config 0.1 to return InvalidHostEntry), fall back to manual
        // line-by-line parsing.
        let parsed_config = SSHConfig::parse_str(&config_content).ok();
        if parsed_config.is_none() {
            log::debug!("SSHConfig::parse_str failed, using manual fallback");
        }

        // Always do manual parsing to get the host list (SSHConfig doesn't expose all hosts).
        let mut hosts = parse_ssh_config_manually(&config_content);

        // When the ssh_config crate parsed successfully, override with pattern-matched
        // details (e.g. settings from `Host *` blocks that apply to all hosts).
        if let Some(ref config) = parsed_config {
            for entry in &mut hosts {
                let settings = config.query(&entry.host);
                if !settings.is_empty() {
                    if let Some(h) = ssh_cfg_get(&settings, "HostName") {
                        entry.hostname = Some(h.to_string());
                    }
                    if let Some(p) =
                        ssh_cfg_get(&settings, "Port").and_then(|s| s.parse::<u16>().ok())
                    {
                        entry.port = Some(p);
                    }
                    if let Some(u) = ssh_cfg_get(&settings, "User") {
                        entry.user = Some(u.to_string());
                    }
                    if let Some(f) = ssh_cfg_get(&settings, "IdentityFile") {
                        entry.identity_file = Some(shellexpand::tilde(f).to_string());
                    }
                    if let Some(f) = ssh_cfg_get(&settings, "CertificateFile") {
                        entry.certificate_file = Some(shellexpand::tilde(f).to_string());
                    }
                    if let Some(proxy_jump) = ssh_cfg_get(&settings, "ProxyJump") {
                        entry.proxy_jump = Some(proxy_jump.to_string());
                    }
                }
            }
        }

        log::debug!("Found {} hosts in SSH config", hosts.len());
        hosts
    }

    #[cfg(not(feature = "ssh_config"))]
    pub async fn list_ssh_config_hosts(&self) -> Vec<SSHConfigEntry> {
        Vec::new()
    }

    /// List containers on the Docker host described by a connection form.
    ///
    /// Remote targets establish a temporary SSH/jump-chain session without
    /// saving or activating it. Local targets invoke the configured Docker CLI
    /// directly.
    pub async fn list_docker_containers_for_config(
        &self,
        config: &SSHConnectionConfig,
    ) -> anyhow::Result<Vec<DockerContainerInfo>> {
        let container = config
            .container
            .as_ref()
            .ok_or_else(|| anyhow!("Container configuration is required"))?;
        let format = "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.State}}";
        let output = if container.local {
            tokio::time::timeout(
                Duration::from_secs(config.options.connect_timeout_secs.max(1)),
                Command::new(&container.docker_path)
                    .args(["ps", "-a", "--format", format])
                    .output(),
            )
            .await
            .map_err(|_| anyhow!("Local Docker container listing timed out"))?
            .with_context(|| {
                format!(
                    "Failed to start local Docker executable '{}'",
                    container.docker_path
                )
            })?
        } else {
            let mut host_config = config.clone();
            host_config.container = None;
            let established = self
                .establish_session_with_retries(
                    &host_config,
                    host_config.options.connect_timeout_secs.max(1),
                )
                .await
                .context("Could not connect to the Docker host")?;
            let handle = established
                .handle
                .ok_or_else(|| anyhow!("Docker host SSH handle is unavailable"))?;
            let command = format!(
                "{} ps -a --format {}",
                crate::remote_ssh::shell::quote_arg(&container.docker_path),
                crate::remote_ssh::shell::quote_arg(format)
            );
            let result = Self::execute_command_internal(
                &handle,
                &command,
                SSHCommandOptions {
                    timeout_ms: Some(config.options.connect_timeout_secs.max(1) * 1000),
                    cancellation_token: None,
                },
            )
            .await?;
            if result.exit_code != 0 {
                anyhow::bail!(
                    "Docker container listing failed on SSH host: {}",
                    result.stderr.trim()
                );
            }
            return parse_docker_container_list(&result.stdout);
        };
        if !output.status.success() {
            anyhow::bail!(
                "Docker container listing failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        parse_docker_container_list(&String::from_utf8_lossy(&output.stdout))
    }

    /// Test a connection without saving it and return a stable staged report
    /// suitable for the connection dialog.
    pub async fn test_connection(&self, config: &SSHConnectionConfig) -> ConnectionTestReport {
        let mut stages = Vec::new();
        if config.uses_local_docker() {
            stages.push(ConnectionTestStage {
                id: "docker-host".to_string(),
                label: "local".to_string(),
                success: false,
                error: None,
            });
        } else {
            match self.resolve_proxy_jump_chain(config).await {
                Ok(jumps) => {
                    for (index, jump) in jumps.iter().enumerate() {
                        stages.push(ConnectionTestStage {
                            id: format!("jump-{}", index + 1),
                            label: connection_label(jump),
                            success: false,
                            error: None,
                        });
                    }
                }
                Err(error) => {
                    return ConnectionTestReport {
                        success: false,
                        stages: vec![ConnectionTestStage {
                            id: "configuration".to_string(),
                            label: "ProxyJump".to_string(),
                            success: false,
                            error: Some(error.to_string()),
                        }],
                        server_info: None,
                        resolved_container_access: None,
                    };
                }
            }
            stages.push(ConnectionTestStage {
                id: "target".to_string(),
                label: connection_label(config),
                success: false,
                error: None,
            });
        }
        if let Some(container) = config.container.as_ref() {
            if !matches!(container.access, ContainerAccess::Sshd) || container.local {
                stages.push(ConnectionTestStage {
                    id: "container".to_string(),
                    label: container.name.clone(),
                    success: false,
                    error: None,
                });
            }
        }

        match self
            .establish_session_with_retries(config, config.options.connect_timeout_secs.max(1))
            .await
        {
            Ok(established) => {
                for stage in &mut stages {
                    stage.success = true;
                }
                ConnectionTestReport {
                    success: true,
                    stages,
                    server_info: established.server_info,
                    resolved_container_access: established
                        .effective_config
                        .container
                        .map(|container| container.access),
                }
            }
            Err(error) => {
                let error_text = error.to_string();
                let failing_index = stages
                    .iter()
                    .position(|stage| {
                        (stage.id.starts_with("jump-") && error_text.contains(&stage.label))
                            || (stage.id == "container"
                                && (error_text.to_ascii_lowercase().contains("docker container")
                                    || error_text.to_ascii_lowercase().contains("container sshd")))
                            || (stage.id == "docker-host"
                                && error_text.to_ascii_lowercase().contains("docker")
                                && !error_text.to_ascii_lowercase().contains("container"))
                    })
                    .unwrap_or_else(|| stages.len().saturating_sub(1));
                for stage in stages.iter_mut().take(failing_index) {
                    stage.success = true;
                }
                if let Some(stage) = stages.get_mut(failing_index) {
                    stage.error = Some(error_text);
                }
                ConnectionTestReport {
                    success: false,
                    stages,
                    server_info: None,
                    resolved_container_access: None,
                }
            }
        }
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

        // Migrate old-format connection IDs that include the port
        // (e.g. "ssh-root@host:22") to the new stable format ("ssh-root@host").
        // This ensures historical sessions can still find the connection after
        // the user changes the port.
        let mut migrated_ids = Vec::new();
        for conn in guard.iter_mut() {
            if let Some(new_id) = Self::migrate_connection_id(&conn.id) {
                let old_id = conn.id.clone();
                log::info!("Migrating saved connection ID: {} -> {}", old_id, new_id);
                conn.id = new_id.clone();
                migrated_ids.push((old_id, new_id));
            }
        }
        if !migrated_ids.is_empty() {
            drop(guard);
            for (old_id, new_id) in &migrated_ids {
                if let Err(e) = self.password_vault.migrate_entry(old_id, new_id).await {
                    log::warn!(
                        "Failed to migrate SSH password vault entry from {} to {}: {}",
                        old_id,
                        new_id,
                        e
                    );
                }
            }
            // Persist the migrated IDs to disk.
            if let Err(e) = self.save_connections().await {
                log::warn!("Failed to persist migrated connection IDs: {}", e);
            }
        } else {
            drop(guard);
        }

        let unavailable = self.saved_connections_without_credentials().await;
        if !unavailable.is_empty() {
            log::warn!(
                "Retained {} saved SSH connection(s) that require credential re-entry",
                unavailable.len()
            );
        }

        let guard = self.saved_connections.read().await;
        log::info!("load_saved_connections: loaded {} connections", guard.len());
        Ok(())
    }

    /// If `id` follows the old format `ssh-{user}@{host}:{port}`, return the
    /// new stable format `ssh-{user}@{host}`.  Otherwise return `None`.
    fn migrate_connection_id(id: &str) -> Option<String> {
        if !id.starts_with("ssh-") {
            return None;
        }
        let rest = &id[4..]; // "{user}@{host}:{port}"
        let at_pos = rest.find('@')?;
        let colon_pos = rest.rfind(':')?;
        if colon_pos <= at_pos {
            return None;
        }
        // Verify the suffix after the last colon is a valid port number.
        let port_str = &rest[colon_pos + 1..];
        if port_str.parse::<u16>().is_ok() {
            let stable = format!("ssh-{}", &rest[..colon_pos]);
            // Only return if the ID actually changes (i.e. the port was present).
            if stable != id {
                return Some(stable);
            }
        }
        None
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

    /// Return password profiles that need the user to re-enter credentials.
    ///
    /// Profile and workspace metadata are deliberately retained: an unavailable
    /// vault entry can be caused by an upgrade, keychain reset, or temporary
    /// decryption failure and must never turn startup recovery into data loss.
    async fn saved_connections_without_credentials(&self) -> Vec<String> {
        let saved_snapshot = self.saved_connections.read().await.clone();
        let mut unavailable_ids = Vec::new();
        for conn in saved_snapshot {
            if !matches!(
                conn.auth_type,
                crate::remote_ssh::types::SavedAuthType::Password
            ) || conn
                .container
                .as_ref()
                .is_some_and(|container| container.local)
            {
                continue;
            }
            match self.password_vault.load(&conn.id).await {
                Ok(Some(_)) => {}
                Ok(None) => unavailable_ids.push(conn.id),
                Err(e) => {
                    log::warn!(
                        "Saved SSH password is unavailable; retaining profile for credential re-entry: id={}, error={}",
                        conn.id,
                        e
                    );
                    unavailable_ids.push(conn.id);
                }
            }
        }
        unavailable_ids
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
        match (&config.auth, config.uses_local_docker()) {
            (_, true) => {
                self.password_vault.remove(&config.id).await?;
            }
            (SSHAuthMethod::Password { password }, false) => {
                if password.is_empty() && self.password_vault.load(&config.id).await?.is_none() {
                    anyhow::bail!(
                        "Cannot save password SSH connection without a password or stored vault entry"
                    );
                }
                if !password.is_empty() {
                    self.password_vault
                        .store(&config.id, password)
                        .await
                        .with_context(|| format!("store ssh password vault for {}", config.id))?;
                }
            }
            (SSHAuthMethod::PrivateKey { .. }, false) => {
                self.password_vault.remove(&config.id).await?;
            }
            (SSHAuthMethod::Agent { .. } | SSHAuthMethod::KeyboardInteractive { .. }, false) => {
                self.password_vault.remove(&config.id).await?;
            }
        }

        let mut guard = self.saved_connections.write().await;

        // Remove existing entry with same id OR same host+username (dedup).
        // Using host+username (without port) so that changing the port replaces
        // the old entry instead of creating a duplicate.
        guard.retain(|c| {
            c.id != config.id
                && !(c.host == config.host
                    && c.username == config.username
                    && c.container == config.container)
        });

        // Add new entry
        guard.push(SavedConnection {
            id: config.id.clone(),
            name: config.name.clone(),
            host: config.host.clone(),
            port: config.port,
            username: config.username.clone(),
            auth_type: match &config.auth {
                SSHAuthMethod::Password { .. } => crate::remote_ssh::types::SavedAuthType::Password,
                SSHAuthMethod::PrivateKey {
                    key_path,
                    certificate_path,
                    ..
                } => crate::remote_ssh::types::SavedAuthType::PrivateKey {
                    key_path: key_path.clone(),
                    certificate_path: certificate_path.clone(),
                },
                SSHAuthMethod::Agent {
                    key_fingerprint,
                    fallback_key_path,
                } => crate::remote_ssh::types::SavedAuthType::Agent {
                    key_fingerprint: key_fingerprint.clone(),
                    fallback_key_path: fallback_key_path.clone(),
                },
                SSHAuthMethod::KeyboardInteractive { .. } => {
                    crate::remote_ssh::types::SavedAuthType::KeyboardInteractive
                }
            },
            default_workspace: config.default_workspace.clone(),
            last_connected: Some(chrono::Utc::now().timestamp() as u64),
            proxy_jump: config.proxy_jump.clone(),
            container: config.container.clone(),
            options: config.options.clone(),
        });

        drop(guard);

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
        self.remove_remote_workspaces_for_connections(&[connection_id.to_string()])
            .await?;
        self.save_connections().await
    }

    async fn remove_remote_workspaces_for_connections(
        &self,
        connection_ids: &[String],
    ) -> anyhow::Result<()> {
        if connection_ids.is_empty() {
            return Ok(());
        }
        let removed = {
            let mut guard = self.remote_workspaces.write().await;
            let before = guard.len();
            guard.retain(|w| !connection_ids.iter().any(|id| id == &w.connection_id));
            before - guard.len()
        };
        if removed > 0 {
            log::warn!(
                "Removed {} persisted remote workspace(s) for unavailable SSH connection(s)",
                removed
            );
            self.save_remote_workspaces().await?;
        }
        Ok(())
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
        let timeout_secs = config.options.connect_timeout_secs.max(1);
        self.connect_with_timeout(config, timeout_secs).await
    }

    /// Connect with custom timeout
    pub async fn connect_with_timeout(
        &self,
        config: SSHConnectionConfig,
        timeout_secs: u64,
    ) -> anyhow::Result<SSHConnectionResult> {
        let established = self
            .establish_session_with_retries(&config, timeout_secs)
            .await?;

        let connection_id = config.id.clone();
        let server_info = established.server_info.clone();

        let mut guard = self.connections.write().await;
        guard.insert(
            connection_id.clone(),
            ActiveConnection {
                handle: established.handle.map(Arc::new),
                jump_handles: established.jump_handles.into_iter().map(Arc::new).collect(),
                config,
                effective_config: established.effective_config,
                server_info: server_info.clone(),
                sftp_session: Arc::new(tokio::sync::RwLock::new(None)),
                server_key: None,
                alive: established.alive,
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

    /// Build the effective connection: local Docker, direct SSH, or an SSH
    /// session carried over one or more direct-tcpip jump channels.
    async fn establish_session(
        &self,
        config: &SSHConnectionConfig,
        timeout_secs: u64,
    ) -> anyhow::Result<EstablishedSession> {
        if config.uses_local_docker() {
            if config
                .container
                .as_ref()
                .is_some_and(|container| matches!(container.access, ContainerAccess::Auto))
            {
                if let Some(session) = self
                    .try_establish_local_container_sshd(config, timeout_secs)
                    .await
                {
                    return Ok(session);
                }
            }
            let server_info = self.probe_local_container(config, timeout_secs).await?;
            return Ok(EstablishedSession {
                handle: None,
                jump_handles: Vec::new(),
                alive: Arc::new(AtomicBool::new(true)),
                server_info: Some(server_info),
                effective_config: resolved_container_config(config, ContainerAccess::DockerExec),
            });
        }

        let jumps = self.resolve_proxy_jump_chain(config).await?;
        if jumps.is_empty() {
            let (handle, alive, mut server_info) =
                self.establish_direct_session(config, timeout_secs).await?;
            if config
                .container
                .as_ref()
                .is_some_and(|container| matches!(container.access, ContainerAccess::Auto))
            {
                if let Some((container_handle, container_alive, container_info, effective_config)) =
                    self.try_establish_remote_container_sshd(&handle, config, timeout_secs)
                        .await
                {
                    return Ok(EstablishedSession {
                        handle: Some(container_handle),
                        jump_handles: vec![handle],
                        alive: container_alive,
                        server_info: container_info,
                        effective_config,
                    });
                }
            }
            if config.uses_docker_exec() {
                server_info = self
                    .probe_remote_container(&handle, config, timeout_secs)
                    .await
                    .map(Some)?;
            }
            return Ok(EstablishedSession {
                handle: Some(handle),
                jump_handles: Vec::new(),
                alive,
                server_info,
                effective_config: if config
                    .container
                    .as_ref()
                    .is_some_and(|container| matches!(container.access, ContainerAccess::Auto))
                {
                    resolved_container_config(config, ContainerAccess::DockerExec)
                } else {
                    config.clone()
                },
            });
        }

        let first = jumps
            .first()
            .expect("non-empty jump chain must have a first hop");
        let (first_handle, _, _) = self
            .establish_direct_session(first, timeout_secs)
            .await
            .with_context(|| {
                format!(
                    "Jump 1 ({}) connection or authentication failed",
                    connection_label(first)
                )
            })?;
        let mut jump_handles = vec![first_handle];

        for (index, hop) in jumps.iter().enumerate().skip(1) {
            let previous = jump_handles
                .last()
                .expect("jump handle must exist for the preceding hop");
            let channel = previous
                .channel_open_direct_tcpip(&hop.host, hop.port as u32, "127.0.0.1", 0)
                .await
                .with_context(|| {
                    format!(
                        "Jump {} ({}) could not be reached through jump {}",
                        index + 1,
                        connection_label(hop),
                        index
                    )
                })?;
            let (handle, _, _) = self
                .establish_stream_session(
                    hop,
                    channel.into_stream(),
                    timeout_secs,
                    &format!("jump {} ({})", index + 1, connection_label(hop)),
                )
                .await
                .with_context(|| {
                    format!(
                        "Jump {} ({}) SSH handshake or authentication failed",
                        index + 1,
                        connection_label(hop)
                    )
                })?;
            jump_handles.push(handle);
        }

        let last_jump = jump_handles
            .last()
            .expect("jump handle must exist for final target");
        let channel = last_jump
            .channel_open_direct_tcpip(&config.host, config.port as u32, "127.0.0.1", 0)
            .await
            .with_context(|| {
                format!(
                    "Final target {} could not be reached through jump {}",
                    connection_label(config),
                    jump_handles.len()
                )
            })?;
        let (handle, alive, mut server_info) = self
            .establish_stream_session(
                config,
                channel.into_stream(),
                timeout_secs,
                &format!("final target {}", connection_label(config)),
            )
            .await
            .with_context(|| {
                format!(
                    "Final target {} SSH handshake or authentication failed",
                    connection_label(config)
                )
            })?;
        if config
            .container
            .as_ref()
            .is_some_and(|container| matches!(container.access, ContainerAccess::Auto))
        {
            if let Some((container_handle, container_alive, container_info, effective_config)) =
                self.try_establish_remote_container_sshd(&handle, config, timeout_secs)
                    .await
            {
                jump_handles.push(handle);
                return Ok(EstablishedSession {
                    handle: Some(container_handle),
                    jump_handles,
                    alive: container_alive,
                    server_info: container_info,
                    effective_config,
                });
            }
        }
        if config.uses_docker_exec() {
            server_info = self
                .probe_remote_container(&handle, config, timeout_secs)
                .await
                .map(Some)?;
        }

        Ok(EstablishedSession {
            handle: Some(handle),
            jump_handles,
            alive,
            server_info,
            effective_config: if config
                .container
                .as_ref()
                .is_some_and(|container| matches!(container.access, ContainerAccess::Auto))
            {
                resolved_container_config(config, ContainerAccess::DockerExec)
            } else {
                config.clone()
            },
        })
    }

    async fn establish_session_with_retries(
        &self,
        config: &SSHConnectionConfig,
        timeout_secs: u64,
    ) -> anyhow::Result<EstablishedSession> {
        let attempts = config.options.connect_attempts.max(1);
        let mut last_error = None;
        for attempt in 1..=attempts {
            match self.establish_session(config, timeout_secs).await {
                Ok(session) => return Ok(session),
                Err(error) if attempt < attempts => {
                    log::warn!(
                        "Workspace connection attempt {}/{} failed for {}: {}",
                        attempt,
                        attempts,
                        connection_label(config),
                        error
                    );
                    last_error = Some(error);
                    tokio::time::sleep(Duration::from_millis(
                        250u64.saturating_mul(u64::from(attempt)),
                    ))
                    .await;
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow!("Workspace connection failed")))
    }

    async fn try_establish_local_container_sshd(
        &self,
        config: &SSHConnectionConfig,
        timeout_secs: u64,
    ) -> Option<EstablishedSession> {
        let container = config.container.as_ref()?;
        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            Command::new(&container.docker_path)
                .args(["port", &container.name, "22/tcp"])
                .output(),
        )
        .await
        .ok()?
        .ok()?;
        if !output.status.success() {
            return None;
        }
        let (host, port) =
            parse_docker_published_endpoint(&String::from_utf8_lossy(&output.stdout))?;
        let mut effective_config = resolved_container_config(config, ContainerAccess::Sshd);
        effective_config.host = host.clone();
        effective_config.port = port;
        match self
            .establish_direct_session(&effective_config, timeout_secs)
            .await
        {
            Ok((handle, alive, server_info)) => {
                log::info!(
                    "Container auto access selected sshd: container={}, endpoint={}:{}",
                    container.name,
                    host,
                    port
                );
                Some(EstablishedSession {
                    handle: Some(handle),
                    jump_handles: Vec::new(),
                    alive,
                    server_info,
                    effective_config,
                })
            }
            Err(error) => {
                log::info!(
                    "Container sshd probe failed; falling back to docker exec: container={}, error={}",
                    container.name,
                    error
                );
                None
            }
        }
    }

    async fn try_establish_remote_container_sshd(
        &self,
        docker_host: &Handle<SSHHandler>,
        config: &SSHConnectionConfig,
        timeout_secs: u64,
    ) -> Option<(
        Handle<SSHHandler>,
        Arc<AtomicBool>,
        Option<ServerInfo>,
        SSHConnectionConfig,
    )> {
        let container = config.container.as_ref()?;
        let command = format!(
            "{} port {} 22/tcp",
            crate::remote_ssh::shell::quote_arg(&container.docker_path),
            crate::remote_ssh::shell::quote_arg(&container.name)
        );
        let result = Self::execute_command_internal(
            docker_host,
            &command,
            SSHCommandOptions {
                timeout_ms: Some(timeout_secs.saturating_mul(1000)),
                cancellation_token: None,
            },
        )
        .await
        .ok()?;
        if result.exit_code != 0 {
            return None;
        }
        let (published_host, port) = parse_docker_published_endpoint(&result.stdout)?;
        let channel = docker_host
            .channel_open_direct_tcpip(&published_host, port as u32, "127.0.0.1", 0)
            .await
            .ok()?;
        let mut effective_config = resolved_container_config(config, ContainerAccess::Sshd);
        effective_config.host = format!("{}#{}", config.host, container.name);
        effective_config.port = port;
        match self
            .establish_stream_session(
                &effective_config,
                channel.into_stream(),
                timeout_secs,
                &format!("container sshd {}:{}", container.name, port),
            )
            .await
        {
            Ok((handle, alive, server_info)) => {
                log::info!(
                    "Container auto access selected sshd: container={}, endpoint={}:{}",
                    container.name,
                    published_host,
                    port
                );
                Some((handle, alive, server_info, effective_config))
            }
            Err(error) => {
                log::info!(
                    "Container sshd probe failed; falling back to docker exec: container={}, error={}",
                    container.name,
                    error
                );
                None
            }
        }
    }

    async fn resolve_proxy_jump_chain(
        &self,
        config: &SSHConnectionConfig,
    ) -> anyhow::Result<Vec<SSHConnectionConfig>> {
        let Some(proxy_jump) = config
            .proxy_jump
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
        else {
            return Ok(Vec::new());
        };

        let mut result = Vec::new();
        for (index, value) in proxy_jump.split(',').map(str::trim).enumerate() {
            if value.is_empty() {
                anyhow::bail!("ProxyJump entry {} is empty", index + 1);
            }
            let (user_override, host_token, port_override) = parse_proxy_jump_token(value)?;
            let lookup = self.get_ssh_config(&host_token).await;
            let entry = lookup.config;
            let host = entry
                .as_ref()
                .and_then(|entry| entry.hostname.clone())
                .unwrap_or_else(|| host_token.clone());
            let username = user_override
                .or_else(|| entry.as_ref().and_then(|entry| entry.user.clone()))
                .or_else(local_username)
                .ok_or_else(|| {
                    anyhow!(
                        "ProxyJump entry '{}' has no user; add user@host or User in ~/.ssh/config",
                        value
                    )
                })?;
            let port = port_override
                .or_else(|| entry.as_ref().and_then(|entry| entry.port))
                .unwrap_or(22);
            let identity_file = entry.as_ref().and_then(|entry| entry.identity_file.clone());
            let auth = if identity_file.is_some() {
                SSHAuthMethod::PrivateKey {
                    key_path: identity_file.expect("identity_file.is_some was checked"),
                    passphrase: None,
                    certificate_path: entry
                        .as_ref()
                        .and_then(|entry| entry.certificate_file.clone()),
                }
            } else if matches!(
                &config.auth,
                SSHAuthMethod::Password { .. } | SSHAuthMethod::KeyboardInteractive { .. }
            ) {
                // Explicit runtime challenge responses may also be needed by a
                // bastion. Per-hop users still come from the jump token/config;
                // keys and certificates remain independently configurable in
                // each Host block.
                config.auth.clone()
            } else {
                SSHAuthMethod::Agent {
                    key_fingerprint: None,
                    fallback_key_path: Some("~/.ssh/id_rsa".to_string()),
                }
            };
            result.push(SSHConnectionConfig {
                id: format!("{}-jump-{}", config.id, index + 1),
                name: value.to_string(),
                host,
                port,
                username,
                auth,
                default_workspace: None,
                proxy_jump: None,
                container: None,
                options: config.options.clone(),
            });
        }
        Ok(result)
    }

    async fn establish_stream_session<R>(
        &self,
        config: &SSHConnectionConfig,
        stream: R,
        timeout_secs: u64,
        stage: &str,
    ) -> anyhow::Result<(Handle<SSHHandler>, Arc<AtomicBool>, Option<ServerInfo>)>
    where
        R: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let ssh_config = build_ssh_client_config();
        let (handler, disconnect_reason, alive) = SSHHandler::with_known_hosts(
            config.host.clone(),
            config.port,
            self.known_hosts.clone(),
        );
        let connect_result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            russh::client::connect_stream(ssh_config, stream, handler),
        )
        .await
        .map_err(|_| {
            anyhow!(
                "{} SSH handshake timed out after {} seconds",
                stage,
                timeout_secs
            )
        })?;
        let mut handle = connect_result.map_err(|error| {
            let real_reason = disconnect_reason
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            match real_reason {
                Some(reason) => anyhow!("{} SSH handshake failed: {}", stage, reason),
                None => anyhow!("{} SSH handshake failed: {:?}", stage, error),
            }
        })?;
        authenticate_handle(&mut handle, config, stage).await?;
        let mut server_info = Self::get_server_info_internal(&handle).await;
        if server_info
            .as_ref()
            .map(|info| info.home_dir.trim().is_empty())
            .unwrap_or(true)
        {
            if let Some(home_dir) = Self::probe_remote_home_dir(&handle).await {
                match server_info.as_mut() {
                    Some(info) => info.home_dir = home_dir,
                    None => {
                        server_info = Some(ServerInfo {
                            os_type: "unknown".to_string(),
                            hostname: "unknown".to_string(),
                            home_dir,
                        });
                    }
                }
            }
        }
        Ok((handle, alive, server_info))
    }

    /// Build a fresh direct SSH session (handshake + auth + server info probe)
    /// without touching the connection map.
    async fn establish_direct_session(
        &self,
        config: &SSHConnectionConfig,
        timeout_secs: u64,
    ) -> anyhow::Result<(Handle<SSHHandler>, Arc<AtomicBool>, Option<ServerInfo>)> {
        let addr = format!("{}:{}", config.host, config.port);
        let stream =
            tokio::time::timeout(Duration::from_secs(timeout_secs), TcpStream::connect(&addr))
                .await
                .map_err(|_| anyhow!("Connection timeout after {} seconds", timeout_secs))?
                .map_err(|e| anyhow!("Failed to connect to {}: {}", addr, e))?;
        self.establish_stream_session(
            config,
            stream,
            timeout_secs,
            &format!("target {}", connection_label(config)),
        )
        .await
    }

    async fn probe_remote_container(
        &self,
        handle: &Handle<SSHHandler>,
        config: &SSHConnectionConfig,
        timeout_secs: u64,
    ) -> anyhow::Result<ServerInfo> {
        let container = config
            .container
            .as_ref()
            .ok_or_else(|| anyhow!("Container configuration is missing"))?;
        validate_container_config(container)?;
        let probe = docker_exec_host_command(
            container,
            "printf '%s\\n' \"$(uname -s 2>/dev/null || printf unknown)\" \"$(hostname 2>/dev/null || printf unknown)\" \"$HOME\"",
            false,
        );
        let result = Self::execute_command_internal(
            handle,
            &probe,
            SSHCommandOptions {
                timeout_ms: Some(timeout_secs.saturating_mul(1000)),
                cancellation_token: None,
            },
        )
        .await
        .with_context(|| {
            format!(
                "Docker container '{}' could not be entered on SSH host {}",
                container.name,
                connection_label(config)
            )
        })?;
        server_info_from_container_probe(container, result)
    }

    async fn probe_local_container(
        &self,
        config: &SSHConnectionConfig,
        timeout_secs: u64,
    ) -> anyhow::Result<ServerInfo> {
        let container = config
            .container
            .as_ref()
            .ok_or_else(|| anyhow!("Local Docker configuration is missing"))?;
        validate_container_config(container)?;
        let args = docker_exec_args(
            container,
            "printf '%s\\n' \"$(uname -s 2>/dev/null || printf unknown)\" \"$(hostname 2>/dev/null || printf unknown)\" \"$HOME\"",
            false,
        );
        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            Command::new(&container.docker_path).args(args).output(),
        )
        .await
        .map_err(|_| {
            anyhow!(
                "Local Docker container '{}' probe timed out after {} seconds",
                container.name,
                timeout_secs
            )
        })?
        .with_context(|| {
            format!(
                "Failed to start local Docker executable '{}'",
                container.docker_path
            )
        })?;
        server_info_from_container_probe(
            container,
            SSHCommandResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                interrupted: false,
                timed_out: false,
            },
        )
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
        let execution_started_at = Instant::now();
        let command_preview = if command.len() > 160 {
            format!("{}...", truncate_at_char_boundary(command, 160))
        } else {
            command.to_string()
        };
        log::debug!(
            "Remote exec started: timeout_ms={:?}, has_cancellation={}, command_preview={}",
            options.timeout_ms,
            options.cancellation_token.is_some(),
            command_preview
        );
        let mut session = handle.channel_open_session().await?;
        session.exec(true, command).await?;

        // Keep bytes intact until the channel closes. SSH packets may split a
        // valid UTF-8 code point at any byte boundary; decoding each packet
        // independently would corrupt otherwise valid non-ASCII output.
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_status: Option<i32> = None;
        let mut interrupted = false;
        let mut timed_out = false;
        let stdout_first_chunk_once = Once::new();
        let stderr_first_chunk_once = Once::new();
        let mut eof_logged = false;
        let mut close_logged = false;
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
                log::warn!(
                    "Remote exec cancellation requested: timeout_ms={:?}, stdout_len={}, stderr_len={}, duration_ms={}, command_preview={}",
                    options.timeout_ms,
                    stdout.len(),
                    stderr.len(),
                    execution_started_at.elapsed().as_millis(),
                    command_preview
                );
                if let Err(e) = Self::interrupt_exec_channel(&session, Sig::INT).await {
                    log::debug!("Failed to interrupt remote exec channel via SIGINT: {}", e);
                }
            }

            if !timed_out && timeout_deadline.is_some_and(|deadline| now >= deadline) {
                timed_out = true;
                interrupt_drain_deadline = Some(now + SSH_COMMAND_INTERRUPT_DRAIN_GRACE);
                log::warn!(
                    "Remote exec timeout reached: timeout_ms={:?}, stdout_len={}, stderr_len={}, duration_ms={}, command_preview={}",
                    options.timeout_ms,
                    stdout.len(),
                    stderr.len(),
                    execution_started_at.elapsed().as_millis(),
                    command_preview
                );
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
                    stdout_first_chunk_once.call_once(|| {
                        log::debug!(
                            "Remote exec first stdout chunk received: timeout_ms={:?}, chunk_len={}, duration_ms={}, command_preview={}",
                            options.timeout_ms,
                            data.len(),
                            execution_started_at.elapsed().as_millis(),
                            command_preview
                        );
                    });
                    stdout.extend_from_slice(data);
                }
                Some(russh::ChannelMsg::ExtendedData { ref data, .. }) => {
                    stderr_first_chunk_once.call_once(|| {
                        log::debug!(
                            "Remote exec first stderr chunk received: timeout_ms={:?}, chunk_len={}, duration_ms={}, command_preview={}",
                            options.timeout_ms,
                            data.len(),
                            execution_started_at.elapsed().as_millis(),
                            command_preview
                        );
                    });
                    stderr.extend_from_slice(data);
                }
                Some(russh::ChannelMsg::ExitStatus {
                    exit_status: status,
                }) => {
                    exit_status = Some(status as i32);
                    log::debug!(
                        "Remote exec exit status received: exit_code={}, stdout_len={}, stderr_len={}, duration_ms={}, command_preview={}",
                        status,
                        stdout.len(),
                        stderr.len(),
                        execution_started_at.elapsed().as_millis(),
                        command_preview
                    );
                }
                Some(russh::ChannelMsg::ExitSignal { signal_name, .. }) => {
                    interrupted = interrupted || matches!(signal_name, Sig::INT | Sig::TERM);
                    log::debug!(
                        "Remote exec exit signal received: signal={:?}, stdout_len={}, stderr_len={}, duration_ms={}, command_preview={}",
                        signal_name,
                        stdout.len(),
                        stderr.len(),
                        execution_started_at.elapsed().as_millis(),
                        command_preview
                    );
                }
                Some(russh::ChannelMsg::Eof) => {
                    if !eof_logged {
                        eof_logged = true;
                        log::debug!(
                            "Remote exec EOF received: stdout_len={}, stderr_len={}, duration_ms={}, command_preview={}",
                            stdout.len(),
                            stderr.len(),
                            execution_started_at.elapsed().as_millis(),
                            command_preview
                        );
                    }
                }
                Some(russh::ChannelMsg::Close) => {
                    if !close_logged {
                        close_logged = true;
                        log::debug!(
                            "Remote exec channel close received: stdout_len={}, stderr_len={}, duration_ms={}, command_preview={}",
                            stdout.len(),
                            stderr.len(),
                            execution_started_at.elapsed().as_millis(),
                            command_preview
                        );
                    }
                }
                None => {
                    log::debug!(
                        "Remote exec stream ended: stdout_len={}, stderr_len={}, duration_ms={}, command_preview={}",
                        stdout.len(),
                        stderr.len(),
                        execution_started_at.elapsed().as_millis(),
                        command_preview
                    );
                    break;
                }
                Some(_) => {}
            }
        }

        let result = SSHCommandResult {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            exit_code: match exit_status {
                Some(exit_code) => exit_code,
                None if timed_out => 124,
                None if interrupted => 130,
                None => -1,
            },
            interrupted,
            timed_out,
        };
        log::debug!(
            "Remote exec completed: exit_code={}, interrupted={}, timed_out={}, stdout_len={}, stderr_len={}, duration_ms={}, command_preview={}",
            result.exit_code,
            result.interrupted,
            result.timed_out,
            result.stdout.len(),
            result.stderr.len(),
            execution_started_at.elapsed().as_millis(),
            command_preview
        );

        Ok(result)
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
        let (alive, config) = {
            let guard = self.connections.read().await;
            let Some(connection) = guard.get(connection_id) else {
                return false;
            };
            (
                connection.alive.load(Ordering::SeqCst),
                connection.effective_config.clone(),
            )
        };
        if !alive {
            return false;
        }
        if !config.uses_local_docker() {
            return true;
        }
        let Some(container) = config.container.as_ref() else {
            return false;
        };
        matches!(
            tokio::time::timeout(
                Duration::from_secs(3),
                Command::new(&container.docker_path)
                    .args(["inspect", "--format", "{{.State.Running}}", &container.name])
                    .output(),
            )
            .await,
            Ok(Ok(output)) if output.status.success()
                && String::from_utf8_lossy(&output.stdout).trim() == "true"
        )
    }

    async fn load_connection_config_from_saved(
        &self,
        connection_id: &str,
    ) -> anyhow::Result<Option<SSHConnectionConfig>> {
        let saved = {
            let guard = self.saved_connections.read().await;
            guard.iter().find(|conn| conn.id == connection_id).cloned()
        };

        let Some(saved) = saved else {
            return Ok(None);
        };

        let local_docker = saved
            .container
            .as_ref()
            .is_some_and(|container| container.local);
        let auth = match saved.auth_type {
            crate::remote_ssh::types::SavedAuthType::Password => {
                let password = if local_docker {
                    String::new()
                } else {
                    self.password_vault.load(connection_id).await?.ok_or_else(|| {
                        anyhow!(
                            "Saved SSH connection {} requires a password, but no stored vault entry is available",
                            connection_id
                        )
                    })?
                };
                SSHAuthMethod::Password { password }
            }
            crate::remote_ssh::types::SavedAuthType::PrivateKey {
                key_path,
                certificate_path,
            } => SSHAuthMethod::PrivateKey {
                key_path,
                passphrase: None,
                certificate_path,
            },
            crate::remote_ssh::types::SavedAuthType::Agent {
                key_fingerprint,
                fallback_key_path,
            } => SSHAuthMethod::Agent {
                key_fingerprint,
                fallback_key_path,
            },
            crate::remote_ssh::types::SavedAuthType::KeyboardInteractive => {
                SSHAuthMethod::KeyboardInteractive {
                    responses: Vec::new(),
                }
            }
        };

        Ok(Some(SSHConnectionConfig {
            id: saved.id,
            name: saved.name,
            host: saved.host,
            port: saved.port,
            username: saved.username,
            auth,
            default_workspace: saved.default_workspace,
            proxy_jump: saved.proxy_jump,
            container: saved.container,
            options: saved.options,
        }))
    }

    /// Ensure the connection is alive; if it was torn down (network blip,
    /// server-side timeout), transparently reconnect using the saved config
    /// and (for password auth) the encrypted password vault.
    ///
    /// Also detects config drift (e.g. the user changed the port after the
    /// connection was established) and forces a reconnect with the updated
    /// parameters so that historical sessions never use a stale port.
    ///
    /// Uses a per-connection mutex to prevent reconnect stampedes when many
    /// concurrent SFTP/exec calls hit a dead session at the same time.
    /// Idempotent: returns Ok(()) immediately when the session is already alive
    /// **and** its config matches the latest saved profile.
    async fn ensure_alive_or_reconnect(&self, connection_id: &str) -> anyhow::Result<()> {
        // Always read the latest saved config — this is the source of truth
        // after the user edits a connection (e.g. changes the port).
        let saved_config = self
            .load_connection_config_from_saved(connection_id)
            .await?;

        let (alive_flag, reconnect_lock, active_config) = {
            let guard = self.connections.read().await;
            if let Some(conn) = guard.get(connection_id) {
                (
                    conn.alive.clone(),
                    conn.reconnect_lock.clone(),
                    Some(conn.config.clone()),
                )
            } else {
                (
                    Arc::new(AtomicBool::new(false)),
                    Arc::new(tokio::sync::Mutex::new(())),
                    None,
                )
            }
        };

        // If the connection is alive, check for config drift before returning.
        if alive_flag.load(Ordering::SeqCst) {
            if let Some(ref saved) = saved_config {
                if let Some(ref active) = active_config {
                    if !saved.connection_params_equal(active) {
                        log::warn!(
                            "SSH config for {} has drifted (e.g. port {} -> {}), forcing reconnect",
                            connection_id,
                            active.port,
                            saved.port
                        );
                        // Mark as dead so the reconnect path below is taken.
                        alive_flag.store(false, Ordering::SeqCst);
                    } else {
                        return Ok(());
                    }
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }

        // Serialize concurrent reconnect attempts for the same connection.
        let _guard = reconnect_lock.lock().await;
        // Re-check under lock; another task may have already restored the session.
        if alive_flag.load(Ordering::SeqCst) {
            // Re-check config drift under lock as well.
            if let Some(ref saved) = saved_config {
                let guard = self.connections.read().await;
                if let Some(conn) = guard.get(connection_id) {
                    if saved.connection_params_equal(&conn.config) {
                        return Ok(());
                    }
                }
            } else {
                return Ok(());
            }
        }

        // Prefer the latest saved config for reconnection; fall back to the
        // active config only when no saved profile exists (should be rare).
        let mut config = match saved_config {
            Some(c) => c,
            None => active_config.ok_or_else(|| {
                anyhow!(
                    "Connection {} not found and no saved SSH profile is available",
                    connection_id
                )
            })?,
        };

        let is_existing_connection = {
            let guard = self.connections.read().await;
            guard.contains_key(connection_id)
        };
        if is_existing_connection {
            log::warn!(
                "SSH session {} is dead; attempting transparent reconnect",
                connection_id
            );
        } else {
            log::info!(
                "SSH session {} is not active; attempting to connect using saved SSH profile",
                connection_id
            );
        }

        // Refresh the password from the encrypted vault if password auth was
        // configured but the in-memory copy is empty (defensive — covers cases
        // where callers cleared it intentionally).
        if !config.uses_local_docker() {
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
        }

        let established = self
            .establish_session_with_retries(&config, config.options.connect_timeout_secs.max(1))
            .await?;
        let server_info = established.server_info.clone();

        // Replace the handle, update the config to the latest saved version,
        // and clear the cached SFTP session so subsequent operations open a
        // fresh channel on the new transport.
        {
            let mut guard = self.connections.write().await;
            if let Some(conn) = guard.get_mut(connection_id) {
                conn.handle = established.handle.map(Arc::new);
                conn.jump_handles = established.jump_handles.into_iter().map(Arc::new).collect();
                conn.config = config;
                conn.effective_config = established.effective_config;
                conn.alive = established.alive;
                if let Some(si) = server_info.as_ref() {
                    conn.server_info = Some(si.clone());
                }
                let mut sftp_guard = conn.sftp_session.write().await;
                *sftp_guard = None;
            } else {
                guard.insert(
                    connection_id.to_string(),
                    ActiveConnection {
                        handle: established.handle.map(Arc::new),
                        jump_handles: established.jump_handles.into_iter().map(Arc::new).collect(),
                        config,
                        effective_config: established.effective_config,
                        server_info,
                        sftp_session: Arc::new(tokio::sync::RwLock::new(None)),
                        server_key: None,
                        alive: established.alive,
                        reconnect_lock: Arc::new(tokio::sync::Mutex::new(())),
                    },
                );
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
        let (handle, config) = {
            let guard = self.connections.read().await;
            let connection = guard
                .get(connection_id)
                .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;
            (
                connection.handle.clone(),
                connection.effective_config.clone(),
            )
        };

        if config.uses_docker_exec() {
            let transport = self.open_workspace_stdio(connection_id, command).await?;
            return collect_workspace_command_result(transport, options).await;
        }
        let handle =
            handle.ok_or_else(|| anyhow!("SSH handle is unavailable for {}", connection_id))?;
        let command = workspace_command(&config, command, false);
        Self::execute_command_internal(&handle, &command, options)
            .await
            .map_err(|error| anyhow!("Command execution failed: {}", error))
    }

    /// Open a long-lived non-PTY exec channel for streaming stdin/stdout protocols.
    pub async fn open_exec_channel(
        &self,
        connection_id: &str,
        command: &str,
    ) -> anyhow::Result<russh::Channel<Msg>> {
        self.ensure_alive_or_reconnect(connection_id).await?;
        let (handle, config) = {
            let guard = self.connections.read().await;
            let connection = guard
                .get(connection_id)
                .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;
            (
                connection.handle.clone(),
                connection.effective_config.clone(),
            )
        };
        if config.uses_local_docker() {
            anyhow::bail!("Local Docker execution does not use an SSH channel");
        }
        let handle =
            handle.ok_or_else(|| anyhow!("SSH handle is unavailable for {}", connection_id))?;
        let command = workspace_command(&config, command, false);

        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| anyhow!("Failed to open SSH exec channel: {}", e))?;
        channel
            .exec(true, command.as_str())
            .await
            .map_err(|e| anyhow!("Failed to start remote command: {}", e))?;
        Ok(channel)
    }

    /// Open a transport-neutral, long-lived stdio process in the effective
    /// workspace target.
    ///
    /// SSH hosts and remote Docker containers are backed by an SSH channel.
    /// Local Docker containers are backed by a supervised local `docker exec`
    /// child. Callers receive the same stdin/stdout/stderr/control/completion
    /// contract for either target.
    pub async fn open_workspace_stdio(
        &self,
        connection_id: &str,
        command: &str,
    ) -> anyhow::Result<crate::remote_ssh::WorkspaceStdio> {
        self.ensure_alive_or_reconnect(connection_id).await?;
        let (handle, config) = {
            let guard = self.connections.read().await;
            let connection = guard
                .get(connection_id)
                .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;
            (
                connection.handle.clone(),
                connection.effective_config.clone(),
            )
        };
        if config.uses_docker_exec() {
            let container = config
                .container
                .as_ref()
                .ok_or_else(|| anyhow!("Docker container configuration is missing"))?;
            validate_container_config(container)?;
            let (supervised_command, pid_file) = supervised_container_command(container, command);
            if config.uses_local_docker() {
                let signal_hook = local_container_signal_hook(container.clone(), pid_file);
                return crate::remote_ssh::WorkspaceStdio::spawn_local_process_with_signal_hook(
                    &container.docker_path,
                    &docker_exec_args(container, &supervised_command, false),
                    Some(signal_hook),
                );
            }

            let handle =
                handle.ok_or_else(|| anyhow!("SSH handle is unavailable for {}", connection_id))?;
            let host_command = docker_exec_host_command(container, &supervised_command, false);
            let channel = handle
                .channel_open_session()
                .await
                .map_err(|error| anyhow!("Failed to open SSH exec channel: {}", error))?;
            channel
                .exec(true, host_command.as_str())
                .await
                .map_err(|error| anyhow!("Failed to start remote Docker command: {}", error))?;
            let signal_hook = remote_container_signal_hook(handle, container.clone(), pid_file);
            return Ok(
                crate::remote_ssh::WorkspaceStdio::from_ssh_channel_with_signal_hook(
                    channel,
                    Some(signal_hook),
                ),
            );
        }

        let channel = self.open_exec_channel(connection_id, command).await?;
        Ok(crate::remote_ssh::WorkspaceStdio::from_ssh_channel(channel))
    }

    /// Open a long-lived exec channel with a PTY attached.
    ///
    /// This gives the command TTY semantics without starting an interactive shell
    /// and typing the command into it, so command wrappers are not echoed into
    /// model-visible output.
    pub async fn open_pty_exec_channel(
        &self,
        connection_id: &str,
        command: &str,
        cols: u32,
        rows: u32,
    ) -> anyhow::Result<russh::Channel<Msg>> {
        self.ensure_alive_or_reconnect(connection_id).await?;
        let (handle, config) = {
            let guard = self.connections.read().await;
            let connection = guard
                .get(connection_id)
                .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;
            (
                connection.handle.clone(),
                connection.effective_config.clone(),
            )
        };
        if config.uses_local_docker() {
            anyhow::bail!("Local Docker execution does not use an SSH channel");
        }
        let handle =
            handle.ok_or_else(|| anyhow!("SSH handle is unavailable for {}", connection_id))?;
        let command = workspace_command(&config, command, true);

        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| anyhow!("Failed to open SSH PTY exec channel: {}", e))?;
        channel
            .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
            .await
            .map_err(|e| anyhow!("Failed to request PTY for remote command: {}", e))?;
        channel
            .exec(true, command.as_str())
            .await
            .map_err(|e| anyhow!("Failed to start remote PTY command: {}", e))?;
        Ok(channel)
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
        let Ok((output, _, status)) = self
            .execute_command(connection_id, "printf '%s' \"$HOME\"")
            .await
        else {
            return self.get_server_info(connection_id).await;
        };
        let home = output.trim().to_string();
        if status != 0 || home.is_empty() || home == "~" {
            return self.get_server_info(connection_id).await;
        }
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

    pub async fn get_effective_connection_config(
        &self,
        connection_id: &str,
    ) -> Option<SSHConnectionConfig> {
        let guard = self.connections.read().await;
        guard
            .get(connection_id)
            .map(|connection| connection.effective_config.clone())
    }

    pub async fn is_local_container_connection(&self, connection_id: &str) -> bool {
        self.get_effective_connection_config(connection_id)
            .await
            .is_some_and(|config| config.uses_local_docker())
    }

    pub async fn is_container_workspace(&self, connection_id: &str) -> bool {
        self.get_effective_connection_config(connection_id)
            .await
            .is_some_and(|config| config.uses_docker_exec())
    }

    pub async fn local_container_exec_spec(
        &self,
        connection_id: &str,
        command: &str,
        tty: bool,
    ) -> anyhow::Result<Option<(String, Vec<String>)>> {
        let Some(config) = self.get_effective_connection_config(connection_id).await else {
            anyhow::bail!("Connection {} not found", connection_id);
        };
        if !config.uses_local_docker() {
            return Ok(None);
        }
        let container = config
            .container
            .as_ref()
            .ok_or_else(|| anyhow!("Local Docker configuration is missing"))?;
        validate_container_config(container)?;
        Ok(Some((
            container.docker_path.clone(),
            docker_exec_args(container, command, tty),
        )))
    }

    pub async fn local_container_shell_spec(
        &self,
        connection_id: &str,
        cwd: Option<&str>,
    ) -> anyhow::Result<Option<(String, Vec<String>)>> {
        let Some(config) = self.get_effective_connection_config(connection_id).await else {
            anyhow::bail!("Connection {} not found", connection_id);
        };
        if !config.uses_local_docker() {
            return Ok(None);
        }
        let container = config
            .container
            .as_ref()
            .ok_or_else(|| anyhow!("Local Docker configuration is missing"))?;
        validate_container_config(container)?;
        let mut args = vec!["exec".to_string(), "-i".to_string(), "-t".to_string()];
        if let Some(user) = container
            .user
            .as_deref()
            .map(str::trim)
            .filter(|user| !user.is_empty())
        {
            args.push("--user".to_string());
            args.push(user.to_string());
        }
        if let Some(cwd) = cwd.map(str::trim).filter(|cwd| !cwd.is_empty()) {
            args.push("--workdir".to_string());
            args.push(cwd.to_string());
        }
        args.push(container.name.clone());
        args.push(container.shell.clone());
        Ok(Some((container.docker_path.clone(), args)))
    }

    async fn execute_workspace_bytes(
        &self,
        connection_id: &str,
        command: &str,
    ) -> anyhow::Result<(Vec<u8>, Vec<u8>, i32)> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let transport = self.open_workspace_stdio(connection_id, command).await?;
        let (mut stdin, mut stdout, mut stderr, _control, completion) = transport.into_parts();
        let _ = stdin.shutdown().await;
        let stdout_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).await.map(|_| bytes)
        });
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.map(|_| bytes)
        });
        let exit = completion.wait().await;
        let stdout = stdout_task
            .await
            .context("Workspace stdout reader task failed")??;
        let stderr = stderr_task
            .await
            .context("Workspace stderr reader task failed")??;
        Ok((stdout, stderr, exit.exit_code.unwrap_or(-1)))
    }

    pub async fn container_read_file(
        &self,
        connection_id: &str,
        path: &str,
    ) -> anyhow::Result<Vec<u8>> {
        self.container_read_file_with_progress(connection_id, path, &mut |_, _| true)
            .await
    }

    pub async fn container_read_file_with_progress(
        &self,
        connection_id: &str,
        path: &str,
        on_progress: &mut impl FnMut(u64, u64) -> bool,
    ) -> anyhow::Result<Vec<u8>> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let path = self.resolve_sftp_path(connection_id, path).await?;
        let total = self
            .container_stat(connection_id, &path)
            .await?
            .and_then(|entry| entry.size)
            .unwrap_or(0);
        if !on_progress(0, total) {
            anyhow::bail!("Transfer cancelled");
        }

        let command = format!("cat -- {}", crate::remote_ssh::shell::quote_arg(&path));
        let transport = self.open_workspace_stdio(connection_id, &command).await?;
        let (mut stdin, mut stdout, mut stderr, control, completion) = transport.into_parts();
        let _ = stdin.shutdown().await;
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes).await;
            bytes
        });
        let mut content = Vec::new();
        let mut chunk = vec![0u8; 256 * 1024];
        loop {
            let read = stdout
                .read(&mut chunk)
                .await
                .with_context(|| format!("Failed to read container file '{}'", path))?;
            if read == 0 {
                break;
            }
            content.extend_from_slice(&chunk[..read]);
            if !on_progress(content.len() as u64, total) {
                let _ = control.kill().await;
                let _ = completion.wait().await;
                anyhow::bail!("Transfer cancelled");
            }
        }
        let stderr = stderr_task.await.unwrap_or_default();
        let exit = completion.wait().await;
        if exit.exit_code.unwrap_or(-1) != 0 {
            anyhow::bail!(
                "Failed to read container file '{}': {}",
                path,
                String::from_utf8_lossy(&stderr).trim()
            );
        }
        let _ = on_progress(content.len() as u64, total);
        Ok(content)
    }

    pub async fn container_write_file(
        &self,
        connection_id: &str,
        path: &str,
        content: &[u8],
    ) -> anyhow::Result<()> {
        self.container_write_file_with_progress(connection_id, path, content, &mut |_, _| true)
            .await
    }

    pub async fn container_write_file_with_progress(
        &self,
        connection_id: &str,
        path: &str,
        content: &[u8],
        on_progress: &mut impl FnMut(u64, u64) -> bool,
    ) -> anyhow::Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let path = self.resolve_sftp_path(connection_id, path).await?;
        let parent = path
            .rsplit_once('/')
            .map(|(parent, _)| if parent.is_empty() { "/" } else { parent })
            .unwrap_or(".");
        let temporary = format!(
            "{}/.bitfun-upload-{}.tmp",
            parent.trim_end_matches('/'),
            uuid::Uuid::new_v4()
        );
        let quoted_temporary = crate::remote_ssh::shell::quote_arg(&temporary);
        let quoted_path = crate::remote_ssh::shell::quote_arg(&path);
        let expected_size = content.len();
        let command = format!(
            "tmp={quoted_temporary}; target={quoted_path}; expected={expected_size}; \
             cleanup() {{ rm -f -- \"$tmp\"; }}; trap cleanup EXIT HUP INT TERM; \
             umask 077; status=0; cat > \"$tmp\" || status=$?; \
             if [ \"$status\" -eq 0 ]; then \
               actual=$(wc -c < \"$tmp\" 2>/dev/null) || status=$?; \
               if [ \"$status\" -eq 0 ]; then \
                 if [ \"$actual\" -eq \"$expected\" ]; then \
                   mv -f -- \"$tmp\" \"$target\" || status=$?; \
                 else status=65; fi; \
               fi; \
             fi; \
             trap - EXIT; cleanup; exit \"$status\""
        );
        let transport = self.open_workspace_stdio(connection_id, &command).await?;
        let (mut stdin, mut stdout, mut stderr, control, completion) = transport.into_parts();
        let stdout_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes).await;
            bytes
        });
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes).await;
            bytes
        });
        let total = content.len() as u64;
        if !on_progress(0, total) {
            let _ = control.kill().await;
            let _ = completion.wait().await;
            anyhow::bail!("Transfer cancelled");
        }

        let mut written = 0u64;
        for chunk in content.chunks(256 * 1024) {
            if let Err(error) = stdin.write_all(chunk).await {
                let _ = control.kill().await;
                let _ = completion.wait().await;
                anyhow::bail!("Failed to stream container file '{}': {}", path, error);
            }
            written += chunk.len() as u64;
            if !on_progress(written, total) {
                let _ = control.kill().await;
                let _ = completion.wait().await;
                anyhow::bail!("Transfer cancelled");
            }
        }
        stdin
            .shutdown()
            .await
            .with_context(|| format!("Failed to finish container file upload '{}'", path))?;
        let exit = completion.wait().await;
        let _stdout = stdout_task.await.unwrap_or_default();
        let stderr = stderr_task.await.unwrap_or_default();
        if exit.exit_code.unwrap_or(-1) != 0 {
            anyhow::bail!(
                "Failed to atomically write container file '{}': {}",
                path,
                String::from_utf8_lossy(&stderr).trim()
            );
        }
        Ok(())
    }

    pub async fn container_read_dir(
        &self,
        connection_id: &str,
        path: &str,
    ) -> anyhow::Result<Vec<crate::remote_ssh::types::RemoteDirEntry>> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let quoted = crate::remote_ssh::shell::quote_arg(&path);
        let script = format!(
            "dir={quoted}; \
             for item in \"$dir\"/.[!.]* \"$dir\"/..?* \"$dir\"/*; do \
               [ -e \"$item\" ] || [ -L \"$item\" ] || continue; \
               name=${{item##*/}}; \
               if [ -d \"$item\" ]; then kind=d; size=; \
               elif [ -L \"$item\" ]; then kind=l; size=$(wc -c < \"$item\" 2>/dev/null || true); \
               else kind=f; size=$(wc -c < \"$item\" 2>/dev/null || true); fi; \
               mtime=$(stat -c %Y \"$item\" 2>/dev/null || stat -f %m \"$item\" 2>/dev/null || true); \
               mode=$(stat -c %a \"$item\" 2>/dev/null || stat -f %Lp \"$item\" 2>/dev/null || true); \
               printf '%s\\0%s\\0%s\\0%s\\0%s\\0%s\\0' \"$name\" \"$item\" \"$kind\" \"$size\" \"$mtime\" \"$mode\"; \
             done"
        );
        let (stdout, stderr, status) = self.execute_workspace_bytes(connection_id, &script).await?;
        if status != 0 {
            anyhow::bail!(
                "Failed to list container directory '{}': {}",
                path,
                String::from_utf8_lossy(&stderr).trim()
            );
        }
        let stdout = String::from_utf8(stdout).with_context(|| {
            format!(
                "Container directory '{}' contains a filename that is not valid UTF-8",
                path
            )
        })?;
        parse_container_dir_output(&stdout)
    }

    pub async fn container_exists(&self, connection_id: &str, path: &str) -> anyhow::Result<bool> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let command = format!(
            "test -e {0} || test -L {0}",
            crate::remote_ssh::shell::quote_arg(&path)
        );
        let (_, _, status) = self.execute_command(connection_id, &command).await?;
        Ok(status == 0)
    }

    pub async fn container_mkdir(
        &self,
        connection_id: &str,
        path: &str,
        recursive: bool,
    ) -> anyhow::Result<()> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let flag = if recursive { "-p " } else { "" };
        self.run_container_fs_command(
            connection_id,
            &format!("mkdir {flag}{}", crate::remote_ssh::shell::quote_arg(&path)),
            "create directory",
            &path,
        )
        .await
    }

    pub async fn container_remove(
        &self,
        connection_id: &str,
        path: &str,
        directory: bool,
    ) -> anyhow::Result<()> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let command = if directory {
            format!("rmdir {}", crate::remote_ssh::shell::quote_arg(&path))
        } else {
            format!("rm -- {}", crate::remote_ssh::shell::quote_arg(&path))
        };
        self.run_container_fs_command(connection_id, &command, "remove", &path)
            .await
    }

    pub async fn container_rename(
        &self,
        connection_id: &str,
        old_path: &str,
        new_path: &str,
    ) -> anyhow::Result<()> {
        let old_path = self.resolve_sftp_path(connection_id, old_path).await?;
        let new_path = self.resolve_sftp_path(connection_id, new_path).await?;
        let command = format!(
            "mv -- {} {}",
            crate::remote_ssh::shell::quote_arg(&old_path),
            crate::remote_ssh::shell::quote_arg(&new_path)
        );
        self.run_container_fs_command(connection_id, &command, "rename", &old_path)
            .await
    }

    pub async fn container_stat(
        &self,
        connection_id: &str,
        path: &str,
    ) -> anyhow::Result<Option<crate::remote_ssh::types::RemoteFileEntry>> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let quoted = crate::remote_ssh::shell::quote_arg(&path);
        let script = format!(
            "item={quoted}; [ -e \"$item\" ] || [ -L \"$item\" ] || exit 44; \
             name=${{item##*/}}; \
             if [ -d \"$item\" ]; then kind=d; size=; \
             elif [ -L \"$item\" ]; then kind=l; size=$(wc -c < \"$item\" 2>/dev/null || true); \
             else kind=f; size=$(wc -c < \"$item\" 2>/dev/null || true); fi; \
             mtime=$(stat -c %Y \"$item\" 2>/dev/null || stat -f %m \"$item\" 2>/dev/null || true); \
             mode=$(stat -c %a \"$item\" 2>/dev/null || stat -f %Lp \"$item\" 2>/dev/null || true); \
             printf '%s\\0%s\\0%s\\0%s\\0%s\\0%s\\0' \"$name\" \"$item\" \"$kind\" \"$size\" \"$mtime\" \"$mode\""
        );
        let (stdout, stderr, status) = self.execute_workspace_bytes(connection_id, &script).await?;
        if status == 44 {
            return Ok(None);
        }
        if status != 0 {
            anyhow::bail!(
                "Failed to stat container path '{}': {}",
                path,
                String::from_utf8_lossy(&stderr).trim()
            );
        }
        let stdout = String::from_utf8(stdout).with_context(|| {
            format!(
                "Container path '{}' is not representable as a UTF-8 workspace path",
                path
            )
        })?;
        parse_container_file_output(&stdout)
    }

    async fn run_container_fs_command(
        &self,
        connection_id: &str,
        command: &str,
        operation: &str,
        path: &str,
    ) -> anyhow::Result<()> {
        let (_, stderr, status) = self.execute_command(connection_id, command).await?;
        if status != 0 {
            anyhow::bail!(
                "Failed to {} container path '{}': {}",
                operation,
                path,
                stderr.trim()
            );
        }
        Ok(())
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
            if conn.effective_config.uses_docker_exec() {
                anyhow::bail!(
                    "SFTP is unavailable for Docker container workspaces; use workspace file operations"
                );
            }
            conn.handle
                .clone()
                .ok_or_else(|| anyhow!("SSH handle is unavailable for {}", connection_id))?
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

    /// Read a file via SFTP with chunked progress reporting.
    ///
    /// Reads the file in `chunk_size`-byte chunks, invoking `on_progress`
    /// after each chunk with `(bytes_read, total_size)`. If the callback
    /// returns `false`, the read is aborted. Returns the full file contents.
    pub async fn sftp_read_with_progress(
        &self,
        connection_id: &str,
        path: &str,
        chunk_size: usize,
        on_progress: &mut impl FnMut(u64, u64) -> bool,
    ) -> anyhow::Result<Vec<u8>> {
        let resolved = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;

        // Determine file size from metadata for progress calculation.
        let metadata = sftp
            .as_ref()
            .metadata(&resolved)
            .await
            .map_err(|e| anyhow!("Failed to stat '{}': {}", resolved, e))?;
        let total = metadata.size.unwrap_or(0);

        let mut file = sftp
            .open(&resolved)
            .await
            .map_err(|e| anyhow!("Failed to open remote file '{}': {}", resolved, e))?;

        let mut buffer = Vec::new();
        let mut chunk = vec![0u8; chunk_size];
        let mut bytes_read: u64 = 0;

        use tokio::io::AsyncReadExt;
        loop {
            let n = file
                .read(&mut chunk)
                .await
                .map_err(|e| anyhow!("Failed to read remote file '{}': {}", resolved, e))?;
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..n]);
            bytes_read += n as u64;
            if !on_progress(bytes_read, total) {
                return Err(anyhow!("Transfer cancelled"));
            }
        }

        // Ensure final 100% progress is reported even if metadata returned 0.
        on_progress(bytes_read, total);

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

    /// Write a file via SFTP with chunked progress reporting.
    ///
    /// Writes `content` in `chunk_size`-byte chunks, invoking `on_progress`
    /// after each chunk with `(bytes_written, total_size)`. If the callback
    /// returns `false`, the write is aborted.
    pub async fn sftp_write_with_progress(
        &self,
        connection_id: &str,
        path: &str,
        content: &[u8],
        chunk_size: usize,
        on_progress: &mut impl FnMut(u64, u64) -> bool,
    ) -> anyhow::Result<()> {
        let path = self.resolve_sftp_path(connection_id, path).await?;
        let sftp = self.get_sftp(connection_id).await?;
        let mut file = sftp
            .create(&path)
            .await
            .map_err(|e| anyhow!("Failed to create remote file '{}': {}", path, e))?;

        use tokio::io::AsyncWriteExt;
        let total = content.len() as u64;
        let mut written: u64 = 0;

        for chunk in content.chunks(chunk_size) {
            file.write_all(chunk)
                .await
                .map_err(|e| anyhow!("Failed to write remote file '{}': {}", path, e))?;
            written += chunk.len() as u64;
            if !on_progress(written, total) {
                return Err(anyhow!("Transfer cancelled"));
            }
        }

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

        for dir in sftp_mkdir_all_prefixes(&path) {
            if let Ok(true) = sftp.as_ref().try_exists(&dir).await {
                continue;
            }

            if let Err(error) = sftp.as_ref().create_dir(&dir).await {
                match sftp.as_ref().try_exists(&dir).await {
                    Ok(true) => continue,
                    Ok(false) | Err(_) => {
                        return Err(anyhow!("Failed to create directory '{}': {}", dir, error));
                    }
                }
            }
        }

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
        let (handle, config) = {
            let guard = self.connections.read().await;
            let conn = guard
                .get(connection_id)
                .ok_or_else(|| anyhow!("Connection {} not found", connection_id))?;
            if !conn.alive.load(Ordering::SeqCst) {
                return Err(anyhow!("Connection {} is not alive", connection_id));
            }
            (
                conn.handle
                    .clone()
                    .ok_or_else(|| anyhow!("SSH handle is unavailable for {}", connection_id))?,
                conn.effective_config.clone(),
            )
        };

        // Open a session channel
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| anyhow!("Failed to open channel: {}", e))?;

        // Request PTY — `false` = don't wait for reply (reply handled in reader loop)
        channel
            .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
            .await
            .map_err(|e| anyhow!("Failed to request PTY: {}", e))?;

        if config.uses_docker_exec() {
            let container = config
                .container
                .as_ref()
                .expect("docker exec connection must have container config");
            let command = docker_exec_host_command(container, &container.shell, true);
            channel
                .exec(false, command)
                .await
                .map_err(|e| anyhow!("Failed to start Docker container shell: {}", e))?;
        } else {
            // Start shell — `false` = don't wait for reply
            channel
                .request_shell(false)
                .await
                .map_err(|e| anyhow!("Failed to start shell: {}", e))?;
        }

        let still_connected = {
            let guard = self.connections.read().await;
            guard.get(connection_id).is_some_and(|conn| {
                conn.alive.load(Ordering::SeqCst)
                    && conn
                        .handle
                        .as_ref()
                        .is_some_and(|active| Arc::ptr_eq(active, &handle))
            })
        };
        if !still_connected {
            let _ = channel.eof().await;
            let _ = channel.close().await;
            return Err(anyhow!(
                "Connection {} disconnected while opening PTY",
                connection_id
            ));
        }

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
                    return Ok(Some(data.to_vec()));
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

fn sftp_mkdir_all_prefixes(path: &str) -> Vec<String> {
    let is_absolute = path.starts_with('/');
    let mut current = String::new();
    let mut prefixes = Vec::new();

    for component in path.split('/').filter(|component| !component.is_empty()) {
        if current.is_empty() {
            if is_absolute {
                current.push('/');
            }
            current.push_str(component);
        } else {
            current.push('/');
            current.push_str(component);
        }
        prefixes.push(current.clone());
    }

    prefixes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_ssh::types::RemoteWorkspace;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    #[cfg(feature = "ssh_config")]
    fn ssh_config_fallback_accepts_utf8_bom_and_defaults_to_agent() {
        let content = strip_utf8_bom(
            "\u{feff}Host 跳板\n  HostName jump.example.com\n  User 构建\n".to_string(),
        );
        let entries = parse_ssh_config_manually(&content);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].host, "跳板");
        assert_eq!(entries[0].user.as_deref(), Some("构建"));
        assert_eq!(entries[0].agent, Some(true));
    }

    fn test_data_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bitfun-remote-ssh-manager-{}-{}-{}",
            name,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn parses_proxy_jump_alias_user_port_and_ipv6() {
        assert_eq!(
            parse_proxy_jump_token("jump-a").unwrap(),
            (None, "jump-a".to_string(), None)
        );
        assert_eq!(
            parse_proxy_jump_token("ops@jump.example:2222").unwrap(),
            (
                Some("ops".to_string()),
                "jump.example".to_string(),
                Some(2222)
            )
        );
        assert_eq!(
            parse_proxy_jump_token("root@[2001:db8::10]:2200").unwrap(),
            (
                Some("root".to_string()),
                "2001:db8::10".to_string(),
                Some(2200)
            )
        );
    }

    #[test]
    fn parses_docker_published_ipv4_and_ipv6_endpoints() {
        assert_eq!(
            parse_docker_published_endpoint("0.0.0.0:22022\n[::]:22022\n"),
            Some(("127.0.0.1".to_string(), 22022))
        );
        assert_eq!(
            parse_docker_published_endpoint("192.168.50.2:22023\n"),
            Some(("192.168.50.2".to_string(), 22023))
        );
        assert_eq!(
            parse_docker_published_endpoint("[fd00::10]:22024\n"),
            Some(("fd00::10".to_string(), 22024))
        );
    }

    #[test]
    fn docker_exec_command_quotes_all_user_controlled_values() {
        let container = ContainerWorkspaceConfig {
            name: "dev container".to_string(),
            access: ContainerAccess::DockerExec,
            local: false,
            docker_path: "/opt/docker cli".to_string(),
            shell: "/bin/bash".to_string(),
            user: Some("build user".to_string()),
            interactive: true,
        };

        assert_eq!(
            docker_exec_host_command(&container, "printf '%s' \"$HOME\"", false),
            "'/opt/docker cli' 'exec' '-i' '--user' 'build user' 'dev container' '/bin/bash' '-lc' 'printf '\\''%s'\\'' \"$HOME\"'"
        );
    }

    #[test]
    fn supervised_container_command_tracks_and_signals_the_container_process_group() {
        let container = ContainerWorkspaceConfig {
            name: "dev".to_string(),
            access: ContainerAccess::DockerExec,
            local: true,
            docker_path: "docker".to_string(),
            shell: "/bin/bash".to_string(),
            user: None,
            interactive: true,
        };
        let (wrapped, pid_file) =
            supervised_container_command(&container, "printf '路径'; sleep 30");
        let signal =
            container_signal_command(&pid_file, crate::remote_ssh::WorkspaceProcessSignal::Kill);

        assert!(pid_file.starts_with("/tmp/.bitfun-exec-"));
        assert!(wrapped.contains("setsid '/bin/bash' -lc"));
        assert!(wrapped.contains("|| tracking=0"));
        assert!(wrapped.contains("printf '%s' \"$child\" > \"$pid_file\""));
        assert!(signal.contains("[ -s \"$pid_file\" ] || exit 75"));
        assert!(signal.contains("kill -KILL -- \"-$pid\""));
        assert!(signal.contains("kill -KILL \"$pid\""));
    }

    #[test]
    #[cfg(unix)]
    fn supervised_container_command_keeps_working_without_a_writable_pid_location() {
        let container = ContainerWorkspaceConfig {
            name: "dev".to_string(),
            access: ContainerAccess::DockerExec,
            local: true,
            docker_path: "docker".to_string(),
            shell: "/bin/sh".to_string(),
            user: None,
            interactive: true,
        };
        let wrapped = supervised_container_command_with_pid_file(
            &container,
            "printf 'compatible'",
            "/dev/null/bitfun-exec.pid",
        );
        let output = std::process::Command::new("sh")
            .args(["-lc", &wrapped])
            .output()
            .unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout, b"compatible");
        assert!(
            String::from_utf8_lossy(&output.stderr).trim().is_empty(),
            "process-control fallback must not add command stderr"
        );
    }

    #[test]
    fn parses_container_directory_entry_with_newline_and_unit_separator() {
        let entries = parse_container_dir_output(
            "src\n\u{1f}name\0/workspace/src\n\u{1f}name\0d\0\01720000000\0755\0",
        )
        .unwrap();
        let entry = &entries[0];

        assert_eq!(entry.name, "src\n\u{1f}name");
        assert_eq!(entry.path, "/workspace/src\n\u{1f}name");
        assert!(entry.is_dir);
        assert_eq!(entry.modified, Some(1_720_000_000_000));
        assert_eq!(entry.permissions.as_deref(), Some("755"));
    }

    #[tokio::test]
    #[ignore = "requires BITFUN_TEST_DOCKER_CONTAINER to name a running container"]
    async fn local_docker_workspace_round_trip() {
        let Ok(container_name) = std::env::var("BITFUN_TEST_DOCKER_CONTAINER") else {
            return;
        };
        let dir = test_data_dir("local-docker-round-trip");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let manager = SSHConnectionManager::new(dir.clone());
        let connection_id = "docker-local-contract";

        manager
            .connect(SSHConnectionConfig {
                id: connection_id.to_string(),
                name: container_name.clone(),
                host: String::new(),
                port: 22,
                username: String::new(),
                auth: SSHAuthMethod::PrivateKey {
                    key_path: String::new(),
                    passphrase: None,
                    certificate_path: None,
                },
                default_workspace: Some("/tmp/bitfun-remote-workspace".to_string()),
                proxy_jump: None,
                container: Some(ContainerWorkspaceConfig {
                    name: container_name,
                    access: ContainerAccess::DockerExec,
                    local: true,
                    docker_path: "docker".to_string(),
                    shell: "/bin/sh".to_string(),
                    user: None,
                    interactive: true,
                }),
                options: Default::default(),
            })
            .await
            .unwrap();

        {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let transport = manager
                .open_workspace_stdio(
                    connection_id,
                    "read value; printf 'stdout:%s' \"$value\"; printf 'stderr:%s' \"$value\" >&2; exit 7",
                )
                .await
                .unwrap();
            let (mut stdin, mut stdout, mut stderr, _control, completion) = transport.into_parts();
            stdin.write_all(b"transport-contract\n").await.unwrap();
            stdin.shutdown().await.unwrap();
            let mut stdout_bytes = Vec::new();
            let mut stderr_bytes = Vec::new();
            stdout.read_to_end(&mut stdout_bytes).await.unwrap();
            stderr.read_to_end(&mut stderr_bytes).await.unwrap();
            let exit = completion.wait().await;
            assert_eq!(stdout_bytes, b"stdout:transport-contract");
            assert_eq!(stderr_bytes, b"stderr:transport-contract");
            assert_eq!(exit.exit_code, Some(7));
        }

        manager
            .container_mkdir(connection_id, "/tmp/bitfun-remote-workspace", true)
            .await
            .unwrap();
        let cancellation = tokio_util::sync::CancellationToken::new();
        let cancel_after_start = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel_after_start.cancel();
        });
        let cancelled_command = manager
            .execute_command_with_options(
                connection_id,
                "trap '' INT; sleep 30; touch /tmp/bitfun-remote-workspace/cancel-leaked",
                SSHCommandOptions {
                    timeout_ms: Some(5_000),
                    cancellation_token: Some(cancellation),
                },
            )
            .await
            .unwrap();
        assert!(cancelled_command.interrupted);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !manager
                .container_exists(connection_id, "/tmp/bitfun-remote-workspace/cancel-leaked")
                .await
                .unwrap(),
            "cancelled Docker command must not continue inside the container"
        );
        let original = b"BitFun container workspace\0binary";
        manager
            .container_write_file(
                connection_id,
                "/tmp/bitfun-remote-workspace/source.bin",
                original,
            )
            .await
            .unwrap();
        assert_eq!(
            manager
                .container_read_file(connection_id, "/tmp/bitfun-remote-workspace/source.bin")
                .await
                .unwrap(),
            original
        );
        manager
            .container_write_file(
                connection_id,
                "/tmp/bitfun-remote-workspace/atomic.bin",
                original,
            )
            .await
            .unwrap();
        let replacement = vec![b'x'; 600_000];
        let cancelled = manager
            .container_write_file_with_progress(
                connection_id,
                "/tmp/bitfun-remote-workspace/atomic.bin",
                &replacement,
                &mut |written, _| written < 262_144,
            )
            .await;
        assert!(cancelled.is_err());
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            manager
                .container_read_file(connection_id, "/tmp/bitfun-remote-workspace/atomic.bin")
                .await
                .unwrap(),
            original,
            "cancelled upload must not replace the existing destination"
        );
        manager
            .container_rename(
                connection_id,
                "/tmp/bitfun-remote-workspace/source.bin",
                "/tmp/bitfun-remote-workspace/renamed.bin",
            )
            .await
            .unwrap();
        let entries = manager
            .container_read_dir(connection_id, "/tmp/bitfun-remote-workspace")
            .await
            .unwrap();
        assert!(entries.iter().any(|entry| entry.name == "renamed.bin"));
        assert_eq!(
            manager
                .container_stat(connection_id, "/tmp/bitfun-remote-workspace/renamed.bin")
                .await
                .unwrap()
                .and_then(|entry| entry.size),
            Some(original.len() as u64)
        );
        manager
            .container_remove(
                connection_id,
                "/tmp/bitfun-remote-workspace/renamed.bin",
                false,
            )
            .await
            .unwrap();
        assert!(!manager
            .container_exists(connection_id, "/tmp/bitfun-remote-workspace/renamed.bin")
            .await
            .unwrap());

        manager.disconnect(connection_id).await.unwrap();
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn retains_legacy_password_connection_and_workspace_without_vault_entry() {
        let dir = test_data_dir("legacy-missing-vault");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let manager = SSHConnectionManager::new(dir.clone());

        tokio::fs::write(
            dir.join("ssh_connections.json"),
            serde_json::to_string_pretty(&serde_json::json!([{
                "id": "ssh-root@example.com:22",
                "name": "root@example.com",
                "host": "example.com",
                "port": 22,
                "username": "root",
                "authType": { "type": "Password" },
                "defaultWorkspace": "/root/project",
                "lastConnected": 1
            }]))
            .unwrap(),
        )
        .await
        .unwrap();
        tokio::fs::write(
            dir.join("remote_workspace.json"),
            serde_json::to_string_pretty(&serde_json::json!([{
                "connectionId": "ssh-root@example.com:22",
                "remotePath": "/root/project",
                "connectionName": "root@example.com",
                "sshHost": "example.com"
            }]))
            .unwrap(),
        )
        .await
        .unwrap();

        manager.load_saved_connections().await.unwrap();
        manager.load_remote_workspace().await.unwrap();
        let removed = manager
            .prune_remote_workspaces_without_saved_connections()
            .await
            .unwrap();

        let saved = manager.get_saved_connections().await;
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, "ssh-root@example.com");
        assert_eq!(saved[0].default_workspace.as_deref(), Some("/root/project"));
        assert!(saved[0].proxy_jump.is_none());
        assert!(saved[0].container.is_none());
        assert!(removed.is_empty());
        let workspaces = manager.get_remote_workspaces().await;
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].connection_id, "ssh-root@example.com");
        assert_eq!(workspaces[0].remote_path, "/root/project");
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn rejects_saving_password_connection_without_password() {
        let dir = test_data_dir("empty-password-save");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let manager = SSHConnectionManager::new(dir.clone());

        let result = manager
            .save_connection(&SSHConnectionConfig {
                id: "ssh-root@example.com:22".to_string(),
                name: "root@example.com".to_string(),
                host: "example.com".to_string(),
                port: 22,
                username: "root".to_string(),
                auth: SSHAuthMethod::Password {
                    password: String::new(),
                },
                default_workspace: None,
                proxy_jump: None,
                container: None,
                options: Default::default(),
            })
            .await;

        assert!(result.is_err());
        assert!(manager.get_saved_connections().await.is_empty());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn restores_connection_config_from_saved_password_profile() {
        let dir = test_data_dir("restore-password-config");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let manager = SSHConnectionManager::new(dir.clone());

        manager
            .save_connection(&SSHConnectionConfig {
                id: "ssh-root@example.com:22".to_string(),
                name: "root@example.com".to_string(),
                host: "example.com".to_string(),
                port: 22,
                username: "root".to_string(),
                auth: SSHAuthMethod::Password {
                    password: "secret".to_string(),
                },
                default_workspace: Some("/root/project".to_string()),
                proxy_jump: Some("jump-a,jump-b".to_string()),
                container: None,
                options: Default::default(),
            })
            .await
            .unwrap();

        let restored = manager
            .load_connection_config_from_saved("ssh-root@example.com:22")
            .await
            .unwrap()
            .expect("expected saved config");

        assert_eq!(restored.host, "example.com");
        assert_eq!(restored.username, "root");
        assert_eq!(restored.default_workspace.as_deref(), Some("/root/project"));
        assert_eq!(restored.proxy_jump.as_deref(), Some("jump-a,jump-b"));
        match restored.auth {
            SSHAuthMethod::Password { password } => assert_eq!(password, "secret"),
            other => panic!("expected password auth, got {:?}", other),
        }

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn restores_legacy_local_docker_profile_without_password_vault_entry() {
        let dir = test_data_dir("legacy-local-docker-password-placeholder");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let manager = SSHConnectionManager::new(dir.clone());

        tokio::fs::write(
            dir.join("ssh_connections.json"),
            serde_json::to_string_pretty(&serde_json::json!([{
                "id": "docker-local-legacy",
                "name": "local container",
                "host": "",
                "port": 22,
                "username": "",
                "authType": { "type": "Password" },
                "defaultWorkspace": "/workspace",
                "lastConnected": 1,
                "container": {
                    "name": "dev",
                    "access": "docker-exec",
                    "local": true
                }
            }]))
            .unwrap(),
        )
        .await
        .unwrap();

        manager.load_saved_connections().await.unwrap();
        let restored = manager
            .load_connection_config_from_saved("docker-local-legacy")
            .await
            .unwrap()
            .expect("legacy local Docker profile must remain restorable");

        assert!(restored.uses_local_docker());
        assert!(matches!(
            restored.auth,
            SSHAuthMethod::Password { ref password } if password.is_empty()
        ));
        assert_eq!(
            restored.options,
            crate::remote_ssh::types::SSHConnectionOptions::default()
        );
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn saved_interactive_profile_never_persists_challenge_responses() {
        let dir = test_data_dir("interactive-secrets");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let manager = SSHConnectionManager::new(dir.clone());

        manager
            .save_connection(&SSHConnectionConfig {
                id: "ssh-alice@example.com".to_string(),
                name: "interactive".to_string(),
                host: "example.com".to_string(),
                port: 22,
                username: "alice".to_string(),
                auth: SSHAuthMethod::KeyboardInteractive {
                    responses: vec!["password-secret".to_string(), "123456".to_string()],
                },
                default_workspace: Some("/workspace".to_string()),
                proxy_jump: Some("jump.example.com".to_string()),
                container: None,
                options: Default::default(),
            })
            .await
            .unwrap();

        let persisted = tokio::fs::read_to_string(dir.join("ssh_connections.json"))
            .await
            .unwrap();
        assert!(persisted.contains("KeyboardInteractive"));
        assert!(!persisted.contains("password-secret"));
        assert!(!persisted.contains("123456"));

        let restored = manager
            .load_connection_config_from_saved("ssh-alice@example.com")
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            restored.auth,
            SSHAuthMethod::KeyboardInteractive { ref responses } if responses.is_empty()
        ));
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn saved_agent_profile_preserves_identity_selection() {
        let dir = test_data_dir("agent-profile");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let manager = SSHConnectionManager::new(dir.clone());

        manager
            .save_connection(&SSHConnectionConfig {
                id: "ssh-agent@example.com".to_string(),
                name: "agent".to_string(),
                host: "example.com".to_string(),
                port: 22,
                username: "agent".to_string(),
                auth: SSHAuthMethod::Agent {
                    key_fingerprint: Some("SHA256:test-fingerprint".to_string()),
                    fallback_key_path: Some("~/.ssh/custom-fallback".to_string()),
                },
                default_workspace: None,
                proxy_jump: None,
                container: None,
                options: Default::default(),
            })
            .await
            .unwrap();

        let restored = manager
            .load_connection_config_from_saved("ssh-agent@example.com")
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            restored.auth,
            SSHAuthMethod::Agent {
                ref key_fingerprint,
                ref fallback_key_path,
            } if key_fingerprint.as_deref() == Some("SHA256:test-fingerprint")
                && fallback_key_path.as_deref() == Some("~/.ssh/custom-fallback")
        ));
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn prunes_remote_workspaces_without_saved_connection() {
        let dir = test_data_dir("missing-saved");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let manager = SSHConnectionManager::new(dir.clone());

        let workspaces = vec![RemoteWorkspace {
            connection_id: "ssh-root@example.com:22".to_string(),
            remote_path: "/root/project".to_string(),
            connection_name: "root@example.com".to_string(),
            ssh_host: "example.com".to_string(),
        }];
        tokio::fs::write(
            dir.join("remote_workspace.json"),
            serde_json::to_string_pretty(&workspaces).unwrap(),
        )
        .await
        .unwrap();

        manager.load_remote_workspace().await.unwrap();
        let removed = manager
            .prune_remote_workspaces_without_saved_connections()
            .await
            .unwrap();

        assert_eq!(removed.len(), 1);
        assert!(manager.get_remote_workspaces().await.is_empty());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[test]
    fn mkdir_all_prefixes_expand_absolute_posix_path() {
        assert_eq!(
            sftp_mkdir_all_prefixes("/home/wgq/workspace/bot_detection/.bitfun/bin"),
            vec![
                "/home".to_string(),
                "/home/wgq".to_string(),
                "/home/wgq/workspace".to_string(),
                "/home/wgq/workspace/bot_detection".to_string(),
                "/home/wgq/workspace/bot_detection/.bitfun".to_string(),
                "/home/wgq/workspace/bot_detection/.bitfun/bin".to_string(),
            ]
        );
    }

    #[test]
    fn mkdir_all_prefixes_collapse_redundant_separators() {
        assert_eq!(
            sftp_mkdir_all_prefixes("/home//wgq///project/"),
            vec![
                "/home".to_string(),
                "/home/wgq".to_string(),
                "/home/wgq/project".to_string(),
            ]
        );
    }
}
