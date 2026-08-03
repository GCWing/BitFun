//! Read-only Appearance marketplace commands and reviewed package download.
//!
//! Package installation remains in the WebView-owned Appearance runtime. The
//! desktop boundary fetches public metadata and returns reviewed ZIP bytes as
//! a raw IPC response after enforcing release identity, version, size and hash.

use bitfun_product_domains::appearance_market::{
    validate_appearance_market_slug, AppearanceAdminSubmissionDetail, AppearanceCursorPage,
    AppearanceMarketListingDetail, AppearanceMarketListingSummary, AppearanceMarketRelease,
    AppearanceMarketSubmission, AppearanceMarketSubmissionStatus, AppearanceReviewDecision,
    AppearanceReviewDecisionRequest, APPEARANCE_MARKET_MAX_PACKAGE_BYTES,
};
use bitfun_services_integrations::appearance_market::{
    AppearanceMarketBrowseRequest, AppearanceMarketClient,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketSlugRequest {
    pub slug: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketDownloadRequest {
    pub slug: String,
    pub release_number: u32,
    pub package_id: String,
    pub package_version: String,
    pub package_sha256: String,
    pub package_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketSubmissionIdRequest {
    pub submission_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketReviewRequest {
    pub submission_id: String,
    pub decision: AppearanceReviewDecision,
    #[serde(default)]
    pub reason: String,
}

#[tauri::command]
pub async fn appearance_market_browse(
    request: AppearanceMarketBrowseRequest,
) -> Result<AppearanceCursorPage<AppearanceMarketListingSummary>, String> {
    let client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    client.browse(&request).await.map_err(market_error)
}

#[tauri::command]
pub async fn appearance_market_get_listing(
    request: AppearanceMarketSlugRequest,
) -> Result<AppearanceMarketListingDetail, String> {
    validate_slug(&request.slug)?;
    let client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    client.listing(&request.slug).await.map_err(market_error)
}

#[tauri::command]
pub async fn appearance_market_download_release(
    request: AppearanceMarketDownloadRequest,
) -> Result<tauri::ipc::Response, String> {
    validate_slug(&request.slug)?;
    let client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    let detail = client.listing(&request.slug).await.map_err(market_error)?;
    let release = find_release(&detail, request.release_number)?;
    if release.listing_id != detail.summary.listing_id
        || detail.summary.package_id != request.package_id
        || release.package_version != request.package_version
        || release.package_sha256 != request.package_sha256
        || release.package_size != request.package_size
    {
        return Err(
            "The Appearance release changed after it was opened. Refresh Skin Market and try again."
                .to_string(),
        );
    }
    validate_minimum_bitfun_version(&release.min_bitfun_version)?;
    if release.yanked {
        return Err("This Appearance release has been yanked and cannot be installed.".to_string());
    }
    if release.package_size > APPEARANCE_MARKET_MAX_PACKAGE_BYTES {
        return Err("The reviewed Appearance package exceeds the 96 MiB limit.".to_string());
    }

    let bytes = client
        .download_release(&detail.summary.slug, release.release_number)
        .await
        .map_err(market_error)?;
    verify_downloaded_package(&bytes, release.package_size, &release.package_sha256)?;

    // `tauri::ipc::Response` preserves a raw binary response. Returning Vec<u8>
    // directly would otherwise be serialized as a large JSON number array.
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn appearance_market_list_submissions() -> Result<Vec<AppearanceMarketSubmission>, String>
{
    let mut client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    client.list_submissions().await.map_err(market_error)
}

#[tauri::command]
pub async fn appearance_market_withdraw_submission(
    request: AppearanceMarketSubmissionIdRequest,
) -> Result<AppearanceMarketSubmission, String> {
    let mut client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    client
        .withdraw_submission(&request.submission_id)
        .await
        .map_err(market_error)
}

#[tauri::command]
pub async fn appearance_market_list_review_submissions(
) -> Result<Vec<AppearanceMarketSubmission>, String> {
    let mut client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    client
        .list_admin_submissions(AppearanceMarketSubmissionStatus::Submitted)
        .await
        .map_err(market_error)
}

#[tauri::command]
pub async fn appearance_market_get_review_submission(
    request: AppearanceMarketSubmissionIdRequest,
) -> Result<AppearanceAdminSubmissionDetail, String> {
    let mut client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    client
        .admin_submission(&request.submission_id)
        .await
        .map_err(market_error)
}

#[tauri::command]
pub async fn appearance_market_review_submission(
    request: AppearanceMarketReviewRequest,
) -> Result<AppearanceAdminSubmissionDetail, String> {
    let mut client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    client
        .review_submission(
            &request.submission_id,
            &AppearanceReviewDecisionRequest {
                decision: request.decision,
                reason: request.reason,
            },
        )
        .await
        .map_err(market_error)
}

fn validate_slug(slug: &str) -> Result<(), String> {
    if validate_appearance_market_slug(slug) {
        Ok(())
    } else {
        Err("Invalid Appearance market listing slug.".to_string())
    }
}

fn find_release(
    detail: &AppearanceMarketListingDetail,
    release_number: u32,
) -> Result<&AppearanceMarketRelease, String> {
    detail
        .releases
        .iter()
        .find(|release| release.release_number == release_number)
        .ok_or_else(|| "Appearance market release not found.".to_string())
}

fn validate_minimum_bitfun_version(minimum: &str) -> Result<(), String> {
    let minimum = semver::Version::parse(minimum)
        .map_err(|_| "The release declares an invalid minimum BitFun version.".to_string())?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|_| "The current BitFun version is invalid.".to_string())?;
    if current < minimum {
        return Err(format!(
            "This Appearance requires BitFun {minimum} or newer. Current version: {current}."
        ));
    }
    Ok(())
}

fn verify_downloaded_package(
    bytes: &[u8],
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), String> {
    let actual_size = bytes.len() as u64;
    if actual_size > APPEARANCE_MARKET_MAX_PACKAGE_BYTES {
        return Err("The downloaded Appearance package exceeds the 96 MiB limit.".to_string());
    }
    if actual_size != expected_size {
        return Err(
            "The downloaded Appearance package size does not match the reviewed release."
                .to_string(),
        );
    }
    let actual_sha256 = format!("{:x}", Sha256::digest(bytes));
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(
            "The downloaded Appearance package hash does not match the reviewed release."
                .to_string(),
        );
    }
    Ok(())
}

fn market_error(error: impl Serialize + std::fmt::Display) -> String {
    serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_reviewed_download_size_and_hash() {
        let bytes = b"reviewed appearance";
        let sha256 = format!("{:x}", Sha256::digest(bytes));
        assert!(verify_downloaded_package(bytes, bytes.len() as u64, &sha256).is_ok());
        assert!(verify_downloaded_package(bytes, 1, &sha256)
            .unwrap_err()
            .contains("size"));
        assert!(
            verify_downloaded_package(bytes, bytes.len() as u64, &"0".repeat(64))
                .unwrap_err()
                .contains("hash")
        );
    }

    #[test]
    fn rejects_invalid_slugs_before_network_access() {
        assert!(validate_slug("tokyo-night").is_ok());
        assert!(validate_slug("../secret").is_err());
    }
}
