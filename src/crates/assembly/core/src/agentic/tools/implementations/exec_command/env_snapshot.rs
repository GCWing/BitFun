use crate::service::remote_ssh::{
    get_global_remote_exec_process_manager, RemoteExecCommandRequest, RemoteExecControlAction,
    RemoteExecControlOrigin, RemoteExecControlRequest, SSHConnectionManager,
};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use terminal_core::ShellType;
use tokio::sync::Mutex;

const ENV_SNAPSHOT_BEGIN: &str = "__BITFUN_REMOTE_ENV_SNAPSHOT_BEGIN__";
const ENV_SNAPSHOT_END: &str = "__BITFUN_REMOTE_ENV_SNAPSHOT_END__";
const ENV_SNAPSHOT_TIMEOUT_MS: u64 = 3_000;
const ENV_SNAPSHOT_MAX_OUTPUT_CHARS: usize = 128 * 1024;
const ENV_SNAPSHOT_TTL: Duration = Duration::from_secs(10 * 60);

static REMOTE_ENV_SNAPSHOT_CACHE: OnceLock<Mutex<HashMap<RemoteEnvSnapshotKey, CachedSnapshot>>> =
    OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteEnvSnapshot {
    pub(super) env: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RemoteEnvSnapshotKey {
    connection_id: String,
    shell_path: String,
    shell_type: String,
}

#[derive(Debug, Clone)]
struct CachedSnapshot {
    captured_at: Instant,
    snapshot: RemoteEnvSnapshot,
}

pub(super) async fn remote_env_snapshot_for(
    ssh_manager: SSHConnectionManager,
    connection_id: &str,
    shell_path: &str,
    shell_type: &ShellType,
) -> Option<RemoteEnvSnapshot> {
    let key = RemoteEnvSnapshotKey {
        connection_id: connection_id.to_string(),
        shell_path: shell_path.to_string(),
        shell_type: shell_type.to_string(),
    };

    if let Some(snapshot) = cached_snapshot(&key).await {
        return Some(snapshot);
    }

    let snapshot =
        match capture_remote_env_snapshot(ssh_manager, connection_id, shell_path, shell_type).await
        {
            Ok(snapshot) => snapshot,
            Err(_) => return None,
        };
    cache_snapshot(key, snapshot.clone()).await;
    Some(snapshot)
}

async fn cached_snapshot(key: &RemoteEnvSnapshotKey) -> Option<RemoteEnvSnapshot> {
    let cache = REMOTE_ENV_SNAPSHOT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let guard = cache.lock().await;
    guard
        .get(key)
        .filter(|entry| entry.captured_at.elapsed() <= ENV_SNAPSHOT_TTL)
        .map(|entry| entry.snapshot.clone())
}

async fn cache_snapshot(key: RemoteEnvSnapshotKey, snapshot: RemoteEnvSnapshot) {
    let cache = REMOTE_ENV_SNAPSHOT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    cache.lock().await.insert(
        key,
        CachedSnapshot {
            captured_at: Instant::now(),
            snapshot,
        },
    );
}

async fn capture_remote_env_snapshot(
    ssh_manager: SSHConnectionManager,
    connection_id: &str,
    shell_path: &str,
    shell_type: &ShellType,
) -> anyhow::Result<RemoteEnvSnapshot> {
    let command = remote_env_snapshot_command(shell_path, shell_type);
    let manager = get_global_remote_exec_process_manager();
    let response = manager
        .exec_command(RemoteExecCommandRequest {
            ssh_manager,
            connection_id: connection_id.to_string(),
            command,
            tty: true,
            yield_time_ms: Some(ENV_SNAPSHOT_TIMEOUT_MS),
            max_output_chars: Some(ENV_SNAPSHOT_MAX_OUTPUT_CHARS),
            lifecycle_tx: None,
            output_capture_tx: None,
        })
        .await?;

    if let Some(session_id) = response.session_id {
        let _ = manager
            .control_session(RemoteExecControlRequest {
                session_id,
                action: RemoteExecControlAction::Kill,
                origin: RemoteExecControlOrigin::ModelTool,
                yield_time_ms: Some(500),
                max_output_chars: Some(2_000),
            })
            .await;
        anyhow::bail!("remote environment snapshot command did not exit before timeout");
    }

    if response.exit_code.is_some_and(|exit_code| exit_code != 0) {
        anyhow::bail!(
            "remote environment snapshot command exited with {:?}",
            response.exit_code
        );
    }

    parse_remote_env_snapshot_output(&response.output)
        .ok_or_else(|| anyhow::anyhow!("remote environment snapshot markers were not found"))
}

fn remote_env_snapshot_command(shell_path: &str, shell_type: &ShellType) -> String {
    let script = format!(
        "printf '%s\\n' {begin}; env; printf '%s\\n' {end}",
        begin = shell_escape(ENV_SNAPSHOT_BEGIN),
        end = shell_escape(ENV_SNAPSHOT_END)
    );
    format!(
        "{} {} {}",
        shell_escape(shell_path),
        remote_env_snapshot_shell_args(shell_type).join(" "),
        shell_escape(&script)
    )
}

fn remote_env_snapshot_shell_args(shell_type: &ShellType) -> &'static [&'static str] {
    match shell_type {
        ShellType::Bash | ShellType::Zsh => &["-lic"],
        _ => &["-lc"],
    }
}

pub(super) fn parse_remote_env_snapshot_output(output: &str) -> Option<RemoteEnvSnapshot> {
    let mut env = HashMap::new();
    let mut inside = false;
    let mut saw_end = false;

    for raw_line in output.lines() {
        let line = raw_line.trim_end_matches('\r');
        if !inside {
            if line == ENV_SNAPSHOT_BEGIN {
                inside = true;
            }
            continue;
        }

        if line == ENV_SNAPSHOT_END {
            saw_end = true;
            break;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if should_import_env_var(key, value) {
            env.insert(key.to_string(), value.to_string());
        }
    }

    (inside && saw_end).then_some(RemoteEnvSnapshot { env })
}

fn should_import_env_var(key: &str, value: &str) -> bool {
    is_valid_env_var_name(key) && !is_volatile_env_var(key) && !value.contains('\0')
}

fn is_valid_env_var_name(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_volatile_env_var(key: &str) -> bool {
    matches!(
        key,
        "_" | "PWD" | "OLDPWD" | "SHLVL" | "TERM" | "COLUMNS" | "LINES"
    )
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_remote_env_snapshot_output, remote_env_snapshot_command,
        remote_env_snapshot_shell_args,
    };
    use terminal_core::ShellType;

    #[test]
    fn parses_env_snapshot_between_markers() {
        let snapshot = parse_remote_env_snapshot_output(
            "noise\r\n__BITFUN_REMOTE_ENV_SNAPSHOT_BEGIN__\r\nPATH=/home/me/.nvm/bin:/usr/bin\r\nNVM_DIR=/home/me/.nvm\r\nPWD=/tmp\r\nBAD-NAME=value\r\n__BITFUN_REMOTE_ENV_SNAPSHOT_END__\r\nmore noise",
        )
        .expect("snapshot should parse");

        assert_eq!(
            snapshot.env.get("PATH").map(String::as_str),
            Some("/home/me/.nvm/bin:/usr/bin")
        );
        assert_eq!(
            snapshot.env.get("NVM_DIR").map(String::as_str),
            Some("/home/me/.nvm")
        );
        assert!(!snapshot.env.contains_key("PWD"));
        assert!(!snapshot.env.contains_key("BAD-NAME"));
    }

    #[test]
    fn snapshot_command_uses_interactive_login_shell_for_bash() {
        let command = remote_env_snapshot_command("/bin/bash", &ShellType::Bash);

        assert!(command.starts_with("'/bin/bash' -lic "));
        assert!(command.contains("__BITFUN_REMOTE_ENV_SNAPSHOT_BEGIN__"));
        assert!(command.contains("__BITFUN_REMOTE_ENV_SNAPSHOT_END__"));
    }

    #[test]
    fn snapshot_shell_args_are_interactive_only_for_known_interactive_shells() {
        assert_eq!(remote_env_snapshot_shell_args(&ShellType::Bash), &["-lic"]);
        assert_eq!(remote_env_snapshot_shell_args(&ShellType::Zsh), &["-lic"]);
        assert_eq!(remote_env_snapshot_shell_args(&ShellType::Sh), &["-lc"]);
    }
}
