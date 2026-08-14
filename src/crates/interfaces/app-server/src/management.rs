//! Structured unavailable capability projection for management methods.
//!
//! No production Host currently provides these owners. Keep the wire methods
//! fail-closed; a future Host must add a scoped adapter with a real consumer.

use bitfun_app_server_protocol::app::{CapabilityAvailability, CapabilityDescriptor};

pub(crate) const MODES_CAPABILITY: &str = "tui.modes";
pub(crate) const MODELS_CAPABILITY: &str = "tui.models";
pub(crate) const SKILLS_CAPABILITY: &str = "tui.skills";
pub(crate) const SUBAGENTS_CAPABILITY: &str = "tui.subagents";
pub(crate) const MCP_CAPABILITY: &str = "tui.mcp";
pub(crate) const EXTERNAL_SOURCES_CAPABILITY: &str = "tui.externalSources";
pub(crate) const NATIVE_HOOKS_CAPABILITY: &str = "tui.nativeHooks";
pub(crate) const EXTERNAL_HOOKS_CAPABILITY: &str = "tui.externalHooks";
pub(crate) const ACCOUNT_CAPABILITY: &str = "tui.account";
pub(crate) const SETTINGS_SYNC_CAPABILITY: &str = "tui.settingsSync";
pub(crate) const WORKTREES_CAPABILITY: &str = "tui.worktrees";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppManagementCapabilities {
    pub(crate) modes: CapabilityAvailability,
    pub(crate) models: CapabilityAvailability,
    pub(crate) skills: CapabilityAvailability,
    pub(crate) subagents: CapabilityAvailability,
    pub(crate) mcp: CapabilityAvailability,
    pub(crate) external_sources: CapabilityAvailability,
    pub(crate) native_hooks: CapabilityAvailability,
    pub(crate) external_hooks: CapabilityAvailability,
    pub(crate) account: CapabilityAvailability,
    pub(crate) settings_sync: CapabilityAvailability,
    pub(crate) worktrees: CapabilityAvailability,
}

impl AppManagementCapabilities {
    pub(crate) fn unavailable(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            modes: unavailable(&reason),
            models: unavailable(&reason),
            skills: unavailable(&reason),
            subagents: unavailable(&reason),
            mcp: unavailable(&reason),
            external_sources: unavailable(&reason),
            native_hooks: unavailable(&reason),
            external_hooks: unavailable(&reason),
            account: unavailable(&reason),
            settings_sync: unavailable(&reason),
            worktrees: unavailable(&reason),
        }
    }

    pub(crate) fn descriptors(&self) -> Vec<CapabilityDescriptor> {
        vec![
            descriptor(MODES_CAPABILITY, self.modes.clone(), &["agent/listModes"]),
            descriptor(
                MODELS_CAPABILITY,
                self.models.clone(),
                &[
                    "config/getTuiModelCatalog",
                    "model/projectReasoningCatalog",
                    "model/list",
                    "model/get",
                    "model/add",
                    "model/update",
                    "model/delete",
                    "model/setDefault",
                ],
            ),
            descriptor(
                SKILLS_CAPABILITY,
                self.skills.clone(),
                &["skill/list", "skill/setEnabled"],
            ),
            descriptor(
                SUBAGENTS_CAPABILITY,
                self.subagents.clone(),
                &["subagent/list", "subagent/setEnabled"],
            ),
            descriptor(
                MCP_CAPABILITY,
                self.mcp.clone(),
                &[
                    "mcp/list",
                    "mcp/toggle",
                    "mcp/add",
                    "mcp/delete",
                    "mcp/externalDecision",
                    "mcp/conflictChoice",
                ],
            ),
            descriptor(
                EXTERNAL_SOURCES_CAPABILITY,
                self.external_sources.clone(),
                &[
                    "externalSource/snapshot",
                    "externalSource/control",
                    "externalSource/review",
                    "externalSource/setNativeCommandChoice",
                    "externalSource/expandCommand",
                    "externalSource/event",
                ],
            ),
            descriptor(
                NATIVE_HOOKS_CAPABILITY,
                self.native_hooks.clone(),
                &["nativeHook/overview"],
            ),
            descriptor(
                EXTERNAL_HOOKS_CAPABILITY,
                self.external_hooks.clone(),
                &[
                    "externalHook/snapshot",
                    "externalHook/plan",
                    "externalHook/apply",
                    "externalHook/mutate",
                ],
            ),
            descriptor(
                ACCOUNT_CAPABILITY,
                self.account.clone(),
                &[
                    "account/snapshot",
                    "account/login",
                    "account/finalizeLogin",
                    "account/logout",
                ],
            ),
            descriptor(
                SETTINGS_SYNC_CAPABILITY,
                self.settings_sync.clone(),
                &[
                    "settingsSync/start",
                    "settingsSync/snapshot",
                    "settingsSync/cancel",
                    "settingsSync/localChanged",
                ],
            ),
            descriptor(
                WORKTREES_CAPABILITY,
                self.worktrees.clone(),
                &[
                    "worktree/repositoryStatus",
                    "worktree/bindSession",
                    "worktree/releaseSession",
                ],
            ),
        ]
    }
}

fn unavailable(reason: &str) -> CapabilityAvailability {
    CapabilityAvailability::Unavailable {
        reason: reason.to_string(),
    }
}

fn descriptor(
    id: &str,
    availability: CapabilityAvailability,
    methods: &[&str],
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: id.to_string(),
        availability,
        methods: methods.iter().map(|method| (*method).to_string()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_follow_host_reported_availability() {
        let reason = "owner unavailable";
        let capabilities = AppManagementCapabilities::unavailable(reason);
        let descriptors = capabilities.descriptors();

        assert_eq!(descriptors.len(), 11);
        for descriptor in descriptors {
            assert!(matches!(
                descriptor.availability,
                CapabilityAvailability::Unavailable { ref reason } if reason == "owner unavailable"
            ));
        }
    }
}
