//! One-click relay server self-deploy orchestration over an existing SSH connection.
//!
//! Drives the open-source relay-server deployment (`src/apps/relay-server/deploy.sh`)
//! on a user-owned server:
//!
//! 1. `run_preflight` — probe OS/arch, Docker access mode, memory, port, existing installs.
//! 2. `start_task` — stage an interactive driver script (run inside a remote PTY so sudo
//!    passwords work) that prepares Docker access, then launches the long build via
//!    `nohup` and `tail -f`s the log.
//! 3. `poll_task` — detect completion via marker/pid for wizard state transitions.
//! 4. `cancel_task` — stop a running task when the wizard closes (kill process tree +
//!    best-effort compose teardown for in-progress deploys).
//! 5. `import_account` — hand a locally-provisioned account to `relay-admin import-user`.
//!
//! Remote deploy state lives under `~/.bitfun/relay-deploy/`; the cloned source
//! tree lives under `~/.bitfun/relay-src/` (never `$HOME/bitfun`, which may be
//! the user's own project).
//!
//! Product / regression invariants (wizard + entry points):
//! `src/web-ui/src/features/relay-deploy/README.md`. Do not change clone destination,
//! password handoff, or “already deployed” semantics without updating that doc.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::manager::SSHConnectionManager;
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
/// Canonical git remote for incremental source updates on the target server.
const REPO_GIT_URL: &str = "https://github.com/GCWing/BitFun.git";
/// Branch tracked by one-click deploy.
const REPO_GIT_BRANCH: &str = "main";
/// Tarball fallback when git is unavailable or clone/fetch fails.
const REPO_TARBALL_URL: &str = "https://github.com/GCWing/BitFun/archive/refs/heads/main.tar.gz";
/// Remote directory (relative to the SSH user's home) holding deploy state.
const DEPLOY_STATE_DIR: &str = ".bitfun/relay-deploy";
/// BitFun-managed source checkout (relative to home). Must stay under `.bitfun/`
/// so deploy never deletes or overwrites a user directory named `bitfun`/`BitFun`.
const SOURCE_DIR: &str = ".bitfun/relay-src";
/// Line printed by task scripts on success; polled to detect completion.
const TASK_DONE_MARKER: &str = "RELAY_TASK_DONE";

/// Long-running remote operations that run detached and are polled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayDeployTask {
    InstallDocker,
    Deploy,
}

impl RelayDeployTask {
    fn stem(self) -> &'static str {
        match self {
            Self::InstallDocker => "install-docker",
            Self::Deploy => "deploy",
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
    let (stdout, _stderr, code) = manager.execute_command(connection_id, &script).await?;
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
/// `port` is used for deploy (written to `relay.port` + compose `.env`); ignored
/// for Docker install.
pub async fn start_task(
    manager: &SSHConnectionManager,
    connection_id: &str,
    task: RelayDeployTask,
    port: u16,
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
        RelayDeployTask::Deploy => deploy_body_script(port),
    };
    let driver = match task {
        RelayDeployTask::InstallDocker => interactive_driver_script(stem, "install"),
        RelayDeployTask::Deploy => interactive_driver_script(stem, "deploy"),
    };

    let body_path = format!("{dir}/{stem}-body.sh");
    let script_path = format!("{dir}/{stem}.sh");
    let port_path = format!("{dir}/relay.port");
    manager
        .sftp_write(connection_id, &body_path, body.as_bytes())
        .await?;
    manager
        .sftp_write(connection_id, &script_path, driver.as_bytes())
        .await?;
    if matches!(task, RelayDeployTask::Deploy) {
        manager
            .sftp_write(connection_id, &port_path, format!("{port}\n").as_bytes())
            .await?;
    }
    // Seed preparing flag before the PTY runs the driver so early polls do not
    // race into "failed" (no pid / no flag yet).
    let prepare_flag = format!("{dir}/{stem}.preparing");
    let log_path = format!("{dir}/{stem}.log");
    let pid_path = format!("{dir}/{stem}.pid");
    exec_ok(
        manager,
        connection_id,
        &format!(
            "chmod 700 {} {} && rm -f {} {} && : > {} && touch {}",
            shell_quote_posix(&body_path),
            shell_quote_posix(&script_path),
            shell_quote_posix(&pid_path),
            shell_quote_posix(&log_path),
            shell_quote_posix(&log_path),
            shell_quote_posix(&prepare_flag),
        ),
    )
    .await?;

    Ok(RelayTaskStart { script_path })
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
PREPF="$D/{stem}.preparing"
running=0
if [ -f "$PIDF" ] && kill -0 "$(cat "$PIDF" 2>/dev/null)" 2>/dev/null; then running=1; fi
# Interactive prepare phase (sudo prompts) before nohup starts.
preparing=0
if [ -f "$PREPF" ]; then preparing=1; fi
log_exists=0
size=0
if [ -f "$LOG" ]; then log_exists=1; size=$(wc -c < "$LOG" | tr -d ' '); fi
marker=0
if [ -f "$LOG" ] && grep -q {TASK_DONE_MARKER} "$LOG"; then marker=1; fi
# Build may still be progressing via docker/buildkit even if the wrapper pid
# briefly looks gone; treat a growing log without a marker as running.
echo "running=$running"
echo "preparing=$preparing"
echo "log_exists=$log_exists"
echo "size=$size"
echo "marker=$marker"
echo "---"
if [ -f "$LOG" ]; then tail -c +{from} "$LOG"; fi
"#,
        from = cursor.saturating_add(1),
    );
    let (stdout, _stderr, code) = manager.execute_command(connection_id, &script).await?;
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
    let log_exists = get("log_exists") == "1";
    let marker = get("marker") == "1";
    let size: u64 = get("size").parse().unwrap_or(cursor);
    let status = decide_task_status(
        marker,
        running,
        preparing,
        log_exists,
        size,
        cursor,
        !output.is_empty(),
    );
    Ok(RelayTaskPoll {
        cursor: size,
        output: output.to_string(),
        status,
    })
}

/// Cancel a running install/deploy task (wizard close / back / retry).
///
/// Kills the nohup body process tree, clears pid/preparing flags, appends a
/// cancel marker to the log, and for deploy best-effort stops an in-progress
/// compose build. Safe to call when nothing is running.
pub async fn cancel_task(
    manager: &SSHConnectionManager,
    connection_id: &str,
    task: RelayDeployTask,
) -> Result<()> {
    let stem = task.stem();
    // Only tear down compose when we interrupt an in-progress deploy — never when
    // cancel is a no-op cleanup before start_task (would stop a healthy relay).
    let compose_teardown = if matches!(task, RelayDeployTask::Deploy) {
        format!(
            r#"
if [ "$was_active" = "1" ]; then
  SRC="$HOME/{SOURCE_DIR}/src/apps/relay-server"
  stop_compose() {{
    if [ ! -d "$SRC" ]; then return 0; fi
    (
      cd "$SRC" || exit 0
      "$@" compose kill >/dev/null 2>&1 || true
      for id in $("$@" ps -aq --filter "label=com.docker.compose.project=relay-server" 2>/dev/null); do
        # Skip the already-running production container name only when we are
        # not mid-redeploy; during cancel of an active build, tear builders down.
        "$@" kill -s KILL "$id" >/dev/null 2>&1 || true
      done
      # BuildKit workers often outlive the compose CLI — stop the default builder.
      "$@" buildx stop >/dev/null 2>&1 || true
      "$@" builder stop >/dev/null 2>&1 || true
    ) || true
  }}
  if command -v docker >/dev/null 2>&1; then
    stop_compose docker
  fi
  if command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
    stop_compose sudo -n docker
  fi
fi
"#,
            SOURCE_DIR = SOURCE_DIR,
        )
    } else {
        String::new()
    };
    let script = format!(
        r#"
set +e
D="$HOME/{DEPLOY_STATE_DIR}"
STEM="{stem}"
LOG="$D/$STEM.log"
PIDF="$D/$STEM.pid"
PREPF="$D/$STEM.preparing"
BODY="$D/$STEM-body.sh"
mkdir -p "$D" 2>/dev/null
was_active=0
[ -f "$PREPF" ] && was_active=1
rm -f "$PREPF"
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
{compose_teardown}
if [ "$was_active" = "1" ]; then
  echo "" >>"$LOG" 2>/dev/null
  echo ">>> Cancelled by client (wizard closed)" >>"$LOG" 2>/dev/null
fi
exit 0
"#,
        DEPLOY_STATE_DIR = DEPLOY_STATE_DIR,
        stem = stem,
        compose_teardown = compose_teardown,
    );
    let (_stdout, stderr, code) = manager.execute_command(connection_id, &script).await?;
    if code != 0 {
        return Err(anyhow!("cancel failed (exit {code}): {stderr}"));
    }
    Ok(())
}

/// Decide poll status from remote probe fields.
///
/// Pending (PTY not started yet) and active prepare/build must not look like
/// failure — the wizard polls immediately after staging scripts.
fn decide_task_status(
    marker: bool,
    running: bool,
    preparing: bool,
    log_exists: bool,
    size: u64,
    cursor: u64,
    got_new_output: bool,
) -> RelayTaskStatus {
    if marker {
        return RelayTaskStatus::Succeeded;
    }
    if running || preparing || !log_exists || size == 0 {
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
    let (stdout, stderr, code) = manager.execute_command(connection_id, &cmd).await?;
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

async fn exec_ok(manager: &SSHConnectionManager, connection_id: &str, command: &str) -> Result<()> {
    let (stdout, stderr, code) = manager.execute_command(connection_id, command).await?;
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
fn prepare_helpers_bash() -> &'static str {
    r#"
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
  command -v git >/dev/null 2>&1 || pkgs+=(git)
  command -v curl >/dev/null 2>&1 || pkgs+=(curl)
  command -v tar >/dev/null 2>&1 || pkgs+=(tar)
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

bitfun_fix_docker_home() {
  export DOCKER_CONFIG="${DOCKER_CONFIG:-$HOME/.bitfun/docker-config}"
  mkdir -p "$DOCKER_CONFIG"
  chmod 700 "$DOCKER_CONFIG" 2>/dev/null || true
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

bitfun_docker() {
  case "${BITFUN_DOCKER_MODE:-direct}" in
    sg) sg docker -c "docker $*" ;;
    sudo)
      if sudo -n true >/dev/null 2>&1; then sudo -n docker "$@"; else sudo docker "$@"; fi
      ;;
    *) docker "$@" ;;
  esac
}

bitfun_run_deploy_sh() {
  local dir="$1"
  local port="${RELAY_PORT:-9700}"
  case "${BITFUN_DOCKER_MODE:-direct}" in
    sudo)
      if sudo -n true >/dev/null 2>&1; then
        sudo -n -E env RELAY_PORT="$port" RELAY_CARGO_BUILD_JOBS="${RELAY_CARGO_BUILD_JOBS:-}" \
          BUILDKIT_PROGRESS=plain DOCKER_CONFIG="${DOCKER_CONFIG:-}" \
          bash "$dir/deploy.sh"
      else
        sudo -E env RELAY_PORT="$port" RELAY_CARGO_BUILD_JOBS="${RELAY_CARGO_BUILD_JOBS:-}" \
          BUILDKIT_PROGRESS=plain DOCKER_CONFIG="${DOCKER_CONFIG:-}" \
          bash "$dir/deploy.sh"
      fi
      ;;
    sg)
      sg docker -c "env RELAY_PORT='$port' RELAY_CARGO_BUILD_JOBS='${RELAY_CARGO_BUILD_JOBS:-}' BUILDKIT_PROGRESS=plain DOCKER_CONFIG='${DOCKER_CONFIG:-}' bash '$dir/deploy.sh'"
      ;;
    *)
      env RELAY_PORT="$port" BUILDKIT_PROGRESS=plain bash "$dir/deploy.sh"
      ;;
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
  PREPARE_FLAG="$D/$STEM.preparing"
fi
PREPARE_FLAG="$D/$STEM.preparing"
# Keep/refresh the preparing flag seeded by start_task — do not clear it first
# or early polls can race into "failed".
rm -f "$PIDF"
: >"$LOG"
touch "$PREPARE_FLAG"
echo ">>> prepare starting (uid=$(id -u) home=$HOME)" | tee -a "$LOG"
cleanup_prepare() {{ rm -f "$PREPARE_FLAG"; }}
trap cleanup_prepare EXIT
bitfun_ensure_tools
export DOCKER_CONFIG="${{DOCKER_CONFIG:-$HOME/.bitfun/docker-config}}"
mkdir -p "$DOCKER_CONFIG"

# install: Docker is not present yet — do NOT resolve daemon access here.
if [ "{kind}" = "install" ]; then
  BITFUN_DOCKER_MODE=direct
else
  bitfun_resolve_docker_mode
fi
export BITFUN_DOCKER_MODE

# Ensure compose plugin when deploying
if [ "{kind}" = "deploy" ]; then
  if ! docker compose version >/dev/null 2>&1 \
     && ! command -v docker-compose >/dev/null 2>&1 \
     && ! sudo -n docker compose version >/dev/null 2>&1 \
     && ! sudo docker compose version >/dev/null 2>&1; then
    echo ">>> docker compose missing; attempting install..."
    if [ "$(id -u)" = "0" ]; then
      apt-get update -y && apt-get install -y docker-compose-plugin 2>/dev/null \
        || yum install -y docker-compose-plugin 2>/dev/null || true
    else
      bitfun_priv apt-get update -y && bitfun_priv apt-get install -y docker-compose-plugin 2>/dev/null \
        || bitfun_priv yum install -y docker-compose-plugin 2>/dev/null || true
    fi
  fi
fi

# Docker install: run in foreground as (elevated) root when possible.
# Long deploy builds still go through nohup so the wizard can follow the log.
if [ "{kind}" = "install" ]; then
  echo ">>> Installing Docker..." | tee -a "$LOG"
  export BITFUN_KEEP_HOME="${{BITFUN_KEEP_HOME:-$HOME}}"
  set +e
  if command -v stdbuf >/dev/null 2>&1; then
    stdbuf -oL -eL env BITFUN_KEEP_HOME="$BITFUN_KEEP_HOME" bash "$BODY" 2>&1 | tee -a "$LOG"
  else
    env BITFUN_KEEP_HOME="$BITFUN_KEEP_HOME" bash "$BODY" 2>&1 | tee -a "$LOG"
  fi
  code=${{PIPESTATUS[0]}}
  set -e
  rm -f "$PREPARE_FLAG" "$PIDF"
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
  RELAY_CARGO_BUILD_JOBS="${{RELAY_CARGO_BUILD_JOBS:-}}" BUILDKIT_PROGRESS=plain \
  "${{RUNNER[@]}}" "$BODY" >"$LOG" 2>&1 < /dev/null &
echo $! >"$PIDF"
rm -f "$PREPARE_FLAG"
trap - EXIT
echo ">>> Following log..."
exec tail -n +1 -f "$LOG"
"#,
        DEPLOY_STATE_DIR = DEPLOY_STATE_DIR,
        stem = stem,
        kind = kind,
        helpers = helpers,
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
mkdir -p "$DOCKER_CONFIG"
# When elevated as root, add the original login user to the docker group.
DEPLOY_USER="${{SUDO_USER:-}}"
if [ -z "$DEPLOY_USER" ] || [ "$DEPLOY_USER" = "root" ]; then
  if [ -n "${{BITFUN_KEEP_HOME:-}}" ] && [ -d "${{BITFUN_KEEP_HOME}}" ]; then
    DEPLOY_USER="$(stat -c '%U' "$BITFUN_KEEP_HOME" 2>/dev/null || true)"
  fi
fi
if [ -z "$DEPLOY_USER" ] || [ "$DEPLOY_USER" = "root" ]; then
  DEPLOY_USER="$(id -un)"
fi
echo ">>> Installing Docker (get.docker.com) as uid=$(id -u) for user=$DEPLOY_USER ..."
curl -fsSL --retry 3 https://get.docker.com -o /tmp/bitfun-get-docker.sh
if [ "$(id -u)" = "0" ]; then
  sh /tmp/bitfun-get-docker.sh
  systemctl enable --now docker
  usermod -aG docker "$DEPLOY_USER" || true
else
  bitfun_priv sh /tmp/bitfun-get-docker.sh
  bitfun_priv systemctl enable --now docker
  bitfun_priv usermod -aG docker "$DEPLOY_USER"
fi
rm -f /tmp/bitfun-get-docker.sh
bitfun_fix_docker_home
# Verify without relying on a new login session
if docker info >/dev/null 2>&1 \
   || sg docker -c 'docker info' >/dev/null 2>&1 \
   || sudo -n docker info >/dev/null 2>&1 \
   || sudo docker info >/dev/null 2>&1; then
  echo ">>> Docker installed and reachable: $(docker --version 2>/dev/null || sudo -n docker --version 2>/dev/null || true)"
  echo {TASK_DONE_MARKER}
else
  echo "ERROR: Docker installed but daemon is not reachable" >&2
  exit 1
fi
"#,
        helpers = helpers,
        TASK_DONE_MARKER = TASK_DONE_MARKER,
    )
}

/// Sync BitFun source: prefer shallow git update, fall back to tarball.
///
/// `src` must be the BitFun-managed path (`~/.bitfun/relay-src`). Destructive
/// replace is safe only there — never use `$HOME/bitfun` / `$HOME/BitFun`.
fn sync_source_bash() -> String {
    format!(
        r#"
bitfun_sync_source() {{
  # Destination is always ~/.bitfun/relay-src (repo ROOT), never $HOME/BitFun.
  # `git clone <url>` without a path would create ./BitFun — we always pass "$src".
  # Tarball extracts BitFun-main/; we use --strip-components=1 into "$src".
  local src="$1"
  local git_url="{REPO_GIT_URL}"
  local branch="{REPO_GIT_BRANCH}"
  local tarball_url="{REPO_TARBALL_URL}"
  local managed_prefix="$HOME/.bitfun/"
  local relay_deploy_sh="src/apps/relay-server/deploy.sh"

  # Refuse to touch anything outside ~/.bitfun/ (protect user project dirs).
  case "$src" in
    "$managed_prefix"*) ;;
    *)
      echo "ERROR: refusing to sync source outside ~/.bitfun/: $src" >&2
      return 1
      ;;
  esac

  bitfun_replace_managed_src() {{
    rm -rf "$src"
    mkdir -p "$(dirname "$src")"
  }}

  # Ensure "$src" is the repo root (contains src/apps/relay-server), not a
  # nested BitFun/ or BitFun-main/ from a mistaken clone/extract.
  bitfun_assert_source_layout() {{
    if [ -f "$src/$relay_deploy_sh" ]; then
      return 0
    fi
    local nested=""
    if [ -f "$src/BitFun/$relay_deploy_sh" ]; then
      nested="$src/BitFun"
    elif [ -f "$src/BitFun-main/$relay_deploy_sh" ]; then
      nested="$src/BitFun-main"
    elif [ -f "$src/bitfun/$relay_deploy_sh" ]; then
      nested="$src/bitfun"
    fi
    if [ -n "$nested" ]; then
      echo ">>> Flattening nested checkout ($(basename "$nested")) into $src..."
      # Move nested repo root contents up one level inside the managed dir only.
      shopt -s dotglob nullglob
      local tmp="$src.__flatten_$$"
      mv "$nested" "$tmp"
      rm -rf "$src"
      mv "$tmp" "$src"
      shopt -u dotglob nullglob
    fi
    if [ ! -f "$src/$relay_deploy_sh" ]; then
      echo "ERROR: source layout invalid under $src (missing $relay_deploy_sh)" >&2
      return 1
    fi
  }}

  bitfun_fetch_tarball() {{
    echo ">>> Downloading BitFun source (tarball fallback)..."
    command -v curl >/dev/null 2>&1 || bitfun_ensure_tools
    command -v tar >/dev/null 2>&1 || bitfun_ensure_tools
    bitfun_replace_managed_src
    mkdir -p "$src"
    # Archive root is BitFun-main/; strip so files land directly in "$src".
    curl -fsSL --retry 3 "$tarball_url" | tar xz -C "$src" --strip-components=1
    bitfun_assert_source_layout
  }}

  if ! command -v git >/dev/null 2>&1; then
    bitfun_ensure_tools || true
  fi

  if command -v git >/dev/null 2>&1; then
    if [ -d "$src/.git" ]; then
      echo ">>> Updating BitFun source (git fetch)..."
      git -C "$src" remote set-url origin "$git_url" 2>/dev/null || true
      if git -C "$src" fetch --depth 1 origin "$branch" \
        && git -C "$src" checkout -f -B "$branch" "origin/$branch" \
        && git -C "$src" clean -fd \
        && bitfun_assert_source_layout; then
        echo ">>> Source updated to $(git -C "$src" rev-parse --short HEAD 2>/dev/null || echo unknown)"
        return 0
      fi
      echo ">>> git update failed; recloning managed source..."
      bitfun_replace_managed_src
    elif [ -e "$src" ]; then
      echo ">>> Managed source exists but is not a git checkout; replacing..."
      bitfun_replace_managed_src
    fi
    echo ">>> Cloning into $src (explicit path; not default BitFun/)..."
    mkdir -p "$(dirname "$src")"
    # Explicit destination avoids creating $PWD/BitFun from the repo name.
    if git clone --depth 1 --branch "$branch" "$git_url" "$src" \
      && bitfun_assert_source_layout; then
      echo ">>> Source cloned at $(git -C "$src" rev-parse --short HEAD 2>/dev/null || echo unknown)"
      return 0
    fi
    echo ">>> git clone failed; falling back to tarball"
  else
    echo ">>> git unavailable; using tarball fallback"
  fi
  bitfun_fetch_tarball
}}
"#,
        REPO_GIT_URL = REPO_GIT_URL,
        REPO_GIT_BRANCH = REPO_GIT_BRANCH,
        REPO_TARBALL_URL = REPO_TARBALL_URL,
    )
}

/// Non-interactive body for deploy (runs under nohup after prepare).
fn deploy_body_script(port: u16) -> String {
    let helpers = prepare_helpers_bash();
    let sync = sync_source_bash();
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
{helpers}
{sync}
export DOCKER_CONFIG="${{DOCKER_CONFIG:-$HOME/.bitfun/docker-config}}"
export BUILDKIT_PROGRESS=plain
BITFUN_DOCKER_MODE="${{BITFUN_DOCKER_MODE:-direct}}"
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
SRC="$HOME/{SOURCE_DIR}"
bitfun_sync_source "$SRC"
cd "$SRC/src/apps/relay-server"
# Persist for compose interpolation (and subsequent start/restart scripts).
printf 'RELAY_PORT=%s\n' "$RELAY_PORT" > .env
chmod 600 .env 2>/dev/null || true
# Until main ships templated compose, rewrite hardcoded 9700 for custom ports.
if [ -f docker-compose.yml ] && ! grep -q '\${{RELAY_PORT' docker-compose.yml; then
  sed -i.bak \
    -e "s/:9700:9700/:${{RELAY_PORT}}:${{RELAY_PORT}}/g" \
    -e "s/RELAY_PORT=9700/RELAY_PORT=${{RELAY_PORT}}/g" \
    -e "s|127\\.0\\.0\\.1:9700/health|127.0.0.1:${{RELAY_PORT}}/health|g" \
    docker-compose.yml 2>/dev/null || true
fi
MEM_KB=$(awk '/MemTotal/ {{print $2}}' /proc/meminfo 2>/dev/null || echo 0)
if [ "${{RELAY_CARGO_BUILD_JOBS:-}}" = "" ] && [ "$MEM_KB" -lt 2097152 ]; then
  export RELAY_CARGO_BUILD_JOBS=1
  echo ">>> Low memory detected; using RELAY_CARGO_BUILD_JOBS=1"
fi
echo ">>> Building and starting the relay container on port $RELAY_PORT (this can take a while)..."
bitfun_run_deploy_sh "$(pwd)"
echo {TASK_DONE_MARKER}
"#,
        helpers = helpers,
        sync = sync,
        DEPLOY_STATE_DIR = DEPLOY_STATE_DIR,
        SOURCE_DIR = SOURCE_DIR,
        port = port,
        TASK_DONE_MARKER = TASK_DONE_MARKER,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        classify_docker_access, decide_task_status, parse_preflight, split_poll_stdout,
        DockerAccessMode, RelayTaskStatus,
    };

    #[test]
    fn decide_status_pending_before_pty_is_running() {
        assert_eq!(
            decide_task_status(false, false, true, true, 0, 0, false),
            RelayTaskStatus::Running
        );
        assert_eq!(
            decide_task_status(false, false, false, false, 0, 0, false),
            RelayTaskStatus::Running
        );
    }

    #[test]
    fn decide_status_growing_log_without_pid_is_running() {
        assert_eq!(
            decide_task_status(false, false, false, true, 1000, 100, true),
            RelayTaskStatus::Running
        );
    }

    #[test]
    fn decide_status_dead_pid_stale_log_is_failed() {
        assert_eq!(
            decide_task_status(false, false, false, true, 1000, 1000, false),
            RelayTaskStatus::Failed
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
    }
}
