//! SelfControl API — frontend submits responses to self-control requests.

use bitfun_core::agentic::tools::implementations::self_control_tool::{
    submit_self_control_response as submit_self_control_response_impl, SelfControlResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitSelfControlResponseRequest {
    pub request_id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[tauri::command]
pub async fn submit_self_control_response(
    request: SubmitSelfControlResponseRequest,
) -> Result<(), String> {
    let response = SelfControlResponse {
        request_id: request.request_id,
        success: request.success,
        result: request.result,
        error: request.error,
    };

    submit_self_control_response_impl(response)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
