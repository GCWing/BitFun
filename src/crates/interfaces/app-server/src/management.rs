//! App Server-owned management capability boundary.

use openbitfun_app_server_protocol::app::{CapabilityAvailability, CapabilityDescriptor};

mod owner;
mod worktree;

pub use owner::AppManagementService;

// These are existing wire IDs. App Server now owns their implementation, but
// changing the values would break capability negotiation with older clients.
pub const MODES_CAPABILITY: &str = "tui.modes";
pub const MODELS_CAPABILITY: &str = "tui.models";
pub const SKILLS_CAPABILITY: &str = "tui.skills";
pub const SUBAGENTS_CAPABILITY: &str = "tui.subagents";
pub const MCP_CAPABILITY: &str = "tui.mcp";
pub const EXTERNAL_SOURCES_CAPABILITY: &str = "tui.externalSources";
pub const NATIVE_HOOKS_CAPABILITY: &str = "tui.nativeHooks";
pub const EXTERNAL_HOOKS_CAPABILITY: &str = "tui.externalHooks";
pub const ACCOUNT_CAPABILITY: &str = "tui.account";
pub const SETTINGS_SYNC_CAPABILITY: &str = "tui.settingsSync";
pub const WORKTREES_CAPABILITY: &str = "tui.worktrees";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManagementCapabilities {
    pub modes: CapabilityAvailability,
    pub models: CapabilityAvailability,
    pub skills: CapabilityAvailability,
    pub subagents: CapabilityAvailability,
    pub mcp: CapabilityAvailability,
    pub external_sources: CapabilityAvailability,
    pub native_hooks: CapabilityAvailability,
    pub external_hooks: CapabilityAvailability,
    pub account: CapabilityAvailability,
    pub settings_sync: CapabilityAvailability,
    pub worktrees: CapabilityAvailability,
}

impl AppManagementCapabilities {
    pub fn available() -> Self {
        Self {
            modes: CapabilityAvailability::Available,
            models: CapabilityAvailability::Available,
            skills: CapabilityAvailability::Available,
            subagents: CapabilityAvailability::Available,
            mcp: CapabilityAvailability::Available,
            external_sources: CapabilityAvailability::Available,
            native_hooks: CapabilityAvailability::Available,
            external_hooks: CapabilityAvailability::Available,
            account: CapabilityAvailability::Available,
            settings_sync: CapabilityAvailability::Available,
            worktrees: CapabilityAvailability::Available,
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
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

    pub fn availability(&self, capability: &str) -> Option<&CapabilityAvailability> {
        match capability {
            MODES_CAPABILITY => Some(&self.modes),
            MODELS_CAPABILITY => Some(&self.models),
            SKILLS_CAPABILITY => Some(&self.skills),
            SUBAGENTS_CAPABILITY => Some(&self.subagents),
            MCP_CAPABILITY => Some(&self.mcp),
            EXTERNAL_SOURCES_CAPABILITY => Some(&self.external_sources),
            NATIVE_HOOKS_CAPABILITY => Some(&self.native_hooks),
            EXTERNAL_HOOKS_CAPABILITY => Some(&self.external_hooks),
            ACCOUNT_CAPABILITY => Some(&self.account),
            SETTINGS_SYNC_CAPABILITY => Some(&self.settings_sync),
            WORKTREES_CAPABILITY => Some(&self.worktrees),
            _ => None,
        }
    }

    pub fn descriptors(&self) -> Vec<CapabilityDescriptor> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppManagementErrorKind {
    Unsupported,
    InvalidRequest,
    NotFound,
    Internal,
}

#[derive(Debug, Clone)]
pub struct AppManagementError {
    pub kind: AppManagementErrorKind,
    pub message: String,
}

impl AppManagementError {
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            kind: AppManagementErrorKind::Unsupported,
            message: message.into(),
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            kind: AppManagementErrorKind::InvalidRequest,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: AppManagementErrorKind::NotFound,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            kind: AppManagementErrorKind::Internal,
            message: message.into(),
        }
    }
}

pub type AppManagementResult<T> = Result<T, AppManagementError>;
