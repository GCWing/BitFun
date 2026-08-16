use serde::{Deserialize, Serialize};
use std::fmt;

pub const BALANCED_HARNESS_PROFILE_ID: &str = "balanced";
pub const MINIMAL_HARNESS_PROFILE_ID: &str = "minimal";
pub const ULTIMATE_HARNESS_PROFILE_ID: &str = "ultimate";

pub const HARNESS_SELECTION_DEFAULT: &str = "default";
pub const HARNESS_SELECTION_USER: &str = "user";
pub const HARNESS_SELECTION_CLI: &str = "cli";
pub const HARNESS_SELECTION_ADAPTER: &str = "adapter";
pub const HARNESS_SELECTION_COMPATIBILITY: &str = "compatibility_projection";

/// Stable, unknown-tolerant Harness Profile identity.
///
/// This remains a string newtype so an older reader can preserve a profile
/// introduced by a newer Host instead of rejecting or rewriting the Session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(transparent)]
pub struct HarnessProfileId(String);

impl HarnessProfileId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_balanced(&self) -> bool {
        self.0 == BALANCED_HARNESS_PROFILE_ID
    }

    pub fn is_minimal(&self) -> bool {
        self.0 == MINIMAL_HARNESS_PROFILE_ID
    }
}

impl Default for HarnessProfileId {
    fn default() -> Self {
        Self::new(BALANCED_HARNESS_PROFILE_ID)
    }
}

impl fmt::Display for HarnessProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable, unknown-tolerant source of a Harness Profile selection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(transparent)]
pub struct HarnessSelectionSource(String);

impl HarnessSelectionSource {
    pub fn new(source: impl Into<String>) -> Self {
        Self(source.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for HarnessSelectionSource {
    fn default() -> Self {
        Self::new(HARNESS_SELECTION_COMPATIBILITY)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SessionExecutionProfile {
    pub harness_profile_id: HarnessProfileId,
    pub schema_version: u32,
    pub selected_by: HarnessSelectionSource,
}

impl SessionExecutionProfile {
    pub fn new(
        harness_profile_id: HarnessProfileId,
        selected_by: HarnessSelectionSource,
    ) -> Self {
        Self {
            harness_profile_id,
            schema_version: 1,
            selected_by,
        }
    }

    pub fn balanced(selected_by: HarnessSelectionSource) -> Self {
        Self::new(HarnessProfileId::default(), selected_by)
    }

    pub fn minimal(selected_by: HarnessSelectionSource) -> Self {
        Self::new(
            HarnessProfileId::new(MINIMAL_HARNESS_PROFILE_ID),
            selected_by,
        )
    }
}

impl Default for SessionExecutionProfile {
    fn default() -> Self {
        Self::balanced(HarnessSelectionSource::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_profile_projects_to_balanced_compatibility() {
        let profile: SessionExecutionProfile = serde_json::from_str("{}").unwrap_or_default();
        assert!(profile.harness_profile_id.is_balanced());
        assert_eq!(
            profile.selected_by.as_str(),
            HARNESS_SELECTION_COMPATIBILITY
        );
    }

    #[test]
    fn unknown_profile_identity_round_trips() {
        let profile = SessionExecutionProfile::new(
            HarnessProfileId::new("future-profile"),
            HarnessSelectionSource::new("future-client"),
        );
        let encoded = serde_json::to_string(&profile).unwrap();
        let decoded: SessionExecutionProfile = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, profile);
    }
}
