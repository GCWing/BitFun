use bitfun_core::external_sources::{
    external_source_read_only_snapshot, get_external_source_control_snapshot,
    ExternalSourceHostCapabilities, ExternalSourceOperationError, ExternalSourceOperationResult,
};
use std::path::PathBuf;

use crate::AppState;

pub(crate) fn supports(method: &str) -> bool {
    matches!(
        method,
        "get_external_source_snapshot"
            | "get_external_source_control_snapshot"
            | "reveal_external_source_location"
            | "apply_external_source_control_action_command"
            | "set_external_source_enabled_command"
            | "set_external_source_conflict_choice_command"
            | "set_external_tool_target_decision_command"
            | "set_external_tool_conflict_choice_command"
            | "set_external_subagent_activation_command"
            | "choose_external_subagent_conflict_command"
            | "set_external_mcp_server_decision_command"
            | "choose_external_mcp_conflict_command"
            | "update_external_integration_policy_command"
    )
}

pub(crate) async fn dispatch(
    method: &str,
    params: serde_json::Value,
    state: &AppState,
) -> ExternalSourceOperationResult<serde_json::Value> {
    if !matches!(
        method,
        "get_external_source_snapshot" | "get_external_source_control_snapshot"
    ) {
        return Err(ExternalSourceOperationError::host_capability_unavailable(
            if supports(method) {
                "This Server Host exposes external integrations as read-only. Use an authenticated Desktop or Peer Host to change them."
            } else {
                "Unknown external source operation"
            },
        ));
    }
    let request = params
        .get("request")
        .ok_or_else(|| ExternalSourceOperationError::invalid_request("missing request"))?;
    let workspace = external_workspace_root(state, request)?;
    let workspace = workspace.as_deref();
    match method {
        "get_external_source_snapshot" => {
            let force_refresh = optional_bool_field(request, "forceRefresh")?;
            let snapshot = external_source_read_only_snapshot(workspace, force_refresh)
                .await
                .map_err(bitfun_core::external_sources::sanitize_external_source_operation_error)?;
            serde_json::to_value(snapshot.into_legacy_v0_compatible())
        }
        "get_external_source_control_snapshot" => {
            let force_refresh = optional_bool_field(request, "forceRefresh")?;
            let snapshot = get_external_source_control_snapshot(
                workspace,
                force_refresh,
                ExternalSourceHostCapabilities::read_only_projection(),
            )
            .await?;
            serde_json::to_value(snapshot)
        }
        _ => unreachable!("write and unknown methods are rejected before request parsing"),
    }
    .map_err(|_| {
        ExternalSourceOperationError::new(
            bitfun_core::external_sources::ExternalSourceOperationErrorCode::Internal,
            "External source response could not be encoded",
            false,
        )
    })
}

fn external_workspace_root(
    state: &AppState,
    request: &serde_json::Value,
) -> ExternalSourceOperationResult<Option<PathBuf>> {
    let workspace = match request.get("workspacePath") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(path)) if !path.trim().is_empty() => {
            Some(PathBuf::from(path))
        }
        _ => {
            return Err(ExternalSourceOperationError::invalid_request(
                "workspacePath must be a non-empty absolute path when provided",
            ))
        }
    };
    if workspace.as_ref().is_some_and(|path| !path.is_absolute()) {
        return Err(ExternalSourceOperationError::invalid_request(
            "External sources require an absolute workspace path",
        ));
    }
    let Some(requested) = workspace else {
        return Ok(None);
    };
    let owned = state.external_workspace_root.as_ref().ok_or_else(|| {
        ExternalSourceOperationError::new(
            bitfun_core::external_sources::ExternalSourceOperationErrorCode::HostUnavailable,
            "The Server Host has no project workspace",
            false,
        )
    })?;
    let requested = requested.canonicalize().map_err(|_| {
        ExternalSourceOperationError::invalid_request(
            "Workspace path is not available on this Host",
        )
    })?;
    if &requested != owned {
        return Err(ExternalSourceOperationError::invalid_request(
            "External compatibility is limited to the Server Host workspace",
        ));
    }
    Ok(Some(requested))
}

fn optional_bool_field(
    request: &serde_json::Value,
    key: &str,
) -> ExternalSourceOperationResult<bool> {
    match request.get(key) {
        None | Some(serde_json::Value::Null) => Ok(false),
        Some(serde_json::Value::Bool(value)) => Ok(*value),
        _ => Err(ExternalSourceOperationError::invalid_request(format!(
            "'{key}' must be a boolean when provided"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_state(external_workspace_root: Option<PathBuf>) -> AppState {
        AppState {
            external_workspace_root,
            allowed_browser_origins: Default::default(),
        }
    }

    #[test]
    fn only_external_source_methods_are_claimed() {
        assert!(supports("get_external_source_snapshot"));
        assert!(supports("get_external_source_control_snapshot"));
        assert!(supports("apply_external_source_control_action_command"));
        assert!(supports("update_external_integration_policy_command"));
        assert!(!supports("open_workspace"));
    }

    #[test]
    fn workspace_paths_must_be_absolute() {
        let state = app_state(None);
        let request = serde_json::json!({ "workspacePath": "relative/project" });
        let error = external_workspace_root(&state, &request).unwrap_err();
        assert_eq!(error.code.as_str(), "invalid_request");
    }

    #[test]
    fn project_paths_require_an_owned_server_workspace() {
        let state = app_state(None);
        let workspace = std::env::current_dir().expect("current directory is available");
        let request = serde_json::json!({ "workspacePath": workspace });
        let error = external_workspace_root(&state, &request).unwrap_err();
        assert_eq!(error.code.as_str(), "host_unavailable");
    }

    #[test]
    fn project_paths_must_match_the_owned_server_workspace() {
        let workspace = std::env::current_dir()
            .expect("current directory is available")
            .canonicalize()
            .expect("current directory can be canonicalized");
        let state = app_state(Some(workspace.clone()));
        let request = serde_json::json!({ "workspacePath": workspace });
        assert_eq!(
            external_workspace_root(&state, &request).unwrap(),
            Some(workspace)
        );
    }

    #[test]
    fn malformed_optional_values_are_rejected() {
        let request = serde_json::json!({ "forceRefresh": "false" });
        let error = optional_bool_field(&request, "forceRefresh").unwrap_err();
        assert_eq!(error.code.as_str(), "invalid_request");

        let state = app_state(None);
        let request = serde_json::json!({ "workspacePath": false });
        let error = external_workspace_root(&state, &request).unwrap_err();
        assert_eq!(error.code.as_str(), "invalid_request");
    }

    #[tokio::test]
    async fn writes_are_rejected_before_request_or_workspace_parsing() {
        let state = app_state(None);
        let error = dispatch(
            "set_external_source_enabled_command",
            serde_json::json!({ "malformed": true }),
            &state,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_str(), "host_capability_unavailable");

        let error = dispatch(
            "apply_external_source_control_action_command",
            serde_json::json!({ "malformed": true }),
            &state,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_str(), "host_capability_unavailable");

        let error = dispatch(
            "reveal_external_source_location",
            serde_json::json!({ "malformed": true }),
            &state,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_str(), "host_capability_unavailable");
    }
}
