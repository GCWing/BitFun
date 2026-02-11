/// HTTP API routes
///
/// Provides RESTful API endpoints
use axum::{extract::State, Json};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct ApiInfo {
    pub name: String,
    pub version: String,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Serialize)]
pub struct EndpointInfo {
    pub path: String,
    pub method: String,
    pub description: String,
}

/// API info endpoint
pub async fn api_info(State(_state): State<AppState>) -> Json<ApiInfo> {
    Json(ApiInfo {
        name: "BitFun Server API".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        endpoints: vec![
            EndpointInfo {
                path: "/health".to_string(),
                method: "GET".to_string(),
                description: "Health check".to_string(),
            },
            EndpointInfo {
                path: "/api/v1/info".to_string(),
                method: "GET".to_string(),
                description: "API info".to_string(),
            },
            EndpointInfo {
                path: "/ws".to_string(),
                method: "WebSocket".to_string(),
                description: "WebSocket connection".to_string(),
            },
        ],
    })
}
