//! Git HostInvoke handlers for CLI Peer Host.

use serde_json::{json, Value};

use openbitfun_core::service::git::GitService;

use crate::peer_host::args::{get_string, request_value};

pub(crate) async fn git_is_repository(args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let repository_path = get_string(request, "repositoryPath")?;
    let is_repo = GitService::is_repository(&repository_path)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check Git repository: path={repository_path}, error={e}");
            format!("Failed to check Git repository: {e}")
        })?;
    Ok(json!(is_repo))
}

/// Read-only ownership-trust probe.
///
/// Granting trust writes to the host user's global Git configuration, so that
/// decision stays on surfaces where the user is at the machine; a controller
/// only gets the diagnosis and the manual command.
pub(crate) async fn git_get_repository_trust(args: &Value) -> Result<Value, String> {
    let request = request_value(args);
    let repository_path = get_string(request, "repositoryPath")?;
    let report = GitService::inspect_trust(&repository_path)
        .await
        .map_err(|e| {
            tracing::error!(
                "Failed to inspect Git repository trust: path={repository_path}, error={e}"
            );
            format!("Failed to inspect Git repository trust: {e}")
        })?;
    serde_json::to_value(report).map_err(|e| format!("Failed to serialize trust report: {e}"))
}
