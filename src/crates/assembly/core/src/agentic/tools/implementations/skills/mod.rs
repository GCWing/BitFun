//! Skill management module
//!
//! Provides Skill registry, loading, and configuration management functionality

pub mod builtin;
pub mod catalog;
pub mod mode_overrides;
pub mod policy;
pub mod registry;
pub mod resolver;
pub mod types;

pub use registry::SkillRegistry;
pub use types::{ModeSkillInfo, ModeSkillStateReason, SkillData, SkillInfo, SkillLocation};

/// Get global Skill registry instance
pub fn get_skill_registry() -> &'static SkillRegistry {
    SkillRegistry::global()
}
