use super::shell_kind::exec_command_shell_kind;
use crate::service::remote_ssh::{
    get_global_remote_exec_process_manager, RemoteExecCommandRequest, RemoteExecControlAction,
    RemoteExecControlOrigin, RemoteExecControlRequest, SSHConnectionManager,
};
use std::sync::OnceLock;
use terminal_core::ShellType;
use tool_runtime::exec_command::{
    parse_remote_exec_env_snapshot_output, remote_exec_env_snapshot_capture_policy,
    remote_exec_env_snapshot_command, ExecCommandRemoteEnvSnapshot,
    ExecCommandRemoteEnvSnapshotCache, ExecCommandRemoteEnvSnapshotCacheKey,
};

static REMOTE_ENV_SNAPSHOT_CACHE: OnceLock<ExecCommandRemoteEnvSnapshotCache> = OnceLock::new();

pub(super) type RemoteEnvSnapshot = ExecCommandRemoteEnvSnapshot;

pub(super) async fn remote_env_snapshot_for(
    ssh_manager: SSHConnectionManager,
    connection_id: &str,
    shell_path: &str,
    shell_type: &ShellType,
) -> Option<RemoteEnvSnapshot> {
    let key = ExecCommandRemoteEnvSnapshotCacheKey::new(
        connection_id,
        shell_path,
        shell_type.to_string(),
    );

    let cache = REMOTE_ENV_SNAPSHOT_CACHE.get_or_init(ExecCommandRemoteEnvSnapshotCache::default);
    if let Some(snapshot) = cache.get(&key).await {
        return Some(snapshot);
    }

    let snapshot =
        match capture_remote_env_snapshot(ssh_manager, connection_id, shell_path, shell_type).await
        {
            Ok(snapshot) => snapshot,
            Err(_) => return None,
        };
    cache.insert(key, snapshot.clone()).await;
    Some(snapshot)
}

async fn capture_remote_env_snapshot(
    ssh_manager: SSHConnectionManager,
    connection_id: &str,
    shell_path: &str,
    shell_type: &ShellType,
) -> anyhow::Result<RemoteEnvSnapshot> {
    let command = remote_env_snapshot_command(shell_path, shell_type);
    let manager = get_global_remote_exec_process_manager();
    let policy = remote_exec_env_snapshot_capture_policy();
    let response = manager
        .exec_command(RemoteExecCommandRequest {
            ssh_manager,
            connection_id: connection_id.to_string(),
            command,
            tty: true,
            yield_time_ms: Some(policy.timeout_ms),
            max_output_chars: Some(policy.max_output_chars),
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
                yield_time_ms: Some(policy.stale_session_control_yield_time_ms),
                max_output_chars: Some(policy.stale_session_control_max_output_chars),
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
    remote_exec_env_snapshot_command(shell_path, exec_command_shell_kind(shell_type))
}

#[cfg(test)]
fn remote_env_snapshot_shell_args(shell_type: &ShellType) -> &'static [&'static str] {
    tool_runtime::exec_command::remote_exec_env_snapshot_shell_args(exec_command_shell_kind(
        shell_type,
    ))
}

pub(super) fn parse_remote_env_snapshot_output(output: &str) -> Option<RemoteEnvSnapshot> {
    parse_remote_exec_env_snapshot_output(output)
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
