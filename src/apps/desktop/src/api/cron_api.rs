//! Scheduled jobs API.

use bitfun_core::service::cron::{
    get_global_cron_service, CreateCronJobRequest, CronJob, UpdateCronJobRequest,
};
use log::{debug, error};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCronJobsRequest {
    pub workspace_path: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCronJobCommandRequest {
    pub job_id: String,
    #[serde(flatten)]
    pub changes: UpdateCronJobRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCronJobRequest {
    pub job_id: String,
}

fn cron_service() -> Result<std::sync::Arc<bitfun_core::service::cron::CronService>, String> {
    get_global_cron_service().ok_or_else(|| "Cron service is not initialized".to_string())
}

#[tauri::command]
pub async fn list_cron_jobs(request: ListCronJobsRequest) -> Result<Vec<CronJob>, String> {
    debug!(
        "Listing scheduled jobs: workspace_path={:?}, session_id={:?}",
        request.workspace_path, request.session_id
    );

    let service = cron_service()?;
    Ok(service
        .list_jobs_filtered(
            request.workspace_path.as_deref(),
            request.session_id.as_deref(),
        )
        .await)
}

#[tauri::command]
pub async fn create_cron_job(request: CreateCronJobRequest) -> Result<CronJob, String> {
    debug!(
        "Creating scheduled job: name={}, session_id={}, workspace_path={}",
        request.name, request.session_id, request.workspace_path
    );

    let service = cron_service()?;
    service.create_job(request).await.map_err(|error| {
        error!("Failed to create scheduled job: {}", error);
        format!("Failed to create scheduled job: {}", error)
    })
}

#[tauri::command]
pub async fn update_cron_job(request: UpdateCronJobCommandRequest) -> Result<CronJob, String> {
    debug!("Updating scheduled job: job_id={}", request.job_id);

    let service = cron_service()?;
    service
        .update_job(&request.job_id, request.changes)
        .await
        .map_err(|error| {
            error!(
                "Failed to update scheduled job {}: {}",
                request.job_id, error
            );
            format!("Failed to update scheduled job: {}", error)
        })
}

#[tauri::command]
pub async fn delete_cron_job(request: DeleteCronJobRequest) -> Result<bool, String> {
    debug!("Deleting scheduled job: job_id={}", request.job_id);

    let service = cron_service()?;
    service.delete_job(&request.job_id).await.map_err(|error| {
        error!(
            "Failed to delete scheduled job {}: {}",
            request.job_id, error
        );
        format!("Failed to delete scheduled job: {}", error)
    })
}
