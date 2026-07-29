//! SSH transport for persistent BitFun dispatch jobs.
//!
//! The target-side runner is the `bitfun dispatch` CLI surface. This module is
//! deliberately only a submit/poll transport: the remote CLI owns jobs,
//! sessions, transcripts, process detachment, and cancellation semantics.
//!
//! Installing the CLI is a separate, explicit operation. `probe` never installs
//! anything; `install_cli_start` downloads an official archive locally, verifies
//! both its signed SHA256 sidecar and archive minisign signature, then stages it
//! under the SSH user's home before starting an owner-only installer.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use super::manager::SSHConnectionManager;
use super::release_verify::{
    release_tag_for_version, require_release_pubkey, verify_minisign, verify_sha256,
    verify_signed_checksum,
};
use super::remote_git::shell_quote_posix;
use super::types::SSHCommandOptions;

const RELEASE_BASE: &str = "https://github.com/GCWing/BitFun/releases";
const RELEASE_VERSION: &str = env!("CARGO_PKG_VERSION");
const INSTALL_STATE_DIR: &str = ".bitfun/dispatch/install";
const REQUEST_STATE_DIR: &str = ".bitfun/dispatch/requests";
const INSTALL_STEM: &str = "install-cli";
const INSTALL_DONE_MARKER: &str = "BITFUN_DISPATCH_CLI_INSTALL_DONE";
const INSTALL_PREPARE_GRACE_SECONDS: u64 = 30;
const COMMAND_TIMEOUT_MS: u64 = 30_000;
const RELEASE_READ_TIMEOUT_SECONDS: u64 = 30;
const MAX_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;
const DISPATCH_PROTOCOL_VERSION: u64 = 1;
const REQUIRED_DISPATCH_CAPABILITIES: [&str; 7] = [
    "persistent_jobs",
    "cursor_events",
    "detached_worker",
    "workspace_serialization",
    "frontend_event_projection",
    "approval_auto",
    "approval_reject_and_report",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchCliRelease {
    pub version: String,
    pub target: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchSshProbe {
    pub cli_installed: bool,
    pub cli_path: Option<String>,
    pub os: String,
    pub arch: String,
    pub install_supported: bool,
    pub install_error: Option<String>,
    pub protocol_error: Option<String>,
    pub release: Option<DispatchCliRelease>,
    pub protocol: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchInstallStart {
    /// Absolute path of the staged driver. The task has already been launched;
    /// this path is returned for diagnostics and must not be executed again.
    pub script_path: String,
    pub version: String,
    pub target: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchInstallStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchInstallPoll {
    pub cursor: u64,
    pub output: String,
    pub status: DispatchInstallStatus,
}

#[derive(Debug)]
struct RemoteTarget {
    os: String,
    arch: String,
    home: String,
    cli_path: Option<String>,
    tar_available: bool,
}

#[derive(Debug)]
struct ResolvedRelease {
    public: DispatchCliRelease,
    filename: String,
    checksum_url: String,
    checksum_signature_url: String,
    archive_signature_url: String,
}

/// Probe the remote OS/architecture and, when present, the target CLI dispatch
/// protocol. A missing or old CLI is a normal result rather than an error.
///
/// Release metadata is resolved only when installation or upgrade is needed.
/// Resolving it verifies the signature over the SHA256 sidecar so the UI can
/// safely show the exact URL, version, and digest before asking for consent.
pub async fn probe(
    manager: &SSHConnectionManager,
    connection_id: &str,
    workspace_path: Option<&str>,
) -> Result<DispatchSshProbe> {
    ensure_plain_ssh_target(manager, connection_id).await?;
    let target = probe_remote_target(manager, connection_id).await?;

    let mut protocol = None;
    let mut protocol_error = None;
    if let Some(cli_path) = target.cli_path.as_deref() {
        let request = workspace_path
            .map(|path| serde_json::json!({ "workspacePath": path }))
            .unwrap_or_else(|| serde_json::json!({}));
        match invoke_json_at_path(
            manager,
            connection_id,
            &target.home,
            cli_path,
            "probe",
            &request,
        )
        .await
        {
            Ok(response) => protocol = Some(response),
            Err(error) => protocol_error = Some(error.to_string()),
        }
    }

    let needs_install = target.cli_path.is_none()
        || !protocol
            .as_ref()
            .is_some_and(dispatch_protocol_is_compatible);
    let (release, install_error) = if needs_install {
        if !target.tar_available {
            (
                None,
                Some("remote target has no tar executable; install tar and retry".to_string()),
            )
        } else {
            match resolve_release(&target.os, &target.arch).await {
                Ok(release) => (Some(release.public), None),
                Err(error) => (None, Some(error.to_string())),
            }
        }
    } else {
        (None, None)
    };
    let install_supported = release.is_some();

    Ok(DispatchSshProbe {
        cli_installed: target.cli_path.is_some(),
        cli_path: target.cli_path,
        os: target.os,
        arch: target.arch,
        install_supported,
        install_error,
        protocol_error,
        release,
        protocol,
    })
}

fn dispatch_protocol_is_compatible(protocol: &Value) -> bool {
    validate_dispatch_protocol(protocol, None).is_ok()
}

/// Validate the target-side protocol immediately before submission.
///
/// `approval_policy = None` is used by installation probing and requires the
/// complete phase-one surface. Submission may validate only the selected
/// unattended approval behavior in addition to the transport invariants.
pub fn validate_dispatch_protocol(protocol: &Value, approval_policy: Option<&str>) -> Result<()> {
    if protocol.get("protocolVersion").and_then(Value::as_u64) != Some(DISPATCH_PROTOCOL_VERSION) {
        return Err(anyhow!(
            "dispatch protocol version is incompatible; expected {}",
            DISPATCH_PROTOCOL_VERSION
        ));
    }
    let Some(capabilities) = protocol.get("capabilities").and_then(Value::as_array) else {
        return Err(anyhow!("dispatch target returned no capability list"));
    };
    let required: &[&str] = match approval_policy {
        Some("auto") => &[
            "persistent_jobs",
            "cursor_events",
            "detached_worker",
            "workspace_serialization",
            "frontend_event_projection",
            "approval_auto",
        ],
        Some("reject-and-report") => &[
            "persistent_jobs",
            "cursor_events",
            "detached_worker",
            "workspace_serialization",
            "frontend_event_projection",
            "approval_reject_and_report",
        ],
        Some(_) => return Err(anyhow!("unsupported dispatch approval policy")),
        None => &REQUIRED_DISPATCH_CAPABILITIES,
    };
    let missing = required
        .iter()
        .copied()
        .filter(|required| {
            !capabilities
                .iter()
                .any(|capability| capability.as_str() == Some(*required))
        })
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(anyhow!(
            "dispatch target is missing required capabilities: {}",
            missing.join(", ")
        ));
    }
    Ok(())
}

/// Explicitly install the matching BitFun CLI release on the SSH target.
///
/// This function fails closed when the build has no release trust root, a
/// checksum/signature is absent, or either verification fails. It never uses
/// sudo and writes only below `~/.local/bin` and `~/.bitfun`.
pub async fn install_cli_start(
    manager: &SSHConnectionManager,
    connection_id: &str,
    expected_release: &DispatchCliRelease,
) -> Result<DispatchInstallStart> {
    ensure_plain_ssh_target(manager, connection_id).await?;
    let target = probe_remote_target(manager, connection_id).await?;
    if !target.tar_available {
        return Err(anyhow!(
            "remote target has no tar executable; install tar and retry"
        ));
    }
    let release = resolve_release(&target.os, &target.arch).await?;
    ensure_confirmed_release(&release.public, expected_release)?;
    let archive = download_verified_archive(&release).await?;

    // Stop an earlier attempt before replacing any of its staged files. A
    // cancellation transport error is not safe to ignore: the old installer
    // may still be reading the archive or paths this attempt would replace.
    install_cli_cancel(manager, connection_id)
        .await
        .context("stop an earlier BitFun CLI installation")?;

    let dir = format!("{}/{}", target.home, INSTALL_STATE_DIR);
    let archive_path = format!("{dir}/{}", release.filename);
    let body_path = format!("{dir}/{INSTALL_STEM}-body.sh");
    let script_path = format!("{dir}/{INSTALL_STEM}.sh");
    let log_path = format!("{dir}/{INSTALL_STEM}.log");
    let pid_path = format!("{dir}/{INSTALL_STEM}.pid");
    let driver_pid_path = format!("{dir}/{INSTALL_STEM}.driver.pid");
    let prepare_path = format!("{dir}/{INSTALL_STEM}.preparing");
    let exit_path = format!("{dir}/{INSTALL_STEM}.exit");
    let install_token = format!("bitfun-install-{}", uuid::Uuid::new_v4().as_simple());

    exec_ok(
        manager,
        connection_id,
        &format!(
            "mkdir -p {dir} && chmod 700 {root} {dispatch} {dir}",
            root = shell_quote_posix(&format!("{}/.bitfun", target.home)),
            dispatch = shell_quote_posix(&format!("{}/.bitfun/dispatch", target.home)),
            dir = shell_quote_posix(&dir),
        ),
    )
    .await?;

    let body = to_unix_script(&install_body_script(
        &dir,
        &archive_path,
        &release.public.version,
    ));
    let driver = to_unix_script(&install_driver_script(&dir, &body_path, &install_token));
    manager
        .sftp_write(connection_id, &archive_path, &archive)
        .await
        .context("stage verified BitFun CLI archive")?;
    manager
        .sftp_write(connection_id, &body_path, body.as_bytes())
        .await
        .context("stage BitFun CLI install body")?;
    manager
        .sftp_write(connection_id, &script_path, driver.as_bytes())
        .await
        .context("stage BitFun CLI install driver")?;

    exec_ok(
        manager,
        connection_id,
        &stage_install_command(
            &archive_path,
            &body_path,
            &script_path,
            &log_path,
            &pid_path,
            &driver_pid_path,
            &prepare_path,
            &exit_path,
            &install_token,
        ),
    )
    .await?;

    // The short-lived PTY driver only starts a nohup body and exits. Draining
    // the channel in the background prevents a server-side channel leak while
    // keeping the installer independent of the caller process.
    let channel = match manager
        .open_pty_exec_channel(
            connection_id,
            &format!(
                "bash {} {}",
                shell_quote_posix(&script_path),
                shell_quote_posix(&install_token)
            ),
            100,
            30,
        )
        .await
    {
        Ok(channel) => channel,
        Err(error) => {
            let _ = install_cli_cancel(manager, connection_id).await;
            return Err(error).context("start remote BitFun CLI installer");
        }
    };
    tokio::spawn(async move {
        let mut channel = channel;
        while channel.wait().await.is_some() {}
    });

    Ok(DispatchInstallStart {
        script_path,
        version: release.public.version,
        target: release.public.target,
        url: release.public.url,
        sha256: release.public.sha256,
    })
}

fn ensure_confirmed_release(
    resolved: &DispatchCliRelease,
    expected: &DispatchCliRelease,
) -> Result<()> {
    if resolved != expected {
        return Err(anyhow!(
            "BitFun CLI release metadata changed after confirmation; probe the target and confirm the new asset"
        ));
    }
    Ok(())
}

pub async fn install_cli_poll(
    manager: &SSHConnectionManager,
    connection_id: &str,
    cursor: u64,
) -> Result<DispatchInstallPoll> {
    ensure_plain_ssh_target(manager, connection_id).await?;
    let script = install_poll_script(cursor);
    let result = manager
        .execute_command_with_options(
            connection_id,
            &script,
            SSHCommandOptions {
                timeout_ms: Some(COMMAND_TIMEOUT_MS),
                cancellation_token: None,
            },
        )
        .await?;
    ensure_command_completed(&result, "poll CLI install")?;
    if result.exit_code != 0 {
        return Err(remote_command_error(
            "poll CLI install",
            result.exit_code,
            &result.stdout,
            &result.stderr,
        ));
    }
    let (head, output) = split_metadata_output(&result.stdout);
    let value = |key: &str| {
        head.lines()
            .find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|rest| rest.strip_prefix('='))
            })
            .unwrap_or("")
            .trim()
    };
    let running = value("running") == "1";
    let preparing = value("preparing") == "1";
    let marker = value("marker") == "1";
    let exit_recorded = value("exit_recorded") == "1";
    let exit_code = value("exit_code").parse::<i32>().ok();
    let size = value("size").parse::<u64>().unwrap_or(cursor);
    let status = if marker {
        DispatchInstallStatus::Succeeded
    } else if running || preparing {
        DispatchInstallStatus::Running
    } else if exit_recorded || size > 0 {
        DispatchInstallStatus::Failed
    } else {
        // start seeds the prepare flag before launching the driver, so an empty
        // state after an explicit start means the driver never became live.
        DispatchInstallStatus::Failed
    };
    let mut output = output.to_string();
    if status == DispatchInstallStatus::Failed && exit_code == Some(130) && output.is_empty() {
        output.push_str("BitFun CLI installation was cancelled.\n");
    }
    Ok(DispatchInstallPoll {
        cursor: size,
        output,
        status,
    })
}

/// Best-effort process-tree cancellation for an in-flight CLI install.
pub async fn install_cli_cancel(manager: &SSHConnectionManager, connection_id: &str) -> Result<()> {
    ensure_plain_ssh_target(manager, connection_id).await?;
    let script = install_cancel_script();
    let result = manager
        .execute_command_with_options(
            connection_id,
            &script,
            SSHCommandOptions {
                timeout_ms: Some(COMMAND_TIMEOUT_MS),
                cancellation_token: None,
            },
        )
        .await?;
    ensure_command_completed(&result, "cancel CLI install")?;
    if result.exit_code != 0 {
        return Err(remote_command_error(
            "cancel CLI install",
            result.exit_code,
            &result.stdout,
            &result.stderr,
        ));
    }
    Ok(())
}

pub async fn submit(
    manager: &SSHConnectionManager,
    connection_id: &str,
    request: &Value,
) -> Result<Value> {
    invoke_json(manager, connection_id, "submit", request).await
}

pub async fn status(
    manager: &SSHConnectionManager,
    connection_id: &str,
    request: &Value,
) -> Result<Value> {
    invoke_json(manager, connection_id, "status", request).await
}

pub async fn cancel(
    manager: &SSHConnectionManager,
    connection_id: &str,
    request: &Value,
) -> Result<Value> {
    invoke_json(manager, connection_id, "cancel", request).await
}

pub async fn list(
    manager: &SSHConnectionManager,
    connection_id: &str,
    request: &Value,
) -> Result<Value> {
    invoke_json(manager, connection_id, "list", request).await
}

async fn invoke_json(
    manager: &SSHConnectionManager,
    connection_id: &str,
    verb: &'static str,
    request: &Value,
) -> Result<Value> {
    ensure_plain_ssh_target(manager, connection_id).await?;
    let target = probe_remote_target(manager, connection_id).await?;
    let cli_path = target.cli_path.as_deref().ok_or_else(|| {
        anyhow!("BitFun CLI is not installed on the SSH target; confirm installation first")
    })?;
    invoke_json_at_path(
        manager,
        connection_id,
        &target.home,
        cli_path,
        verb,
        request,
    )
    .await
}

/// Transfer the request as an owner-only file instead of embedding its JSON in
/// the SSH command. Prompts can contain credentials or private source context;
/// command arguments would expose them to process listings and manager debug
/// previews on both machines.
async fn invoke_json_at_path(
    manager: &SSHConnectionManager,
    connection_id: &str,
    home: &str,
    cli_path: &str,
    verb: &'static str,
    request: &Value,
) -> Result<Value> {
    let request_dir = format!("{home}/{REQUEST_STATE_DIR}");
    exec_ok(
        manager,
        connection_id,
        &format!(
            "mkdir -p {dir} && chmod 700 {root} {dispatch} {dir}",
            root = shell_quote_posix(&format!("{home}/.bitfun")),
            dispatch = shell_quote_posix(&format!("{home}/.bitfun/dispatch")),
            dir = shell_quote_posix(&request_dir)
        ),
    )
    .await?;
    let request_path = format!("{request_dir}/{}.json", uuid::Uuid::new_v4().as_simple());
    let request_bytes = serde_json::to_vec(request).context("serialize dispatch request")?;
    // Pre-create with 0600 before SFTP opens it. Creating first and chmodding
    // afterwards would leave a prompt briefly governed by the server's umask
    // (commonly 0644).
    exec_ok(
        manager,
        connection_id,
        &format!(
            "umask 077; : > {request}; chmod 600 {request}",
            request = shell_quote_posix(&request_path)
        ),
    )
    .await?;
    if let Err(error) = manager
        .sftp_write(connection_id, &request_path, &request_bytes)
        .await
        .context("stage dispatch request")
    {
        let _ = manager.sftp_remove(connection_id, &request_path).await;
        return Err(error);
    }

    let command = dispatch_command(cli_path, verb, &request_path);
    let result = manager
        .execute_command_with_options(
            connection_id,
            &command,
            SSHCommandOptions {
                timeout_ms: Some(COMMAND_TIMEOUT_MS),
                cancellation_token: None,
            },
        )
        .await;
    // The remote EXIT trap normally removes it. This covers channel-open and
    // transport failures before the shell installed that trap.
    let _ = manager.sftp_remove(connection_id, &request_path).await;
    let result = result?;
    ensure_command_completed(&result, &format!("dispatch {verb}"))?;
    if result.exit_code != 0 {
        return Err(remote_command_error(
            &format!("dispatch {verb}"),
            result.exit_code,
            &result.stdout,
            &result.stderr,
        ));
    }
    serde_json::from_str(result.stdout.trim()).with_context(|| {
        format!(
            "dispatch {verb} returned invalid JSON: {}",
            bounded_detail(&result.stdout)
        )
    })
}

fn dispatch_command(cli_path: &str, verb: &str, request_path: &str) -> String {
    let cli = shell_quote_posix(cli_path);
    let request = shell_quote_posix(request_path);
    let verb = shell_quote_posix(verb);
    format!(
        "request={request}; \
         cleanup() {{ rm -f \"$request\"; }}; \
         trap cleanup EXIT; \
         trap 'exit 130' HUP INT TERM; \
         {cli} dispatch {verb} < \"$request\""
    )
}

async fn ensure_plain_ssh_target(
    manager: &SSHConnectionManager,
    connection_id: &str,
) -> Result<()> {
    let active_container = manager
        .get_connection_config(connection_id)
        .await
        .is_some_and(|config| config.container.is_some());
    let saved_container = manager
        .get_saved_connections()
        .await
        .into_iter()
        .find(|config| config.id == connection_id)
        .is_some_and(|config| config.container.is_some());
    if active_container || saved_container {
        return Err(anyhow!(
            "SSH dispatch does not support Docker-container connection targets"
        ));
    }
    Ok(())
}

async fn probe_remote_target(
    manager: &SSHConnectionManager,
    connection_id: &str,
) -> Result<RemoteTarget> {
    let script = probe_remote_target_script();
    let result = manager
        .execute_command_with_options(
            connection_id,
            script,
            SSHCommandOptions {
                timeout_ms: Some(COMMAND_TIMEOUT_MS),
                cancellation_token: None,
            },
        )
        .await?;
    ensure_command_completed(&result, "probe SSH dispatch target")?;
    if result.exit_code != 0 {
        return Err(remote_command_error(
            "probe SSH dispatch target",
            result.exit_code,
            &result.stdout,
            &result.stderr,
        ));
    }
    let get = |key: &str| {
        result
            .stdout
            .lines()
            .find_map(|line| {
                line.strip_prefix(key)
                    .and_then(|rest| rest.strip_prefix('='))
            })
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let home = get("home");
    if home.is_empty() {
        return Err(anyhow!("could not resolve remote $HOME"));
    }
    let cli_path = get("cli");
    Ok(RemoteTarget {
        os: get("os"),
        arch: get("arch"),
        home,
        cli_path: (!cli_path.is_empty()).then_some(cli_path),
        tar_available: get("tar") == "1",
    })
}

fn probe_remote_target_script() -> &'static str {
    r#"
LC_ALL=C
printf 'os=%s\n' "$(uname -s 2>/dev/null || true)"
printf 'arch=%s\n' "$(uname -m 2>/dev/null || true)"
printf 'home=%s\n' "$HOME"
if command -v tar >/dev/null 2>&1; then printf 'tar=1\n'; else printf 'tar=0\n'; fi
if [ -x "$HOME/.local/bin/bitfun" ]; then
  BITFUN_BIN="$HOME/.local/bin/bitfun"
else
  BITFUN_BIN="$(command -v bitfun 2>/dev/null || true)"
fi
printf 'cli=%s\n' "$BITFUN_BIN"
"#
}

async fn resolve_release(os: &str, arch: &str) -> Result<ResolvedRelease> {
    let pubkey = require_release_pubkey()?;
    let target = release_target(os, arch)?;
    let version = RELEASE_VERSION.split('+').next().unwrap_or(RELEASE_VERSION);
    let filename = format!("bitfun-cli-{version}-{target}.tar.gz");
    let tag = release_tag_for_version(RELEASE_VERSION);
    let url = format!("{RELEASE_BASE}/download/{tag}/{filename}");
    let checksum_url = format!("{url}.sha256");
    let checksum_signature_url = format!("{checksum_url}.sig");
    let archive_signature_url = format!("{url}.sig");
    let client = release_http_client()?;
    let checksum = fetch_required_text(&client, &checksum_url).await?;
    let signature = fetch_required_text(&client, &checksum_signature_url).await?;
    let sha256 = verify_signed_checksum(&checksum, &signature, pubkey, &filename)?;

    Ok(ResolvedRelease {
        public: DispatchCliRelease {
            version: version.to_string(),
            target: target.to_string(),
            url,
            sha256,
        },
        filename,
        checksum_url,
        checksum_signature_url,
        archive_signature_url,
    })
}

fn release_target(os: &str, arch: &str) -> Result<&'static str> {
    match (os.trim(), arch.trim()) {
        ("Linux", "x86_64" | "amd64") => Ok("x86_64-unknown-linux-gnu"),
        ("Linux", "aarch64" | "arm64") => Ok("aarch64-unknown-linux-gnu"),
        ("Darwin", "x86_64" | "amd64") => Ok("x86_64-apple-darwin"),
        ("Darwin", "aarch64" | "arm64") => Ok("aarch64-apple-darwin"),
        (os, arch) => Err(anyhow!(
            "BitFun SSH dispatch CLI install does not support {os} {arch}"
        )),
    }
}

fn release_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        // Per-read rather than whole-request: a genuine slow link may take
        // longer than an arbitrary archive deadline, but a stalled source must
        // still fail instead of hanging the installer forever.
        .read_timeout(Duration::from_secs(RELEASE_READ_TIMEOUT_SECONDS))
        .build()
        .context("build BitFun release HTTP client")
}

async fn fetch_required_text(client: &reqwest::Client, url: &str) -> Result<String> {
    client
        .get(url)
        .send()
        .await
        .with_context(|| format!("request {url}"))?
        .error_for_status()
        .with_context(|| format!("download {url}"))?
        .text()
        .await
        .with_context(|| format!("read {url}"))
}

async fn download_verified_archive(release: &ResolvedRelease) -> Result<Vec<u8>> {
    let pubkey = require_release_pubkey()?;
    let client = release_http_client()?;

    // Re-fetch and verify the signed sidecar at install time instead of trusting
    // a possibly stale preflight result.
    let checksum = fetch_required_text(&client, &release.checksum_url).await?;
    let checksum_signature = fetch_required_text(&client, &release.checksum_signature_url).await?;
    let expected =
        verify_signed_checksum(&checksum, &checksum_signature, pubkey, &release.filename)?;
    if !expected.eq_ignore_ascii_case(&release.public.sha256) {
        return Err(anyhow!(
            "release checksum changed after preflight; refusing to install"
        ));
    }

    let mut response = client
        .get(&release.public.url)
        .send()
        .await
        .with_context(|| format!("request {}", release.public.url))?
        .error_for_status()
        .with_context(|| format!("download {}", release.public.url))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ARCHIVE_BYTES as u64)
    {
        return Err(anyhow!(
            "BitFun CLI archive exceeds the {} MB safety limit",
            MAX_ARCHIVE_BYTES / (1024 * 1024)
        ));
    }
    let mut archive = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .with_context(|| format!("read {}", release.public.url))?
    {
        extend_bounded_archive(&mut archive, &chunk, MAX_ARCHIVE_BYTES)?;
    }
    verify_sha256(&archive, &expected, &release.filename)?;

    let archive_signature = fetch_required_text(&client, &release.archive_signature_url).await?;
    verify_minisign(&archive, &archive_signature, pubkey)?;
    Ok(archive)
}

fn extend_bounded_archive(archive: &mut Vec<u8>, chunk: &[u8], limit: usize) -> Result<()> {
    if archive.len().saturating_add(chunk.len()) > limit {
        return Err(anyhow!(
            "BitFun CLI archive exceeds the {} MB safety limit",
            limit / (1024 * 1024)
        ));
    }
    archive.extend_from_slice(chunk);
    Ok(())
}

fn install_body_script(dir: &str, archive_path: &str, expected_version: &str) -> String {
    format!(
        r#"#!/bin/bash
set -euo pipefail
umask 077
D={dir}
ARCHIVE={archive}
EXPECTED_VERSION={version}
TOKEN="${{1:-}}"
PIDF="$D/{INSTALL_STEM}.pid"
EXITF="$D/{INSTALL_STEM}.exit"
TMP="$D/unpack.$$"
PRIMARY_TARGET="$HOME/.local/bin/bitfun"
LEGACY_TARGET="$HOME/.local/bin/bitfun-cli"
PRIMARY_NEW="$HOME/.local/bin/.bitfun-dispatch-new-$$"
LEGACY_NEW="$HOME/.local/bin/.bitfun-cli-dispatch-new-$$"
PRIMARY_BACKUP="$D/previous-bitfun.$$"
LEGACY_BACKUP="$D/previous-bitfun-cli.$$"
HAD_PRIMARY=0
HAD_LEGACY=0
PRIMARY_INSTALLED=0
LEGACY_INSTALLED=0
COMMITTED=0
rollback_install() {{
  if [ "$LEGACY_INSTALLED" = "1" ]; then rm -f "$LEGACY_TARGET"; fi
  if [ "$PRIMARY_INSTALLED" = "1" ]; then rm -f "$PRIMARY_TARGET"; fi
  if [ "$HAD_LEGACY" = "1" ] && [ -f "$LEGACY_BACKUP" ]; then
    mv -f "$LEGACY_BACKUP" "$LEGACY_TARGET" \
      || echo "ERROR: could not restore previous bitfun-cli" >&2
  fi
  if [ "$HAD_PRIMARY" = "1" ] && [ -f "$PRIMARY_BACKUP" ]; then
    mv -f "$PRIMARY_BACKUP" "$PRIMARY_TARGET" \
      || echo "ERROR: could not restore previous bitfun" >&2
  fi
}}
finish() {{
  code=$?
  trap - EXIT HUP INT TERM
  if [ "$code" -ne 0 ] && [ "$COMMITTED" != "1" ]; then rollback_install; fi
  rm -f "$PRIMARY_NEW" "$LEGACY_NEW"
  if [ "$COMMITTED" = "1" ]; then rm -f "$PRIMARY_BACKUP" "$LEGACY_BACKUP"; fi
  rm -rf "$TMP"
  printf '%s\n' "$code" >"$EXITF"
  marker_token="$(sed -n '2p' "$PIDF" 2>/dev/null | tr -d '[:space:]')"
  if [ -n "$TOKEN" ] && [ "$marker_token" = "$TOKEN" ]; then rm -f "$PIDF"; fi
  exit "$code"
}}
trap finish EXIT
trap 'exit 130' HUP INT TERM
rm -f "$EXITF"
mkdir -p "$TMP" "$HOME/.local/bin" "$HOME/.bitfun"
chmod 700 "$HOME/.bitfun"
tar -xzf "$ARCHIVE" -C "$TMP"
PRIMARY=""
LEGACY=""
for candidate in "$TMP"/*/bitfun; do
  [ -f "$candidate" ] || continue
  [ -z "$PRIMARY" ] || {{ echo "ERROR: archive contains multiple bitfun binaries" >&2; exit 1; }}
  PRIMARY="$candidate"
done
for candidate in "$TMP"/*/bitfun-cli; do
  [ -f "$candidate" ] || continue
  [ -z "$LEGACY" ] || {{ echo "ERROR: archive contains multiple bitfun-cli binaries" >&2; exit 1; }}
  LEGACY="$candidate"
done
[ -n "$PRIMARY" ] || {{ echo "ERROR: archive contains no bitfun binary" >&2; exit 1; }}
[ -n "$LEGACY" ] || {{ echo "ERROR: archive contains no bitfun-cli binary" >&2; exit 1; }}
cp "$PRIMARY" "$PRIMARY_NEW"
cp "$LEGACY" "$LEGACY_NEW"
chmod 755 "$PRIMARY_NEW" "$LEGACY_NEW"
staged="$("$PRIMARY_NEW" --version 2>/dev/null || true)"
case "$staged" in
  *"$EXPECTED_VERSION"*) ;;
  *) echo "ERROR: staged CLI version did not match $EXPECTED_VERSION: $staged" >&2; exit 1 ;;
esac
"$LEGACY_NEW" --version >/dev/null 2>&1 \
  || {{ echo "ERROR: staged bitfun-cli companion did not run" >&2; exit 1; }}
if [ -e "$PRIMARY_TARGET" ]; then
  mv -f "$PRIMARY_TARGET" "$PRIMARY_BACKUP"
  HAD_PRIMARY=1
fi
if [ -e "$LEGACY_TARGET" ]; then
  mv -f "$LEGACY_TARGET" "$LEGACY_BACKUP"
  HAD_LEGACY=1
fi
mv -f "$PRIMARY_NEW" "$PRIMARY_TARGET"
PRIMARY_INSTALLED=1
mv -f "$LEGACY_NEW" "$LEGACY_TARGET"
LEGACY_INSTALLED=1
installed="$("$PRIMARY_TARGET" --version 2>/dev/null || true)"
case "$installed" in
  *"$EXPECTED_VERSION"*) ;;
  *) echo "ERROR: installed CLI version did not match $EXPECTED_VERSION: $installed" >&2; exit 1 ;;
esac
"$LEGACY_TARGET" --version >/dev/null 2>&1 \
  || {{ echo "ERROR: installed bitfun-cli companion did not run" >&2; exit 1; }}
COMMITTED=1
rm -f "$ARCHIVE"
echo "Installed $installed at $HOME/.local/bin/bitfun"
echo {INSTALL_DONE_MARKER}
"#,
        dir = shell_quote_posix(dir),
        archive = shell_quote_posix(archive_path),
        version = shell_quote_posix(expected_version),
    )
}

fn install_driver_script(dir: &str, body_path: &str, install_token: &str) -> String {
    format!(
        r#"#!/bin/bash
set -euo pipefail
umask 077
D={dir}
BODY={body}
EXPECTED_TOKEN={token}
TOKEN="${{1:-}}"
LOG="$D/{INSTALL_STEM}.log"
PIDF="$D/{INSTALL_STEM}.pid"
PIDF_TMP="$PIDF.$$"
DRIVER_PIDF="$D/{INSTALL_STEM}.driver.pid"
DRIVER_PIDF_TMP="$DRIVER_PIDF.$$"
PREPF="$D/{INSTALL_STEM}.preparing"
EXITF="$D/{INSTALL_STEM}.exit"
body_pid=
[ "$TOKEN" = "$EXPECTED_TOKEN" ] || exit 2
cleanup_prepare() {{
  if [ -n "$body_pid" ] && [ ! -f "$PIDF" ]; then
    kill "$body_pid" 2>/dev/null || true
  fi
  prepare_token="$(tr -d '[:space:]' < "$PREPF" 2>/dev/null || true)"
  if [ "$prepare_token" = "$TOKEN" ]; then rm -f "$PREPF"; fi
  driver_marker_token="$(sed -n '2p' "$DRIVER_PIDF" 2>/dev/null | tr -d '[:space:]')"
  if [ "$driver_marker_token" = "$TOKEN" ]; then rm -f "$DRIVER_PIDF"; fi
  rm -f "$PIDF_TMP" "$DRIVER_PIDF_TMP"
}}
cancel_prepare() {{ cleanup_prepare; exit 130; }}
trap cleanup_prepare EXIT
trap cancel_prepare HUP INT TERM
printf '%s\n%s\n' "$$" "$TOKEN" >"$DRIVER_PIDF_TMP"
mv -f "$DRIVER_PIDF_TMP" "$DRIVER_PIDF"
prepare_token="$(tr -d '[:space:]' < "$PREPF" 2>/dev/null || true)"
[ "$prepare_token" = "$TOKEN" ] || exit 130
rm -f "$PIDF" "$EXITF"
: >"$LOG"
prepare_token="$(tr -d '[:space:]' < "$PREPF" 2>/dev/null || true)"
[ "$prepare_token" = "$TOKEN" ] || exit 130
nohup bash "$BODY" "$TOKEN" >"$LOG" 2>&1 < /dev/null &
body_pid=$!
printf '%s\n%s\n' "$body_pid" "$TOKEN" >"$PIDF_TMP"
mv -f "$PIDF_TMP" "$PIDF"
body_pid=
rm -f "$PREPF" "$DRIVER_PIDF"
trap - EXIT HUP INT TERM
exit 0
"#,
        dir = shell_quote_posix(dir),
        body = shell_quote_posix(body_path),
        token = shell_quote_posix(install_token),
    )
}

#[allow(clippy::too_many_arguments)]
fn stage_install_command(
    archive_path: &str,
    body_path: &str,
    script_path: &str,
    log_path: &str,
    pid_path: &str,
    driver_pid_path: &str,
    prepare_path: &str,
    exit_path: &str,
    install_token: &str,
) -> String {
    format!(
        "chmod 600 {archive} \
         && chmod 700 {body} {script} \
         && rm -f {pid} {driver_pid} {exit} \
         && : > {log} && chmod 600 {log} \
         && printf '%s\\n' {token} > {prepare} && chmod 600 {prepare}",
        archive = shell_quote_posix(archive_path),
        body = shell_quote_posix(body_path),
        script = shell_quote_posix(script_path),
        pid = shell_quote_posix(pid_path),
        driver_pid = shell_quote_posix(driver_pid_path),
        exit = shell_quote_posix(exit_path),
        log = shell_quote_posix(log_path),
        prepare = shell_quote_posix(prepare_path),
        token = shell_quote_posix(install_token),
    )
}

fn install_poll_script(cursor: u64) -> String {
    format!(
        r#"
D="$HOME/{INSTALL_STATE_DIR}"
LOG="$D/{INSTALL_STEM}.log"
PIDF="$D/{INSTALL_STEM}.pid"
DRIVER_PIDF="$D/{INSTALL_STEM}.driver.pid"
PREPF="$D/{INSTALL_STEM}.preparing"
EXITF="$D/{INSTALL_STEM}.exit"
BODY="$D/{INSTALL_STEM}-body.sh"
DRIVER="$D/{INSTALL_STEM}.sh"
process_command() {{
  p="$1"
  case "$p" in ''|*[!0-9]*) return 1 ;; esac
  kill -0 "$p" 2>/dev/null || return 1
  ps -ww -p "$p" -o command= 2>/dev/null
}}
installer_matches() {{
  p="$1"
  token="$2"
  [ -n "$token" ] || return 1
  command="$(process_command "$p")" || return 1
  case "$command" in
    *"$BODY"*"$token"*) return 0 ;;
    *) return 1 ;;
  esac
}}
driver_matches() {{
  p="$1"
  token="$2"
  [ -n "$token" ] || return 1
  command="$(process_command "$p")" || return 1
  case "$command" in
    *"$DRIVER"*"$token"*) return 0 ;;
    *) return 1 ;;
  esac
}}
running=0
if [ -f "$PIDF" ]; then
  pid="$(sed -n '1p' "$PIDF" | tr -d '[:space:]')"
  token="$(sed -n '2p' "$PIDF" | tr -d '[:space:]')"
  if installer_matches "$pid" "$token"; then
    running=1
  else
    rm -f "$PIDF"
  fi
fi
preparing=0
if [ -f "$PREPF" ]; then
  driver_live=0
  prepare_token="$(tr -d '[:space:]' < "$PREPF" 2>/dev/null || true)"
  if [ -f "$DRIVER_PIDF" ]; then
    driver_pid="$(sed -n '1p' "$DRIVER_PIDF" | tr -d '[:space:]')"
    driver_token="$(sed -n '2p' "$DRIVER_PIDF" | tr -d '[:space:]')"
    if [ "$driver_token" = "$prepare_token" ] && driver_matches "$driver_pid" "$driver_token"; then
      driver_live=1
    else
      rm -f "$DRIVER_PIDF"
    fi
  fi
  if [ "$driver_live" = "1" ]; then
    preparing=1
  else
    now="$(date +%s 2>/dev/null || true)"
    mtime="$(stat -c %Y "$PREPF" 2>/dev/null || stat -f %m "$PREPF" 2>/dev/null || true)"
    if [ -n "$now" ] && [ -n "$mtime" ] && [ $((now - mtime)) -lt {grace} ]; then
      preparing=1
    fi
  fi
fi
size=0
if [ -f "$LOG" ]; then size="$(wc -c < "$LOG" | tr -d ' ')"; fi
marker=0
if [ -f "$LOG" ] && grep -q {marker} "$LOG"; then marker=1; fi
exit_recorded=0
exit_code=
if [ -f "$EXITF" ]; then
  exit_recorded=1
  exit_code="$(tr -d '[:space:]' < "$EXITF")"
fi
printf 'running=%s\n' "$running"
printf 'preparing=%s\n' "$preparing"
printf 'size=%s\n' "$size"
printf 'marker=%s\n' "$marker"
printf 'exit_recorded=%s\n' "$exit_recorded"
printf 'exit_code=%s\n' "$exit_code"
printf '%s\n' '---'
from={from}
if [ "$from" -gt "$size" ]; then from=1; fi
if [ -f "$LOG" ]; then tail -c +"$from" "$LOG"; fi
"#,
        grace = INSTALL_PREPARE_GRACE_SECONDS,
        marker = shell_quote_posix(INSTALL_DONE_MARKER),
        from = cursor.saturating_add(1),
    )
}

fn install_cancel_script() -> String {
    format!(
        r#"
set +e
D="$HOME/{INSTALL_STATE_DIR}"
LOG="$D/{INSTALL_STEM}.log"
PIDF="$D/{INSTALL_STEM}.pid"
DRIVER_PIDF="$D/{INSTALL_STEM}.driver.pid"
PREPF="$D/{INSTALL_STEM}.preparing"
EXITF="$D/{INSTALL_STEM}.exit"
BODY="$D/{INSTALL_STEM}-body.sh"
DRIVER="$D/{INSTALL_STEM}.sh"
active=0
[ -f "$PREPF" ] && active=1
# Invalidate a driver that has not reached its guarded spawn point yet.
rm -f "$PREPF"
installer_matches() {{
  p="$1"
  token="$2"
  case "$p" in ''|*[!0-9]*) return 1 ;; esac
  [ -n "$token" ] || return 1
  kill -0 "$p" 2>/dev/null || return 1
  command="$(ps -ww -p "$p" -o command= 2>/dev/null)" || return 1
  case "$command" in
    *"$BODY"*"$token"*) return 0 ;;
    *) return 1 ;;
  esac
}}
driver_matches() {{
  p="$1"
  token="$2"
  case "$p" in ''|*[!0-9]*) return 1 ;; esac
  [ -n "$token" ] || return 1
  kill -0 "$p" 2>/dev/null || return 1
  command="$(ps -ww -p "$p" -o command= 2>/dev/null)" || return 1
  case "$command" in
    *"$DRIVER"*"$token"*) return 0 ;;
    *) return 1 ;;
  esac
}}
kill_tree() {{
  p="$1"
  sig="$2"
  case "$p" in ''|*[!0-9]*) return 0 ;; esac
  for child in $(pgrep -P "$p" 2>/dev/null); do kill_tree "$child" "$sig"; done
  kill "-$sig" "$p" 2>/dev/null || true
}}
if [ -f "$DRIVER_PIDF" ]; then
  driver_pid="$(sed -n '1p' "$DRIVER_PIDF" | tr -d '[:space:]')"
  driver_token="$(sed -n '2p' "$DRIVER_PIDF" | tr -d '[:space:]')"
  if driver_matches "$driver_pid" "$driver_token"; then
    active=1
    kill_tree "$driver_pid" TERM
    sleep 1
    if driver_matches "$driver_pid" "$driver_token"; then
      kill_tree "$driver_pid" KILL
    fi
  fi
fi
if [ -f "$PIDF" ]; then
  pid="$(sed -n '1p' "$PIDF" | tr -d '[:space:]')"
  token="$(sed -n '2p' "$PIDF" | tr -d '[:space:]')"
  if installer_matches "$pid" "$token"; then
    active=1
    kill_tree "$pid" TERM
    sleep 1
    if installer_matches "$pid" "$token"; then kill_tree "$pid" KILL; fi
  fi
fi
rm -f "$PIDF" "$DRIVER_PIDF" "$PREPF"
if [ "$active" = "1" ]; then
  printf '130\n' >"$EXITF"
  printf '\nBitFun CLI installation cancelled by client.\n' >>"$LOG"
fi
exit 0
"#
    )
}

fn to_unix_script(script: &str) -> String {
    script.replace("\r\n", "\n")
}

fn split_metadata_output(stdout: &str) -> (&str, &str) {
    if let Some((head, output)) = stdout.split_once("---\r\n") {
        return (head, output);
    }
    if let Some((head, output)) = stdout.split_once("---\n") {
        return (head, output);
    }
    let mut offset = 0usize;
    for line in stdout.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return (&stdout[..offset], &stdout[offset + line.len()..]);
        }
        offset += line.len();
    }
    (stdout, "")
}

async fn exec_ok(manager: &SSHConnectionManager, connection_id: &str, command: &str) -> Result<()> {
    let result = manager
        .execute_command_with_options(
            connection_id,
            command,
            SSHCommandOptions {
                timeout_ms: Some(COMMAND_TIMEOUT_MS),
                cancellation_token: None,
            },
        )
        .await?;
    ensure_command_completed(&result, "remote setup")?;
    if result.exit_code != 0 {
        return Err(remote_command_error(
            "remote setup",
            result.exit_code,
            &result.stdout,
            &result.stderr,
        ));
    }
    Ok(())
}

fn ensure_command_completed(
    result: &super::types::SSHCommandResult,
    operation: &str,
) -> Result<()> {
    if result.timed_out {
        return Err(anyhow!("{operation} timed out"));
    }
    if result.interrupted {
        return Err(anyhow!("{operation} was cancelled"));
    }
    Ok(())
}

fn remote_command_error(
    operation: &str,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) -> anyhow::Error {
    let detail = if stderr.trim().is_empty() {
        bounded_detail(stdout)
    } else {
        bounded_detail(stderr)
    };
    anyhow!("{operation} failed (exit {exit_code}): {detail}")
}

fn bounded_detail(value: &str) -> String {
    value.trim().chars().take(500).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_install_scripts_are_lf_only_and_never_use_sudo() {
        let body = install_body_script(
            "/home/user/.bitfun/dispatch/install",
            "/home/user/.bitfun/dispatch/install/archive.tar.gz",
            "1.2.3",
        );
        let driver = install_driver_script(
            "/home/user/.bitfun/dispatch/install",
            "/home/user/.bitfun/dispatch/install/install-cli-body.sh",
            "bitfun-install-test-token",
        );
        for (name, script) in [("body", body), ("driver", driver)] {
            let script = to_unix_script(&script);
            assert!(!script.contains('\r'), "{name} must be LF-only");
            assert!(
                !script.contains("sudo"),
                "{name} must never modify privileged paths"
            );
            assert!(!script.contains("/usr/"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn generated_install_scripts_parse_as_bash() {
        for script in [
            install_body_script(
                "/home/user/.bitfun/dispatch/install",
                "/home/user/.bitfun/dispatch/install/archive.tar.gz",
                "1.2.3",
            ),
            install_driver_script(
                "/home/user/.bitfun/dispatch/install",
                "/home/user/.bitfun/dispatch/install/install-cli-body.sh",
                "bitfun-install-test-token",
            ),
            install_poll_script(17),
            install_cancel_script(),
        ] {
            let output = std::process::Command::new("bash")
                .args(["-n", "-c", &to_unix_script(&script)])
                .output()
                .expect("parse generated shell");
            assert!(
                output.status.success(),
                "generated shell is invalid:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn stale_installer_pid_never_signals_an_unrelated_process() {
        let temp = tempfile::tempdir().expect("temp dir");
        let state_dir = temp.path().join(INSTALL_STATE_DIR);
        std::fs::create_dir_all(&state_dir).expect("install state dir");
        let pid_path = state_dir.join(format!("{INSTALL_STEM}.pid"));
        let mut unrelated = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn unrelated process");

        std::fs::write(
            &pid_path,
            format!("{}\nstale-install-token\n", unrelated.id()),
        )
        .expect("stale pid marker");
        let poll = std::process::Command::new("bash")
            .args(["-c", &install_poll_script(0)])
            .env("HOME", temp.path())
            .output()
            .expect("poll stale installer");
        assert!(poll.status.success());
        assert!(
            String::from_utf8_lossy(&poll.stdout).contains("running=0"),
            "a reused PID must not be reported as the installer"
        );
        assert!(!pid_path.exists(), "poll must clear the stale marker");

        std::fs::write(
            &pid_path,
            format!("{}\nstale-install-token\n", unrelated.id()),
        )
        .expect("restore stale pid marker");
        std::fs::write(
            state_dir.join(format!("{INSTALL_STEM}.driver.pid")),
            format!("{}\nstale-driver-token\n", unrelated.id()),
        )
        .expect("stale driver pid marker");
        std::fs::write(
            state_dir.join(format!("{INSTALL_STEM}.preparing")),
            "stale-driver-token\n",
        )
        .expect("stale prepare marker");
        let cancel = std::process::Command::new("bash")
            .args(["-c", &install_cancel_script()])
            .env("HOME", temp.path())
            .output()
            .expect("cancel stale installer");
        assert!(cancel.status.success());
        assert!(
            unrelated
                .try_wait()
                .expect("inspect unrelated process")
                .is_none(),
            "cancellation must never signal a process that does not match the installer identity"
        );
        assert!(!pid_path.exists(), "cancel must clear the stale marker");

        unrelated.kill().expect("stop unrelated process");
        unrelated.wait().expect("reap unrelated process");
    }

    #[cfg(unix)]
    #[test]
    fn matching_installer_identity_is_cancelled() {
        let temp = tempfile::tempdir().expect("temp dir");
        let state_dir = temp.path().join(INSTALL_STATE_DIR);
        std::fs::create_dir_all(&state_dir).expect("install state dir");
        let body_path = state_dir.join(format!("{INSTALL_STEM}-body.sh"));
        std::fs::write(&body_path, "sleep 30\n").expect("installer body");
        let token = "bitfun-install-test-token";
        let mut installer = std::process::Command::new("bash")
            .arg(&body_path)
            .arg(token)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn installer");
        std::fs::write(
            state_dir.join(format!("{INSTALL_STEM}.pid")),
            format!("{}\n{token}\n", installer.id()),
        )
        .expect("installer pid marker");

        let cancel = std::process::Command::new("bash")
            .args(["-c", &install_cancel_script()])
            .env("HOME", temp.path())
            .output()
            .expect("cancel installer");
        assert!(cancel.status.success());
        let stopped = installer
            .try_wait()
            .expect("inspect installer process")
            .is_some();
        if !stopped {
            installer.kill().expect("cleanup installer process");
            installer.wait().expect("reap installer process");
        }
        assert!(
            stopped,
            "the exact PID/token/body identity must be cancelled"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cancelled_prepare_token_prevents_the_driver_from_spawning() {
        let temp = tempfile::tempdir().expect("temp dir");
        let state_dir = temp.path().join(INSTALL_STATE_DIR);
        std::fs::create_dir_all(&state_dir).expect("install state dir");
        let body_path = state_dir.join(format!("{INSTALL_STEM}-body.sh"));
        let sentinel = temp.path().join("installer-started");
        std::fs::write(
            &body_path,
            format!("touch {}\n", shell_quote_posix(&sentinel.to_string_lossy())),
        )
        .expect("installer body");
        let token = "bitfun-install-cancelled-before-driver";
        let driver = install_driver_script(
            &state_dir.to_string_lossy(),
            &body_path.to_string_lossy(),
            token,
        );

        let output = std::process::Command::new("bash")
            .args(["-c", &driver, "install-driver", token])
            .output()
            .expect("run invalidated driver");
        assert_eq!(output.status.code(), Some(130));
        assert!(
            !sentinel.exists(),
            "a driver whose prepare token was removed must not start the body"
        );
        assert!(
            !state_dir
                .join(format!("{INSTALL_STEM}.driver.pid"))
                .exists(),
            "the invalidated driver must clean its identity marker"
        );
    }

    #[cfg(unix)]
    #[test]
    fn dispatch_command_round_trips_quoted_paths_and_removes_request() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("temp dir");
        let bin_dir = temp.path().join("bin with space");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");
        let cli = bin_dir.join("bitfun's");
        std::fs::write(&cli, "#!/bin/sh\ncat\n").expect("fake CLI");
        let mut permissions = std::fs::metadata(&cli).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&cli, permissions).unwrap();

        let request = temp.path().join("request with ' quote.json");
        let payload = r#"{"prompt":"$(touch should-not-run); it's literal"}"#;
        std::fs::write(&request, payload).expect("request");
        let command =
            dispatch_command(&cli.to_string_lossy(), "submit", &request.to_string_lossy());
        assert!(
            !command.contains(payload),
            "request JSON must never be embedded in the shell command"
        );
        let output = std::process::Command::new("bash")
            .args(["-c", &command])
            .output()
            .expect("run quoted dispatch command");
        assert!(
            output.status.success(),
            "quoted command failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), payload);
        assert!(!request.exists(), "EXIT trap must remove the request");
        assert!(!temp.path().join("should-not-run").exists());
    }

    #[test]
    fn release_target_accepts_phase_one_unix_architectures() {
        assert_eq!(
            release_target("Linux", "amd64").unwrap(),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            release_target("Darwin", "arm64").unwrap(),
            "aarch64-apple-darwin"
        );
        assert!(release_target("Windows", "x86_64").is_err());
    }

    #[test]
    fn probe_prefers_the_managed_cli_over_an_incompatible_path_copy() {
        let script = probe_remote_target_script();
        let managed = script
            .find(r#"[ -x "$HOME/.local/bin/bitfun" ]"#)
            .expect("managed CLI check");
        let path_lookup = script.find("command -v bitfun").expect("PATH fallback");
        assert!(managed < path_lookup);
    }

    #[test]
    fn poll_metadata_split_accepts_lf_and_crlf() {
        let (head, output) = split_metadata_output("running=1\n---\nhello\n");
        assert_eq!(head, "running=1\n");
        assert_eq!(output, "hello\n");
        let (head, output) = split_metadata_output("running=1\r\n---\r\nhello\r\n");
        assert_eq!(head, "running=1\r\n");
        assert_eq!(output, "hello\r\n");
    }

    #[test]
    fn checksum_parser_used_by_dispatch_rejects_malformed_sidecars() {
        assert!(crate::remote_ssh::release_verify::parse_sha256(
            "not-a-checksum",
            "archive.tar.gz"
        )
        .is_err());
    }

    #[test]
    fn streaming_archive_limit_is_enforced_before_appending_a_chunk() {
        let mut archive = vec![1, 2, 3];
        extend_bounded_archive(&mut archive, &[4], 4).unwrap();
        assert_eq!(archive, vec![1, 2, 3, 4]);
        assert!(extend_bounded_archive(&mut archive, &[5], 4).is_err());
        assert_eq!(
            archive,
            vec![1, 2, 3, 4],
            "the over-limit chunk must never be buffered"
        );
    }

    #[test]
    fn installation_is_bound_to_the_exact_confirmed_release() {
        let confirmed = DispatchCliRelease {
            version: "1.2.3".to_string(),
            target: "aarch64-apple-darwin".to_string(),
            url: "https://example.test/bitfun.tar.gz".to_string(),
            sha256: "a".repeat(64),
        };
        ensure_confirmed_release(&confirmed, &confirmed).expect("exact asset");

        let mut changed = confirmed.clone();
        changed.sha256 = "b".repeat(64);
        assert!(ensure_confirmed_release(&changed, &confirmed).is_err());
    }

    #[test]
    fn incompatible_dispatch_protocols_require_an_upgrade() {
        let capabilities = REQUIRED_DISPATCH_CAPABILITIES;
        let compatible = serde_json::json!({
            "protocolVersion": 1,
            "capabilities": capabilities,
        });
        assert!(dispatch_protocol_is_compatible(&compatible));

        let old = serde_json::json!({
            "protocolVersion": 0,
            "capabilities": capabilities,
        });
        assert!(!dispatch_protocol_is_compatible(&old));

        let missing = serde_json::json!({
            "protocolVersion": 1,
            "capabilities": ["persistent_jobs", "cursor_events"],
        });
        assert!(!dispatch_protocol_is_compatible(&missing));

        let reject_only = serde_json::json!({
            "protocolVersion": 1,
            "capabilities": [
                "persistent_jobs",
                "cursor_events",
                "detached_worker",
                "workspace_serialization",
                "frontend_event_projection",
                "approval_reject_and_report"
            ],
        });
        validate_dispatch_protocol(&reject_only, Some("reject-and-report"))
            .expect("selected policy is supported");
        assert!(validate_dispatch_protocol(&reject_only, Some("auto")).is_err());
    }
}
