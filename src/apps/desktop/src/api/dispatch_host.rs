//! Target-side detached-dispatch adapter for account device RPC.
//!
//! Desktop deliberately delegates execution to the same `openbitfun dispatch`
//! runner used by SSH and CLI Peer Host. This keeps one durable job/session
//! owner and avoids creating a second desktop-only dispatch implementation.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context};
use serde_json::Value;
use tokio::io::AsyncWriteExt;

const TARGET_COMMAND_TIMEOUT: Duration = Duration::from_secs(110);
const MAX_TARGET_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) async fn dispatch(command: &str, args: Value) -> anyhow::Result<Value> {
    let verb = target_cli_verb(command)
        .ok_or_else(|| anyhow!("Unknown detached dispatch target command '{command}'"))?;
    let executable = discover_cli().ok_or_else(|| {
        anyhow!(
            "OpenBitFun CLI dispatch runner is not installed on this device; install `openbitfun` in ~/.local/bin or PATH"
        )
    })?;
    invoke_cli(&executable, verb, args).await
}

pub(crate) fn is_target_command(command: &str) -> bool {
    target_cli_verb(command).is_some()
}

fn target_cli_verb(command: &str) -> Option<&'static str> {
    match command {
        "dispatch_target_probe" => Some("probe"),
        "dispatch_target_submit" => Some("submit"),
        "dispatch_target_status" => Some("status"),
        "dispatch_target_cancel" => Some("cancel"),
        "dispatch_target_list" => Some("list"),
        "dispatch_target_answer" => Some("answer"),
        "dispatch_target_append" => Some("append"),
        "dispatch_target_continue" => Some("continue"),
        "dispatch_target_query" => Some("query"),
        "dispatch_target_workspace_provision" => Some("__workspace_provision"),
        "dispatch_target_workspace_bundle_begin" => Some("__workspace_bundle_begin"),
        "dispatch_target_workspace_bundle_chunk" => Some("__workspace_bundle_chunk"),
        "dispatch_target_workspace_bundle_commit" => Some("__workspace_bundle_commit"),
        "dispatch_target_workspace_sync" => Some("__workspace_sync"),
        "dispatch_target_workspace_sync_chunk" => Some("__workspace_sync_chunk"),
        _ => None,
    }
}

async fn invoke_cli(executable: &Path, verb: &str, args: Value) -> anyhow::Result<Value> {
    let request = serde_json::to_vec(&args).context("serialize target dispatch request")?;
    let mut child = openbitfun_core::util::process_manager::create_tokio_command(executable)
        .arg("dispatch")
        .arg(verb)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("start dispatch runner {}", executable.display()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Dispatch runner stdin was unavailable"))?;
    stdin
        .write_all(&request)
        .await
        .context("write target dispatch request")?;
    stdin.shutdown().await?;
    drop(stdin);

    let output = tokio::time::timeout(TARGET_COMMAND_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| anyhow!("Target dispatch command timed out"))?
        .context("wait for target dispatch command")?;
    if output.stdout.len() > MAX_TARGET_RESPONSE_BYTES
        || output.stderr.len() > MAX_TARGET_RESPONSE_BYTES
    {
        anyhow::bail!("Target dispatch response exceeded the 4 MiB safety limit");
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .context("Target dispatch runner returned non-UTF-8 output")?
        .trim();
    let response: Value = serde_json::from_str(stdout).with_context(|| {
        format!(
            "Target dispatch runner returned invalid JSON: {}",
            bounded_text(stdout)
        )
    })?;
    if !output.status.success() {
        let message = response
            .get("error")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| String::from_utf8_lossy(&output.stderr).trim().to_string());
        anyhow::bail!(
            "{}",
            if message.is_empty() {
                "Target dispatch runner failed".to_string()
            } else {
                message
            }
        );
    }
    Ok(response)
}

fn discover_cli() -> Option<PathBuf> {
    let executable_name = if cfg!(windows) {
        "openbitfun.exe"
    } else {
        "openbitfun"
    };
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local").join("bin").join(executable_name));
    }
    if let Ok(current) = std::env::current_exe() {
        if let Some(parent) = current.parent() {
            candidates.push(parent.join(executable_name));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates
            .extend(std::env::split_paths(&path).map(|directory| directory.join(executable_name)));
    }
    candidates.into_iter().find_map(resolve_cli_candidate)
}

fn resolve_cli_candidate(candidate: PathBuf) -> Option<PathBuf> {
    let canonical = candidate.canonicalize().ok()?;
    std::fs::symlink_metadata(&canonical)
        .is_ok_and(|metadata| !metadata.file_type().is_symlink() && metadata.is_file())
        .then_some(canonical)
}

fn bounded_text(value: &str) -> String {
    const MAX_CHARS: usize = 240;
    let mut result = value.chars().take(MAX_CHARS).collect::<String>();
    if value.chars().count() > MAX_CHARS {
        result.push_str("...");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbound_controller_commands_are_not_target_commands() {
        assert!(is_target_command("dispatch_target_status"));
        assert!(!is_target_command("dispatch_status"));
        assert!(!is_target_command("account_device_rpc"));
    }

    /// The registry marks every `dispatch_target_*` name as host control plane
    /// (routed to the dispatch runner before the peer bridge); this table must
    /// answer exactly that family, no more and no less.
    #[test]
    fn target_command_family_matches_registry() {
        use openbitfun_product_domains::remote_surface::{operations, PeerStance};
        for op in operations() {
            let registry_says_target = op.id.starts_with("dispatch_target_");
            assert_eq!(
                is_target_command(op.id),
                registry_says_target,
                "{} target-command classification disagrees with the registry",
                op.id
            );
            if registry_says_target {
                assert_eq!(op.peer, PeerStance::HostControlPlane, "{}", op.id);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn cli_discovery_accepts_a_symlink_to_a_regular_binary() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let binary = temp.path().join("openbitfun-real");
        std::fs::write(&binary, b"binary").expect("binary");
        let link = temp.path().join("openbitfun");
        symlink(&binary, &link).expect("symlink");

        assert_eq!(
            resolve_cli_candidate(link),
            Some(binary.canonicalize().expect("canonical binary"))
        );
    }
}
