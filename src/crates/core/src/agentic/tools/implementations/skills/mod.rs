//! Skill management module
//!
//! Provides Skill registry, loading, and configuration management functionality

pub mod builtin;
pub mod registry;
pub mod types;

pub use registry::SkillRegistry;
pub use types::{SkillData, SkillInfo, SkillLocation};

/// Get global Skill registry instance
pub fn get_skill_registry() -> &'static SkillRegistry {
    SkillRegistry::global()
}
