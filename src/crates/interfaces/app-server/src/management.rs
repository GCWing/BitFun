//! Host-injected management service and capability boundary.

use bitfun_app_server_protocol::app::{CapabilityAvailability, CapabilityDescriptor};

mod service;

pub use service::AppManagementService;

pub const MODES_CAPABILITY: &str = "tui.modes";
pub const MODELS_CAPABILITY: &str = "tui.models";
pub const SKILLS_CAPABILITY: &str = "tui.skills";
pub const SUBAGENTS_CAPABILITY: &str = "tui.subagents";
pub const MCP_CAPABILITY: &str = "tui.mcp";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManagementCapabilities {
    pub modes: CapabilityAvailability,
    pub models: CapabilityAvailability,
    pub skills: CapabilityAvailability,
    pub subagents: CapabilityAvailability,
    pub mcp: CapabilityAvailability,
}

impl AppManagementCapabilities {
    pub fn available() -> Self {
        Self {
            modes: CapabilityAvailability::Available,
            models: CapabilityAvailability::Available,
            skills: CapabilityAvailability::Available,
            subagents: CapabilityAvailability::Available,
            mcp: CapabilityAvailability::Available,
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
        }
    }

    pub fn availability(&self, capability: &str) -> Option<&CapabilityAvailability> {
        match capability {
            MODES_CAPABILITY => Some(&self.modes),
            MODELS_CAPABILITY => Some(&self.models),
            SKILLS_CAPABILITY => Some(&self.skills),
            SUBAGENTS_CAPABILITY => Some(&self.subagents),
            MCP_CAPABILITY => Some(&self.mcp),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_follow_host_reported_availability() {
        let reason = "owner unavailable";
        let capabilities = AppManagementCapabilities::unavailable(reason);
        let descriptors = capabilities.descriptors();

        assert_eq!(descriptors.len(), 5);
        for descriptor in descriptors {
            assert!(matches!(
                descriptor.availability,
                CapabilityAvailability::Unavailable { ref reason } if reason == "owner unavailable"
            ));
        }
    }
}
