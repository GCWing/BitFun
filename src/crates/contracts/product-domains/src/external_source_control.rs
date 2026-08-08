//! Versioned control-plane facts for external AI application sources.
//!
//! Capability payloads remain in their typed owners. This module projects only
//! lifecycle, policy, support, runtime, diagnostics, and closed control actions
//! that product surfaces and remote hosts need to share.

use crate::external_sources::{
    ExecutionDomainId, ExternalMcpActivationState, ExternalSourceCatalogSnapshot,
    ExternalSourceDiagnostic, ExternalSourceHostCapabilities, ExternalSourceLifecycleState,
    ExternalSourcePublicSnapshot, ExternalSourceScope, ExternalToolActivationState,
};
use crate::external_subagents::ExternalSubagentActivationState;
use serde::{Deserialize, Serialize};

pub const EXTERNAL_SOURCE_CONTROL_SCHEMA_V1: u32 = 1;
pub const EXTERNAL_APPLICATION_SCHEMA_V2: u32 = 2;
pub const EXTERNAL_APPLICATION_REVIEW_PAGE_MAX_ITEMS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSourceOperationStage {
    ValidateRequest,
    Discover,
    Reconcile,
    ApplyPreference,
    ActivateRuntime,
    ProjectResponse,
    ExecuteRemote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExternalSourceRecoveryActionV1 {
    Refresh,
    Retry,
    Review,
    ResolveConflict,
    InstallRuntime,
    ReconnectHost,
    ExitSafeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSourceDiscoveryState {
    Pending,
    Current,
    LastKnownGood,
    Failed,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSourceDesiredState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExternalSourceReviewState {
    NotRequired,
    Required { content_version: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSourceRuntimeState {
    NotApplicable,
    Inactive,
    Starting,
    Active,
    Degraded,
    Quarantined,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSourceSupportState {
    Supported,
    Partial,
    Unsupported,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalSourceEffectiveStatus {
    Discovering,
    Disabled,
    ReviewRequired,
    Conflict,
    Active,
    Degraded,
    Unsupported,
    Available,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalCapabilityKindV1 {
    Command,
    Tool,
    Subagent,
    Mcp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceControlSourceV1 {
    pub stable_key: String,
    pub ecosystem_id: String,
    pub display_name: String,
    pub scope: ExternalSourceScope,
    pub content_version: String,
    pub discovery: ExternalSourceDiscoveryState,
    pub desired: ExternalSourceDesiredState,
    pub review: ExternalSourceReviewState,
    pub runtime: ExternalSourceRuntimeState,
    pub support: ExternalSourceSupportState,
    pub effective_status: ExternalSourceEffectiveStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalCapabilityControlV1 {
    pub kind: ExternalCapabilityKindV1,
    pub revision: u64,
    pub item_count: usize,
    pub pending_review_count: usize,
    pub unresolved_conflict_count: usize,
    pub runtime: ExternalSourceRuntimeState,
    pub support: ExternalSourceSupportState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceControlSnapshotV1 {
    pub schema_version: u32,
    pub execution_domain_id: ExecutionDomainId,
    pub refresh_generation: u64,
    pub preference_revision: u64,
    pub safe_mode: bool,
    pub host_capabilities: ExternalSourceHostCapabilities,
    pub sources: Vec<ExternalSourceControlSourceV1>,
    pub capabilities: Vec<ExternalCapabilityControlV1>,
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
    pub recovery_actions: Vec<ExternalSourceRecoveryActionV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceSurfaceSnapshotV1 {
    pub control: ExternalSourceControlSnapshotV1,
    pub catalog: ExternalSourcePublicSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExternalSourceControlActionV1 {
    Refresh,
    SetSourceEnabled { source_key: String, enabled: bool },
    SetSafeMode { enabled: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSourceControlRequestV1 {
    pub schema_version: u32,
    pub operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_preference_revision: Option<u64>,
    pub action: ExternalSourceControlActionV1,
}

impl ExternalSourceControlRequestV1 {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != EXTERNAL_SOURCE_CONTROL_SCHEMA_V1 {
            return Err("unsupported external source control schema");
        }
        if self.operation_id.is_empty()
            || self.operation_id.len() > 160
            || self.operation_id.trim() != self.operation_id
            || self.operation_id.chars().any(char::is_control)
        {
            return Err("invalid external source control operation id");
        }
        match &self.action {
            ExternalSourceControlActionV1::SetSourceEnabled { source_key, .. }
                if source_key.is_empty()
                    || source_key.len() > 4096
                    || source_key.chars().any(char::is_control) =>
            {
                Err("invalid external source key")
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationTargetScopeV2 {
    UserDefault,
    WorkspaceOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationDesiredConnectionV2 {
    Unspecified,
    Connected,
    Disconnected,
    Deferred,
    NeedsReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationUserDecisionV2 {
    None,
    Connected,
    Disconnected,
    Deferred,
    NeedsReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationDiscoveryStateV2 {
    NotDiscovered,
    Discovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationConnectionStateV2 {
    Disconnected,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationHealthV2 {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationEffectiveStatusV2 {
    Connected,
    ConfigurationAvailable,
    NoConfiguration,
    NeedsAttention,
    TemporarilyUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationPrimaryActionV2 {
    None,
    View,
    Connect,
    Review,
    Retry,
    ViewReason,
}

/// Derives the shared application summary and its single emphasized action.
/// Safe Mode is projected separately and intentionally is not an input.
pub const fn derive_external_application_status_v2(
    needs_attention: bool,
    temporarily_unavailable: bool,
    can_retry: bool,
    connection: ExternalApplicationConnectionStateV2,
    discovery: ExternalApplicationDiscoveryStateV2,
) -> (
    ExternalApplicationEffectiveStatusV2,
    ExternalApplicationPrimaryActionV2,
) {
    if needs_attention {
        (
            ExternalApplicationEffectiveStatusV2::NeedsAttention,
            ExternalApplicationPrimaryActionV2::Review,
        )
    } else if temporarily_unavailable {
        (
            ExternalApplicationEffectiveStatusV2::TemporarilyUnavailable,
            if can_retry {
                ExternalApplicationPrimaryActionV2::Retry
            } else {
                ExternalApplicationPrimaryActionV2::ViewReason
            },
        )
    } else if matches!(connection, ExternalApplicationConnectionStateV2::Connected) {
        (
            ExternalApplicationEffectiveStatusV2::Connected,
            ExternalApplicationPrimaryActionV2::View,
        )
    } else if matches!(discovery, ExternalApplicationDiscoveryStateV2::Discovered) {
        (
            ExternalApplicationEffectiveStatusV2::ConfigurationAvailable,
            ExternalApplicationPrimaryActionV2::Connect,
        )
    } else {
        (
            ExternalApplicationEffectiveStatusV2::NoConfiguration,
            ExternalApplicationPrimaryActionV2::None,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationDefaultConnectionPolicyV2 {
    Connect,
    DiscoverOnly,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationRiskLevelV2 {
    Low,
    Moderate,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationSafetyCeilingV2 {
    Blocked,
    ReviewRequired,
    Automatic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExternalApplicationRecoveryActionV2 {
    Refresh,
    Retry,
    ReconnectHost,
    Review,
    UpgradeHost,
    ViewReason,
    ExitSafeMode,
    ResolveConflict,
    InstallRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationHostCapabilitiesV2 {
    pub can_read_snapshot: bool,
    pub can_read_review: bool,
    pub can_mutate: bool,
    pub can_manage_user_default: bool,
    pub can_manage_workspace_override: bool,
    pub can_refresh: bool,
    pub can_set_safe_mode: bool,
}

impl ExternalApplicationHostCapabilitiesV2 {
    pub const fn read_write() -> Self {
        Self {
            can_read_snapshot: true,
            can_read_review: true,
            can_mutate: true,
            can_manage_user_default: true,
            can_manage_workspace_override: true,
            can_refresh: true,
            can_set_safe_mode: true,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            can_read_snapshot: true,
            can_read_review: true,
            can_mutate: false,
            can_manage_user_default: false,
            can_manage_workspace_override: false,
            can_refresh: true,
            can_set_safe_mode: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationRiskSummaryV2 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_level: Option<ExternalApplicationRiskLevelV2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationReviewItemKindV2 {
    Command,
    Tool,
    Subagent,
    Mcp,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewItemRefV2 {
    pub kind: ExternalApplicationReviewItemKindV2,
    pub stable_id: String,
}

impl ExternalApplicationReviewItemRefV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_external_application_reference(&self.stable_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationOwnerGenerationV2 {
    pub owner: ExternalApplicationReviewItemKindV2,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewCategoryCountV2 {
    pub kind: ExternalApplicationReviewItemKindV2,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewRecommendationSummaryV2 {
    pub recommended_count: usize,
    pub optional_count: usize,
    pub blocked_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewSummaryV2 {
    pub review_id: String,
    pub total_count: usize,
    pub category_counts: Vec<ExternalApplicationReviewCategoryCountV2>,
    pub max_selection_count: usize,
    pub risk_summary: ExternalApplicationRiskSummaryV2,
    pub recommendation_summary: ExternalApplicationReviewRecommendationSummaryV2,
    pub safety_ceiling: ExternalApplicationSafetyCeilingV2,
}

impl ExternalApplicationReviewSummaryV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_external_application_id(&self.review_id)?;
        if self.max_selection_count > self.total_count {
            return Err("external application max selection count exceeds total count");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationSummaryV2 {
    pub application_id: String,
    pub ecosystem_id: String,
    pub display_name: String,
    pub discovery: ExternalApplicationDiscoveryStateV2,
    pub connection: ExternalApplicationConnectionStateV2,
    pub desired_connection: ExternalApplicationDesiredConnectionV2,
    pub health: ExternalApplicationHealthV2,
    pub effective_status: ExternalApplicationEffectiveStatusV2,
    pub primary_action: ExternalApplicationPrimaryActionV2,
    pub default_connection_policy: ExternalApplicationDefaultConnectionPolicyV2,
    pub default_connection_reason: String,
    pub enabled_count: usize,
    pub pending_review_count: usize,
    pub blocked_count: usize,
    pub conflict_count: usize,
    pub risk_summary: ExternalApplicationRiskSummaryV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice_key: Option<String>,
    pub user_decision: ExternalApplicationUserDecisionV2,
    pub recovery_actions: Vec<ExternalApplicationRecoveryActionV2>,
}

impl ExternalApplicationSummaryV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_external_application_id(&self.application_id)?;
        validate_external_application_id(&self.ecosystem_id)?;
        validate_external_application_text(&self.display_name)?;
        validate_external_application_id(&self.default_connection_reason)?;
        if let Some(notice_key) = &self.notice_key {
            validate_external_application_reference(notice_key)?;
        }
        for reason_code in &self.risk_summary.reason_codes {
            validate_external_application_id(reason_code)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationSnapshotV2 {
    pub schema_version: u32,
    pub execution_domain_id: ExecutionDomainId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_scope_id: Option<String>,
    pub effective_connection_scope: ExternalApplicationTargetScopeV2,
    pub refresh_generation: u64,
    pub preference_revision: u64,
    pub safe_mode: bool,
    pub host_capabilities: ExternalApplicationHostCapabilitiesV2,
    pub applications: Vec<ExternalApplicationSummaryV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_summary: Option<ExternalApplicationReviewSummaryV2>,
}

impl ExternalApplicationSnapshotV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_external_application_schema(self.schema_version)?;
        if let Some(workspace_scope_id) = &self.workspace_scope_id {
            validate_external_application_id(workspace_scope_id)?;
        }
        for application in &self.applications {
            application.validate()?;
        }
        if let Some(review_summary) = &self.review_summary {
            review_summary.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewItemV2 {
    pub item_ref: ExternalApplicationReviewItemRefV2,
    pub display_name: String,
    pub display_summary: String,
    pub risk_level: ExternalApplicationRiskLevelV2,
    pub risk_reason_codes: Vec<String>,
    pub recommended: bool,
    pub safety_ceiling: ExternalApplicationSafetyCeilingV2,
}

impl ExternalApplicationReviewItemV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        self.item_ref.validate()?;
        validate_external_application_text(&self.display_name)?;
        validate_external_application_text(&self.display_summary)?;
        for reason_code in &self.risk_reason_codes {
            validate_external_application_id(reason_code)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewPageRequestV2 {
    pub schema_version: u32,
    pub execution_domain_id: ExecutionDomainId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_scope_id: Option<String>,
    pub target_scope: ExternalApplicationTargetScopeV2,
    pub review_id: String,
    pub preference_revision: u64,
    pub expected_generations: Vec<ExternalApplicationOwnerGenerationV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    pub page_size: usize,
}

impl ExternalApplicationReviewPageRequestV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_external_application_schema(self.schema_version)?;
        validate_external_application_target_scope(self.target_scope, &self.workspace_scope_id)?;
        validate_external_application_id(&self.review_id)?;
        if let Some(cursor) = &self.cursor {
            validate_external_application_reference(cursor)?;
        }
        if self.page_size == 0 || self.page_size > EXTERNAL_APPLICATION_REVIEW_PAGE_MAX_ITEMS {
            return Err("external application review page size must be between 1 and 128");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewPageV2 {
    pub schema_version: u32,
    pub execution_domain_id: ExecutionDomainId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_scope_id: Option<String>,
    pub target_scope: ExternalApplicationTargetScopeV2,
    pub review_id: String,
    pub preference_revision: u64,
    pub expected_generations: Vec<ExternalApplicationOwnerGenerationV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub total_count: usize,
    pub items: Vec<ExternalApplicationReviewItemV2>,
}

impl ExternalApplicationReviewPageV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_external_application_schema(self.schema_version)?;
        validate_external_application_target_scope(self.target_scope, &self.workspace_scope_id)?;
        validate_external_application_id(&self.review_id)?;
        if self.items.len() > EXTERNAL_APPLICATION_REVIEW_PAGE_MAX_ITEMS {
            return Err("external application review page exceeds 128 items");
        }
        if self.items.len() > self.total_count {
            return Err("external application review page exceeds total count");
        }
        if let Some(cursor) = &self.cursor {
            validate_external_application_reference(cursor)?;
        }
        if let Some(cursor) = &self.next_cursor {
            validate_external_application_reference(cursor)?;
        }
        for item in &self.items {
            item.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationReviewSelectionBaselineV2 {
    Recommended,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewSelectionOverrideV2 {
    pub item_ref: ExternalApplicationReviewItemRefV2,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExternalApplicationControlActionV2 {
    ConnectApplication {
        application_id: String,
    },
    DisconnectApplication {
        application_id: String,
    },
    SetApplicationDeferred {
        application_id: String,
    },
    SubmitApplicationReview {
        review_id: String,
        expected_generations: Vec<ExternalApplicationOwnerGenerationV2>,
        selection_baseline: ExternalApplicationReviewSelectionBaselineV2,
        selection_overrides: Vec<ExternalApplicationReviewSelectionOverrideV2>,
    },
    Refresh,
    SetSourceEnabled {
        source_key: String,
        enabled: bool,
    },
    SetSafeMode {
        enabled: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationControlRequestV2 {
    pub schema_version: u32,
    pub execution_domain_id: ExecutionDomainId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_scope_id: Option<String>,
    pub target_scope: ExternalApplicationTargetScopeV2,
    pub operation_id: String,
    pub expected_preference_revision: u64,
    pub action: ExternalApplicationControlActionV2,
}

impl ExternalApplicationControlRequestV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_external_application_schema(self.schema_version)?;
        validate_external_application_target_scope(self.target_scope, &self.workspace_scope_id)?;
        validate_external_application_id(&self.operation_id)?;
        match &self.action {
            ExternalApplicationControlActionV2::ConnectApplication { application_id }
            | ExternalApplicationControlActionV2::DisconnectApplication { application_id }
            | ExternalApplicationControlActionV2::SetApplicationDeferred { application_id } => {
                validate_external_application_id(application_id)
            }
            ExternalApplicationControlActionV2::SubmitApplicationReview {
                review_id,
                selection_overrides,
                ..
            } => {
                validate_external_application_id(review_id)?;
                for selection in selection_overrides {
                    selection.item_ref.validate()?;
                }
                Ok(())
            }
            ExternalApplicationControlActionV2::SetSourceEnabled { source_key, .. } => {
                validate_external_application_reference(source_key)
            }
            ExternalApplicationControlActionV2::Refresh
            | ExternalApplicationControlActionV2::SetSafeMode { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalApplicationOperationOutcomeV2 {
    Applied,
    Rejected,
    Blocked,
    Stale,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationReviewItemResultV2 {
    pub item_ref: ExternalApplicationReviewItemRefV2,
    pub outcome: ExternalApplicationOperationOutcomeV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    pub recovery_actions: Vec<ExternalApplicationRecoveryActionV2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalApplicationControlResultV2 {
    pub schema_version: u32,
    pub operation_id: String,
    pub preference_revision: u64,
    pub outcome: ExternalApplicationOperationOutcomeV2,
    pub item_results: Vec<ExternalApplicationReviewItemResultV2>,
}

impl ExternalApplicationControlResultV2 {
    pub fn validate(&self) -> Result<(), &'static str> {
        validate_external_application_schema(self.schema_version)?;
        validate_external_application_id(&self.operation_id)?;
        for item in &self.item_results {
            item.item_ref.validate()?;
            if let Some(reason_code) = &item.reason_code {
                validate_external_application_id(reason_code)?;
            }
        }
        Ok(())
    }
}

fn validate_external_application_schema(schema_version: u32) -> Result<(), &'static str> {
    if schema_version == EXTERNAL_APPLICATION_SCHEMA_V2 {
        Ok(())
    } else {
        Err("unsupported external application schema")
    }
}

fn validate_external_application_target_scope(
    target_scope: ExternalApplicationTargetScopeV2,
    workspace_scope_id: &Option<String>,
) -> Result<(), &'static str> {
    match (target_scope, workspace_scope_id) {
        (ExternalApplicationTargetScopeV2::UserDefault, None) => Ok(()),
        (ExternalApplicationTargetScopeV2::UserDefault, Some(_)) => {
            Err("user-default scope must not include a workspace scope id")
        }
        (ExternalApplicationTargetScopeV2::WorkspaceOverride, Some(workspace_scope_id)) => {
            validate_external_application_id(workspace_scope_id)
        }
        (ExternalApplicationTargetScopeV2::WorkspaceOverride, None) => {
            Err("workspace-override scope requires a workspace scope id")
        }
    }
}

fn validate_external_application_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > 160
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err("invalid external application identifier")
    } else {
        Ok(())
    }
}

fn validate_external_application_reference(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > 4096
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Err("invalid external application reference")
    } else {
        Ok(())
    }
}

fn validate_external_application_text(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        Err("invalid external application text")
    } else {
        Ok(())
    }
}

impl ExternalSourceControlSnapshotV1 {
    pub fn from_catalog(
        catalog: &ExternalSourceCatalogSnapshot,
        execution_domain_id: ExecutionDomainId,
        safe_mode: bool,
        host_capabilities: ExternalSourceHostCapabilities,
    ) -> Self {
        let sources = catalog
            .sources
            .iter()
            .map(|source| project_source(catalog, source, safe_mode))
            .collect();
        let capabilities = vec![
            command_control(catalog),
            tool_control(catalog, safe_mode),
            subagent_control(catalog, safe_mode),
            mcp_control(catalog, safe_mode),
        ];
        let mut recovery_actions = Vec::new();
        if catalog.discovery_pending || !catalog.diagnostics.is_empty() {
            recovery_actions.push(ExternalSourceRecoveryActionV1::Refresh);
        }
        if !catalog.tool_approval_requests.is_empty()
            || !catalog.mcp_approval_requests.is_empty()
            || !catalog.pending_subagent_approvals.is_empty()
        {
            recovery_actions.push(ExternalSourceRecoveryActionV1::Review);
        }
        if catalog
            .command_conflicts
            .iter()
            .any(|conflict| conflict.selected_candidate_id.is_none())
            || catalog
                .tool_conflicts
                .iter()
                .any(|conflict| conflict.selected_candidate_id.is_none())
            || catalog
                .mcp_conflicts
                .iter()
                .any(|conflict| conflict.selected_candidate_id.is_none())
            || catalog
                .subagent_conflicts
                .iter()
                .any(|conflict| conflict.selected_candidate_id.is_none())
        {
            recovery_actions.push(ExternalSourceRecoveryActionV1::ResolveConflict);
        }
        if catalog.diagnostics.iter().any(|diagnostic| {
            diagnostic.code.contains("runtime") || diagnostic.code.contains("dependency")
        }) {
            recovery_actions.push(ExternalSourceRecoveryActionV1::InstallRuntime);
        }
        if safe_mode {
            recovery_actions.push(ExternalSourceRecoveryActionV1::ExitSafeMode);
        }
        Self {
            schema_version: EXTERNAL_SOURCE_CONTROL_SCHEMA_V1,
            execution_domain_id,
            refresh_generation: catalog.generation,
            preference_revision: catalog.preference_revision,
            safe_mode,
            host_capabilities,
            sources,
            capabilities,
            diagnostics: catalog.diagnostics.clone(),
            recovery_actions,
        }
    }
}

fn project_source(
    catalog: &ExternalSourceCatalogSnapshot,
    source: &crate::external_sources::ExternalSourceCatalogEntry,
    safe_mode: bool,
) -> ExternalSourceControlSourceV1 {
    let discovery = match source.lifecycle {
        ExternalSourceLifecycleState::Available
        | ExternalSourceLifecycleState::Restricted
        | ExternalSourceLifecycleState::Degraded => ExternalSourceDiscoveryState::Current,
        ExternalSourceLifecycleState::UsingLastValidVersion => {
            ExternalSourceDiscoveryState::LastKnownGood
        }
        ExternalSourceLifecycleState::Unavailable => ExternalSourceDiscoveryState::Failed,
        ExternalSourceLifecycleState::Removed => ExternalSourceDiscoveryState::Removed,
        ExternalSourceLifecycleState::Suppressed => ExternalSourceDiscoveryState::Current,
    };
    let desired = if matches!(source.lifecycle, ExternalSourceLifecycleState::Suppressed) {
        ExternalSourceDesiredState::Disabled
    } else {
        ExternalSourceDesiredState::Enabled
    };
    let review = review_state(catalog, &source.record.key, &source.record.content_version);
    let runtime = source_runtime(catalog, &source.record.key, safe_mode);
    let support = match source.record.health {
        crate::external_sources::ExternalSourceHealth::Available => {
            ExternalSourceSupportState::Supported
        }
        crate::external_sources::ExternalSourceHealth::Partial
        | crate::external_sources::ExternalSourceHealth::Degraded => {
            ExternalSourceSupportState::Partial
        }
        crate::external_sources::ExternalSourceHealth::Unavailable => {
            ExternalSourceSupportState::Unavailable
        }
    };
    let has_conflict = source_has_conflict(catalog, &source.record.key);
    let effective_status =
        effective_status(discovery, desired, &review, runtime, support, has_conflict);
    ExternalSourceControlSourceV1 {
        stable_key: source.stable_key.clone(),
        ecosystem_id: source.record.ecosystem_id.as_str().to_string(),
        display_name: source.record.display_name.clone(),
        scope: source.record.scope,
        content_version: source.record.content_version.clone(),
        discovery,
        desired,
        review,
        runtime,
        support,
        effective_status,
    }
}

fn review_state(
    catalog: &ExternalSourceCatalogSnapshot,
    source: &crate::external_sources::SourceKey,
    content_version: &str,
) -> ExternalSourceReviewState {
    let pending_tool = catalog
        .tool_approval_requests
        .iter()
        .any(|request| request.target_id.source == *source);
    let pending_mcp = catalog
        .mcp_approval_requests
        .iter()
        .any(|request| request.definition.id.source == *source);
    let pending_subagent = catalog.subagents.iter().any(|candidate| {
        catalog
            .pending_subagent_approvals
            .contains(&candidate.candidate_id)
            && candidate.source_keys.contains(source)
    });
    if pending_tool || pending_mcp || pending_subagent {
        ExternalSourceReviewState::Required {
            content_version: content_version.to_string(),
        }
    } else {
        // Activation states combine review and runtime outcomes, so they
        // cannot truthfully reconstruct an independent approved/declined fact
        // after load failures or mixed per-source decisions. V1 therefore
        // reports only the review fact the catalog owns directly: pending or
        // not pending.
        ExternalSourceReviewState::NotRequired
    }
}

fn source_runtime(
    catalog: &ExternalSourceCatalogSnapshot,
    source: &crate::external_sources::SourceKey,
    safe_mode: bool,
) -> ExternalSourceRuntimeState {
    let tool_states = catalog.tools.iter().filter_map(|entry| {
        (entry.definition.id.target.source == *source).then_some(&entry.activation)
    });
    let mcp_states = catalog.mcp_servers.iter().filter_map(|entry| {
        (entry.definition.id.source == *source).then_some(&entry.activation_state)
    });
    let subagent_states = catalog.subagents.iter().filter_map(|entry| {
        entry
            .source_keys
            .contains(source)
            .then_some(&entry.activation_state)
    });

    let mut applicable = false;
    let mut active = false;
    let mut starting = false;
    let mut degraded = false;
    let mut unsupported = false;
    for state in tool_states {
        applicable = true;
        active |= matches!(state, ExternalToolActivationState::Active);
        degraded |= matches!(state, ExternalToolActivationState::LoadFailed { .. });
        unsupported |= matches!(
            state,
            ExternalToolActivationState::Unsupported { .. }
                | ExternalToolActivationState::RuntimeUnavailable { .. }
        );
    }
    for state in mcp_states {
        applicable = true;
        active |= matches!(state, ExternalMcpActivationState::Active);
        starting |= matches!(state, ExternalMcpActivationState::Starting);
        unsupported |= matches!(
            state,
            ExternalMcpActivationState::Unsupported { .. }
                | ExternalMcpActivationState::RuntimeUnavailable { .. }
        );
    }
    for state in subagent_states {
        applicable = true;
        active |= matches!(state, ExternalSubagentActivationState::Active);
        degraded |= matches!(
            state,
            ExternalSubagentActivationState::Blocked | ExternalSubagentActivationState::Unavailable
        );
    }
    if !applicable {
        ExternalSourceRuntimeState::NotApplicable
    } else if safe_mode {
        ExternalSourceRuntimeState::Inactive
    } else if degraded {
        ExternalSourceRuntimeState::Degraded
    } else if unsupported {
        ExternalSourceRuntimeState::Unsupported
    } else if starting {
        ExternalSourceRuntimeState::Starting
    } else if active {
        ExternalSourceRuntimeState::Active
    } else {
        ExternalSourceRuntimeState::Inactive
    }
}

fn source_has_conflict(
    catalog: &ExternalSourceCatalogSnapshot,
    source: &crate::external_sources::SourceKey,
) -> bool {
    catalog.command_conflicts.iter().any(|conflict| {
        conflict.selected_candidate_id.is_none()
            && conflict
                .candidates
                .iter()
                .any(|candidate| candidate.source == *source)
    }) || catalog.tool_conflicts.iter().any(|conflict| {
        conflict.selected_candidate_id.is_none()
            && conflict
                .candidates
                .iter()
                .any(|candidate| candidate.source.as_ref() == Some(source))
    }) || catalog.mcp_conflicts.iter().any(|conflict| {
        conflict.selected_candidate_id.is_none()
            && conflict
                .candidates
                .iter()
                .any(|candidate| candidate.source.as_ref() == Some(source))
    }) || catalog.subagent_conflicts.iter().any(|conflict| {
        conflict.selected_candidate_id.is_none()
            && catalog.subagents.iter().any(|candidate| {
                candidate.source_keys.contains(source)
                    && conflict
                        .candidates
                        .iter()
                        .any(|item| item.candidate_id == candidate.candidate_id)
            })
    })
}

fn effective_status(
    discovery: ExternalSourceDiscoveryState,
    desired: ExternalSourceDesiredState,
    review: &ExternalSourceReviewState,
    runtime: ExternalSourceRuntimeState,
    support: ExternalSourceSupportState,
    has_conflict: bool,
) -> ExternalSourceEffectiveStatus {
    if discovery == ExternalSourceDiscoveryState::Pending {
        ExternalSourceEffectiveStatus::Discovering
    } else if discovery == ExternalSourceDiscoveryState::Removed {
        ExternalSourceEffectiveStatus::Removed
    } else if desired == ExternalSourceDesiredState::Disabled {
        ExternalSourceEffectiveStatus::Disabled
    } else if matches!(review, ExternalSourceReviewState::Required { .. }) {
        ExternalSourceEffectiveStatus::ReviewRequired
    } else if has_conflict {
        ExternalSourceEffectiveStatus::Conflict
    } else if runtime == ExternalSourceRuntimeState::Active {
        ExternalSourceEffectiveStatus::Active
    } else if matches!(
        runtime,
        ExternalSourceRuntimeState::Degraded | ExternalSourceRuntimeState::Quarantined
    ) || discovery == ExternalSourceDiscoveryState::LastKnownGood
    {
        ExternalSourceEffectiveStatus::Degraded
    } else if matches!(runtime, ExternalSourceRuntimeState::Unsupported)
        || matches!(
            support,
            ExternalSourceSupportState::Unsupported | ExternalSourceSupportState::Unavailable
        )
    {
        ExternalSourceEffectiveStatus::Unsupported
    } else {
        ExternalSourceEffectiveStatus::Available
    }
}

fn command_control(catalog: &ExternalSourceCatalogSnapshot) -> ExternalCapabilityControlV1 {
    ExternalCapabilityControlV1 {
        kind: ExternalCapabilityKindV1::Command,
        revision: catalog.generation,
        item_count: catalog.commands.len(),
        pending_review_count: 0,
        unresolved_conflict_count: catalog
            .command_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .count(),
        runtime: ExternalSourceRuntimeState::NotApplicable,
        support: capability_support(catalog, ExternalCapabilityKindV1::Command),
    }
}

fn tool_control(
    catalog: &ExternalSourceCatalogSnapshot,
    safe_mode: bool,
) -> ExternalCapabilityControlV1 {
    ExternalCapabilityControlV1 {
        kind: ExternalCapabilityKindV1::Tool,
        revision: catalog.generation,
        item_count: catalog.tools.len(),
        pending_review_count: catalog.tool_approval_requests.len(),
        unresolved_conflict_count: catalog
            .tool_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .count(),
        runtime: aggregate_tool_runtime(catalog, safe_mode),
        support: capability_support(catalog, ExternalCapabilityKindV1::Tool),
    }
}

fn subagent_control(
    catalog: &ExternalSourceCatalogSnapshot,
    safe_mode: bool,
) -> ExternalCapabilityControlV1 {
    ExternalCapabilityControlV1 {
        kind: ExternalCapabilityKindV1::Subagent,
        revision: catalog.subagent_generation,
        item_count: catalog.subagents.len(),
        pending_review_count: catalog.pending_subagent_approvals.len(),
        unresolved_conflict_count: catalog
            .subagent_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .count(),
        runtime: aggregate_subagent_runtime(
            catalog
                .subagents
                .iter()
                .map(|candidate| &candidate.activation_state),
            safe_mode,
        ),
        support: capability_support(catalog, ExternalCapabilityKindV1::Subagent),
    }
}

fn mcp_control(
    catalog: &ExternalSourceCatalogSnapshot,
    safe_mode: bool,
) -> ExternalCapabilityControlV1 {
    let runtime = aggregate_mcp_runtime(
        catalog
            .mcp_servers
            .iter()
            .map(|server| &server.activation_state),
        safe_mode,
    );
    ExternalCapabilityControlV1 {
        kind: ExternalCapabilityKindV1::Mcp,
        revision: catalog.mcp_generation,
        item_count: catalog.mcp_servers.len(),
        pending_review_count: catalog.mcp_approval_requests.len(),
        unresolved_conflict_count: catalog
            .mcp_conflicts
            .iter()
            .filter(|conflict| conflict.selected_candidate_id.is_none())
            .count(),
        runtime,
        support: capability_support(catalog, ExternalCapabilityKindV1::Mcp),
    }
}

fn aggregate_tool_runtime(
    catalog: &ExternalSourceCatalogSnapshot,
    safe_mode: bool,
) -> ExternalSourceRuntimeState {
    aggregate_tool_runtime_states(catalog.tools.iter().map(|tool| &tool.activation), safe_mode)
}

#[derive(Clone, Copy)]
enum RuntimeFact {
    Inactive,
    Starting,
    Active,
    Degraded,
    Unsupported,
}

fn aggregate_runtime(
    facts: impl IntoIterator<Item = RuntimeFact>,
    safe_mode: bool,
) -> ExternalSourceRuntimeState {
    if safe_mode {
        return ExternalSourceRuntimeState::Inactive;
    }
    let mut active = false;
    let mut starting = false;
    let mut degraded = false;
    let mut unsupported = false;
    for fact in facts {
        match fact {
            RuntimeFact::Inactive => {}
            RuntimeFact::Starting => starting = true,
            RuntimeFact::Active => active = true,
            RuntimeFact::Degraded => degraded = true,
            RuntimeFact::Unsupported => unsupported = true,
        }
    }
    if degraded {
        ExternalSourceRuntimeState::Degraded
    } else if unsupported {
        ExternalSourceRuntimeState::Unsupported
    } else if starting {
        ExternalSourceRuntimeState::Starting
    } else if active {
        ExternalSourceRuntimeState::Active
    } else {
        ExternalSourceRuntimeState::Inactive
    }
}

fn aggregate_tool_runtime_states<'a>(
    states: impl IntoIterator<Item = &'a ExternalToolActivationState>,
    safe_mode: bool,
) -> ExternalSourceRuntimeState {
    aggregate_runtime(
        states.into_iter().map(|state| match state {
            ExternalToolActivationState::Active => RuntimeFact::Active,
            ExternalToolActivationState::LoadFailed { .. } => RuntimeFact::Degraded,
            ExternalToolActivationState::Unsupported { .. }
            | ExternalToolActivationState::RuntimeUnavailable { .. } => RuntimeFact::Unsupported,
            _ => RuntimeFact::Inactive,
        }),
        safe_mode,
    )
}

fn aggregate_subagent_runtime<'a>(
    states: impl IntoIterator<Item = &'a ExternalSubagentActivationState>,
    safe_mode: bool,
) -> ExternalSourceRuntimeState {
    aggregate_runtime(
        states.into_iter().map(|state| match state {
            ExternalSubagentActivationState::Active => RuntimeFact::Active,
            ExternalSubagentActivationState::Blocked
            | ExternalSubagentActivationState::Unavailable => RuntimeFact::Degraded,
            _ => RuntimeFact::Inactive,
        }),
        safe_mode,
    )
}

fn aggregate_mcp_runtime<'a>(
    states: impl IntoIterator<Item = &'a ExternalMcpActivationState>,
    safe_mode: bool,
) -> ExternalSourceRuntimeState {
    aggregate_runtime(
        states.into_iter().map(|state| match state {
            ExternalMcpActivationState::Active => RuntimeFact::Active,
            ExternalMcpActivationState::Starting => RuntimeFact::Starting,
            ExternalMcpActivationState::Unsupported { .. }
            | ExternalMcpActivationState::RuntimeUnavailable { .. } => RuntimeFact::Unsupported,
            _ => RuntimeFact::Inactive,
        }),
        safe_mode,
    )
}

fn capability_support(
    catalog: &ExternalSourceCatalogSnapshot,
    kind: ExternalCapabilityKindV1,
) -> ExternalSourceSupportState {
    let asset_kind = match kind {
        ExternalCapabilityKindV1::Command => {
            crate::external_sources::ExternalSourceAssetKind::Command
        }
        ExternalCapabilityKindV1::Tool => crate::external_sources::ExternalSourceAssetKind::Tool,
        ExternalCapabilityKindV1::Subagent => {
            crate::external_sources::ExternalSourceAssetKind::Subagent
        }
        ExternalCapabilityKindV1::Mcp => crate::external_sources::ExternalSourceAssetKind::Mcp,
    };
    if catalog.diagnostics.iter().any(|diagnostic| {
        diagnostic.asset_kind == asset_kind
            && matches!(
                diagnostic.severity,
                crate::external_sources::ExternalSourceDiagnosticSeverity::Error
            )
    }) {
        ExternalSourceSupportState::Partial
    } else {
        ExternalSourceSupportState::Supported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_capability_reports_explicit_unsupported_state() {
        let state = ExternalToolActivationState::Unsupported {
            reason: "runtime is not supported".to_string(),
        };

        assert_eq!(
            aggregate_tool_runtime_states([&state], false),
            ExternalSourceRuntimeState::Unsupported
        );
    }

    #[test]
    fn subagent_capability_reports_blocked_state_as_degraded() {
        assert_eq!(
            aggregate_subagent_runtime([&ExternalSubagentActivationState::Blocked], false),
            ExternalSourceRuntimeState::Degraded
        );
    }

    #[test]
    fn mcp_capability_reports_runtime_unavailable_state() {
        let state = ExternalMcpActivationState::RuntimeUnavailable {
            reason: "runtime is unavailable".to_string(),
        };

        assert_eq!(
            aggregate_mcp_runtime([&state], false),
            ExternalSourceRuntimeState::Unsupported
        );
    }
}
