use bitfun_core::agentic::insights::{InsightsReport, InsightsReportMeta, InsightsService};
use log::{error, info};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateInsightsRequest {
    pub days: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadInsightsReportRequest {
    pub path: String,
}

#[tauri::command]
pub async fn generate_insights(
    request: GenerateInsightsRequest,
) -> Result<InsightsReport, String> {
    let days = request.days.unwrap_or(30);
    info!("Generating insights for the last {} days", days);

    InsightsService::generate(days)
        .await
        .map_err(|e| {
            error!("Failed to generate insights: {}", e);
            format!("Failed to generate insights: {}", e)
        })
}

#[tauri::command]
pub async fn get_latest_insights() -> Result<Vec<InsightsReportMeta>, String> {
    InsightsService::load_latest_reports().await.map_err(|e| {
        error!("Failed to load latest insights: {}", e);
        format!("Failed to load latest insights: {}", e)
    })
}

#[tauri::command]
pub async fn load_insights_report(
    request: LoadInsightsReportRequest,
) -> Result<InsightsReport, String> {
    InsightsService::load_report(&request.path).await.map_err(|e| {
        error!("Failed to load insights report: {}", e);
        format!("Failed to load insights report: {}", e)
    })
}

#[tauri::command]
pub async fn has_insights_data(
    request: GenerateInsightsRequest,
) -> Result<bool, String> {
    let days = request.days.unwrap_or(30);
    InsightsService::has_data(days).await.map_err(|e| {
        error!("Failed to check insights data: {}", e);
        format!("Failed to check insights data: {}", e)
    })
}

#[tauri::command]
pub async fn cancel_insights_generation() -> Result<(), String> {
    InsightsService::cancel().await.map_err(|e| {
        error!("Failed to cancel insights generation: {}", e);
        e
    })
}
