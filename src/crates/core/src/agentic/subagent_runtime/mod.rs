//! Generic subagent runtime primitives.
//!
//! This module is intentionally smaller than a scheduler.  It may contain
//! shared mechanics that are already proven generic across hidden subagent
//! execution paths, but it must not import Deep Review modules or define
//! Deep Review product policy.

pub(crate) mod queue_timing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct DelegationPolicy {
    pub allow_subagent_spawn: bool,
    pub nesting_depth: u8,
}

impl Default for DelegationPolicy {
    fn default() -> Self {
        Self::top_level()
    }
}

impl DelegationPolicy {
    pub(crate) fn top_level() -> Self {
        Self {
            allow_subagent_spawn: true,
            nesting_depth: 0,
        }
    }

    pub(crate) fn spawn_child(self) -> Self {
        Self {
            allow_subagent_spawn: false,
            nesting_depth: self.nesting_depth.saturating_add(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SubagentContextMode {
    #[default]
    Fresh,
    Fork,
}

impl SubagentContextMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Fork => "fork",
        }
    }
}
