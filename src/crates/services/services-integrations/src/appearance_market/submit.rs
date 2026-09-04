//! Shared Appearance submission orchestration for the desktop adapter and the
//! PublishAppearance agent tool. The public product calls these "Skins" while
//! the package and runtime contract deliberately remain Appearance.

use super::client::AppearanceMarketClient;
use super::package::validate_appearance_market_package;
use crate::miniapp_market::MarketClientError;
use openbitfun_product_domains::appearance_market::{
    AppearanceMarketSubmission, AppearanceMarketSubmissionDraftRequest,
    AppearanceMarketSubmissionStatus, APPEARANCE_MARKET_MAX_PACKAGE_BYTES,
};
use std::path::Path;

pub type AppearanceSubmitProgress<'a> =
    &'a mut (dyn FnMut(Option<&str>, &'static str, u32, u32) + Send);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppearanceReleaseTarget {
    NewListing,
    ExistingListing {
        listing_id: String,
        next_release: u32,
    },
    PendingReview {
        submission_id: String,
        release_number: u32,
    },
}

pub fn resolve_appearance_release_target(
    submissions: &[AppearanceMarketSubmission],
    slug: &str,
) -> AppearanceReleaseTarget {
    if let Some(pending) = submissions.iter().find(|submission| {
        submission.slug == slug && submission.status == AppearanceMarketSubmissionStatus::Submitted
    }) {
        return AppearanceReleaseTarget::PendingReview {
            submission_id: pending.submission_id.clone(),
            release_number: pending.release_number,
        };
    }
    let listing_id = submissions
        .iter()
        .filter(|submission| submission.slug == slug)
        .find_map(|submission| submission.listing_id.clone());
    let latest_approved = submissions
        .iter()
        .filter(|submission| {
            submission.slug == slug
                && submission.status == AppearanceMarketSubmissionStatus::Approved
        })
        .map(|submission| submission.release_number)
        .max();
    match (listing_id, latest_approved) {
        (Some(listing_id), latest) => AppearanceReleaseTarget::ExistingListing {
            listing_id,
            next_release: latest.unwrap_or(0) + 1,
        },
        _ => AppearanceReleaseTarget::NewListing,
    }
}

pub fn suggest_appearance_slug(name: &str, fallback_seed: &str) -> String {
    let mut value = String::new();
    let mut last_hyphen = true;
    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            value.push(character);
            last_hyphen = false;
        } else if !last_hyphen {
            value.push('-');
            last_hyphen = true;
        }
        if value.len() >= 63 {
            break;
        }
    }
    let value = value.trim_matches('-').to_string();
    if value.len() >= 3 {
        return value;
    }
    let seed = fallback_seed
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("appearance-{seed}")
}

pub async fn submit_appearance_package(
    client: &mut AppearanceMarketClient,
    package_path: &Path,
    draft: &AppearanceMarketSubmissionDraftRequest,
    progress: AppearanceSubmitProgress<'_>,
) -> Result<AppearanceMarketSubmission, MarketClientError> {
    let metadata = tokio::fs::metadata(package_path).await.map_err(|source| {
        submission_error(
            "invalid_package_path",
            format!(
                "Could not read Appearance package metadata for {}: {source}",
                package_path.display()
            ),
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(submission_error(
            "invalid_package_path",
            "The Appearance package path must point to a non-empty regular file.",
        ));
    }
    if metadata.len() > APPEARANCE_MARKET_MAX_PACKAGE_BYTES {
        return Err(submission_error(
            "package_too_large",
            "The compressed Appearance package exceeds 96 MiB.",
        ));
    }
    let has_expected_extension = package_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.to_ascii_lowercase()
                .ends_with(".openbitfun-appearance")
        });
    if !has_expected_extension {
        return Err(submission_error(
            "invalid_package_extension",
            "Marketplace packages must use the .openbitfun-appearance extension.",
        ));
    }

    let bytes = tokio::fs::read(package_path).await.map_err(|source| {
        submission_error(
            "package_read_failed",
            format!(
                "Could not read Appearance package {}: {source}",
                package_path.display()
            ),
        )
    })?;
    let validated = validate_appearance_market_package(&bytes).map_err(|source| {
        submission_error(source.code, format!("Invalid Appearance package: {source}"))
    })?;
    progress(None, "validating", 1, 1);

    let submission = client.create_submission(draft).await?;
    let submission_id = submission.submission_id.clone();
    progress(Some(&submission_id), "package", 0, 1);
    let uploaded = client
        .upload_submission_package(&submission_id, bytes)
        .await?;
    if uploaded.package_sha256.as_deref() != Some(validated.sha256.as_str())
        || uploaded.package_size != Some(validated.size)
        || uploaded.package_id.as_deref() != Some(validated.meta.package_id.as_str())
    {
        return Err(submission_error(
            "uploaded_package_mismatch",
            "The Skin market did not confirm the uploaded Appearance package identity.",
        ));
    }
    progress(Some(&submission_id), "package", 1, 1);
    let submission = client.submit_submission(&submission_id).await?;
    progress(Some(&submission_id), "submitted", 1, 1);
    Ok(submission)
}

fn submission_error(code: impl Into<String>, message: impl Into<String>) -> MarketClientError {
    MarketClientError {
        code: code.into(),
        message: message.into(),
        request_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbitfun_product_domains::appearance_market::{
        AppearanceMarketLicense, AppearancePackageMode,
    };

    fn submission(
        slug: &str,
        release_number: u32,
        status: AppearanceMarketSubmissionStatus,
        listing_id: Option<&str>,
    ) -> AppearanceMarketSubmission {
        AppearanceMarketSubmission {
            submission_id: format!("submission-{slug}-{release_number}"),
            listing_id: listing_id.map(str::to_string),
            slug: slug.to_string(),
            release_number,
            package_id: Some("sample.appearance".to_string()),
            name: Some("Sample Appearance".to_string()),
            description: Some("Sample".to_string()),
            author: None,
            mode: Some(AppearancePackageMode::Dark),
            package_version: Some("1.0.0".to_string()),
            min_openbitfun_version: "1.0.0".to_string(),
            required_capabilities: Vec::new(),
            changelog: "Initial".to_string(),
            license: AppearanceMarketLicense {
                spdx_expression: Some("MIT".to_string()),
                custom_url: None,
            },
            repository_url: None,
            status,
            publication_status: None,
            package_sha256: None,
            package_size: None,
            preview_url: None,
            rejection_reason: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn release_target_starts_at_one() {
        assert_eq!(
            resolve_appearance_release_target(&[], "fresh-skin"),
            AppearanceReleaseTarget::NewListing
        );
    }

    #[test]
    fn release_target_increments_approved_releases() {
        let history = vec![
            submission(
                "calm-dark",
                1,
                AppearanceMarketSubmissionStatus::Approved,
                Some("listing-1"),
            ),
            submission(
                "calm-dark",
                2,
                AppearanceMarketSubmissionStatus::Approved,
                Some("listing-1"),
            ),
        ];
        assert_eq!(
            resolve_appearance_release_target(&history, "calm-dark"),
            AppearanceReleaseTarget::ExistingListing {
                listing_id: "listing-1".to_string(),
                next_release: 3,
            }
        );
    }

    #[test]
    fn release_target_blocks_parallel_review() {
        let history = vec![submission(
            "calm-dark",
            2,
            AppearanceMarketSubmissionStatus::Submitted,
            Some("listing-1"),
        )];
        assert_eq!(
            resolve_appearance_release_target(&history, "calm-dark"),
            AppearanceReleaseTarget::PendingReview {
                submission_id: "submission-calm-dark-2".to_string(),
                release_number: 2,
            }
        );
    }

    #[test]
    fn slug_suggestion_is_ascii_and_stable() {
        assert_eq!(
            suggest_appearance_slug("Ocean Night!", "unused"),
            "ocean-night"
        );
        assert_eq!(
            suggest_appearance_slug("海风", "9ab31c77-extra"),
            "appearance-9ab31c77"
        );
    }
}
