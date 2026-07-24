//! Ecosystem-neutral policy contracts for external integrations.
//!
//! Product assembly registers ecosystems and declares capability defaults and
//! safety ceilings. This module only preserves, evaluates, and projects policy;
//! it does not know about OpenCode or any other concrete ecosystem.

use crate::external_sources::{
    validate_id, EcosystemId, ExternalIntegrationCapabilityId, ExternalSourceContractError,
};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};

pub const EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR: u32 = 1;

/// Product-facing integration modes stay open so an older Host can preserve a
/// policy written by a newer Host. Unknown values fail closed during policy
/// evaluation and are never projected as selectable UI options.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ExternalIntegrationMode {
    Recommended,
    DiscoverOnly,
    Disabled,
    Custom,
    Unknown(String),
}

impl ExternalIntegrationMode {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Recommended => "recommended",
            Self::DiscoverOnly => "discover_only",
            Self::Disabled => "disabled",
            Self::Custom => "custom",
            Self::Unknown(value) => value,
        }
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    fn parse(value: String) -> Result<Self, ExternalSourceContractError> {
        validate_id(&value, "external integration mode")?;
        Ok(match value.as_str() {
            "recommended" => Self::Recommended,
            "discover_only" => Self::DiscoverOnly,
            "disabled" => Self::Disabled,
            "custom" => Self::Custom,
            _ => Self::Unknown(value),
        })
    }
}

impl Default for ExternalIntegrationMode {
    fn default() -> Self {
        Self::Recommended
    }
}

impl Serialize for ExternalIntegrationMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ExternalIntegrationMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Access decisions are ordered from most restrictive to most permissive.
/// Unknown values are preserved for forward compatibility and evaluate to
/// `Disabled` on Hosts that do not understand them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ExternalIntegrationAccess {
    Disabled,
    DiscoverOnly,
    AskBeforeUse,
    Auto,
    Unknown(String),
}

impl ExternalIntegrationAccess {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Disabled => "disabled",
            Self::DiscoverOnly => "discover_only",
            Self::AskBeforeUse => "ask_before_use",
            Self::Auto => "auto",
            Self::Unknown(value) => value,
        }
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    fn parse(value: String) -> Result<Self, ExternalSourceContractError> {
        validate_id(&value, "external integration access")?;
        Ok(match value.as_str() {
            "disabled" => Self::Disabled,
            "discover_only" => Self::DiscoverOnly,
            "ask_before_use" => Self::AskBeforeUse,
            "auto" => Self::Auto,
            _ => Self::Unknown(value),
        })
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Unknown(_) | Self::Disabled => 0,
            Self::DiscoverOnly => 1,
            Self::AskBeforeUse => 2,
            Self::Auto => 3,
        }
    }

    fn at_most(self, ceiling: Self) -> (Self, bool) {
        if matches!(self, Self::Unknown(_)) {
            return (Self::Disabled, true);
        }
        if self.rank() <= ceiling.rank() {
            (self, false)
        } else {
            (ceiling, true)
        }
    }
}

impl Default for ExternalIntegrationAccess {
    fn default() -> Self {
        Self::DiscoverOnly
    }
}

impl Serialize for ExternalIntegrationAccess {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ExternalIntegrationAccess {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExternalEcosystemPolicy {
    pub mode: ExternalIntegrationMode,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capability_overrides: BTreeMap<ExternalIntegrationCapabilityId, ExternalIntegrationAccess>,
    /// Preserves fields introduced by a newer minor schema during read-modify-write.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl Default for ExternalEcosystemPolicy {
    fn default() -> Self {
        Self {
            mode: ExternalIntegrationMode::Recommended,
            capability_overrides: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExternalIntegrationPolicySettings {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ecosystems: BTreeMap<EcosystemId, ExternalEcosystemPolicy>,
    /// Preserves fields introduced by a newer minor schema during read-modify-write.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl Default for ExternalIntegrationPolicySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            ecosystems: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ExternalEcosystemPolicyOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ExternalIntegrationMode>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capability_overrides: BTreeMap<ExternalIntegrationCapabilityId, ExternalIntegrationAccess>,
    /// Preserves fields introduced by a newer minor schema during read-modify-write.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ExternalIntegrationPolicyOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ecosystems: BTreeMap<EcosystemId, ExternalEcosystemPolicyOverride>,
    /// Preserves fields introduced by a newer minor schema during read-modify-write.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl ExternalIntegrationPolicyOverride {
    pub fn is_empty(&self) -> bool {
        self.enabled.is_none() && self.ecosystems.is_empty() && self.extensions.is_empty()
    }
}

fn current_external_integration_schema_major() -> u32 {
    EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExternalIntegrationPolicyDocument {
    #[serde(default = "current_external_integration_schema_major")]
    pub schema_major: u32,
    pub user_defaults: ExternalIntegrationPolicySettings,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub workspace_overrides: BTreeMap<String, ExternalIntegrationPolicyOverride>,
    /// Preserves fields introduced by a newer minor schema during read-modify-write.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl Default for ExternalIntegrationPolicyDocument {
    fn default() -> Self {
        Self {
            schema_major: EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR,
            user_defaults: ExternalIntegrationPolicySettings::default(),
            workspace_overrides: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }
}

/// Capability defaults are registered by product assembly so policy evaluation
/// remains neutral and future ecosystems can declare their own safe profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalIntegrationCapabilityDescriptor {
    pub capability_id: ExternalIntegrationCapabilityId,
    pub recommended_access: ExternalIntegrationAccess,
    pub safety_ceiling: ExternalIntegrationAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalIntegrationEcosystemDescriptor {
    pub ecosystem_id: EcosystemId,
    pub display_name: String,
    pub adapter_revision: String,
    pub capabilities: Vec<ExternalIntegrationCapabilityDescriptor>,
}

/// Public, compatibility-safe projection of user policy settings. Persistence
/// extension fields intentionally stay out of Host APIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalIntegrationPolicySettingsView {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ecosystems: BTreeMap<EcosystemId, ExternalEcosystemPolicyView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalEcosystemPolicyView {
    pub mode: ExternalIntegrationMode,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capability_overrides: BTreeMap<ExternalIntegrationCapabilityId, ExternalIntegrationAccess>,
}

impl From<&ExternalIntegrationPolicySettings> for ExternalIntegrationPolicySettingsView {
    fn from(settings: &ExternalIntegrationPolicySettings) -> Self {
        Self {
            enabled: settings.enabled,
            ecosystems: settings
                .ecosystems
                .iter()
                .map(|(id, policy)| {
                    (
                        id.clone(),
                        ExternalEcosystemPolicyView {
                            mode: policy.mode.clone(),
                            capability_overrides: policy.capability_overrides.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalIntegrationPolicyOverrideView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ecosystems: BTreeMap<EcosystemId, ExternalEcosystemPolicyOverrideView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalEcosystemPolicyOverrideView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ExternalIntegrationMode>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capability_overrides: BTreeMap<ExternalIntegrationCapabilityId, ExternalIntegrationAccess>,
}

impl From<&ExternalIntegrationPolicyOverride> for ExternalIntegrationPolicyOverrideView {
    fn from(policy: &ExternalIntegrationPolicyOverride) -> Self {
        Self {
            enabled: policy.enabled,
            ecosystems: policy
                .ecosystems
                .iter()
                .map(|(id, ecosystem)| {
                    (
                        id.clone(),
                        ExternalEcosystemPolicyOverrideView {
                            mode: ecosystem.mode.clone(),
                            capability_overrides: ecosystem.capability_overrides.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExternalIntegrationPolicyStatus {
    Compatible,
    IncompatibleSchema,
    Unknown(String),
}

impl ExternalIntegrationPolicyStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Compatible => "compatible",
            Self::IncompatibleSchema => "incompatible_schema",
            Self::Unknown(value) => value,
        }
    }

    pub fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    fn parse(value: String) -> Result<Self, ExternalSourceContractError> {
        validate_id(&value, "external integration policy status")?;
        Ok(match value.as_str() {
            "compatible" => Self::Compatible,
            "incompatible_schema" => Self::IncompatibleSchema,
            _ => Self::Unknown(value),
        })
    }
}

impl Default for ExternalIntegrationPolicyStatus {
    fn default() -> Self {
        Self::Compatible
    }
}

impl Serialize for ExternalIntegrationPolicyStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ExternalIntegrationPolicyStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectiveExternalEcosystemPolicy {
    pub ecosystem_id: EcosystemId,
    pub mode: ExternalIntegrationMode,
    pub capabilities: BTreeMap<ExternalIntegrationCapabilityId, ExternalIntegrationAccess>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub policy_limited_capabilities: BTreeSet<ExternalIntegrationCapabilityId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectiveExternalIntegrationPolicy {
    pub enabled: bool,
    pub ecosystems: BTreeMap<EcosystemId, EffectiveExternalEcosystemPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalIntegrationPolicySnapshot {
    pub schema_major: u32,
    pub status: ExternalIntegrationPolicyStatus,
    pub user_defaults: ExternalIntegrationPolicySettingsView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_override: Option<ExternalIntegrationPolicyOverrideView>,
    pub global_effective: EffectiveExternalIntegrationPolicy,
    pub effective: EffectiveExternalIntegrationPolicy,
    pub registered_ecosystems: Vec<ExternalIntegrationEcosystemDescriptor>,
}

impl Default for ExternalIntegrationPolicySnapshot {
    fn default() -> Self {
        Self {
            schema_major: EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR,
            status: ExternalIntegrationPolicyStatus::Compatible,
            user_defaults: ExternalIntegrationPolicySettingsView::from(
                &ExternalIntegrationPolicySettings::default(),
            ),
            workspace_override: None,
            global_effective: EffectiveExternalIntegrationPolicy {
                enabled: true,
                ecosystems: BTreeMap::new(),
            },
            effective: EffectiveExternalIntegrationPolicy {
                enabled: true,
                ecosystems: BTreeMap::new(),
            },
            registered_ecosystems: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExternalIntegrationPolicyScope {
    User,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
#[non_exhaustive]
pub enum ExternalIntegrationPolicyOperation {
    SetEnabled {
        enabled: bool,
    },
    SetEcosystemMode {
        ecosystem_id: EcosystemId,
        mode: ExternalIntegrationMode,
    },
    SetCapabilityAccess {
        ecosystem_id: EcosystemId,
        capability_id: ExternalIntegrationCapabilityId,
        access: ExternalIntegrationAccess,
    },
    ResetWorkspace,
    ResetIncompatiblePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalIntegrationPolicyMutation {
    pub expected_preference_revision: u64,
    pub scope: ExternalIntegrationPolicyScope,
    pub change: ExternalIntegrationPolicyOperation,
}

fn validate_registered_ecosystems(
    registered_ecosystems: &[ExternalIntegrationEcosystemDescriptor],
) -> Result<(), ExternalSourceContractError> {
    let mut ecosystem_ids = BTreeSet::new();
    for ecosystem in registered_ecosystems {
        if !ecosystem_ids.insert(ecosystem.ecosystem_id.clone()) {
            return Err(ExternalSourceContractError::InvalidPolicyDescriptor(
                "duplicate ecosystem id",
            ));
        }
        validate_id(&ecosystem.adapter_revision, "adapter revision")?;
        let mut capability_ids = BTreeSet::new();
        for capability in &ecosystem.capabilities {
            if !capability_ids.insert(capability.capability_id.clone()) {
                return Err(ExternalSourceContractError::InvalidPolicyDescriptor(
                    "duplicate capability id",
                ));
            }
            if !capability.recommended_access.is_known() || !capability.safety_ceiling.is_known() {
                return Err(ExternalSourceContractError::InvalidPolicyDescriptor(
                    "unknown capability access",
                ));
            }
            if capability.recommended_access.rank() > capability.safety_ceiling.rank() {
                return Err(ExternalSourceContractError::InvalidPolicyDescriptor(
                    "recommended access exceeds the safety ceiling",
                ));
            }
        }
    }
    Ok(())
}

pub fn evaluate_external_integration_policy(
    document: &ExternalIntegrationPolicyDocument,
    workspace_key: Option<&str>,
    registered_ecosystems: &[ExternalIntegrationEcosystemDescriptor],
) -> Result<EffectiveExternalIntegrationPolicy, ExternalSourceContractError> {
    if document.schema_major != EXTERNAL_INTEGRATION_POLICY_SCHEMA_MAJOR {
        return Err(ExternalSourceContractError::UnsupportedPolicySchemaMajor(
            document.schema_major,
        ));
    }
    validate_registered_ecosystems(registered_ecosystems)?;
    let workspace_override = workspace_key.and_then(|key| document.workspace_overrides.get(key));
    let enabled = workspace_override
        .and_then(|policy| policy.enabled)
        .unwrap_or(document.user_defaults.enabled);
    let mut ecosystems = BTreeMap::new();
    for descriptor in registered_ecosystems {
        let user_policy = document
            .user_defaults
            .ecosystems
            .get(&descriptor.ecosystem_id)
            .cloned()
            .unwrap_or_default();
        let workspace_policy =
            workspace_override.and_then(|policy| policy.ecosystems.get(&descriptor.ecosystem_id));
        let mode = if enabled {
            workspace_policy
                .and_then(|policy| policy.mode.clone())
                .unwrap_or_else(|| user_policy.mode.clone())
        } else {
            ExternalIntegrationMode::Disabled
        };
        let mut capabilities = BTreeMap::new();
        let mut policy_limited_capabilities = BTreeSet::new();
        for capability in &descriptor.capabilities {
            let configured = workspace_policy
                .and_then(|policy| policy.capability_overrides.get(&capability.capability_id))
                .or_else(|| {
                    user_policy
                        .capability_overrides
                        .get(&capability.capability_id)
                });
            let requested = if !enabled {
                ExternalIntegrationAccess::Disabled
            } else {
                match &mode {
                    ExternalIntegrationMode::Recommended => capability.recommended_access.clone(),
                    ExternalIntegrationMode::DiscoverOnly => {
                        ExternalIntegrationAccess::DiscoverOnly
                    }
                    ExternalIntegrationMode::Disabled | ExternalIntegrationMode::Unknown(_) => {
                        ExternalIntegrationAccess::Disabled
                    }
                    ExternalIntegrationMode::Custom => configured
                        .cloned()
                        .unwrap_or(ExternalIntegrationAccess::DiscoverOnly),
                }
            };
            let (effective, limited) = requested.at_most(capability.safety_ceiling.clone());
            if limited {
                policy_limited_capabilities.insert(capability.capability_id.clone());
            }
            capabilities.insert(capability.capability_id.clone(), effective);
        }
        ecosystems.insert(
            descriptor.ecosystem_id.clone(),
            EffectiveExternalEcosystemPolicy {
                ecosystem_id: descriptor.ecosystem_id.clone(),
                mode,
                capabilities,
                policy_limited_capabilities,
            },
        );
    }
    Ok(EffectiveExternalIntegrationPolicy {
        enabled,
        ecosystems,
    })
}

pub fn external_integration_policy_snapshot(
    document: &ExternalIntegrationPolicyDocument,
    workspace_key: Option<&str>,
    registered_ecosystems: Vec<ExternalIntegrationEcosystemDescriptor>,
) -> Result<ExternalIntegrationPolicySnapshot, ExternalSourceContractError> {
    validate_registered_ecosystems(&registered_ecosystems)?;
    let workspace_override = workspace_key
        .and_then(|key| document.workspace_overrides.get(key))
        .map(ExternalIntegrationPolicyOverrideView::from);
    let (status, global_effective, effective) = match (
        evaluate_external_integration_policy(document, None, &registered_ecosystems),
        evaluate_external_integration_policy(document, workspace_key, &registered_ecosystems),
    ) {
        (Ok(global_effective), Ok(effective)) => (
            ExternalIntegrationPolicyStatus::Compatible,
            global_effective,
            effective,
        ),
        (
            Err(ExternalSourceContractError::UnsupportedPolicySchemaMajor(_)),
            Err(ExternalSourceContractError::UnsupportedPolicySchemaMajor(_)),
        ) => {
            return incompatible_external_integration_policy_snapshot(
                document.schema_major,
                registered_ecosystems,
            )
        }
        (Err(error), _) | (_, Err(error)) => return Err(error),
    };
    Ok(ExternalIntegrationPolicySnapshot {
        schema_major: document.schema_major,
        status,
        user_defaults: ExternalIntegrationPolicySettingsView::from(&document.user_defaults),
        workspace_override,
        global_effective,
        effective,
        registered_ecosystems,
    })
}

/// Build a public, fail-closed projection for a policy document whose major
/// schema is not understood by this Host. The raw document stays exclusively
/// at the persistence boundary and is never reflected through Host APIs.
pub fn incompatible_external_integration_policy_snapshot(
    schema_major: u32,
    registered_ecosystems: Vec<ExternalIntegrationEcosystemDescriptor>,
) -> Result<ExternalIntegrationPolicySnapshot, ExternalSourceContractError> {
    validate_registered_ecosystems(&registered_ecosystems)?;
    let disabled = || EffectiveExternalIntegrationPolicy {
        enabled: false,
        ecosystems: registered_ecosystems
            .iter()
            .map(|descriptor| {
                (
                    descriptor.ecosystem_id.clone(),
                    EffectiveExternalEcosystemPolicy {
                        ecosystem_id: descriptor.ecosystem_id.clone(),
                        mode: ExternalIntegrationMode::Disabled,
                        capabilities: descriptor
                            .capabilities
                            .iter()
                            .map(|capability| {
                                (
                                    capability.capability_id.clone(),
                                    ExternalIntegrationAccess::Disabled,
                                )
                            })
                            .collect(),
                        policy_limited_capabilities: descriptor
                            .capabilities
                            .iter()
                            .map(|capability| capability.capability_id.clone())
                            .collect(),
                    },
                )
            })
            .collect(),
    };
    Ok(ExternalIntegrationPolicySnapshot {
        schema_major,
        status: ExternalIntegrationPolicyStatus::IncompatibleSchema,
        user_defaults: ExternalIntegrationPolicySettingsView {
            enabled: false,
            ecosystems: BTreeMap::new(),
        },
        workspace_override: None,
        global_effective: disabled(),
        effective: disabled(),
        registered_ecosystems,
    })
}
