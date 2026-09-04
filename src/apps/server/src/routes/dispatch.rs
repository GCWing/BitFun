//! Narrow detached-dispatch capability for the loopback Server Host.
//!
//! This route owns no Agent Runtime and no target session. It only exposes the
//! same platform-neutral controller used by Desktop, backed by saved SSH
//! profiles and the observer-only outbound index.

use openbitfun_core::external_sources::{
    ExternalSourceOperationError, ExternalSourceOperationErrorCode, ExternalSourceOperationResult,
};
use openbitfun_core::service::dispatch::{
    answer_dispatch, append_dispatch, cancel_dispatch, cancel_dispatch_cli_install,
    get_dispatch_status, list_dispatch_jobs, list_dispatch_targets, poll_dispatch_cli_install,
    probe_dispatch_target, start_dispatch_cli_install, submit_dispatch, sync_dispatch_model_config,
    sync_dispatch_result, DispatchAnswerRequest, DispatchAppendRequest, DispatchConnectionRequest,
    DispatchInstallPollRequest, DispatchInstallStartRequest, DispatchJobRequest,
    DispatchListJobsRequest, DispatchListTargetsRequest, DispatchProbeTargetRequest,
    DispatchStatusRequest, DispatchSubmitRequest, DispatchSyncResultRequest, OutboundDispatchStore,
};
use serde::de::DeserializeOwned;

use crate::{AppState, DispatchHostState};

pub(crate) fn supports(method: &str) -> bool {
    matches!(
        method,
        "dispatch_list_targets"
            | "dispatch_probe_target"
            | "dispatch_install_cli_start"
            | "dispatch_install_cli_poll"
            | "dispatch_install_cli_cancel"
            | "dispatch_sync_model_config"
            | "dispatch_submit"
            | "dispatch_status"
            | "dispatch_sync_result"
            | "dispatch_cancel"
            | "dispatch_list_jobs"
            | "dispatch_answer"
            | "dispatch_append"
    )
}

pub(crate) async fn dispatch(
    method: &str,
    params: serde_json::Value,
    state: &AppState,
) -> ExternalSourceOperationResult<serde_json::Value> {
    let host = state.dispatch_host.as_deref().ok_or_else(|| {
        ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::HostUnavailable,
            "Detached dispatch is not initialized on this Server Host",
            false,
        )
    })?;

    match method {
        "dispatch_list_targets" => {
            let request = parse_request::<DispatchListTargetsRequest>(&params)?;
            encode(
                list_dispatch_targets(&host.ssh_manager, request)
                    .await
                    .map_err(operation_error)?,
            )
        }
        "dispatch_probe_target" => {
            let request = parse_request::<DispatchProbeTargetRequest>(&params)?;
            encode(
                probe_dispatch_target(&host.ssh_manager, request)
                    .await
                    .map_err(operation_error)?,
            )
        }
        "dispatch_install_cli_start" => {
            let request = parse_request::<DispatchInstallStartRequest>(&params)?;
            encode(
                start_dispatch_cli_install(&host.ssh_manager, request)
                    .await
                    .map_err(operation_error)?,
            )
        }
        "dispatch_install_cli_poll" => {
            let request = parse_request::<DispatchInstallPollRequest>(&params)?;
            encode(
                poll_dispatch_cli_install(&host.ssh_manager, request)
                    .await
                    .map_err(operation_error)?,
            )
        }
        "dispatch_install_cli_cancel" => {
            let request = parse_request::<DispatchConnectionRequest>(&params)?;
            cancel_dispatch_cli_install(&host.ssh_manager, request)
                .await
                .map_err(operation_error)?;
            Ok(serde_json::Value::Null)
        }
        "dispatch_sync_model_config" => {
            let request = parse_request::<DispatchConnectionRequest>(&params)?;
            sync_dispatch_model_config(&host.ssh_manager, request)
                .await
                .map_err(operation_error)?;
            Ok(serde_json::Value::Null)
        }
        "dispatch_submit" => {
            let request = parse_request::<DispatchSubmitRequest>(&params)?;
            submit_dispatch(&host.ssh_manager, &store(host), request)
                .await
                .map_err(operation_error)
        }
        "dispatch_status" => {
            let request = parse_request::<DispatchStatusRequest>(&params)?;
            get_dispatch_status(&host.ssh_manager, &store(host), request)
                .await
                .map_err(operation_error)
        }
        "dispatch_sync_result" => {
            let request = parse_request::<DispatchSyncResultRequest>(&params)?;
            sync_dispatch_result(&host.ssh_manager, &store(host), request)
                .await
                .map_err(operation_error)
        }
        "dispatch_cancel" => {
            let request = parse_request::<DispatchJobRequest>(&params)?;
            cancel_dispatch(&host.ssh_manager, &store(host), request)
                .await
                .map_err(operation_error)
        }
        "dispatch_list_jobs" => {
            let request = parse_request::<DispatchListJobsRequest>(&params)?;
            list_dispatch_jobs(&host.ssh_manager, &store(host), request)
                .await
                .map_err(operation_error)
        }
        "dispatch_answer" => {
            let request = parse_request::<DispatchAnswerRequest>(&params)?;
            answer_dispatch(&host.ssh_manager, &store(host), request)
                .await
                .map_err(operation_error)
        }
        "dispatch_append" => {
            let request = parse_request::<DispatchAppendRequest>(&params)?;
            append_dispatch(&host.ssh_manager, &store(host), request)
                .await
                .map_err(operation_error)
        }
        _ => Err(ExternalSourceOperationError::host_capability_unavailable(
            "Unknown detached dispatch operation",
        )),
    }
}

fn store(host: &DispatchHostState) -> OutboundDispatchStore {
    OutboundDispatchStore::new(&host.path_manager)
}

fn parse_request<T: DeserializeOwned>(
    params: &serde_json::Value,
) -> ExternalSourceOperationResult<T> {
    let request = params.get("request").ok_or_else(|| {
        ExternalSourceOperationError::invalid_request("missing structured request")
    })?;
    serde_json::from_value(request.clone()).map_err(|error| {
        ExternalSourceOperationError::invalid_request(format!(
            "invalid detached dispatch request: {error}"
        ))
    })
}

fn encode(value: impl serde::Serialize) -> ExternalSourceOperationResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|_| {
        ExternalSourceOperationError::new(
            ExternalSourceOperationErrorCode::Internal,
            "Detached dispatch response could not be encoded",
            false,
        )
    })
}

fn operation_error(error: anyhow::Error) -> ExternalSourceOperationError {
    ExternalSourceOperationError::new(
        ExternalSourceOperationErrorCode::DependencyFailed,
        error.to_string(),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_only_the_narrow_dispatch_contract() {
        for method in [
            "dispatch_list_targets",
            "dispatch_probe_target",
            "dispatch_install_cli_start",
            "dispatch_install_cli_poll",
            "dispatch_install_cli_cancel",
            "dispatch_sync_model_config",
            "dispatch_submit",
            "dispatch_status",
            "dispatch_sync_result",
            "dispatch_cancel",
            "dispatch_list_jobs",
            "dispatch_answer",
            "dispatch_append",
        ] {
            assert!(supports(method), "{method}");
        }
        assert!(!supports("start_dialog_turn"));
        assert!(!supports("account_execute_on_device"));
    }

    #[test]
    fn structured_requests_are_required() {
        let error = parse_request::<DispatchJobRequest>(&serde_json::json!({})).unwrap_err();
        assert_eq!(error.code, ExternalSourceOperationErrorCode::InvalidRequest);
    }

    #[test]
    fn connection_scoped_routes_accept_a_structured_connection_request() {
        let request = parse_request::<DispatchConnectionRequest>(&serde_json::json!({
            "request": { "connectionId": "ssh-target" }
        }))
        .unwrap();

        assert_eq!(request.connection_id, "ssh-target");
    }
}
