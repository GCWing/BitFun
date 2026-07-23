//! Git HostInvoke handlers for CLI Peer Host.

use serde_json::{json, Value};

use bitfun_core::service::git::GitService;

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
