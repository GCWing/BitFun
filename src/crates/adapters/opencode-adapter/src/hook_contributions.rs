//! Pure translation from statically discovered OpenCode tool Hook descriptors
//! to ecosystem-neutral source-projection declarations.
//!
//! This module does not load or invoke Hook handlers. The production caller is
//! the existing managed-package read projection in `source_adapter`.

use bitfun_product_domains::external_hook_contributions::{
    ExternalHookContributionDeclaration, ExternalHookContributionId, ExternalHookPoint,
    ExternalHookRiskCapability, ExternalHookSafetyDeclaration,
};
use bitfun_product_domains::external_sources::{ExternalSourceContractError, SourceKey};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::fmt;

pub(crate) const OPENCODE_PLUGIN_PROVIDER_ID: &str = "opencode.plugins";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OpenCodeToolHookEvent {
    Before,
    After,
}

impl OpenCodeToolHookEvent {
    fn from_event_name(value: &str) -> Option<Self> {
        match value {
            "tool.execute.before" => Some(Self::Before),
            "tool.execute.after" => Some(Self::After),
            _ => None,
        }
    }

    fn stable_discriminator(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OpenCodeLocalHookHandlerId(ExternalHookContributionId);

impl OpenCodeLocalHookHandlerId {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ExternalSourceContractError> {
        ExternalHookContributionId::new(value).map(Self)
    }

    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenCodeHookDescriptor {
    event: OpenCodeToolHookEvent,
    local_handler_id: OpenCodeLocalHookHandlerId,
}

impl OpenCodeHookDescriptor {
    pub(crate) fn new(
        event: OpenCodeToolHookEvent,
        local_handler_id: OpenCodeLocalHookHandlerId,
    ) -> Self {
        Self {
            event,
            local_handler_id,
        }
    }

    /// Converts one event key found by the existing static TypeScript source
    /// projection. Other OpenCode Hooks remain projection-only diagnostics.
    pub(crate) fn from_static_projection_event(event_name: &str) -> Option<Self> {
        let event = OpenCodeToolHookEvent::from_event_name(event_name)?;
        Some(Self::new(
            event,
            OpenCodeLocalHookHandlerId::new(event_name)
                .expect("static OpenCode Hook event name must be a valid identifier"),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenCodeHookMappingError {
    UnexpectedProvider,
    DuplicateDescriptorIdentity,
}

impl OpenCodeHookMappingError {
    /// Stable low-cardinality code for logs and diagnostics.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::UnexpectedProvider => "unexpected_provider",
            Self::DuplicateDescriptorIdentity => "duplicate_descriptor_identity",
        }
    }
}

impl fmt::Display for OpenCodeHookMappingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for OpenCodeHookMappingError {}

pub(crate) fn map_hook_contributions(
    source_key: SourceKey,
    descriptors: Vec<OpenCodeHookDescriptor>,
) -> Result<Vec<ExternalHookContributionDeclaration>, OpenCodeHookMappingError> {
    if source_key.provider_id.as_str() != OPENCODE_PLUGIN_PROVIDER_ID {
        return Err(OpenCodeHookMappingError::UnexpectedProvider);
    }

    let mut identities = HashSet::new();
    if descriptors.iter().any(|descriptor| {
        !identities.insert((descriptor.event, descriptor.local_handler_id.clone()))
    }) {
        return Err(OpenCodeHookMappingError::DuplicateDescriptorIdentity);
    }

    Ok(descriptors
        .into_iter()
        .map(|descriptor| {
            let (hook_point, declared_risks) = match descriptor.event {
                OpenCodeToolHookEvent::Before => (
                    ExternalHookPoint::ToolBefore,
                    BTreeSet::from([
                        ExternalHookRiskCapability::ReadToolArguments,
                        ExternalHookRiskCapability::ModifyToolArguments,
                    ]),
                ),
                OpenCodeToolHookEvent::After => (
                    ExternalHookPoint::ToolAfter,
                    BTreeSet::from([
                        ExternalHookRiskCapability::ReadToolArguments,
                        ExternalHookRiskCapability::ReadToolResult,
                        ExternalHookRiskCapability::ModifyToolResult,
                    ]),
                ),
            };
            let local_handler_id = descriptor.local_handler_id.as_str();

            ExternalHookContributionDeclaration {
                contribution_id: ExternalHookContributionId::new(stable_id(
                    "opencode-hook-contribution-v1",
                    &source_key,
                    local_handler_id,
                    descriptor.event,
                ))
                .expect("hashed OpenCode contribution id must be valid"),
                hook_point,
                safety: ExternalHookSafetyDeclaration {
                    declared_risks,
                    complete: false,
                },
            }
        })
        .collect())
}

fn stable_id(
    domain: &str,
    source_key: &SourceKey,
    local_handler_id: &str,
    event: OpenCodeToolHookEvent,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(source_key.stable_key().as_bytes());
    hasher.update([0]);
    hasher.update(local_handler_id.as_bytes());
    hasher.update([0]);
    hasher.update(event.stable_discriminator().as_bytes());
    format!("{domain}:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::{
        map_hook_contributions, OpenCodeHookDescriptor, OpenCodeHookMappingError,
        OpenCodeLocalHookHandlerId, OpenCodeToolHookEvent, OPENCODE_PLUGIN_PROVIDER_ID,
    };
    use bitfun_product_domains::external_hook_contributions::{
        ExternalHookPoint, ExternalHookRiskCapability,
    };
    use bitfun_product_domains::external_sources::SourceKey;
    use std::collections::BTreeSet;

    fn source(source_id: &str) -> SourceKey {
        SourceKey::new(OPENCODE_PLUGIN_PROVIDER_ID, source_id).unwrap()
    }

    fn descriptor(event: OpenCodeToolHookEvent, handler: &str) -> OpenCodeHookDescriptor {
        OpenCodeHookDescriptor::new(event, OpenCodeLocalHookHandlerId::new(handler).unwrap())
    }

    #[test]
    fn maps_before_and_after_with_exact_incomplete_risk_facts() {
        let declarations = map_hook_contributions(
            source("project-plugin"),
            vec![
                descriptor(OpenCodeToolHookEvent::Before, "before-handler"),
                descriptor(OpenCodeToolHookEvent::After, "after-handler"),
            ],
        )
        .unwrap();

        assert_eq!(declarations[0].hook_point, ExternalHookPoint::ToolBefore);
        assert_eq!(
            declarations[0].safety.declared_risks,
            BTreeSet::from([
                ExternalHookRiskCapability::ReadToolArguments,
                ExternalHookRiskCapability::ModifyToolArguments,
            ])
        );
        assert_eq!(declarations[1].hook_point, ExternalHookPoint::ToolAfter);
        assert_eq!(
            declarations[1].safety.declared_risks,
            BTreeSet::from([
                ExternalHookRiskCapability::ReadToolArguments,
                ExternalHookRiskCapability::ReadToolResult,
                ExternalHookRiskCapability::ModifyToolResult,
            ])
        );
        assert!(declarations.iter().all(|item| !item.safety.complete));
    }

    #[test]
    fn discovered_event_mapping_is_closed_to_reviewed_tool_hooks() {
        assert!(
            OpenCodeHookDescriptor::from_static_projection_event("tool.execute.before").is_some()
        );
        assert!(
            OpenCodeHookDescriptor::from_static_projection_event("tool.execute.after").is_some()
        );
        assert!(
            OpenCodeHookDescriptor::from_static_projection_event("session.compacted").is_none()
        );
    }

    #[test]
    fn mapper_rejects_duplicate_identity_and_wrong_provider() {
        let duplicate = || descriptor(OpenCodeToolHookEvent::Before, "shared-handler");
        assert_eq!(
            map_hook_contributions(source("project-plugin"), vec![duplicate(), duplicate()]),
            Err(OpenCodeHookMappingError::DuplicateDescriptorIdentity)
        );
        assert_eq!(
            map_hook_contributions(
                SourceKey::new("codex.plugins", "project-plugin").unwrap(),
                vec![duplicate()],
            ),
            Err(OpenCodeHookMappingError::UnexpectedProvider)
        );
    }

    #[test]
    fn stable_ids_are_source_qualified_and_event_specific() {
        let first = map_hook_contributions(
            source("first-plugin"),
            vec![
                descriptor(OpenCodeToolHookEvent::Before, "handler"),
                descriptor(OpenCodeToolHookEvent::After, "handler"),
            ],
        )
        .unwrap();
        let second = map_hook_contributions(
            source("second-plugin"),
            vec![descriptor(OpenCodeToolHookEvent::Before, "handler")],
        )
        .unwrap();

        assert_ne!(first[0].contribution_id, first[1].contribution_id);
        assert_ne!(first[0].contribution_id, second[0].contribution_id);
    }
}
