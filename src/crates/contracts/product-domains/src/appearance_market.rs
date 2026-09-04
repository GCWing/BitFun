//! Appearance marketplace contracts and pure publication policy.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const APPEARANCE_MARKET_API_VERSION: &str = "v1";
pub const APPEARANCE_MARKET_PACKAGE_CONTENT_TYPE: &str =
    "application/vnd.openbitfun.appearance+zip";
pub const APPEARANCE_MARKET_MAX_PACKAGE_BYTES: u64 = 96 * 1024 * 1024;
pub const APPEARANCE_MARKET_MAX_UNCOMPRESSED_BYTES: u64 = 128 * 1024 * 1024;
pub const APPEARANCE_MARKET_MAX_MANIFEST_BYTES: u64 = 256 * 1024;
pub const APPEARANCE_MARKET_MAX_PREVIEW_BYTES: u64 = 4 * 1024 * 1024;
pub const APPEARANCE_MARKET_MAX_PREVIEW_PIXELS: u64 = 16_000_000;
pub const APPEARANCE_MARKET_MAX_ENTRIES: usize = 64;
pub const APPEARANCE_MARKET_DEFAULT_PAGE_SIZE: u32 = 20;
pub const APPEARANCE_MARKET_MAX_PAGE_SIZE: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMarketSubmissionStatus {
    Draft,
    Submitted,
    Approved,
    Rejected,
    Withdrawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMarketPublicationStatus {
    Published,
    Yanked,
    Unpublished,
}

impl AppearanceMarketSubmissionStatus {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Draft, Self::Submitted)
                | (Self::Draft, Self::Withdrawn)
                | (Self::Submitted, Self::Approved)
                | (Self::Submitted, Self::Rejected)
                | (Self::Submitted, Self::Withdrawn)
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMarketSort {
    #[default]
    Newest,
    Downloads,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppearancePackageMode {
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketUserSummary {
    pub github_id: i64,
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketListingSummary {
    pub listing_id: String,
    pub slug: String,
    pub package_id: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub mode: AppearancePackageMode,
    pub package_version: String,
    pub latest_release: u32,
    #[serde(rename = "minOpenBitFunVersion")]
    pub min_openbitfun_version: String,
    pub required_capabilities: Vec<String>,
    pub owner: AppearanceMarketUserSummary,
    pub preview_url: String,
    pub download_count: u64,
    pub published_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketListingDetail {
    #[serde(flatten)]
    pub summary: AppearanceMarketListingSummary,
    pub changelog: String,
    pub license: AppearanceMarketLicense,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
    pub releases: Vec<AppearanceMarketRelease>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketRelease {
    pub release_id: String,
    pub listing_id: String,
    pub release_number: u32,
    pub package_version: String,
    #[serde(rename = "minOpenBitFunVersion")]
    pub min_openbitfun_version: String,
    pub package_sha256: String,
    pub package_size: u64,
    pub review_bundle_hash: String,
    pub published_at: i64,
    pub yanked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketLicense {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spdx_expression: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_url: Option<String>,
}

impl AppearanceMarketLicense {
    pub fn is_declared(&self) -> bool {
        self.spdx_expression
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || self
                .custom_url
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketSubmission {
    pub submission_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listing_id: Option<String>,
    pub slug: String,
    pub release_number: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<AppearancePackageMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    #[serde(rename = "minOpenBitFunVersion")]
    pub min_openbitfun_version: String,
    pub required_capabilities: Vec<String>,
    pub changelog: String,
    pub license: AppearanceMarketLicense,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
    pub status: AppearanceMarketSubmissionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication_status: Option<AppearanceMarketPublicationStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketSubmissionDraftRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listing_id: Option<String>,
    pub slug: String,
    pub release_number: u32,
    #[serde(rename = "minOpenBitFunVersion")]
    pub min_openbitfun_version: String,
    pub changelog: String,
    pub license: AppearanceMarketLicense,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceAdminSubmissionDetail {
    pub submission: AppearanceMarketSubmission,
    /// GitHub account that owns the submission, so reviewers can see who sent it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submitter: Option<AppearanceMarketUserSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_bundle_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceMarketPackageMeta {
    pub package_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub mode: AppearancePackageMode,
    pub package_version: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceReviewDecision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceReviewDecisionRequest {
    pub decision: AppearanceReviewDecision,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceCursorPage<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

pub fn validate_appearance_market_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if !(3..=63).contains(&bytes.len()) {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return false;
    }
    bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

pub fn compute_appearance_review_bundle_hash(
    package_sha256: &str,
    canonical_metadata_json: &str,
    preview_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(package_sha256.as_bytes());
    hasher.update([0]);
    hasher.update(canonical_metadata_json.as_bytes());
    hasher.update([0]);
    hasher.update(preview_sha256.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_contract_is_strict_and_stable() {
        assert!(validate_appearance_market_slug("tokyo-night"));
        assert!(validate_appearance_market_slug("skin-1"));
        assert!(!validate_appearance_market_slug("AB"));
        assert!(!validate_appearance_market_slug("-leading"));
        assert!(!validate_appearance_market_slug("contains_underscore"));
    }

    #[test]
    fn submission_state_machine_rejects_republication() {
        assert!(AppearanceMarketSubmissionStatus::Draft
            .can_transition_to(AppearanceMarketSubmissionStatus::Submitted));
        assert!(AppearanceMarketSubmissionStatus::Submitted
            .can_transition_to(AppearanceMarketSubmissionStatus::Approved));
        assert!(!AppearanceMarketSubmissionStatus::Approved
            .can_transition_to(AppearanceMarketSubmissionStatus::Submitted));
    }

    #[test]
    fn review_hash_binds_the_preview() {
        let first = compute_appearance_review_bundle_hash("package", "{}", "preview-one");
        let second = compute_appearance_review_bundle_hash("package", "{}", "preview-two");
        assert_ne!(first, second);
    }

    #[test]
    fn package_mode_uses_the_web_manifest_spelling() {
        assert_eq!(
            serde_json::to_string(&AppearancePackageMode::Dark).unwrap(),
            "\"dark\""
        );
    }

    #[test]
    fn publication_status_serializes_as_a_stable_api_value() {
        assert_eq!(
            serde_json::to_string(&AppearanceMarketPublicationStatus::Yanked).unwrap(),
            "\"yanked\""
        );
        assert_eq!(
            serde_json::to_string(&AppearanceMarketPublicationStatus::Unpublished).unwrap(),
            "\"unpublished\""
        );
    }

    #[test]
    fn listing_detail_json_contract_matches_the_shared_typescript_fixture() {
        let fixture = include_str!(
            "../../../../shared/appearance-market-contract-fixtures/listing-detail.json"
        );
        let listing: AppearanceMarketListingDetail = serde_json::from_str(fixture).unwrap();
        let serialized = serde_json::to_value(&listing).unwrap();
        let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();

        assert_eq!(serialized, expected);
        assert_eq!(listing.summary.package_id, "community.ocean-night");
        assert_eq!(listing.releases[0].package_version, "2.1.0");
    }

    #[test]
    fn listing_contract_rejects_the_retired_product_version_field() {
        let fixture = include_str!(
            "../../../../shared/appearance-market-contract-fixtures/listing-detail.json"
        );
        let mut listing: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let value = listing
            .as_object_mut()
            .unwrap()
            .remove("minOpenBitFunVersion")
            .unwrap();
        let retired_field = ["min", "Bit", "fun", "Version"].concat();
        listing
            .as_object_mut()
            .unwrap()
            .insert(retired_field, value);

        let error = serde_json::from_value::<AppearanceMarketListingDetail>(listing).unwrap_err();
        assert!(error.to_string().contains("minOpenBitFunVersion"));
    }
}
