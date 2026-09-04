//! Shared submission orchestration: package an installed MiniApp and walk it
//! through the market's draft → upload → submit flow. Used by the desktop
//! Tauri command and the PublishMiniApp agent tool so the two paths cannot
//! drift.

use super::client::{MarketClient, MarketClientError};
use super::package::build_market_package;
use openbitfun_product_domains::miniapp::market::{
    MarketSubmission, MarketSubmissionDraftRequest, MarketSubmissionStatus, MARKET_CATEGORIES,
    MARKET_MAX_SCREENSHOTS, MARKET_MAX_SCREENSHOT_BYTES,
};
use openbitfun_product_domains::miniapp::types::MiniApp;
use std::path::Path;

/// Upload progress callback: (submission_id, phase, completed, total).
/// Phases mirror the desktop UI contract: validating, package, screenshots,
/// submitted.
pub type SubmitProgress<'a> = &'a mut (dyn FnMut(Option<&str>, &'static str, u32, u32) + Send);

/// Where an installed app's next submission should land on the market.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseTarget {
    /// No listing with this slug is owned by the caller: first release.
    NewListing,
    /// The caller already owns the listing: publish `next_release` to it.
    ExistingListing {
        listing_id: String,
        next_release: u32,
    },
    /// A submission for this slug is still under review; publishing another
    /// release now would just be rejected or create reviewer noise.
    PendingReview {
        submission_id: String,
        release_number: u32,
    },
}

/// Derive the release target for `slug` from the caller's own submission
/// history. Release numbers on the server are `MAX(releases)+1`, and releases
/// are only created on approval, so approved submissions are the ground truth.
pub fn resolve_release_target(submissions: &[MarketSubmission], slug: &str) -> ReleaseTarget {
    if let Some(pending) = submissions
        .iter()
        .find(|s| s.slug == slug && s.status == MarketSubmissionStatus::Submitted)
    {
        return ReleaseTarget::PendingReview {
            submission_id: pending.submission_id.clone(),
            release_number: pending.release_number,
        };
    }
    let listing_id = submissions
        .iter()
        .filter(|s| s.slug == slug)
        .find_map(|s| s.listing_id.clone());
    let latest_approved = submissions
        .iter()
        .filter(|s| s.slug == slug && s.status == MarketSubmissionStatus::Approved)
        .map(|s| s.release_number)
        .max();
    match (listing_id, latest_approved) {
        (Some(listing_id), Some(latest)) => ReleaseTarget::ExistingListing {
            listing_id,
            next_release: latest + 1,
        },
        (Some(listing_id), None) => ReleaseTarget::ExistingListing {
            listing_id,
            next_release: 1,
        },
        _ => ReleaseTarget::NewListing,
    }
}

/// Suggest a marketplace slug from an app name, matching the submissions UI:
/// lowercase ASCII letters, digits and hyphens, 3–63 chars.
pub fn suggest_market_slug(name: &str, fallback_seed: &str) -> String {
    let mut value = String::new();
    let mut last_hyphen = true;
    for ch in name.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            value.push(ch);
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
    let seed: String = fallback_seed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_lowercase();
    format!("miniapp-{seed}")
}

/// Map a locally installed app's category (utility, media, dev, productivity,
/// game, …) onto the market's fixed category set.
pub fn map_local_category_to_market(category: &str) -> String {
    let normalized = category.trim().to_lowercase();
    let mapped = match normalized.as_str() {
        "utility" | "utilities" | "tool" | "tools" => "utilities",
        "dev" | "developer" | "development" => "developer",
        "media" | "creative" | "design" => "creative",
        "productivity" => "productivity",
        "game" | "games" | "entertainment" => "entertainment",
        "data" => "data",
        "education" => "education",
        other => other,
    };
    if MARKET_CATEGORIES.contains(&mapped) {
        mapped.to_string()
    } else {
        "other".to_string()
    }
}

/// Read one marketplace image from disk, enforcing the market's type and size limits.
pub async fn read_screenshot_file(
    path: &Path,
) -> Result<(&'static str, Vec<u8>), MarketClientError> {
    let metadata = tokio::fs::metadata(path).await.map_err(|error| {
        screenshot_error(format!(
            "Could not read marketplace image metadata for {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file() || metadata.len() > MARKET_MAX_SCREENSHOT_BYTES {
        return Err(screenshot_error(format!(
            "Each marketplace image must be a file no larger than 5 MiB: {}",
            path.display()
        )));
    }
    let media_type = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => {
            return Err(screenshot_error(format!(
                "Marketplace images must be PNG, JPEG, or WebP: {}",
                path.display()
            )))
        }
    };
    let bytes = tokio::fs::read(path).await.map_err(|error| {
        screenshot_error(format!(
            "Could not read marketplace image {}: {error}",
            path.display()
        ))
    })?;
    Ok((media_type, bytes))
}

/// Package `app` with the draft's public metadata and drive the full market
/// submission flow. The reviewed listing metadata is part of the immutable
/// submission snapshot, so the package is built from the app's source and
/// permissions but carries the draft's name/description/icon/category/tags —
/// the package and the review record cannot disagree.
pub async fn submit_installed_app(
    client: &mut MarketClient,
    app: &MiniApp,
    draft: &MarketSubmissionDraftRequest,
    screenshot_paths: &[String],
    progress: SubmitProgress<'_>,
) -> Result<MarketSubmission, MarketClientError> {
    if screenshot_paths.is_empty() || screenshot_paths.len() > MARKET_MAX_SCREENSHOTS {
        return Err(screenshot_error(
            "Choose between 1 and 5 marketplace images.".to_string(),
        ));
    }
    let mut package_app = app.clone();
    package_app.name = draft.name.clone();
    package_app.description = draft.description.clone();
    package_app.icon = draft.icon.clone();
    package_app.category = draft.category.clone();
    package_app.tags = draft.tags.clone();
    let package = build_market_package(&package_app).map_err(|error| MarketClientError {
        code: "invalid_package".to_string(),
        message: error.to_string(),
        request_id: None,
    })?;
    progress(None, "validating", 1, 1);

    let submission = client.create_submission(draft).await?;
    let submission_id = submission.submission_id.clone();
    progress(Some(&submission_id), "package", 0, 1);
    client
        .upload_submission_package(&submission_id, package)
        .await?;
    progress(Some(&submission_id), "package", 1, 1);

    let screenshot_total = screenshot_paths.len() as u32;
    for (position, path) in screenshot_paths.iter().enumerate() {
        let (media_type, bytes) = read_screenshot_file(Path::new(path)).await?;
        client
            .upload_submission_screenshot(&submission_id, position as u32, media_type, bytes)
            .await?;
        progress(
            Some(&submission_id),
            "screenshots",
            position as u32 + 1,
            screenshot_total,
        );
    }
    let submission = client.submit_submission(&submission_id).await?;
    progress(Some(&submission_id), "submitted", 1, 1);
    Ok(submission)
}

fn screenshot_error(message: String) -> MarketClientError {
    MarketClientError {
        code: "invalid_screenshot".to_string(),
        message,
        request_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbitfun_product_domains::miniapp::market::MarketLicense;
    use openbitfun_product_domains::miniapp::types::MiniAppPermissions;

    fn submission(
        slug: &str,
        release: u32,
        status: MarketSubmissionStatus,
        listing_id: Option<&str>,
    ) -> MarketSubmission {
        MarketSubmission {
            submission_id: format!("sub-{slug}-{release}"),
            listing_id: listing_id.map(str::to_string),
            slug: slug.to_string(),
            release_number: release,
            name: "App".to_string(),
            description: "Desc".to_string(),
            icon: "📦".to_string(),
            category: "utilities".to_string(),
            tags: Vec::new(),
            min_openbitfun_version: "1.0.0".to_string(),
            changelog: "Initial".to_string(),
            license: MarketLicense {
                spdx_expression: Some("MIT".to_string()),
                custom_url: None,
            },
            repository_url: None,
            permissions: MiniAppPermissions::default(),
            status,
            package_sha256: None,
            package_size: None,
            screenshot_urls: Vec::new(),
            rejection_reason: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn resolve_release_target_first_release() {
        assert_eq!(
            resolve_release_target(&[], "fresh-app"),
            ReleaseTarget::NewListing
        );
    }

    #[test]
    fn resolve_release_target_bumps_after_approval() {
        let history = vec![
            submission("my-app", 1, MarketSubmissionStatus::Approved, Some("l1")),
            submission("my-app", 2, MarketSubmissionStatus::Approved, Some("l1")),
            submission("other", 5, MarketSubmissionStatus::Approved, Some("l2")),
        ];
        assert_eq!(
            resolve_release_target(&history, "my-app"),
            ReleaseTarget::ExistingListing {
                listing_id: "l1".to_string(),
                next_release: 3,
            }
        );
    }

    #[test]
    fn resolve_release_target_flags_pending_review() {
        let history = vec![
            submission("my-app", 1, MarketSubmissionStatus::Approved, Some("l1")),
            submission("my-app", 2, MarketSubmissionStatus::Submitted, Some("l1")),
        ];
        assert_eq!(
            resolve_release_target(&history, "my-app"),
            ReleaseTarget::PendingReview {
                submission_id: "sub-my-app-2".to_string(),
                release_number: 2,
            }
        );
    }

    #[test]
    fn resolve_release_target_ignores_withdrawn_and_rejected() {
        let history = vec![
            submission("my-app", 1, MarketSubmissionStatus::Withdrawn, None),
            submission("my-app", 1, MarketSubmissionStatus::Rejected, None),
        ];
        assert_eq!(
            resolve_release_target(&history, "my-app"),
            ReleaseTarget::NewListing
        );
    }

    #[test]
    fn suggest_market_slug_normalizes_names() {
        assert_eq!(
            suggest_market_slug("Regex Playground!", "unused"),
            "regex-playground"
        );
        assert_eq!(suggest_market_slug("My  App", "unused"), "my-app");
    }

    #[test]
    fn suggest_market_slug_falls_back_for_short_or_cjk_names() {
        assert_eq!(
            suggest_market_slug("正则游乐场", "1a2b3c4d-5e6f"),
            "miniapp-1a2b3c4d"
        );
    }

    #[test]
    fn map_local_category_covers_local_and_market_values() {
        assert_eq!(map_local_category_to_market("utility"), "utilities");
        assert_eq!(map_local_category_to_market("dev"), "developer");
        assert_eq!(map_local_category_to_market("game"), "entertainment");
        assert_eq!(map_local_category_to_market("media"), "creative");
        assert_eq!(map_local_category_to_market("productivity"), "productivity");
        assert_eq!(map_local_category_to_market("developer"), "developer");
        assert_eq!(map_local_category_to_market("nonsense"), "other");
    }
}
