//! Stable hook categories and sources used by the portable hook registry.

use super::settings::AgentHookEvent;
use std::fmt;

/// The Codex lifecycle events plus BitFun/OpenCode execution categories.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RuntimeHookKind {
    Lifecycle(AgentHookEvent),
    SuccessfulToolPostCall,
    PluginHook(String),
}

/// Origin of a registered hook.  The declaration order is the stable source
/// precedence used when snapshots are sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RuntimeHookSource {
    Builtin { priority: u16 },
    UserCommand,
    ProjectCommand,
    ImportedCommand,
    OpenCodePlugin,
}

impl RuntimeHookSource {
    pub const fn is_open_code_plugin(self) -> bool {
        matches!(self, Self::OpenCodePlugin)
    }
}

impl fmt::Display for RuntimeHookSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin { .. } => f.write_str("builtin"),
            Self::UserCommand => f.write_str("user-command"),
            Self::ProjectCommand => f.write_str("project-command"),
            Self::ImportedCommand => f.write_str("imported-command"),
            Self::OpenCodePlugin => f.write_str("opencode-plugin"),
        }
    }
}
