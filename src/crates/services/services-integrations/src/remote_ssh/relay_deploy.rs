//! One-click relay server self-deploy orchestration over an existing SSH connection.
//!
//! Drives the open-source relay-server deployment on a user-owned server:
//!
//! 1. `run_preflight` — probe OS/arch, Docker access mode, memory, port, existing installs.
//! 2. `start_task` — stage an interactive driver script (run inside a remote PTY so sudo
//!    passwords work) that installs Docker when needed, then pulls and starts the signed
//!    multi-platform image via `nohup` while `tail -f` streams its log.
//! 3. `poll_task` — detect completion via marker/pid for wizard state transitions.
//! 4. `cancel_task` — stop a running task when the wizard closes (kill process tree;
//!    the image script restores any staged previous container).
//! 5. `import_account` — hand a locally-provisioned account to `relay-admin import-user`.
//!
//! Remote deploy state lives under `~/.bitfun/relay-deploy/`. One-click deploy
//! never clones the repository or compiles on the customer server.
//!
//! Product / regression invariants (wizard + entry points):
//! `src/web-ui/src/features/relay-deploy/README.md`. Do not change clone destination,
//! password handoff, or “already deployed” semantics without updating that doc.
//! China mirror helpers live in `src/apps/relay-server/mirror.sh` and are embedded
//! here so detection/apply runs before GitHub/Docker downloads.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::manager::SSHConnectionManager;
#[cfg(test)]
use super::release_verify::RELEASE_PUBKEY;
use super::release_verify::{release_pubkey, release_tag_for_version, verify_minisign};
use super::remote_git::shell_quote_posix;

/// Default public relay port, matching `src/apps/relay-server/docker-compose.yml`.
pub const RELAY_PORT: u16 = 9700;

/// Validate a user-selected relay listen port (1–65535; 0 → default).
pub fn normalize_relay_port(port: u16) -> Result<u16> {
    if port == 0 {
        return Ok(RELAY_PORT);
    }
    // u16 already caps at 65535; reject only the zero case above.
    Ok(port)
}
/// Relay container name, matching docker-compose.yml.
const RELAY_CONTAINER_NAME: &str = "bitfun-relay";
/// Account DB path inside the relay container (RELAY_DB_PATH in docker-compose.yml).
const RELAY_CONTAINER_DB: &str = "/app/data/bitfun_relay.db";
/// Canonical repository URLs supplied to the shared regional-routing helper.
const REPO_GIT_URL: &str = "https://github.com/GCWing/BitFun.git";
/// Tarball fallback when git is unavailable or clone/fetch fails.
const REPO_TARBALL_URL: &str = "https://github.com/GCWing/BitFun/archive/refs/heads/main.tar.gz";
/// Release asset bases. GitHub is authoritative; OpenBitFun mirrors the same
/// signed bytes and is used when GitHub metadata is unavailable.
const RELEASE_BASE: &str = "https://github.com/GCWing/BitFun/releases";
const OPENBITFUN_RELEASE_BASE: &str = "https://openbitfun.com/release";
const RELAY_IMAGE_REPOSITORY: &str = "ghcr.io/gcwing/bitfun-relay-server";
const RELAY_IMAGE_DESCRIPTOR_ASSET: &str = "relay-image.json";
/// Canonical China-mirror helper (shared with `src/apps/relay-server/deploy.sh`).
/// Embedded so Desktop orchestration can select Docker-install and image routes.
const RELAY_MIRROR_SH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/relay-server/mirror.sh"
));
/// Published-binary download + runtime deploy (shared with `deploy.sh`, so the
/// manual and one-click paths run the same code).
const RELAY_RELEASE_DOWNLOAD_SH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/relay-server/release-download.sh"
));
/// Remote directory (relative to the SSH user's home) holding deploy state.
const DEPLOY_STATE_DIR: &str = ".bitfun/relay-deploy";
/// Line printed by task scripts on success; polled to detect completion.
const TASK_DONE_MARKER: &str = "RELAY_TASK_DONE";
/// How long the seeded `preparing` flag may sit with no live driver process
/// before the task counts as dead. Covers PTY startup and the shell prompt; an
/// alive driver (an open sudo password prompt, say) is never bounded by this.
const PREPARE_GRACE_SECONDS: u64 = 90;

/// Long-running remote operations that run detached and are polled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayDeployTask {
    InstallDocker,
    Deploy,
}

/// Signed release metadata for the immutable multi-platform Relay image.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RelayImageDescriptor {
    schema_version: u8,
    image: String,
    tag: String,
    version: String,
    digest: String,
    platforms: Vec<String>,
}

impl RelayDeployTask {
    fn stem(self) -> &'static str {
        match self {
            Self::InstallDocker => "install-docker",
            Self::Deploy => "deploy",
        }
    }
}

/// Network route used by Docker installation and Relay image pulls.
///
/// `Auto` keeps server-side detection as the default. The explicit variants
/// are a user-facing escape hatch for cloud IPs whose geolocation or outbound
/// routing does not reflect where the server is actually hosted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayMirrorMode {
    #[default]
    Auto,
    Cn,
    Global,
}

impl RelayMirrorMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cn => "cn",
            Self::Global => "global",
        }
    }
}

/// Fine-grained Docker access classification for the current SSH session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockerAccessMode {
    Ok,
    GroupInactive,
    SudoNopass,
    SudoNeedsPassword,
    BrokenDockerHome,
    DaemonDown,
    Missing,
}

/// Result of the remote environment probe.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayPreflight {
    /// `uname -s`, e.g. "Linux".
    pub os: String,
    /// `uname -m`, e.g. "x86_64" / "aarch64".
    pub arch: String,
    /// True for Linux x86_64/aarch64, the architectures deploy.sh supports.
    pub arch_supported: bool,
    pub docker_installed: bool,
    /// `docker compose` (v2) or legacy `docker-compose` available (direct or via sudo).
    pub compose_available: bool,
    /// Legacy coarse daemon string: "ok" | "sudo" | "unreachable".
    pub docker_daemon: String,
    /// Structured access mode for the wizard / interactive driver.
    pub docker_access_mode: DockerAccessMode,
    pub active_has_docker_group: bool,
    pub in_docker_group_file: bool,
    pub docker_home_writable: bool,
    pub tar_available: bool,
    pub curl_available: bool,
    /// Root or passwordless sudo.
    pub sudo_available: bool,
    /// `sudo` exists but `sudo -n` fails (password required).
    pub sudo_needs_password: bool,
    pub mem_total_mb: u64,
    /// Free space under `$HOME` in MB (task scripts and logs).
    pub home_free_mb: u64,
    /// Free space on Docker's data root in MB (images and layers).
    pub docker_free_mb: u64,
    /// Selected listen port already bound by another process.
    pub port_busy: bool,
    /// Port that was probed (`port_busy` / selected-port health).
    pub probed_port: u16,
    /// Selected port is published by the existing `bitfun-relay` container (or
    /// answers `/health` as that relay). Used to distinguish "our relay" from
    /// an unrelated occupant when the user changes the listen port.
    pub port_owned_by_relay: bool,
    /// A `bitfun-relay` container already exists (any state).
    pub container_exists: bool,
    /// A `bitfun-relay` container is currently running.
    pub container_running: bool,
    /// Host port published by the running relay (0 if unknown / not running).
    pub existing_relay_port: u16,
    /// Relay answers `/health` on the selected port and/or the existing
    /// container port (independent of which port the user typed).
    pub relay_healthy: bool,
    pub home_dir: String,
}

/// Result of staging an interactive driver script for a PTY session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayTaskStart {
    /// Absolute remote path of the interactive driver to run in a PTY.
    pub script_path: String,
}

/// Incremental poll result for a detached task.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayTaskPoll {
    /// Byte offset to pass to the next poll.
    pub cursor: u64,
    /// Log output appended since the previous cursor.
    pub output: String,
    pub status: RelayTaskStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayTaskStatus {
    Running,
    Succeeded,
    Failed,
}

/// Probe the target server. Never fails on individual checks: probe errors
/// surface as `false`/empty fields so the UI can render them.
pub async fn run_preflight(
    manager: &SSHConnectionManager,
    connection_id: &str,
    port: u16,
) -> Result<RelayPreflight> {
    let port = normalize_relay_port(port)?;
    let script = format!(
        r#"
PORT="{port}"
echo "probed_port=$PORT"
echo "os=$(uname -s 2>/dev/null)"
echo "arch=$(uname -m 2>/dev/null)"
echo "home=$HOME"
if command -v docker >/dev/null 2>&1; then echo "docker=1"; else echo "docker=0"; fi
COMPOSE=0
if docker compose version >/dev/null 2>&1 || command -v docker-compose >/dev/null 2>&1; then COMPOSE=1; fi
if [ "$COMPOSE" = "0" ] && sudo -n docker compose version >/dev/null 2>&1; then COMPOSE=1; fi
echo "compose=$COMPOSE"
if docker info >/dev/null 2>&1; then echo "daemon=ok"
elif sudo -n docker info >/dev/null 2>&1; then echo "daemon=sudo"
elif command -v docker >/dev/null 2>&1 && (systemctl is-active docker >/dev/null 2>&1 || service docker status >/dev/null 2>&1); then echo "daemon=down"
else echo "daemon=unreachable"; fi
if command -v curl >/dev/null 2>&1; then echo "curl=1"; else echo "curl=0"; fi
if command -v tar >/dev/null 2>&1; then echo "tar=1"; else echo "tar=0"; fi
if [ "$(id -u)" = "0" ]; then echo "sudo=1"; elif sudo -n true >/dev/null 2>&1; then echo "sudo=1"; else echo "sudo=0"; fi
if [ "$(id -u)" != "0" ] && command -v sudo >/dev/null 2>&1 && ! sudo -n true >/dev/null 2>&1; then echo "sudo_needs_password=1"; else echo "sudo_needs_password=0"; fi
if id -nG 2>/dev/null | tr ' ' '\n' | grep -qx docker; then echo "active_docker_group=1"; else echo "active_docker_group=0"; fi
U=$(id -un 2>/dev/null || true)
if getent group docker 2>/dev/null | grep -qE "(^|:|,)${{U}}(,|$)"; then echo "in_docker_group_file=1"; else echo "in_docker_group_file=0"; fi
if [ ! -e "$HOME/.docker" ]; then echo "docker_home_writable=1"
elif [ -w "$HOME/.docker" ] && {{ [ ! -e "$HOME/.docker/buildx" ] || [ -w "$HOME/.docker/buildx" ]; }}; then echo "docker_home_writable=1"
else echo "docker_home_writable=0"; fi
echo "mem_kb=$(awk '/MemTotal/ {{print $2}}' /proc/meminfo 2>/dev/null || echo 0)"
# Free space where the work actually lands: ~/.bitfun holds task state and
# Docker's data root holds the pulled image and writable layers.
echo "home_free_kb=$(df -Pk "$HOME" 2>/dev/null | awk 'NR==2 {{print $4}}' || echo 0)"
DOCKER_ROOT=$(docker info -f '{{{{.DockerRootDir}}}}' 2>/dev/null \
  || sudo -n docker info -f '{{{{.DockerRootDir}}}}' 2>/dev/null || echo /var/lib/docker)
[ -d "$DOCKER_ROOT" ] || DOCKER_ROOT=/var
echo "docker_free_kb=$(df -Pk "$DOCKER_ROOT" 2>/dev/null | awk 'NR==2 {{print $4}}' || echo 0)"
if command -v ss >/dev/null 2>&1; then PORTS=$(ss -ltn 2>/dev/null); else PORTS=$(netstat -ltn 2>/dev/null); fi
if printf '%s\n' "$PORTS" | awk '{{print $4}}' | grep -q ":${{PORT}}$"; then echo "port_busy=1"; else echo "port_busy=0"; fi
# Prefer a docker CLI that can talk to the daemon (plain or passwordless sudo).
D=docker
if ! docker info >/dev/null 2>&1; then
  if sudo -n docker info >/dev/null 2>&1; then D="sudo -n docker"; else D=""; fi
fi
CONTAINER=0
RUNNING=0
EXISTING_PORT=0
if [ -n "$D" ]; then
  if $D ps -a --format '{{{{.Names}}}}' 2>/dev/null | grep -qx bitfun-relay; then CONTAINER=1; fi
  if $D ps --format '{{{{.Names}}}}' 2>/dev/null | grep -qx bitfun-relay; then RUNNING=1; fi
  if [ "$CONTAINER" = "1" ]; then
    # First published host port on the container (compose maps RELAY_PORT:RELAY_PORT).
    EXISTING_PORT=$($D inspect -f '{{{{range $p, $conf := .NetworkSettings.Ports}}}}{{{{range $conf}}}}{{{{if .HostPort}}}}{{{{.HostPort}}}}{{{{end}}}}{{{{end}}}}{{{{end}}}}' bitfun-relay 2>/dev/null | awk 'NF {{print $1; exit}}')
    EXISTING_PORT=$(printf '%s' "$EXISTING_PORT" | tr -cd '0-9')
  fi
fi
# Fallback: last deploy wrote ~/.bitfun/relay-deploy/relay.port
if [ -z "$EXISTING_PORT" ] || [ "$EXISTING_PORT" = "0" ]; then
  if [ -f "$HOME/.bitfun/relay-deploy/relay.port" ]; then
    EXISTING_PORT=$(tr -cd '0-9' < "$HOME/.bitfun/relay-deploy/relay.port")
  fi
fi
[ -n "$EXISTING_PORT" ] || EXISTING_PORT=0
echo "container=$CONTAINER"
echo "container_running=$RUNNING"
echo "existing_port=$EXISTING_PORT"
HEALTHY=0
SELECTED_HEALTHY=0
if curl -fsS -m 3 "http://127.0.0.1:${{PORT}}/health" >/dev/null 2>&1; then
  HEALTHY=1
  SELECTED_HEALTHY=1
fi
if [ "$HEALTHY" = "0" ] && [ "$EXISTING_PORT" != "0" ] && [ "$EXISTING_PORT" != "$PORT" ]; then
  if curl -fsS -m 3 "http://127.0.0.1:${{EXISTING_PORT}}/health" >/dev/null 2>&1; then HEALTHY=1; fi
fi
echo "healthy=$HEALTHY"
PORT_OWNED=0
if [ "$SELECTED_HEALTHY" = "1" ]; then PORT_OWNED=1
elif [ "$EXISTING_PORT" != "0" ] && [ "$EXISTING_PORT" = "$PORT" ] && [ "$RUNNING" = "1" ]; then PORT_OWNED=1
fi
echo "port_owned=$PORT_OWNED"
"#,
        port = port,
    );
    let (stdout, _stderr, code) = exec_script(manager, connection_id, &script).await?;
    if code != 0 {
        return Err(anyhow!("preflight probe failed (exit {code})"));
    }
    Ok(parse_preflight(&stdout, port))
}

fn parse_preflight(out: &str, fallback_port: u16) -> RelayPreflight {
    let get = |key: &str| -> String {
        out.lines()
            .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let probed_port: u16 = get("probed_port").parse().unwrap_or(fallback_port);
    let os = get("os");
    let arch = get("arch");
    let arch_supported = os == "Linux"
        && (arch == "x86_64" || arch == "amd64" || arch == "aarch64" || arch == "arm64");
    let mem_kb: u64 = get("mem_kb").parse().unwrap_or(0);
    let home_free_kb: u64 = get("home_free_kb").parse().unwrap_or(0);
    let docker_free_kb: u64 = get("docker_free_kb").parse().unwrap_or(0);
    let docker_installed = get("docker") == "1";
    let active_has_docker_group = get("active_docker_group") == "1";
    let in_docker_group_file = get("in_docker_group_file") == "1";
    let docker_home_writable = get("docker_home_writable") != "0";
    let sudo_available = get("sudo") == "1";
    let sudo_needs_password = get("sudo_needs_password") == "1";
    let daemon_raw = {
        let d = get("daemon");
        if d.is_empty() {
            "unreachable".into()
        } else {
            d
        }
    };
    let docker_access_mode = classify_docker_access(
        docker_installed,
        &daemon_raw,
        active_has_docker_group,
        in_docker_group_file,
        docker_home_writable,
        sudo_available,
        sudo_needs_password,
    );
    let docker_daemon = match docker_access_mode {
        DockerAccessMode::Ok | DockerAccessMode::BrokenDockerHome => "ok".into(),
        DockerAccessMode::SudoNopass
        | DockerAccessMode::SudoNeedsPassword
        | DockerAccessMode::GroupInactive => "sudo".into(),
        DockerAccessMode::DaemonDown | DockerAccessMode::Missing => "unreachable".into(),
    };
    RelayPreflight {
        os,
        arch,
        arch_supported,
        docker_installed,
        compose_available: get("compose") == "1",
        docker_daemon,
        docker_access_mode,
        active_has_docker_group,
        in_docker_group_file,
        docker_home_writable,
        tar_available: get("tar") == "1",
        curl_available: get("curl") == "1",
        sudo_available,
        sudo_needs_password,
        mem_total_mb: mem_kb / 1024,
        home_free_mb: home_free_kb / 1024,
        docker_free_mb: docker_free_kb / 1024,
        port_busy: get("port_busy") == "1",
        probed_port,
        port_owned_by_relay: get("port_owned") == "1",
        container_exists: get("container") == "1",
        container_running: get("container_running") == "1",
        existing_relay_port: get("existing_port").parse().unwrap_or(0),
        relay_healthy: get("healthy") == "1",
        home_dir: get("home"),
    }
}

fn classify_docker_access(
    docker_installed: bool,
    daemon_raw: &str,
    active_has_docker_group: bool,
    in_docker_group_file: bool,
    docker_home_writable: bool,
    sudo_available: bool,
    sudo_needs_password: bool,
) -> DockerAccessMode {
    if !docker_installed {
        return DockerAccessMode::Missing;
    }
    if daemon_raw == "ok" {
        if !docker_home_writable {
            return DockerAccessMode::BrokenDockerHome;
        }
        return DockerAccessMode::Ok;
    }
    if daemon_raw == "down" {
        return DockerAccessMode::DaemonDown;
    }
    if in_docker_group_file && !active_has_docker_group {
        return DockerAccessMode::GroupInactive;
    }
    if daemon_raw == "sudo" || sudo_available {
        return DockerAccessMode::SudoNopass;
    }
    if sudo_needs_password {
        return DockerAccessMode::SudoNeedsPassword;
    }
    if daemon_raw == "unreachable" {
        return DockerAccessMode::DaemonDown;
    }
    DockerAccessMode::Missing
}

/// Stage an interactive driver script for the task. Does **not** launch it —
/// the wizard runs the script inside a remote PTY so sudo can prompt.
///
/// `port` is used for deploy (written to `relay.port`); ignored for Docker install.
pub async fn start_task(
    manager: &SSHConnectionManager,
    connection_id: &str,
    task: RelayDeployTask,
    port: u16,
    mirror_mode: RelayMirrorMode,
) -> Result<RelayTaskStart> {
    let home = resolve_home(manager, connection_id).await?;
    let dir = format!("{home}/{DEPLOY_STATE_DIR}");
    let stem = task.stem();
    let port = normalize_relay_port(port)?;

    // Stop any leftover task from a previous attempt / closed wizard.
    let _ = cancel_task(manager, connection_id, task).await;

    exec_ok(
        manager,
        connection_id,
        &format!(
            "mkdir -p {} && chmod 700 {}",
            shell_quote_posix(&dir),
            shell_quote_posix(&dir)
        ),
    )
    .await?;

    let body = match task {
        RelayDeployTask::InstallDocker => install_docker_body_script(),
        RelayDeployTask::Deploy => {
            // Authenticate the registry digest here, where the compiled-in
            // release trust root exists. The remote host then only needs
            // Docker's normal content-addressed pull verification.
            let descriptor = verified_latest_relay_image_descriptor().await?;
            deploy_body_script_with_image(port, &descriptor)
        }
    };
    let driver = match task {
        RelayDeployTask::InstallDocker => interactive_driver_script(stem, "install"),
        RelayDeployTask::Deploy => interactive_driver_script(stem, "deploy"),
    };

    let body_path = format!("{dir}/{stem}-body.sh");
    let script_path = format!("{dir}/{stem}.sh");
    let port_path = format!("{dir}/relay.port");
    let mirror_mode_path = format!("{dir}/relay.mirror-mode");
    // Upload as LF-only: bash on the relay host runs a stray CR as a command.
    let body = to_unix_script(&body);
    let driver = to_unix_script(&driver);
    manager
        .sftp_write(connection_id, &body_path, body.as_bytes())
        .await?;
    manager
        .sftp_write(connection_id, &script_path, driver.as_bytes())
        .await?;
    manager
        .sftp_write(
            connection_id,
            &mirror_mode_path,
            format!("{}\n", mirror_mode.as_str()).as_bytes(),
        )
        .await?;
    if matches!(task, RelayDeployTask::Deploy) {
        manager
            .sftp_write(connection_id, &port_path, format!("{port}\n").as_bytes())
            .await?;
    }
    // Seed preparing flag before the PTY runs the driver so early polls do not
    // race into "failed" (no pid / no flag yet). Clear any driver pid from a
    // previous attempt so a recycled pid cannot read as "still preparing".
    let prepare_flag = format!("{dir}/{stem}.preparing");
    let log_path = format!("{dir}/{stem}.log");
    let pid_path = format!("{dir}/{stem}.pid");
    let driver_pid_path = format!("{dir}/{stem}.driver.pid");
    exec_ok(
        manager,
        connection_id,
        &stage_scripts_command(
            &body_path,
            &script_path,
            &pid_path,
            &driver_pid_path,
            &log_path,
            &prepare_flag,
        ),
    )
    .await?;

    Ok(RelayTaskStart { script_path })
}

/// Strip CR from a file already on the relay host, in place.
///
/// Deliberately `tr -d '\r'` and not `sed 's/<CR>$//'`: `tr` expands the `\r`
/// escape itself, so the command contains no raw CR byte. A raw CR would be
/// carried through `to_unix_script` on its way out — the CR remover travelling
/// through the CR remover — and any text-mode hop that rewrites line endings
/// would silently turn this into a no-op. Removing every CR rather than only
/// trailing ones is safe here because the scripts are generated bash that never
/// contains an intentional CR (`embedded_scripts_are_lf_only` enforces that).
///
/// `sed -i` is avoided too: its syntax differs between GNU and BSD userlands.
/// The rewrite replaces the file, so callers must `chmod` afterwards, and the
/// scratch file is cleaned up even when the rewrite fails.
fn strip_cr_command(path: &str) -> String {
    let src = shell_quote_posix(path);
    let tmp = shell_quote_posix(&format!("{path}.lf"));
    format!("{{ tr -d '\\r' < {src} > {tmp} && mv {tmp} {src}; }} || {{ rm -f {tmp}; false; }}")
}

/// Prepare uploaded scripts for the PTY: normalize line endings, make them
/// executable, and seed the liveness files `poll_task` reads.
///
/// The CR strip runs **on the relay host, after upload and before execution**,
/// so a CR-free script on disk does not depend on the uploader having called
/// `to_unix_script` (which it does — this is the second line of defence, and
/// the one that still holds if a future upload path forgets).
fn stage_scripts_command(
    body_path: &str,
    script_path: &str,
    pid_path: &str,
    driver_pid_path: &str,
    log_path: &str,
    prepare_flag: &str,
) -> String {
    format!(
        "{strip_body} && {strip_driver} \
         && chmod 700 {body} {script} \
         && rm -f {pid} {driver_pid} {log} \
         && : > {log} && touch {flag}",
        strip_body = strip_cr_command(body_path),
        strip_driver = strip_cr_command(script_path),
        body = shell_quote_posix(body_path),
        script = shell_quote_posix(script_path),
        pid = shell_quote_posix(pid_path),
        driver_pid = shell_quote_posix(driver_pid_path),
        log = shell_quote_posix(log_path),
        flag = shell_quote_posix(prepare_flag),
    )
}

/// Poll a detached task: incremental log output plus liveness/completion status.
pub async fn poll_task(
    manager: &SSHConnectionManager,
    connection_id: &str,
    task: RelayDeployTask,
    cursor: u64,
) -> Result<RelayTaskPoll> {
    let stem = task.stem();
    let script = format!(
        r#"
D="$HOME/{DEPLOY_STATE_DIR}"
LOG="$D/{stem}.log"
PIDF="$D/{stem}.pid"
DRVF="$D/{stem}.driver.pid"
PREPF="$D/{stem}.preparing"
running=0
if [ -f "$PIDF" ] && kill -0 "$(cat "$PIDF" 2>/dev/null)" 2>/dev/null; then running=1; fi
# Interactive prepare phase (sudo prompts) before nohup starts. The prompt can
# sit for minutes, so an alive driver keeps "preparing" regardless of age.
preparing=0
driver_gone=0
if [ -f "$PREPF" ]; then
  preparing=1
  if [ ! -f "$DRVF" ] || ! kill -0 "$(cat "$DRVF" 2>/dev/null)" 2>/dev/null; then
    # No driver process. Either the PTY has not started it yet (normal for the
    # first few seconds) or it died before installing its cleanup trap — a bad
    # script upload, for instance. Without this bound the flag start_task seeded
    # would never clear and the wizard would report "running" forever.
    prep_age=-1
    prep_now="$(date +%s 2>/dev/null || echo '')"
    prep_mtime="$(stat -c %Y "$PREPF" 2>/dev/null || stat -f %m "$PREPF" 2>/dev/null || echo '')"
    if [ -n "$prep_now" ] && [ -n "$prep_mtime" ]; then
      prep_age=$((prep_now - prep_mtime))
    fi
    if [ "$prep_age" -ge {prepare_grace_seconds} ]; then
      preparing=0
      driver_gone=1
    fi
  fi
fi
log_exists=0
size=0
if [ -f "$LOG" ]; then log_exists=1; size=$(wc -c < "$LOG" | tr -d ' '); fi
marker=0
if [ -f "$LOG" ] && grep -q {TASK_DONE_MARKER} "$LOG"; then marker=1; fi
# A pull or health check may still be progressing even if the wrapper pid
# briefly looks gone; treat a growing log without a marker as running.
echo "running=$running"
echo "preparing=$preparing"
echo "driver_gone=$driver_gone"
echo "log_exists=$log_exists"
echo "size=$size"
echo "marker=$marker"
echo "---"
if [ -f "$LOG" ]; then tail -c +{from} "$LOG"; fi
"#,
        from = cursor.saturating_add(1),
        prepare_grace_seconds = PREPARE_GRACE_SECONDS,
    );
    let (stdout, _stderr, code) = exec_script(manager, connection_id, &script).await?;
    if code != 0 {
        return Err(anyhow!("poll failed (exit {code})"));
    }
    let (head, output) = split_poll_stdout(&stdout);
    let get = |key: &str| -> String {
        head.lines()
            .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let running = get("running") == "1";
    let preparing = get("preparing") == "1";
    let driver_gone = get("driver_gone") == "1";
    let log_exists = get("log_exists") == "1";
    let marker = get("marker") == "1";
    let size: u64 = get("size").parse().unwrap_or(cursor);
    let status = decide_task_status(
        marker,
        running,
        preparing,
        driver_gone,
        log_exists,
        size,
        cursor,
        !output.is_empty(),
    );
    // The driver writes its errors to the PTY, not the log, so a prepare-phase
    // death leaves the wizard's log pane empty. Say where to look.
    let mut output = output.to_string();
    if status == RelayTaskStatus::Failed && driver_gone && size == 0 {
        output.push_str(
            "\n>>> The prepare step exited before starting the task. \
             See the terminal above for the error.\n",
        );
    }
    Ok(RelayTaskPoll {
        cursor: size,
        output,
        status,
    })
}

/// Cancel a running install/deploy task (wizard close / back / retry).
///
/// Kills the nohup body process tree, clears pid/preparing flags, and appends a
/// cancel marker to the log. The image deploy's TERM trap restores any previous
/// container. Safe to call when nothing is running.
pub async fn cancel_task(
    manager: &SSHConnectionManager,
    connection_id: &str,
    task: RelayDeployTask,
) -> Result<()> {
    let stem = task.stem();
    let script = format!(
        r#"
set +e
D="$HOME/{DEPLOY_STATE_DIR}"
STEM="{stem}"
LOG="$D/$STEM.log"
PIDF="$D/$STEM.pid"
PREPF="$D/$STEM.preparing"
DRVF="$D/$STEM.driver.pid"
BODY="$D/$STEM-body.sh"
mkdir -p "$D" 2>/dev/null
was_active=0
[ -f "$PREPF" ] && was_active=1
rm -f "$PREPF" "$DRVF"
kill_tree() {{
  local p="$1"
  local sig="$2"
  [ -n "$p" ] || return 0
  for c in $(pgrep -P "$p" 2>/dev/null); do
    kill_tree "$c" "$sig"
  done
  kill "-$sig" "$p" 2>/dev/null || true
}}
if [ -f "$PIDF" ]; then
  pid="$(cat "$PIDF" 2>/dev/null | tr -d '[:space:]')"
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    was_active=1
    kill_tree "$pid" TERM
    sleep 1
    if kill -0 "$pid" 2>/dev/null; then
      kill_tree "$pid" KILL
    fi
  fi
  rm -f "$PIDF"
fi
# Body may have been reparented to init after nohup; match the body script only
# (do not pkill broad relay-deploy patterns — that can kill this cancel script).
if [ -n "$BODY" ] && pgrep -f "$BODY" >/dev/null 2>&1; then
  was_active=1
  pkill -TERM -f "$BODY" 2>/dev/null || true
  sleep 1
  pkill -KILL -f "$BODY" 2>/dev/null || true
fi
if [ "$was_active" = "1" ]; then
  echo "" >>"$LOG" 2>/dev/null
  echo ">>> Cancelled by client (wizard closed)" >>"$LOG" 2>/dev/null
fi
exit 0
"#,
        DEPLOY_STATE_DIR = DEPLOY_STATE_DIR,
        stem = stem,
    );
    let (_stdout, stderr, code) = exec_script(manager, connection_id, &script).await?;
    if code != 0 {
        return Err(anyhow!("cancel failed (exit {code}): {stderr}"));
    }
    Ok(())
}

/// Decide poll status from remote probe fields.
///
/// Pending (PTY not started yet) and active prepare/pull must not look like
/// failure — the wizard polls immediately after staging scripts.
#[allow(clippy::too_many_arguments)]
fn decide_task_status(
    marker: bool,
    running: bool,
    preparing: bool,
    driver_gone: bool,
    log_exists: bool,
    size: u64,
    cursor: u64,
    got_new_output: bool,
) -> RelayTaskStatus {
    if marker {
        return RelayTaskStatus::Succeeded;
    }
    if running || preparing {
        return RelayTaskStatus::Running;
    }
    // The prepare step is definitively dead and never handed off to the body.
    // Checked before the empty-log case, which would otherwise read as "still
    // starting up" forever.
    if driver_gone {
        return RelayTaskStatus::Failed;
    }
    if !log_exists || size == 0 {
        return RelayTaskStatus::Running;
    }
    // Log still growing since last poll — keep running even if pid check flaked.
    if got_new_output || cursor < size {
        return RelayTaskStatus::Running;
    }
    RelayTaskStatus::Failed
}

/// Split poll script stdout into the metadata head and incremental log body.
///
/// Accepts LF, CRLF, or a standalone `---` line so SSH/OS line endings cannot
/// drop the entire log payload.
fn split_poll_stdout(stdout: &str) -> (&str, &str) {
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

/// Import a locally-provisioned account into the running relay container.
///
/// `account_json` is the serialized `ImportableAccount` produced client-side
/// by `bitfun_relay_service::admin::provision` — it contains only derived
/// artifacts (salts, Argon2id hash, wrapped master key). The file is written
/// with 0600 permissions and removed immediately after the import attempt.
pub async fn import_account(
    manager: &SSHConnectionManager,
    connection_id: &str,
    account_json: &str,
) -> Result<()> {
    let home = resolve_home(manager, connection_id).await?;
    let dir = format!("{home}/{DEPLOY_STATE_DIR}");
    exec_ok(
        manager,
        connection_id,
        &format!(
            "mkdir -p {} && chmod 700 {}",
            shell_quote_posix(&dir),
            shell_quote_posix(&dir)
        ),
    )
    .await?;
    let path = format!("{dir}/import-{}.json", uuid::Uuid::new_v4().as_simple());
    manager
        .sftp_write(connection_id, &path, account_json.as_bytes())
        .await?;

    let quoted = shell_quote_posix(&path);
    let cmd = format!(
        "chmod 600 {q}; \
         dps() {{ docker ps --format '{{{{.Names}}}}' 2>/dev/null; }}; \
         dexec() {{ docker exec -i {name} /app/relay-admin --db {db} import-user; }}; \
         if docker info >/dev/null 2>&1; then :; \
         elif sg docker -c 'docker info' >/dev/null 2>&1; then \
           dps() {{ sg docker -c \"docker ps --format '{{{{.Names}}}}'\" 2>/dev/null; }}; \
           dexec() {{ sg docker -c \"docker exec -i {name} /app/relay-admin --db {db} import-user\"; }}; \
         elif sudo -n docker info >/dev/null 2>&1; then \
           dps() {{ sudo -n docker ps --format '{{{{.Names}}}}' 2>/dev/null; }}; \
           dexec() {{ sudo -n docker exec -i {name} /app/relay-admin --db {db} import-user; }}; \
         else \
           dps() {{ sudo docker ps --format '{{{{.Names}}}}' 2>/dev/null; }}; \
           dexec() {{ sudo docker exec -i {name} /app/relay-admin --db {db} import-user; }}; \
         fi; \
         if dps | grep -qx {name}; then \
           cat {q} | dexec; rc=$?; rm -f {q}; exit $rc; \
         else \
           echo 'relay container {name} is not running' >&2; rm -f {q}; exit 1; \
         fi",
        q = quoted,
        name = RELAY_CONTAINER_NAME,
        db = RELAY_CONTAINER_DB,
    );
    let (stdout, stderr, code) = exec_script(manager, connection_id, &cmd).await?;
    if code != 0 {
        let detail = relay_admin_error(&stdout, &stderr);
        return Err(anyhow!(detail));
    }
    Ok(())
}

/// Health-check the relay from the server itself (loopback).
pub async fn check_relay_health(
    manager: &SSHConnectionManager,
    connection_id: &str,
    port: u16,
) -> Result<bool> {
    let port = normalize_relay_port(port)?;
    let (_o, _e, code) = manager
        .execute_command(
            connection_id,
            &format!("curl -fsS -m 5 http://127.0.0.1:{port}/health >/dev/null 2>&1"),
        )
        .await?;
    Ok(code == 0)
}

/// Extract the meaningful relay-admin failure line, if present.
fn relay_admin_error(stdout: &str, stderr: &str) -> String {
    for line in stderr.lines().chain(stdout.lines()) {
        let l = line.trim();
        if l.contains("already exists") || l.contains("Error") || l.contains("error") {
            return l.trim_start_matches("Error: ").to_string();
        }
    }
    let tail = stderr.trim();
    if tail.is_empty() {
        "account import failed".to_string()
    } else {
        tail.chars().take(300).collect()
    }
}

async fn resolve_home(manager: &SSHConnectionManager, connection_id: &str) -> Result<String> {
    let (out, _e, code) = manager
        .execute_command(connection_id, "printf %s \"$HOME\"")
        .await?;
    let home = out.trim();
    if code != 0 || home.is_empty() {
        return Err(anyhow!("could not resolve remote $HOME"));
    }
    Ok(home.to_string())
}

/// Strip CR from anything sent to the relay host as bash.
///
/// Git for Windows checks out with CRLF by default, so both `include_str!`
/// (mirror.sh / release-download.sh) and this file's own `r#"..."#` remote
/// scripts can carry CRLF into the generated script. Remote bash then executes
/// the CR on the first blank line, prints `line N: $'\r': command not found`
/// and — under `set -euo pipefail` — aborts the deploy right there. `.gitattributes`
/// pins LF for fresh checkouts; this keeps existing CRLF working trees safe too.
fn to_unix_script(script: &str) -> String {
    script.replace("\r\n", "\n")
}

/// `execute_command` for remote bash, with line endings normalized first.
async fn exec_script(
    manager: &SSHConnectionManager,
    connection_id: &str,
    script: &str,
) -> Result<(String, String, i32)> {
    manager
        .execute_command(connection_id, &to_unix_script(script))
        .await
}

async fn exec_ok(manager: &SSHConnectionManager, connection_id: &str, command: &str) -> Result<()> {
    let (stdout, stderr, code) = exec_script(manager, connection_id, command).await?;
    if code != 0 {
        return Err(anyhow!(
            "remote command failed (exit {code}): {}",
            if stderr.trim().is_empty() {
                stdout.trim().chars().take(300).collect::<String>()
            } else {
                stderr.trim().chars().take(300).collect::<String>()
            }
        ));
    }
    Ok(())
}

/// Shared interactive prepare helpers embedded in driver scripts.
fn prepare_helpers_bash() -> String {
    // Mirror helpers first so prepare/install/deploy can call bitfun_mirror_init
    // before apt/git/docker downloads.
    format!(
        r#"
# --- begin BitFun relay mirror.sh (embedded) ---
{mirror}
# --- end BitFun relay mirror.sh ---
"#,
        mirror = RELAY_MIRROR_SH
    ) + r#"
# Privilege helpers:
# - Never use `sudo -v` when NOPASSWD is set — on many cloud images `sudo -v`
#   still demands a password even though `sudo -n true` works.
# - Prefer already-root → passwordless sudo → interactive sudo / sudo su -.
# - When elevating via `su -`, keep the original HOME so ~/.bitfun paths stay valid.

bitfun_have_passwordless_sudo() {
  [ "$(id -u)" != "0" ] && sudo -n true >/dev/null 2>&1
}

# Run a command with the best available privilege (root / sudo -n / sudo).
bitfun_priv() {
  if [ "$(id -u)" = "0" ]; then
    "$@"
  elif sudo -n true >/dev/null 2>&1; then
    sudo -n "$@"
  else
    sudo "$@"
  fi
}

# For Docker install: if not root, re-exec this driver as root once.
# Passwordless path uses `sudo su -` (no prompt). Interactive path prompts once.
# Sets BITFUN_ELEVATED=1 to avoid loops. Preserves HOME for ~/.bitfun/*.
bitfun_elevate_install_driver() {
  local self="$1"
  if [ "$(id -u)" = "0" ] || [ "${BITFUN_ELEVATED:-0}" = "1" ]; then
    return 0
  fi
  local keep_home="${BITFUN_KEEP_HOME:-$HOME}"
  local q_self q_home
  q_self=$(printf '%q' "$self")
  q_home=$(printf '%q' "$keep_home")
  if bitfun_have_passwordless_sudo; then
    echo ">>> Root needed for Docker install; elevating via passwordless sudo su -..."
    exec sudo -n su - -c "export BITFUN_ELEVATED=1 BITFUN_KEEP_HOME=$q_home HOME=$q_home; cd $q_home 2>/dev/null || cd /; bash $q_self"
  fi
  echo ">>> Root needed for Docker install; elevating via sudo su - (password may be required)..."
  exec sudo su - -c "export BITFUN_ELEVATED=1 BITFUN_KEEP_HOME=$q_home HOME=$q_home; cd $q_home 2>/dev/null || cd /; bash $q_self"
}

bitfun_ensure_tools() {
  local pkgs=()
  if [ "$#" -eq 0 ]; then set -- git curl tar; fi
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || pkgs+=("$tool")
  done
  if [ "${#pkgs[@]}" -eq 0 ]; then return 0; fi
  echo ">>> Installing missing tools (${pkgs[*]})..."
  if [ "$(id -u)" = "0" ]; then
    if command -v apt-get >/dev/null 2>&1; then apt-get update -y && apt-get install -y "${pkgs[@]}"
    elif command -v dnf >/dev/null 2>&1; then dnf install -y "${pkgs[@]}"
    elif command -v yum >/dev/null 2>&1; then yum install -y "${pkgs[@]}"
    else echo "ERROR: missing tools (${pkgs[*]}) and no supported package manager" >&2; return 1; fi
  else
    if command -v apt-get >/dev/null 2>&1; then bitfun_priv apt-get update -y && bitfun_priv apt-get install -y "${pkgs[@]}"
    elif command -v dnf >/dev/null 2>&1; then bitfun_priv dnf install -y "${pkgs[@]}"
    elif command -v yum >/dev/null 2>&1; then bitfun_priv yum install -y "${pkgs[@]}"
    else echo "ERROR: missing tools (${pkgs[*]}); install them then retry" >&2; return 1; fi
  fi
}

# Install Docker Engine for the original SSH user. The caller must initialize
# mirror routing first and, when interactive sudo is needed, re-exec the driver
# through bitfun_elevate_install_driver before calling this helper.
bitfun_install_docker_engine() {
  local deploy_user="${SUDO_USER:-}" installed=0
  if [ -z "$deploy_user" ] || [ "$deploy_user" = "root" ]; then
    if [ -n "${BITFUN_KEEP_HOME:-}" ] && [ -d "${BITFUN_KEEP_HOME}" ]; then
      deploy_user="$(stat -c '%U' "$BITFUN_KEEP_HOME" 2>/dev/null || true)"
    fi
  fi
  if [ -z "$deploy_user" ] || [ "$deploy_user" = "root" ]; then
    deploy_user="$(id -un)"
  fi

  bitfun_ensure_tools curl
  echo ">>> Installing Docker as uid=$(id -u) for user=$deploy_user (mirror_mode=${BITFUN_MIRROR_MODE:-global}) ..."
  if [ "${BITFUN_MIRROR_MODE:-}" = "cn" ]; then
    if bitfun_mirror_install_docker_aliyun; then
      installed=1
    else
      echo ">>> Aliyun docker-ce install failed; falling back to get.docker.com mirror..."
    fi
  fi
  if [ "$installed" != "1" ]; then
    bitfun_mirror_fetch_docker_install_script /tmp/bitfun-get-docker.sh \
      || curl -fsSL --retry 3 https://get.docker.com -o /tmp/bitfun-get-docker.sh
    if [ "$(id -u)" = "0" ]; then
      sh /tmp/bitfun-get-docker.sh
    else
      bitfun_priv sh /tmp/bitfun-get-docker.sh
    fi
    rm -f /tmp/bitfun-get-docker.sh
  fi

  if [ "$(id -u)" = "0" ]; then
    systemctl enable --now docker 2>/dev/null || service docker start
    usermod -aG docker "$deploy_user" || true
  else
    bitfun_priv systemctl enable --now docker 2>/dev/null || bitfun_priv service docker start
    bitfun_priv usermod -aG docker "$deploy_user"
  fi
  if [ "${BITFUN_MIRROR_MODE:-}" = "cn" ]; then
    bitfun_mirror_apply_docker_daemon || true
  fi
  bitfun_fix_docker_home
  if [ "$(id -u)" = "0" ] && [ -n "$deploy_user" ] && [ "$deploy_user" != "root" ] \
     && [ -d "$HOME/.bitfun" ]; then
    echo ">>> Restoring ownership of $HOME/.bitfun to $deploy_user..."
    chown -R "$deploy_user" "$HOME/.bitfun" 2>/dev/null || true
  fi

  if docker info >/dev/null 2>&1 \
     || sg docker -c 'docker info' >/dev/null 2>&1 \
     || sudo -n docker info >/dev/null 2>&1 \
     || sudo docker info >/dev/null 2>&1; then
    echo ">>> Docker installed and reachable: $(docker --version 2>/dev/null || sudo -n docker --version 2>/dev/null || true)"
    return 0
  fi
  echo "ERROR: Docker installed but daemon is not reachable" >&2
  return 1
}

# Owner of $HOME — the SSH user even when this script runs elevated with their
# HOME preserved (BITFUN_KEEP_HOME).
bitfun_home_owner() {
  stat -c '%U:%G' "$HOME" 2>/dev/null || stat -f '%Su:%Sg' "$HOME" 2>/dev/null || true
}

# Make DOCKER_CONFIG usable by whoever is running now.
#
# The Docker-install task runs as root but keeps the SSH user's HOME, so it used
# to leave ~/.bitfun/docker-config (and its config.json) owned by root:root 0700.
# Every later unprivileged deploy then hit
#   WARNING: Error loading config file: .../config.json: permission denied
# and the docker CLI misparsed the command that followed. Repair ownership when
# we have the rights, and otherwise move to a config dir we can actually read.
bitfun_fix_docker_config() {
  export DOCKER_CONFIG="${DOCKER_CONFIG:-$HOME/.bitfun/docker-config}"
  mkdir -p "$DOCKER_CONFIG" 2>/dev/null || true
  if [ "$(id -u)" = "0" ]; then
    # Hand the tree back to the SSH user; root reads it either way.
    local owner
    owner="$(bitfun_home_owner)"
    if [ -n "$owner" ] && [ "$owner" != "root:root" ]; then
      chown -R "$owner" "$DOCKER_CONFIG" 2>/dev/null || true
    fi
  elif [ ! -r "$DOCKER_CONFIG" ] || [ ! -w "$DOCKER_CONFIG" ] \
    || { [ -e "$DOCKER_CONFIG/config.json" ] && [ ! -r "$DOCKER_CONFIG/config.json" ]; }; then
    echo ">>> $DOCKER_CONFIG is not usable by $(id -un) (left root-owned by an earlier install)."
    bitfun_priv chown -R "$(id -un):$(id -gn)" "$DOCKER_CONFIG" 2>/dev/null || true
    if [ ! -r "$DOCKER_CONFIG" ] || [ ! -w "$DOCKER_CONFIG" ] \
      || { [ -e "$DOCKER_CONFIG/config.json" ] && [ ! -r "$DOCKER_CONFIG/config.json" ]; }; then
      DOCKER_CONFIG="$HOME/.bitfun/docker-config-$(id -u)"
      export DOCKER_CONFIG
      mkdir -p "$DOCKER_CONFIG"
      echo ">>> Could not repair it; using DOCKER_CONFIG=$DOCKER_CONFIG instead."
    fi
  fi
  chmod 700 "$DOCKER_CONFIG" 2>/dev/null || true
}

bitfun_fix_docker_home() {
  bitfun_fix_docker_config
  if [ -e "$HOME/.docker" ] && [ ! -w "$HOME/.docker" ]; then
    echo ">>> $HOME/.docker is not writable (often root-owned buildx lock)."
    echo ">>> Fixing ownership..."
    if [ "$(id -u)" = "0" ]; then
      # Prefer original deploy user if HOME still points at their tree.
      local owner
      owner="$(stat -c '%U:%G' "$HOME" 2>/dev/null || echo root:root)"
      chown -R "$owner" "$HOME/.docker" 2>/dev/null \
        || chown -R "$(id -un):$(id -gn)" "$HOME/.docker"
    else
      bitfun_priv chown -R "$(id -un):$(id -gn)" "$HOME/.docker"
    fi
  fi
  if [ -e "$HOME/.docker" ] && [ ! -w "$HOME/.docker" ]; then
    echo ">>> Still not writable; using isolated DOCKER_CONFIG=$DOCKER_CONFIG"
  fi
}

bitfun_start_docker_daemon() {
  if docker info >/dev/null 2>&1 || sudo -n docker info >/dev/null 2>&1; then return 0; fi
  echo ">>> Starting Docker daemon..."
  if [ "$(id -u)" = "0" ]; then
    systemctl enable --now docker 2>/dev/null || service docker start 2>/dev/null || true
  elif sudo -n true >/dev/null 2>&1; then
    sudo -n systemctl enable --now docker 2>/dev/null || sudo -n service docker start 2>/dev/null || true
  else
    echo ">>> sudo password may be required to start Docker..."
    sudo systemctl enable --now docker 2>/dev/null || sudo service docker start 2>/dev/null || true
  fi
  sleep 1
}

# Sets BITFUN_DOCKER_MODE to: direct | sg | sudo
bitfun_resolve_docker_mode() {
  bitfun_fix_docker_home
  bitfun_start_docker_daemon
  if docker info >/dev/null 2>&1; then
    BITFUN_DOCKER_MODE=direct
    return 0
  fi
  if id -nG 2>/dev/null | tr ' ' '\n' | grep -qx docker; then
    if sg docker -c 'docker info' >/dev/null 2>&1; then
      BITFUN_DOCKER_MODE=sg
      return 0
    fi
  elif getent group docker 2>/dev/null | grep -qE "(^|:|,)$(id -un)(,|$)"; then
    echo ">>> User is in docker group but session has not activated it; using sg docker."
    if sg docker -c 'docker info' >/dev/null 2>&1; then
      BITFUN_DOCKER_MODE=sg
      return 0
    fi
  fi
  if sudo -n docker info >/dev/null 2>&1; then
    echo ">>> Using passwordless sudo for Docker."
    BITFUN_DOCKER_MODE=sudo
    return 0
  fi
  echo ">>> Docker needs interactive sudo (enter password if prompted)..."
  if sudo docker info >/dev/null 2>&1; then
    BITFUN_DOCKER_MODE=sudo
    return 0
  fi
  echo "ERROR: cannot reach Docker daemon" >&2
  return 1
}

# POSIX single-quote each argument so `sg -c` cannot re-split or glob them.
# `sg docker -c "docker $*"` loses argument boundaries: a context path with a
# space, or a `-f '{{.State.Running}}'` format string, arrives mangled.
bitfun_shell_join() {
  local out="" arg
  for arg in "$@"; do
    out="$out'$(printf '%s' "$arg" | sed "s/'/'\\\\''/g")' "
  done
  printf '%s' "$out"
}

bitfun_docker() {
  case "${BITFUN_DOCKER_MODE:-direct}" in
    sg) sg docker -c "$(bitfun_shell_join docker "$@")" ;;
    sudo)
      if sudo -n true >/dev/null 2>&1; then sudo -n docker "$@"; else sudo docker "$@"; fi
      ;;
    *) docker "$@" ;;
  esac
}

"#
}

/// Interactive driver: prepare (TTY/sudo OK) → nohup body → tail -f log.
fn interactive_driver_script(stem: &str, kind: &str) -> String {
    let helpers = prepare_helpers_bash();
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
D="$HOME/{DEPLOY_STATE_DIR}"
STEM="{stem}"
LOG="$D/$STEM.log"
PIDF="$D/$STEM.pid"
BODY="$D/$STEM-body.sh"
mkdir -p "$D"
chmod 700 "$D"
# Claim the prepare phase before anything that can fail: poll_task treats a
# missing/dead driver pid as "prepare died" once its grace window elapses.
DRIVER_PIDF="$D/$STEM.driver.pid"
echo $$ >"$DRIVER_PIDF"
{helpers}

echo ">>> BitFun relay {kind}: interactive prepare"
echo ">>> Closing the wizard stops this task."
# Preserve the SSH user's home across root elevation (su - would otherwise use /root).
export BITFUN_KEEP_HOME="${{BITFUN_KEEP_HOME:-$HOME}}"
# install: elevate to root first (passwordless sudo su - when available).
if [ "{kind}" = "install" ]; then
  bitfun_elevate_install_driver "$D/$STEM.sh"
fi
# After elevation HOME may need restoring from BITFUN_KEEP_HOME.
if [ -n "${{BITFUN_KEEP_HOME:-}}" ]; then
  export HOME="$BITFUN_KEEP_HOME"
  D="$HOME/{DEPLOY_STATE_DIR}"
  LOG="$D/$STEM.log"
  PIDF="$D/$STEM.pid"
  BODY="$D/$STEM-body.sh"
  DRIVER_PIDF="$D/$STEM.driver.pid"
fi
PREPARE_FLAG="$D/$STEM.preparing"
MIRROR_MODE_FILE="$D/relay.mirror-mode"
# Re-claim the prepare phase: an elevated re-exec is a different process, and D
# may have moved with HOME.
echo $$ >"$DRIVER_PIDF"
# Keep/refresh the preparing flag seeded by start_task — do not clear it first
# or early polls can race into "failed".
rm -f "$PIDF"
: >"$LOG"
touch "$PREPARE_FLAG"
echo ">>> prepare starting (uid=$(id -u) home=$HOME)" | tee -a "$LOG"
cleanup_prepare() {{ rm -f "$PREPARE_FLAG" "$DRIVER_PIDF"; }}
trap cleanup_prepare EXIT
# Region/mirrors before apt tool install and Docker/GitHub downloads.
export BITFUN_REPO_GIT_URL="{REPO_GIT_URL}"
export BITFUN_REPO_TARBALL_URL="{REPO_TARBALL_URL}"
if [ -f "$MIRROR_MODE_FILE" ]; then
  requested_mirror_mode="$(tr -d '[:space:]' < "$MIRROR_MODE_FILE")"
  case "$requested_mirror_mode" in
    auto|cn|global) export BITFUN_MIRROR="$requested_mirror_mode" ;;
    *) echo "ERROR: invalid relay mirror mode: $requested_mirror_mode" >&2; exit 1 ;;
  esac
fi
bitfun_mirror_init
export DOCKER_CONFIG="${{DOCKER_CONFIG:-$HOME/.bitfun/docker-config}}"
# May exist root-owned from an older Docker-install run; repair or relocate it
# instead of letting an unwritable dir abort the run under `set -e`.
bitfun_fix_docker_config

# Deploy is genuinely one-click: if Docker is absent, install it through
# bitfun_priv/bitfun_mirror_priv (interactive sudo is allowed), then continue as
# the original SSH user so cancellation can still signal the detached task.
if [ "{kind}" = "deploy" ] && ! command -v docker >/dev/null 2>&1; then
  echo ">>> Docker is not installed; installing it before pulling Relay..." | tee -a "$LOG"
  bitfun_install_docker_engine 2>&1 | tee -a "$LOG"
fi

# Standalone install resolves nothing; deploy always needs live daemon access.
if [ "{kind}" = "install" ]; then
  BITFUN_DOCKER_MODE=direct
else
  bitfun_resolve_docker_mode
fi
export BITFUN_DOCKER_MODE

# Docker install runs in the foreground. The image pull/start task goes through
# nohup so the wizard can poll and follow its log.
if [ "{kind}" = "install" ]; then
  echo ">>> Installing Docker..." | tee -a "$LOG"
  export BITFUN_KEEP_HOME="${{BITFUN_KEEP_HOME:-$HOME}}"
  set +e
  if command -v stdbuf >/dev/null 2>&1; then
    stdbuf -oL -eL env BITFUN_KEEP_HOME="$BITFUN_KEEP_HOME" \
      BITFUN_MIRROR="${{BITFUN_MIRROR:-auto}}" \
      BITFUN_MIRROR_MODE="${{BITFUN_MIRROR_MODE:-}}" \
      BITFUN_MIRROR_REASON="${{BITFUN_MIRROR_REASON:-}}" \
      bash "$BODY" 2>&1 | tee -a "$LOG"
  else
    env BITFUN_KEEP_HOME="$BITFUN_KEEP_HOME" \
      BITFUN_MIRROR="${{BITFUN_MIRROR:-auto}}" \
      BITFUN_MIRROR_MODE="${{BITFUN_MIRROR_MODE:-}}" \
      BITFUN_MIRROR_REASON="${{BITFUN_MIRROR_REASON:-}}" \
      bash "$BODY" 2>&1 | tee -a "$LOG"
  fi
  code=${{PIPESTATUS[0]}}
  set -e
  rm -f "$PREPARE_FLAG" "$PIDF" "$DRIVER_PIDF"
  trap - EXIT
  if [ "$code" -ne 0 ]; then
    echo "ERROR: Docker install failed (exit $code)" | tee -a "$LOG"
    exit "$code"
  fi
  echo ">>> Docker install finished." | tee -a "$LOG"
  exit 0
fi

if command -v stdbuf >/dev/null 2>&1; then RUNNER=(stdbuf -oL -eL bash); else RUNNER=(bash); fi
echo ">>> Starting background task (log: $LOG)" | tee -a "$LOG"
nohup env BITFUN_DOCKER_MODE="$BITFUN_DOCKER_MODE" DOCKER_CONFIG="$DOCKER_CONFIG" \
  BITFUN_MIRROR="${{BITFUN_MIRROR:-auto}}" \
  BITFUN_MIRROR_MODE="${{BITFUN_MIRROR_MODE:-}}" \
  BITFUN_MIRROR_REASON="${{BITFUN_MIRROR_REASON:-}}" \
  "${{RUNNER[@]}}" "$BODY" >"$LOG" 2>&1 < /dev/null &
echo $! >"$PIDF"
# The body pid now drives liveness; `exec tail` below would leave a stale driver
# pid behind, so retire it here rather than in the (never-reached) EXIT trap.
rm -f "$PREPARE_FLAG" "$DRIVER_PIDF"
trap - EXIT
echo ">>> Following log..."
exec tail -n +1 -f "$LOG"
"#,
        DEPLOY_STATE_DIR = DEPLOY_STATE_DIR,
        stem = stem,
        kind = kind,
        helpers = helpers,
        REPO_GIT_URL = REPO_GIT_URL,
        REPO_TARBALL_URL = REPO_TARBALL_URL,
    )
}

/// Docker install body (usually run as root after driver elevation).
fn install_docker_body_script() -> String {
    let helpers = prepare_helpers_bash();
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
{helpers}
# Prefer the original SSH user's home (set by elevated driver).
if [ -n "${{BITFUN_KEEP_HOME:-}}" ]; then export HOME="$BITFUN_KEEP_HOME"; fi
export DOCKER_CONFIG="${{DOCKER_CONFIG:-$HOME/.bitfun/docker-config}}"
mkdir -p "$DOCKER_CONFIG" 2>/dev/null || true
export BITFUN_REPO_GIT_URL="{REPO_GIT_URL}"
export BITFUN_REPO_TARBALL_URL="{REPO_TARBALL_URL}"
bitfun_mirror_init
bitfun_install_docker_engine
echo {TASK_DONE_MARKER}
"#,
        helpers = helpers,
        REPO_GIT_URL = REPO_GIT_URL,
        REPO_TARBALL_URL = REPO_TARBALL_URL,
        TASK_DONE_MARKER = TASK_DONE_MARKER,
    )
}

/// Preamble + shared script that pulls the published multi-platform Relay image.
/// The container name, volumes, ports and relay-admin path stay identical to the
/// former source-compose deployment.
///
/// The body lives in `src/apps/relay-server/release-download.sh` so the manual
/// `deploy.sh` path runs exactly the same code, the same way `mirror.sh` is
/// shared. Desktop additionally supplies a signed, immutable registry digest.
fn release_binary_deploy_bash() -> String {
    format!(
        r#"
export BITFUN_GITHUB_RELEASE_BASE="{RELEASE_BASE}"
export BITFUN_OPENBITFUN_RELEASE_BASE="{OPENBITFUN_RELEASE_BASE}"
# --- begin BitFun relay release-download.sh ---
{release_download}
# --- end BitFun relay release-download.sh ---
"#,
        RELEASE_BASE = RELEASE_BASE,
        OPENBITFUN_RELEASE_BASE = OPENBITFUN_RELEASE_BASE,
        release_download = RELAY_RELEASE_DOWNLOAD_SH,
    )
}

/// Download and authenticate the image descriptor before any remote mutation.
/// The official release is preferred; openbitfun is a byte mirror and remains
/// safe because the same compiled-in minisign key must verify its descriptor.
async fn verified_latest_relay_image_descriptor() -> Result<RelayImageDescriptor> {
    let pubkey = release_pubkey().ok_or_else(|| {
        anyhow!("this build has no Relay release trust root; refusing image deployment")
    })?;
    let client = crate::reqwest_client_builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(30))
        .build()?;
    let bases = [
        format!("{RELEASE_BASE}/latest/download"),
        OPENBITFUN_RELEASE_BASE.to_string(),
    ];

    let mut last_error = String::from("descriptor was unavailable from every source");
    for base in bases {
        let descriptor_url = format!("{base}/{RELAY_IMAGE_DESCRIPTOR_ASSET}");
        let Some(descriptor_text) = fetch_text(&client, &descriptor_url).await else {
            last_error = format!("{descriptor_url} was unavailable");
            continue;
        };
        let Some(signature) = fetch_text(&client, &format!("{descriptor_url}.sig")).await else {
            last_error = format!("{descriptor_url}.sig was unavailable");
            continue;
        };
        if let Err(error) = verify_minisign(descriptor_text.as_bytes(), &signature, pubkey) {
            last_error = format!("{descriptor_url} signature did not verify: {error}");
            log::warn!("Relay image descriptor rejected: {last_error}");
            continue;
        }
        let descriptor: RelayImageDescriptor = match serde_json::from_str(&descriptor_text) {
            Ok(descriptor) => descriptor,
            Err(error) => {
                last_error = format!("{descriptor_url} is invalid JSON: {error}");
                continue;
            }
        };
        if let Err(error) = validate_relay_image_descriptor(&descriptor) {
            last_error = format!("{descriptor_url} is invalid: {error}");
            log::warn!("Relay image descriptor rejected: {last_error}");
            continue;
        }
        return Ok(descriptor);
    }

    Err(anyhow!(
        "could not verify the latest signed Relay image descriptor: {last_error}"
    ))
}

fn validate_relay_image_descriptor(descriptor: &RelayImageDescriptor) -> Result<()> {
    if descriptor.schema_version != 1 {
        return Err(anyhow!(
            "unsupported schema version {}",
            descriptor.schema_version
        ));
    }
    if descriptor.image != RELAY_IMAGE_REPOSITORY {
        return Err(anyhow!("unexpected image repository"));
    }
    let version = semver::Version::parse(&descriptor.version)
        .map_err(|_| anyhow!("image version is not valid SemVer"))?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err(anyhow!("latest Relay image must be a stable release"));
    }
    if descriptor.tag != release_tag_for_version(&descriptor.version) {
        return Err(anyhow!("descriptor tag does not match its version"));
    }
    let digest = descriptor.digest.as_bytes();
    if digest.len() != 71
        || !descriptor.digest.starts_with("sha256:")
        || !digest[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(anyhow!("image digest is not canonical lowercase SHA256"));
    }
    for platform in ["linux/amd64", "linux/arm64"] {
        if !descriptor
            .platforms
            .iter()
            .any(|candidate| candidate == platform)
        {
            return Err(anyhow!("image does not declare {platform}"));
        }
    }
    Ok(())
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Option<String> {
    client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()
}

/// Non-interactive body for deploy (runs under nohup after prepare). It has one
/// network operation: pull the authenticated image through the selected route.
fn deploy_body_script_with_image(port: u16, descriptor: &RelayImageDescriptor) -> String {
    let helpers = prepare_helpers_bash();
    let release_binary_deploy = release_binary_deploy_bash();
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
{helpers}
{release_binary_deploy}
export BITFUN_RELAY_IMAGE={image}
export BITFUN_RELAY_IMAGE_DIGEST={digest}
export BITFUN_RELEASE_TAG={tag}
export BITFUN_RELEASE_VERSION={version}
export BITFUN_REQUIRE_IMAGE_DIGEST=1
export DOCKER_CONFIG="${{DOCKER_CONFIG:-$HOME/.bitfun/docker-config}}"
BITFUN_DOCKER_MODE="${{BITFUN_DOCKER_MODE:-direct}}"
# Repair DOCKER_CONFIG unconditionally: when the driver already resolved a
# non-direct mode, bitfun_resolve_docker_mode (which normally does this) is
# skipped below, and the docker CLI then fails on an unreadable config.json.
bitfun_fix_docker_config
if [ "$BITFUN_DOCKER_MODE" = "direct" ] && ! docker info >/dev/null 2>&1; then
  bitfun_resolve_docker_mode
fi
# Prefer the port staged by the desktop wizard; fall back to embedded default.
PORT_FILE="$HOME/{DEPLOY_STATE_DIR}/relay.port"
if [ -f "$PORT_FILE" ]; then
  RELAY_PORT="$(tr -d '[:space:]' < "$PORT_FILE")"
fi
RELAY_PORT="${{RELAY_PORT:-{port}}}"
export RELAY_PORT
echo ">>> Using RELAY_PORT=$RELAY_PORT"
bitfun_mirror_init
bitfun_try_release_deploy
echo {TASK_DONE_MARKER}
"#,
        helpers = helpers,
        release_binary_deploy = release_binary_deploy,
        image = shell_quote_posix(&descriptor.image),
        digest = shell_quote_posix(&descriptor.digest),
        tag = shell_quote_posix(&descriptor.tag),
        version = shell_quote_posix(&descriptor.version),
        DEPLOY_STATE_DIR = DEPLOY_STATE_DIR,
        port = port,
        TASK_DONE_MARKER = TASK_DONE_MARKER,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        classify_docker_access, decide_task_status, deploy_body_script_with_image,
        install_docker_body_script, interactive_driver_script, parse_preflight,
        prepare_helpers_bash, release_binary_deploy_bash, release_tag_for_version,
        split_poll_stdout, stage_scripts_command, to_unix_script, validate_relay_image_descriptor,
        verify_minisign, DockerAccessMode, RelayImageDescriptor, RelayTaskStatus,
        RELAY_IMAGE_REPOSITORY, RELAY_MIRROR_SH, RELAY_RELEASE_DOWNLOAD_SH, RELEASE_PUBKEY,
    };

    fn test_image_descriptor() -> RelayImageDescriptor {
        RelayImageDescriptor {
            schema_version: 1,
            image: RELAY_IMAGE_REPOSITORY.to_string(),
            tag: "v1.2.3".to_string(),
            version: "1.2.3".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
            platforms: vec!["linux/amd64".into(), "linux/arm64".into()],
        }
    }

    #[test]
    fn embedded_mirror_script_exposes_init_and_cn_defaults() {
        assert!(
            RELAY_MIRROR_SH.contains("bitfun_mirror_init"),
            "mirror.sh must define bitfun_mirror_init"
        );
        assert!(
            RELAY_MIRROR_SH.contains("rsproxy.cn"),
            "mirror.sh must default cargo to rsproxy"
        );
        assert!(
            RELAY_MIRROR_SH.contains("ghfast.top"),
            "mirror.sh must default GitHub proxy"
        );
        assert!(
            RELAY_MIRROR_SH.contains("bitfun_mirror_country_via_bash_tcp"),
            "mirror.sh must keep a country fallback for minimal hosts without curl"
        );
        assert!(
            RELAY_MIRROR_SH.contains("bitfun_mirror_restore_host"),
            "mirror.sh must support switching a managed host back to global mode"
        );
        assert!(
            RELAY_MIRROR_SH.contains("BITFUN_MIRROR_REASON"),
            "mirror selection must log why auto detection chose its route"
        );
        assert!(
            !RELAY_MIRROR_SH.contains("data[\"bitfun-cn-mirror\"]"),
            "daemon.json must contain only dockerd-supported directives"
        );
        assert!(
            !RELAY_MIRROR_SH.contains("bitfun_mirror_apply_cargo_config"),
            "relay deploy must not rewrite the SSH user's global Cargo config"
        );
        let helpers = prepare_helpers_bash();
        assert!(
            helpers.contains("bitfun_mirror_init"),
            "prepare helpers must embed mirror.sh"
        );
        assert!(
            helpers.contains("bitfun_install_docker_engine"),
            "prepare helpers must support install-and-continue deployment"
        );
        let driver = interactive_driver_script("deploy", "deploy");
        assert!(
            driver.contains("relay.mirror-mode")
                && driver.contains("auto|cn|global) export BITFUN_MIRROR"),
            "the wizard's explicit mirror choice must reach remote preparation"
        );
    }

    /// A CRLF checkout (Git for Windows' `core.autocrlf=true` default) used to
    /// ship CRLF straight into the uploaded scripts, and the relay host failed
    /// with `line 37: $'\r': command not found` — line 37 being the first blank
    /// line of the embedded mirror.sh.
    #[test]
    fn embedded_scripts_are_lf_only() {
        for (name, script) in [
            ("mirror.sh", RELAY_MIRROR_SH),
            ("release-download.sh", RELAY_RELEASE_DOWNLOAD_SH),
        ] {
            assert!(
                !script.contains('\r'),
                "{name} must be checked out LF-only (see .gitattributes)"
            );
        }
    }

    /// The remote-side half of the CR guarantee: whatever bytes reached the host,
    /// the staged scripts are LF before the PTY runs them. Runs the real command
    /// against real CRLF files rather than asserting on its text.
    #[cfg(unix)]
    #[test]
    fn staging_strips_cr_on_the_host_before_execution() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let p = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
        let (body_path, script_path) = (p("deploy-body.sh"), p("deploy.sh"));
        let (pid_path, driver_pid_path) = (p("deploy.pid"), p("deploy.driver.pid"));
        let (log_path, prepare_flag) = (p("deploy.log"), p("deploy.preparing"));

        // Simulate an uploader that skipped normalization.
        let crlf = "#!/usr/bin/env bash\r\nset -euo pipefail\r\n\r\necho hi\r\n";
        std::fs::write(&body_path, crlf).expect("write body");
        std::fs::write(&script_path, crlf).expect("write driver");
        // Stale files from a previous attempt that staging must clear.
        std::fs::write(&pid_path, "1234").expect("write pid");
        std::fs::write(&driver_pid_path, "5678").expect("write driver pid");

        let command = stage_scripts_command(
            &body_path,
            &script_path,
            &pid_path,
            &driver_pid_path,
            &log_path,
            &prepare_flag,
        );
        let output = std::process::Command::new("bash")
            .args(["-c", &command])
            .output()
            .expect("run staging command");
        assert!(
            output.status.success(),
            "staging command failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );

        for path in [&body_path, &script_path] {
            let staged = std::fs::read_to_string(path).expect("read staged script");
            assert_eq!(
                staged, "#!/usr/bin/env bash\nset -euo pipefail\n\necho hi\n",
                "{path} must be LF-only on the host"
            );
            let mode = std::fs::metadata(path).expect("stat").permissions().mode();
            assert_eq!(
                mode & 0o777,
                0o700,
                "{path} must stay owner-only executable"
            );
        }
        // The rewrite must not leave its scratch file behind.
        assert!(!std::path::Path::new(&format!("{script_path}.lf")).exists());
        // No raw CR in the command itself: it would be eaten by to_unix_script
        // or by any text-mode hop, silently disabling the strip.
        assert!(
            !command.contains('\r'),
            "the CR strip must not depend on a raw CR surviving transport"
        );
        assert!(
            !std::path::Path::new(&pid_path).exists(),
            "stale pid cleared"
        );
        assert!(
            !std::path::Path::new(&driver_pid_path).exists(),
            "stale driver pid cleared"
        );
        assert!(std::path::Path::new(&prepare_flag).exists(), "flag seeded");
        assert_eq!(
            std::fs::read_to_string(&log_path).expect("read log"),
            "",
            "log must be truncated for the incremental cursor"
        );
    }

    #[test]
    fn uploaded_scripts_are_normalized_to_lf() {
        // Simulate a CRLF working tree: every generated script must still leave
        // this crate as LF-only bash.
        let crlf = "#!/usr/bin/env bash\r\nset -euo pipefail\r\n\r\necho hi\r\n";
        assert_eq!(
            to_unix_script(crlf),
            "#!/usr/bin/env bash\nset -euo pipefail\n\necho hi\n"
        );

        for (name, script) in [
            (
                "deploy driver",
                interactive_driver_script("deploy", "deploy"),
            ),
            (
                "install driver",
                interactive_driver_script("install-docker", "install"),
            ),
            (
                "deploy body",
                deploy_body_script_with_image(9700, &test_image_descriptor()),
            ),
            ("install body", install_docker_body_script()),
        ] {
            assert!(
                !to_unix_script(&script).contains('\r'),
                "{name} must reach the relay host without CR"
            );
        }
    }

    /// `deploy.sh` on the relay host is this driver, not the repo script — it
    /// was previously the only generated script with no syntax coverage.
    #[cfg(unix)]
    #[test]
    fn generated_driver_scripts_are_valid_bash() {
        for (stem, kind) in [("deploy", "deploy"), ("install-docker", "install")] {
            let script = to_unix_script(&interactive_driver_script(stem, kind));
            let output = std::process::Command::new("bash")
                .args(["-n", "-c", &script])
                .output()
                .expect("parse generated driver script");
            assert!(
                output.status.success(),
                "generated {kind} driver is invalid:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn generated_install_body_is_valid_bash() {
        let script = to_unix_script(&install_docker_body_script());
        let output = std::process::Command::new("bash")
            .args(["-n", "-c", &script])
            .output()
            .expect("parse generated install script");
        assert!(
            output.status.success(),
            "generated install body is invalid:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The Docker-install task runs as root with the SSH user's HOME, so it
    /// creates ~/.bitfun/docker-config root-owned. Left that way, the next
    /// unprivileged deploy hits `config.json: permission denied` and the docker
    /// CLI can mis-dispatch the pull that follows.
    #[test]
    fn docker_config_ownership_is_repaired_across_privilege_levels() {
        let helpers = prepare_helpers_bash();
        assert!(
            helpers.contains("bitfun_fix_docker_config"),
            "helpers must expose a DOCKER_CONFIG repair"
        );

        let install = install_docker_body_script();
        assert!(
            install.contains("bitfun_install_docker_engine"),
            "standalone install must use the shared Docker installer"
        );
        assert!(
            helpers.contains(r#"chown -R "$deploy_user" "$HOME/.bitfun""#),
            "root install must hand ~/.bitfun back to the SSH user"
        );

        // The driver exports BITFUN_DOCKER_MODE, so the body skips
        // bitfun_resolve_docker_mode (which is the other caller of the repair)
        // for every non-direct mode. It has to repair the config itself.
        let body = deploy_body_script_with_image(9700, &test_image_descriptor());
        let repair = body
            .find("bitfun_fix_docker_config")
            .expect("deploy body must repair DOCKER_CONFIG");
        let mode_check = body
            .find(r#"if [ "$BITFUN_DOCKER_MODE" = "direct" ]"#)
            .expect("deploy body must keep the direct-mode probe");
        assert!(
            repair < mode_check,
            "DOCKER_CONFIG must be repaired before any docker call, not only in direct mode"
        );

        let driver = interactive_driver_script("deploy", "deploy");
        assert!(
            driver.contains("Docker is not installed; installing it before pulling Relay")
                && driver.contains("bitfun_install_docker_engine"),
            "the deploy button must install a missing Docker engine and continue"
        );
        assert!(
            !driver.contains("docker compose missing"),
            "the pull-only path must not require Docker Compose"
        );
    }

    /// `sg docker -c "docker $*"` re-parsed its arguments through a second
    /// shell, losing every boundary — paths with spaces and `-f '{{...}}'`
    /// format strings arrived mangled.
    #[test]
    fn sg_docker_preserves_argument_boundaries() {
        let helpers = prepare_helpers_bash();
        // Match the dispatch line, not the comment above it that quotes the
        // old form for context.
        assert!(
            !helpers.contains(r#"sg) sg docker -c "docker $*""#),
            "sg path must not re-split arguments through an unquoted $*"
        );
        assert!(
            helpers.contains("bitfun_shell_join"),
            "sg path must quote each argument"
        );
    }

    #[cfg(unix)]
    #[test]
    fn shell_join_round_trips_through_a_second_shell() {
        let helpers = to_unix_script(&prepare_helpers_bash());
        let script = format!(
            r#"{helpers}
sh -c "$(bitfun_shell_join printf '%s\n' 'a b' "it's" '{{{{.State.Running}}}}' '*')"
"#
        );
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            .output()
            .expect("run shell join round trip");
        assert!(
            output.status.success(),
            "shell join failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "a b\nit's\n{{.State.Running}}\n*\n",
            "each argument must survive the second shell verbatim"
        );
    }

    #[test]
    fn one_click_uses_digest_pinned_prebuilt_images_and_regional_routes() {
        let script = release_binary_deploy_bash();
        assert!(script.contains("BITFUN_RELAY_IMAGE_DIGEST"));
        assert!(script.contains("m.daocloud.io/${BITFUN_RELAY_IMAGE}"));
        assert!(script.contains("ghcr.nju.edu.cn/${BITFUN_RELAY_IMAGE#ghcr.io/}"));
        assert!(script.contains("official GHCR fallback"));
        assert!(script.contains("BITFUN_GITHUB_HEALTHY_BPS"));
        assert!(script.contains("bitfun_probe_github_throughput"));
        assert!(script.contains("524288"));
        assert!(script.contains("pull --platform"));
        assert!(script.contains("bitfun_restore_previous_relay"));
        assert!(script.contains("--name bitfun-relay"));
        assert!(script.contains("relay-server_relay-db:/app/data"));
        assert!(script.contains("RELAY_PAGE_PUBLIC_BASE_URL"));
        assert!(script.contains("RELAY_PAGE_AUTH_BASE_URL"));
        assert!(script.contains("trap 'bitfun_restore_previous_relay"));
        assert!(script.contains("name=^bitfun-relay-before-image-"));
        assert!(!script.contains("docker build"));
        assert!(!script.contains("cargo build"));
        assert!(
            !script.contains("tar -x") && !script.contains("docker load"),
            "the raw archive is only a throughput probe, never an install path"
        );
    }

    /// `docker logs` relays the container's stderr on its own stderr, and the
    /// relay logs through tracing — so `2>/dev/null` hid the one message that
    /// explained the failure (`version 'GLIBC_2.38' not found`).
    #[test]
    fn health_check_failure_keeps_container_diagnostics() {
        let script = release_binary_deploy_bash();
        let failure = script
            .split_once("failed its health check")
            .expect("health failure branch")
            .1;
        let logs = failure
            .split_once("logs --tail 40 bitfun-relay")
            .expect("failure branch must dump container logs")
            .1;
        assert!(
            logs.starts_with(" 2>&1"),
            "container stderr must be kept, not sent to /dev/null"
        );
        assert!(
            failure.contains("Container state:"),
            "must report whether the container died or was up but not answering"
        );
    }

    #[test]
    fn release_tag_tracks_stable_and_nightly_channels() {
        assert_eq!(release_tag_for_version("0.2.13"), "v0.2.13");
        assert_eq!(
            release_tag_for_version("0.2.14-nightly.20260724+abc123"),
            "nightly"
        );
    }

    #[test]
    fn signed_descriptor_is_strictly_bound_to_repository_tag_digest_and_platforms() {
        let descriptor = test_image_descriptor();
        validate_relay_image_descriptor(&descriptor).unwrap();

        for invalid in [
            RelayImageDescriptor {
                image: "ghcr.io/attacker/relay".into(),
                ..descriptor.clone()
            },
            RelayImageDescriptor {
                tag: "v9.9.9".into(),
                ..descriptor.clone()
            },
            RelayImageDescriptor {
                digest: format!("sha256:{}", "A".repeat(64)),
                ..descriptor.clone()
            },
            RelayImageDescriptor {
                platforms: vec!["linux/amd64".into()],
                ..descriptor.clone()
            },
        ] {
            assert!(validate_relay_image_descriptor(&invalid).is_err());
        }

        let body = deploy_body_script_with_image(9700, &descriptor);
        assert!(body.contains(&format!("export BITFUN_RELAY_IMAGE={}", descriptor.image)));
        assert!(body.contains(&format!(
            "export BITFUN_RELAY_IMAGE_DIGEST={}",
            descriptor.digest
        )));
        assert!(body.contains("export BITFUN_RELEASE_TAG=v1.2.3"));
        assert!(body.contains("export BITFUN_RELEASE_VERSION=1.2.3"));
        assert!(body.contains("export BITFUN_REQUIRE_IMAGE_DIGEST=1"));
        assert!(!body.contains("bitfun_sync_source"));
        assert!(!body.contains("bitfun_run_deploy_sh"));
    }

    /// Same fixture as the CLI updater, produced with the real `minisign` CLI.
    /// Descriptor bytes from any origin must pass this verification before a
    /// digest is sent to a relay host.
    const FIXTURE_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXkgRTNFMDg3NENFQzFDMjJDMwpSV1RESWh6c1RJZmc0MXcyR3dpZWkwek5ES2FMWW05ZFFWcEVXTlEvVWxweXQybWJTMkpFMVUyTQo=";
    const FIXTURE_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIG1pbmlzaWduIHNlY3JldCBrZXkKUlVUREloenNUSWZnNDBMTitwb25aT3RCVy9VYmJtNWhkR1poM0lCb3IwUDBKaVZmZmM1cFJaNlZSNUpaSzNUUm1yWWpYMXFLQ2svWTdZUDhHdkRZT3YvanVoZlpnZmhyWEFRPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzg0OTUxOTM1CWZpbGU6YXJjaGl2ZS50YXIuZ3oJaGFzaGVkCjhWL21EUVAwZGdlZXVNU1lxWlpsOWdFSGUwOTJQTk9yRG1BMUV6ZHNQOUlEYkcyT1dneTFsQ1puUDBJaFIwQnJpMFBCeENRcUdDR2dpb0l0UGtSMUN3PT0K";
    const FIXTURE_DATA: &[u8] = b"hello-bitfun\n";

    #[test]
    fn checksum_signature_verifies_and_rejects_tampering() {
        verify_minisign(FIXTURE_DATA, FIXTURE_SIGNATURE, FIXTURE_PUBKEY)
            .expect("minisign signature in Tauri's base64 wrapper must verify");
        assert!(verify_minisign(b"tampered\n", FIXTURE_SIGNATURE, FIXTURE_PUBKEY).is_err());
        assert!(verify_minisign(FIXTURE_DATA, "bm90LWEtc2ln", FIXTURE_PUBKEY).is_err());
    }

    /// The official key is embedded as the default trust root, so even keyless
    /// development builds can verify the published image descriptor before
    /// asserting a digest to the remote.
    #[test]
    fn builds_always_carry_a_release_trust_root() {
        assert!(RELEASE_PUBKEY.is_none() || RELEASE_PUBKEY == Some(""));
        assert!(super::release_pubkey().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn generated_deploy_script_is_valid_bash() {
        let script = deploy_body_script_with_image(9700, &test_image_descriptor());
        let output = std::process::Command::new("bash")
            .args(["-n", "-c", &script])
            .output()
            .expect("parse generated deploy script");
        assert!(
            output.status.success(),
            "generated deploy script is invalid:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    #[test]
    fn china_image_pull_fails_over_between_digest_pinned_routes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let script_path = temp.path().join("release-image.sh");
        let trace_path = temp.path().join("pulls.log");
        std::fs::write(&script_path, release_binary_deploy_bash()).expect("write image script");

        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(
                r#"
set -euo pipefail
source "$1"
export TRACE="$2"
export BITFUN_MIRROR_MODE=cn
export BITFUN_RELAY_IMAGE_DIGEST="sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
bitfun_image_docker_with_timeout() {
  shift
  printf '%s\n' "$*" >>"$TRACE"
  case "$*" in
    *ghcr.nju.edu.cn*) return 1 ;;
    *m.daocloud.io*) return 0 ;;
    *) return 1 ;;
  esac
}
bitfun_image_docker() {
  if [ "$1 $2" = "image inspect" ]; then echo amd64; return 0; fi
  return 1
}
selected="$(bitfun_pull_relay_image linux/amd64)"
test "$selected" = "m.daocloud.io/ghcr.io/gcwing/bitfun-relay-server@$BITFUN_RELAY_IMAGE_DIGEST"
"#,
            )
            .arg("image-route-failover")
            .arg(&script_path)
            .arg(&trace_path)
            .output()
            .expect("run image route harness");
        assert!(
            output.status.success(),
            "image route harness failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let pulls = std::fs::read_to_string(trace_path).expect("read pull trace");
        let routes: Vec<_> = pulls.lines().collect();
        assert_eq!(routes.len(), 2);
        assert!(routes[0].contains("ghcr.nju.edu.cn/gcwing/bitfun-relay-server@sha256:"));
        assert!(routes[1].contains("m.daocloud.io/ghcr.io/gcwing/bitfun-relay-server@sha256:"));
    }

    #[cfg(unix)]
    #[test]
    fn automatic_image_pull_keeps_github_when_healthy_and_uses_mirror_when_slow() {
        let temp = tempfile::tempdir().expect("temp dir");
        let script_path = temp.path().join("release-image.sh");
        std::fs::write(&script_path, release_binary_deploy_bash()).expect("write image script");

        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(
                r#"
set -euo pipefail
source "$1"
export BITFUN_MIRROR_REQUESTED_MODE=auto
export BITFUN_RELAY_IMAGE_DIGEST="sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
bitfun_probe_github_throughput() { echo "$MOCK_SPEED"; }
bitfun_image_docker_with_timeout() { return 0; }
bitfun_image_docker() {
  if [ "$1 $2" = "image inspect" ]; then echo amd64; return 0; fi
  return 1
}

MOCK_SPEED=524288
healthy="$(bitfun_pull_relay_image linux/amd64)"
test "$healthy" = "ghcr.io/gcwing/bitfun-relay-server@$BITFUN_RELAY_IMAGE_DIGEST"

MOCK_SPEED=524287
slow="$(bitfun_pull_relay_image linux/amd64)"
test "$slow" = "ghcr.nju.edu.cn/gcwing/bitfun-relay-server@$BITFUN_RELAY_IMAGE_DIGEST"
"#,
            )
            .arg("image-speed-policy")
            .arg(&script_path)
            .output()
            .expect("run image speed harness");
        assert!(
            output.status.success(),
            "image speed harness failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    #[test]
    fn mirror_docker_config_round_trip_preserves_unmanaged_settings() {
        use std::{fs, process::Command};

        let temp = tempfile::tempdir().expect("create mirror test dir");
        let mirror_path = temp.path().join("mirror.sh");
        let daemon_path = temp.path().join("etc/docker/daemon.json");
        fs::create_dir_all(daemon_path.parent().expect("daemon parent"))
            .expect("create daemon dir");
        fs::write(&mirror_path, RELAY_MIRROR_SH).expect("write embedded mirror script");
        fs::write(
            &daemon_path,
            r#"{
  "debug": true,
  "registry-mirrors": ["https://user.example"]
}
"#,
        )
        .expect("write initial daemon config");

        let output = Command::new("bash")
            .arg("-c")
            .arg(
                r#"
set -euo pipefail
export HOME="$2/home"
export BITFUN_DOCKER_DAEMON_JSON="$2/etc/docker/daemon.json"
mkdir -p "$HOME"
source "$1"
bitfun_mirror_priv() { "$@"; }
bitfun_mirror_backup_file() { :; }
bitfun_mirror_restart_docker_if_needed() { :; }
bitfun_mirror_write_docker_daemon_json \
  "https://docker.1ms.run https://dockerproxy.net"
python3 - "$BITFUN_DOCKER_DAEMON_JSON" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    data = json.load(f)
assert data["debug"] is True
assert data["registry-mirrors"] == [
    "https://user.example",
    "https://docker.1ms.run",
    "https://dockerproxy.net",
]
assert "bitfun-cn-mirror" not in data
PY
bitfun_mirror_remove_docker_daemon
python3 - "$BITFUN_DOCKER_DAEMON_JSON" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    data = json.load(f)
assert data == {
    "debug": True,
    "registry-mirrors": ["https://user.example"],
}
PY
"#,
            )
            .arg("mirror-round-trip")
            .arg(&mirror_path)
            .arg(temp.path())
            .output()
            .expect("run mirror round-trip test");

        assert!(
            output.status.success(),
            "mirror round trip failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn decide_status_pending_before_pty_is_running() {
        // preparing, no log yet.
        assert_eq!(
            decide_task_status(false, false, true, false, true, 0, 0, false),
            RelayTaskStatus::Running
        );
        // Nothing staged yet at all.
        assert_eq!(
            decide_task_status(false, false, false, false, false, 0, 0, false),
            RelayTaskStatus::Running
        );
    }

    #[test]
    fn decide_status_growing_log_without_pid_is_running() {
        assert_eq!(
            decide_task_status(false, false, false, false, true, 1000, 100, true),
            RelayTaskStatus::Running
        );
    }

    #[test]
    fn decide_status_dead_pid_stale_log_is_failed() {
        assert_eq!(
            decide_task_status(false, false, false, false, true, 1000, 1000, false),
            RelayTaskStatus::Failed
        );
    }

    /// A driver that died before installing its cleanup trap (bad script upload,
    /// syntax error) leaves the seeded `preparing` flag and an empty log. That
    /// used to read as "running" forever; the wizard never surfaced the failure.
    #[test]
    fn decide_status_dead_driver_with_empty_log_is_failed() {
        assert_eq!(
            decide_task_status(false, false, false, true, true, 0, 0, false),
            RelayTaskStatus::Failed
        );
        // An alive driver still outranks the grace window — a sudo password
        // prompt can legitimately sit for minutes.
        assert_eq!(
            decide_task_status(false, false, true, false, true, 0, 0, false),
            RelayTaskStatus::Running
        );
        // Success wins even if the prepare flag was left behind.
        assert_eq!(
            decide_task_status(true, false, false, true, true, 10, 0, true),
            RelayTaskStatus::Succeeded
        );
    }

    #[test]
    fn split_poll_stdout_accepts_lf() {
        let (head, out) = split_poll_stdout("running=1\nsize=12\nmarker=0\n---\nhello\n");
        assert!(head.contains("running=1"));
        assert_eq!(out, "hello\n");
    }

    #[test]
    fn split_poll_stdout_accepts_crlf() {
        let (head, out) = split_poll_stdout("running=1\r\nsize=12\r\nmarker=0\r\n---\r\nworld\r\n");
        assert!(head.contains("running=1"));
        assert_eq!(out, "world\r\n");
    }

    #[test]
    fn split_poll_stdout_missing_marker_yields_empty_body() {
        let (head, out) = split_poll_stdout("running=0\nsize=0\nmarker=0\n");
        assert!(head.contains("running=0"));
        assert_eq!(out, "");
    }

    #[test]
    fn classify_broken_docker_home() {
        assert_eq!(
            classify_docker_access(true, "ok", true, true, false, true, false),
            DockerAccessMode::BrokenDockerHome
        );
    }

    #[test]
    fn classify_group_inactive() {
        assert_eq!(
            classify_docker_access(true, "unreachable", false, true, true, false, true),
            DockerAccessMode::GroupInactive
        );
    }

    #[test]
    fn parse_preflight_reads_new_fields() {
        let out = r#"
os=Linux
arch=x86_64
home=/home/ubuntu
docker=1
compose=1
daemon=ok
curl=1
tar=1
sudo=0
sudo_needs_password=1
active_docker_group=0
in_docker_group_file=1
docker_home_writable=0
mem_kb=2097152
home_free_kb=12582912
docker_free_kb=8388608
port_busy=0
container=1
container_running=1
existing_port=9700
healthy=1
port_owned=0
"#;
        let pf = parse_preflight(out, 9701);
        assert!(pf.arch_supported);
        assert_eq!(pf.docker_access_mode, DockerAccessMode::BrokenDockerHome);
        assert!(pf.in_docker_group_file);
        assert!(!pf.docker_home_writable);
        assert!(pf.tar_available);
        assert!(pf.sudo_needs_password);
        assert_eq!(pf.probed_port, 9701);
        assert!(pf.container_exists);
        assert!(pf.container_running);
        assert_eq!(pf.existing_relay_port, 9700);
        assert!(pf.relay_healthy);
        assert!(!pf.port_owned_by_relay);
        assert_eq!(pf.home_free_mb, 12288);
        assert_eq!(pf.docker_free_mb, 8192);
    }
}
