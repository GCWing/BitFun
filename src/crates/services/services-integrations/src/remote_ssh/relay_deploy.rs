//! One-click relay server self-deploy orchestration over an existing SSH connection.
//!
//! Drives the open-source relay-server deployment (`src/apps/relay-server/deploy.sh`)
//! on a user-owned server:
//!
//! 1. `run_preflight` — probe OS/arch, Docker, memory, port and existing installs.
//! 2. `start_task` / `poll_task` — run long operations (Docker install, source
//!    download + compose deploy) as detached remote shell scripts with a log
//!    file, so the operation survives SSH disconnects and the client polls
//!    incremental output.
//! 3. `import_account` — hand a locally-provisioned account (derived artifacts
//!    only, never the plaintext password) to `relay-admin import-user` inside
//!    the relay container.
//!
//! All remote state lives under `~/.bitfun/relay-deploy/` on the target server.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::manager::SSHConnectionManager;
use super::remote_git::shell_quote_posix;

/// Public relay port, matching `src/apps/relay-server/docker-compose.yml`.
pub const RELAY_PORT: u16 = 9700;
/// Relay container name, matching docker-compose.yml.
const RELAY_CONTAINER_NAME: &str = "bitfun-relay";
/// Account DB path inside the relay container (RELAY_DB_PATH in docker-compose.yml).
const RELAY_CONTAINER_DB: &str = "/app/data/bitfun_relay.db";
/// Source tarball of the BitFun main repository (avoids requiring git on the server).
const REPO_TARBALL_URL: &str = "https://github.com/GCWing/BitFun/archive/refs/heads/main.tar.gz";
/// Remote directory (relative to the SSH user's home) holding deploy state.
const DEPLOY_STATE_DIR: &str = ".bitfun/relay-deploy";
/// Remote directory (relative to home) the BitFun source tree is unpacked into.
const SOURCE_DIR: &str = "bitfun";
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
    /// `docker compose` (v2) or legacy `docker-compose` available.
    pub compose_available: bool,
    /// "ok": daemon reachable as-is; "sudo": reachable only via sudo;
    /// "unreachable": docker missing or daemon down.
    pub docker_daemon: String,
    pub curl_available: bool,
    /// Root or passwordless sudo (needed for Docker install).
    pub sudo_available: bool,
    pub mem_total_mb: u64,
    /// RELAY_PORT already bound by another process.
    pub port_busy: bool,
    /// A `bitfun-relay` container already exists.
    pub container_exists: bool,
    /// Relay answers on http://127.0.0.1:9700/health from the server itself.
    pub relay_healthy: bool,
    pub home_dir: String,
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
) -> Result<RelayPreflight> {
    let script = r#"
echo "os=$(uname -s 2>/dev/null)"
echo "arch=$(uname -m 2>/dev/null)"
echo "home=$HOME"
if command -v docker >/dev/null 2>&1; then echo "docker=1"; else echo "docker=0"; fi
if docker compose version >/dev/null 2>&1 || command -v docker-compose >/dev/null 2>&1; then echo "compose=1"; else echo "compose=0"; fi
if docker info >/dev/null 2>&1; then echo "daemon=ok"; elif sudo -n docker info >/dev/null 2>&1; then echo "daemon=sudo"; else echo "daemon=unreachable"; fi
if command -v curl >/dev/null 2>&1; then echo "curl=1"; else echo "curl=0"; fi
if [ "$(id -u)" = "0" ]; then echo "sudo=1"; elif sudo -n true >/dev/null 2>&1; then echo "sudo=1"; else echo "sudo=0"; fi
echo "mem_kb=$(awk '/MemTotal/ {print $2}' /proc/meminfo 2>/dev/null || echo 0)"
if command -v ss >/dev/null 2>&1; then PORTS=$(ss -ltn 2>/dev/null); else PORTS=$(netstat -ltn 2>/dev/null); fi
if printf '%s\n' "$PORTS" | awk '{print $4}' | grep -q ':9700$'; then echo "port_busy=1"; else echo "port_busy=0"; fi
if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -qx bitfun-relay || sudo -n docker ps -a --format '{{.Names}}' 2>/dev/null | grep -qx bitfun-relay; then echo "container=1"; else echo "container=0"; fi
if curl -fsS -m 3 http://127.0.0.1:9700/health >/dev/null 2>&1; then echo "healthy=1"; else echo "healthy=0"; fi
"#;
    let (stdout, _stderr, code) = manager.execute_command(connection_id, script).await?;
    if code != 0 {
        return Err(anyhow!("preflight probe failed (exit {code})"));
    }
    Ok(parse_preflight(&stdout))
}

fn parse_preflight(out: &str) -> RelayPreflight {
    let get = |key: &str| -> String {
        out.lines()
            .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let os = get("os");
    let arch = get("arch");
    let arch_supported = os == "Linux"
        && (arch == "x86_64" || arch == "amd64" || arch == "aarch64" || arch == "arm64");
    let mem_kb: u64 = get("mem_kb").parse().unwrap_or(0);
    RelayPreflight {
        os,
        arch,
        arch_supported,
        docker_installed: get("docker") == "1",
        compose_available: get("compose") == "1",
        docker_daemon: {
            let d = get("daemon");
            if d.is_empty() {
                "unreachable".into()
            } else {
                d
            }
        },
        curl_available: get("curl") == "1",
        sudo_available: get("sudo") == "1",
        mem_total_mb: mem_kb / 1024,
        port_busy: get("port_busy") == "1",
        container_exists: get("container") == "1",
        relay_healthy: get("healthy") == "1",
        home_dir: get("home"),
    }
}

/// Start a detached remote task. Returns immediately; use `poll_task` to
/// follow progress. Any previous log for the same task is truncated.
pub async fn start_task(
    manager: &SSHConnectionManager,
    connection_id: &str,
    task: RelayDeployTask,
) -> Result<()> {
    let home = resolve_home(manager, connection_id).await?;
    let dir = format!("{home}/{DEPLOY_STATE_DIR}");
    let stem = task.stem();
    let script_body = match task {
        RelayDeployTask::InstallDocker => install_docker_script(),
        RelayDeployTask::Deploy => deploy_script(),
    };

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
    let script_path = format!("{dir}/{stem}.sh");
    manager
        .sftp_write(connection_id, &script_path, script_body.as_bytes())
        .await?;

    // Detach fully: stdio redirected, stdin from /dev/null, so the SSH exec
    // channel closes immediately and the task survives disconnects.
    // Prefer stdbuf line buffering so docker/cargo output reaches the log file
    // while the task is still running (file redirects otherwise fully buffer).
    let launch = format!(
        "cd {dir} && chmod 700 {stem}.sh && rm -f {stem}.log {stem}.pid \
         && (command -v stdbuf >/dev/null 2>&1 && RUNNER='stdbuf -oL -eL bash' || RUNNER=bash) \
         && nohup $RUNNER ./{stem}.sh > {stem}.log 2>&1 < /dev/null & echo $! > {stem}.pid"
    );
    exec_ok(manager, connection_id, &launch).await?;
    Ok(())
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
running=0
if [ -f "$PIDF" ] && kill -0 "$(cat "$PIDF" 2>/dev/null)" 2>/dev/null; then running=1; fi
size=0
if [ -f "$LOG" ]; then size=$(wc -c < "$LOG" | tr -d ' '); fi
marker=0
if [ -f "$LOG" ] && grep -q {TASK_DONE_MARKER} "$LOG"; then marker=1; fi
echo "running=$running"
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
    let marker = get("marker") == "1";
    let size: u64 = get("size").parse().unwrap_or(cursor);
    let status = if marker {
        RelayTaskStatus::Succeeded
    } else if running {
        RelayTaskStatus::Running
    } else {
        RelayTaskStatus::Failed
    };
    Ok(RelayTaskPoll {
        cursor: size,
        output: output.to_string(),
        status,
    })
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
         DOCKER=docker; \
         docker info >/dev/null 2>&1 || DOCKER='sudo docker'; \
         if $DOCKER ps --format '{{{{.Names}}}}' 2>/dev/null | grep -qx {name}; then \
           cat {q} | $DOCKER exec -i {name} /app/relay-admin --db {db} import-user; \
           rc=$?; rm -f {q}; exit $rc; \
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
) -> Result<bool> {
    let (_o, _e, code) = manager
        .execute_command(
            connection_id,
            &format!("curl -fsS -m 5 http://127.0.0.1:{RELAY_PORT}/health >/dev/null 2>&1"),
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

/// Script that installs Docker via the official convenience script.
fn install_docker_script() -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$(id -u)" = "0" ]; then SUDO=""; else SUDO="sudo -n"; fi
echo ">>> Installing Docker (get.docker.com)..."
curl -fsSL --retry 3 https://get.docker.com -o /tmp/bitfun-get-docker.sh
$SUDO sh /tmp/bitfun-get-docker.sh
rm -f /tmp/bitfun-get-docker.sh
$SUDO systemctl enable --now docker 2>/dev/null || true
if [ -n "$SUDO" ]; then $SUDO usermod -aG docker "$(id -un)" 2>/dev/null || true; fi
echo ">>> Docker installed: $($SUDO docker --version 2>/dev/null || true)"
echo {TASK_DONE_MARKER}
"#
    )
}

/// Script that downloads the BitFun source tarball and runs deploy.sh.
fn deploy_script() -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
DOCKER="docker"
if ! docker info >/dev/null 2>&1; then
  if sudo -n docker info >/dev/null 2>&1; then
    DOCKER="sudo docker"
  else
    echo "ERROR: docker daemon is not reachable (try re-login for group membership)" >&2
    exit 1
  fi
fi
echo ">>> Downloading BitFun source..."
SRC="$HOME/{SOURCE_DIR}"
rm -rf "$SRC"
mkdir -p "$SRC"
curl -fsSL --retry 3 {REPO_TARBALL_URL} | tar xz -C "$SRC" --strip-components=1
cd "$SRC/src/apps/relay-server"
# Small VPS safety: limit rustc parallelism when memory is tight.
MEM_KB=$(awk '/MemTotal/ {{print $2}}' /proc/meminfo 2>/dev/null || echo 0)
if [ "${{RELAY_CARGO_BUILD_JOBS:-}}" = "" ] && [ "$MEM_KB" -lt 2097152 ]; then
  export RELAY_CARGO_BUILD_JOBS=1
  echo ">>> Low memory detected; using RELAY_CARGO_BUILD_JOBS=1"
fi
# Stream BuildKit lines into the detached log file (avoid fancy TTY progress).
export BUILDKIT_PROGRESS=plain
echo ">>> Building and starting the relay container (this can take a while)..."
if [ "$DOCKER" = "sudo docker" ]; then
  sudo -E env RELAY_CARGO_BUILD_JOBS="${{RELAY_CARGO_BUILD_JOBS:-}}" \
    BUILDKIT_PROGRESS=plain bash deploy.sh
else
  bash deploy.sh
fi
echo {TASK_DONE_MARKER}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::split_poll_stdout;

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
}
