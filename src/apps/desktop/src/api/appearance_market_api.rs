//! Appearance marketplace commands, reviewed package download and explicit
//! local-package submission.
//!
//! Package installation remains in the WebView-owned Appearance runtime. The
//! desktop boundary fetches public metadata and returns reviewed ZIP bytes as
//! a raw IPC response after enforcing release identity, version, size and hash.
//! Manual submission receives only an absolute controller-device path and
//! delegates package validation and upload orchestration to the shared service.

use bitfun_product_domains::appearance_market::{
    validate_appearance_market_slug, AppearanceAdminSubmissionDetail, AppearanceCursorPage,
    AppearanceMarketLicense, AppearanceMarketListingDetail, AppearanceMarketListingSummary,
    AppearanceMarketRelease, AppearanceMarketSubmission, AppearanceMarketSubmissionDraftRequest,
    AppearanceMarketSubmissionStatus, AppearanceReviewDecision, AppearanceReviewDecisionRequest,
    APPEARANCE_MARKET_MAX_PACKAGE_BYTES,
};
use bitfun_services_integrations::appearance_market::{
    resolve_appearance_release_target, submit_appearance_package, suggest_appearance_slug,
    AppearanceMarketBrowseRequest, AppearanceMarketClient, AppearanceReleaseTarget,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketSubmitPackageRequest {
    pub package_path: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub min_bitfun_version: String,
    #[serde(default)]
    pub changelog: String,
    pub license: AppearanceMarketLicense,
    #[serde(default)]
    pub repository_url: Option<String>,
}

struct NormalizedManualSubmission {
    package_path: PathBuf,
    slug: String,
    min_bitfun_version: String,
    changelog: String,
    license: AppearanceMarketLicense,
    repository_url: Option<String>,
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
pub async fn appearance_market_submit_package(
    request: AppearanceMarketSubmitPackageRequest,
) -> Result<AppearanceMarketSubmission, String> {
    let normalized = normalize_manual_submission(request)?;
    let mut client = AppearanceMarketClient::from_environment()
        .await
        .map_err(market_error)?;
    let submissions = client.list_submissions().await.map_err(market_error)?;
    let (listing_id, release_number) = match resolve_appearance_release_target(
        &submissions,
        &normalized.slug,
    ) {
        AppearanceReleaseTarget::NewListing => (None, 1),
        AppearanceReleaseTarget::ExistingListing {
            listing_id,
            next_release,
        } => (Some(listing_id), next_release),
        AppearanceReleaseTarget::PendingReview {
            submission_id,
            release_number,
        } => {
            return Err(format!(
                "Skin '{}' release {release_number} is already under review (submission {submission_id}). Withdraw it before uploading another release.",
                normalized.slug
            ));
        }
    };
    let changelog = if normalized.changelog.is_empty() {
        if release_number == 1 {
            "Initial release.".to_string()
        } else {
            "General updates and improvements.".to_string()
        }
    } else {
        normalized.changelog
    };
    let draft = AppearanceMarketSubmissionDraftRequest {
        listing_id,
        slug: normalized.slug,
        release_number,
        min_bitfun_version: normalized.min_bitfun_version,
        changelog,
        license: normalized.license,
        repository_url: normalized.repository_url,
    };
    let mut progress = |_submission_id: Option<&str>, _phase: &'static str, _done, _total| {};
    submit_appearance_package(&mut client, &normalized.package_path, &draft, &mut progress)
        .await
        .map_err(market_error)
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

fn normalize_manual_submission(
    request: AppearanceMarketSubmitPackageRequest,
) -> Result<NormalizedManualSubmission, String> {
    let package_path = PathBuf::from(request.package_path.trim());
    if !package_path.is_absolute() {
        return Err("Choose an Appearance package from this device before submitting.".to_string());
    }
    if !package_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".bitfun-appearance"))
    {
        return Err("Skin submissions must use a .bitfun-appearance package.".to_string());
    }
    let fallback_name = package_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("appearance");
    let fallback_seed = package_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("package");
    let slug = if request.slug.trim().is_empty() {
        suggest_appearance_slug(fallback_name, fallback_seed)
    } else {
        request.slug.trim().to_ascii_lowercase()
    };
    validate_slug(&slug)?;

    let min_bitfun_version = if request.min_bitfun_version.trim().is_empty() {
        env!("CARGO_PKG_VERSION").to_string()
    } else {
        request.min_bitfun_version.trim().to_string()
    };
    semver::Version::parse(&min_bitfun_version)
        .map_err(|_| "Minimum BitFun version must use semantic version syntax.".to_string())?;

    let spdx_expression = trimmed(request.license.spdx_expression);
    let custom_url = trimmed(request.license.custom_url);
    if spdx_expression.is_none() == custom_url.is_none() {
        return Err("Declare exactly one SPDX expression or custom license URL.".to_string());
    }
    let changelog = request.changelog.trim().to_string();
    if changelog.chars().count() > 2_000 {
        return Err("Appearance changelogs may contain at most 2000 characters.".to_string());
    }

    Ok(NormalizedManualSubmission {
        package_path,
        slug,
        min_bitfun_version,
        changelog,
        license: AppearanceMarketLicense {
            spdx_expression,
            custom_url,
        },
        repository_url: request
            .repository_url
            .and_then(|value| trimmed(Some(value))),
    })
}

fn trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
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

    #[test]
    fn normalizes_manual_submission_defaults_without_opening_the_package() {
        let package_path = if cfg!(windows) {
            r"C:\tmp\Ocean Night.bitfun-appearance"
        } else {
            "/tmp/Ocean Night.bitfun-appearance"
        };
        let normalized = normalize_manual_submission(AppearanceMarketSubmitPackageRequest {
            package_path: package_path.to_string(),
            slug: String::new(),
            min_bitfun_version: String::new(),
            changelog: "  Initial release  ".to_string(),
            license: AppearanceMarketLicense {
                spdx_expression: Some(" MIT ".to_string()),
                custom_url: None,
            },
            repository_url: Some("  https://github.com/example/skin  ".to_string()),
        })
        .expect("manual submission should normalize");

        assert_eq!(normalized.slug, "ocean-night");
        assert_eq!(normalized.min_bitfun_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(normalized.changelog, "Initial release");
        assert_eq!(normalized.license.spdx_expression.as_deref(), Some("MIT"));
        assert_eq!(
            normalized.repository_url.as_deref(),
            Some("https://github.com/example/skin")
        );
    }

    #[test]
    fn manual_submission_requires_exactly_one_license_form() {
        let package_path = if cfg!(windows) {
            r"C:\tmp\ocean-night.bitfun-appearance"
        } else {
            "/tmp/ocean-night.bitfun-appearance"
        };
        let result = normalize_manual_submission(AppearanceMarketSubmitPackageRequest {
            package_path: package_path.to_string(),
            slug: "ocean-night".to_string(),
            min_bitfun_version: "0.2.15".to_string(),
            changelog: String::new(),
            license: AppearanceMarketLicense {
                spdx_expression: None,
                custom_url: None,
            },
            repository_url: None,
        });

        assert!(result.is_err_and(|error| error.contains("exactly one")));
    }
}
