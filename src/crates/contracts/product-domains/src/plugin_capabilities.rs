//! Provider-neutral capability contributions produced by executable plugin adapters.
//!
//! Ecosystem-specific modules retain source parsing, contributor attribution,
//! validation, Host protocols, and lifecycle. Product assembly consumes these
//! values to publish existing Agent and Skill capabilities without receiving
//! raw ecosystem configuration or depending on an adapter-specific DTO.

use crate::external_subagents::ExternalSubagentMode;
use crate::tool_permissions::PermissionConstraintLayer;
use std::path::PathBuf;

/// Adapter-defined identity for one plugin contributor.
///
/// `identity_key` scopes ownership and equality. `behavior_key` is the stable
/// input used by product publication to preserve runtime identity across
/// adapter refactors. They are separate because two contributors may expose
/// the same behavior identity while remaining distinct owners.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PluginContributorIdentity {
    identity_key: String,
    behavior_key: String,
    label: String,
}

impl PluginContributorIdentity {
    pub fn new(
        identity_key: impl Into<String>,
        behavior_key: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            identity_key: identity_key.into(),
            behavior_key: behavior_key.into(),
            label: label.into(),
        }
    }

    pub fn identity_key(&self) -> &str {
        &self.identity_key
    }

    pub fn behavior_key(&self) -> &str {
        &self.behavior_key
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PluginToolRef {
    contributor: PluginContributorIdentity,
    id: String,
}

impl PluginToolRef {
    pub fn new(contributor: PluginContributorIdentity, id: impl Into<String>) -> Self {
        Self {
            contributor,
            id: id.into(),
        }
    }

    pub fn contributor(&self) -> &PluginContributorIdentity {
        &self.contributor
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone)]
pub struct PluginAgentProjection {
    pub contributor: PluginContributorIdentity,
    pub logical_id: String,
    pub description: String,
    pub prompt: String,
    pub mode: ExternalSubagentMode,
    pub hidden: bool,
    pub temperature: Option<f64>,
    pub permission_constraints: PermissionConstraintLayer,
    pub plugin_tools: Vec<PluginToolRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSkillRootContribution {
    pub path: PathBuf,
    pub precedence: usize,
}

#[derive(Debug, Clone, Default)]
pub struct PluginCapabilityProjection {
    pub agents: Vec<PluginAgentProjection>,
    pub skill_roots: Vec<PluginSkillRootContribution>,
}
