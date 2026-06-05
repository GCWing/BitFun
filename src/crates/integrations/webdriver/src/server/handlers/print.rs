use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::executor::BridgeExecutor;
use crate::platform::PrintOptions;
use crate::server::response::{WebDriverResponse, WebDriverResult};
use crate::server::AppState;

pub async fn print(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<PrintOptions>,
) -> WebDriverResult {
    let executor = BridgeExecutor::from_session_id(state, &session_id).await?;
    let pdf = executor.print_page(request).await?;
    Ok(WebDriverResponse::success(pdf))
}
