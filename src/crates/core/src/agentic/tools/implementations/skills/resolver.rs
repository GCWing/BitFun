//! Skill resolution helpers.
//!
//! This module combines the built-in policy layer with user/project overrides
//! and produces a single effective availability decision for a skill in a mode.

use super::mode_overrides::UserModeSkillOverrides;
use super::policy::resolve_builtin_default_enabled;
use super::types::{ModeSkillStateReason, SkillInfo, SkillLocation};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeSkillState {
    pub default_enabled: bool,
    pub effective_enabled: bool,
    pub reason: ModeSkillStateReason,
}

pub fn resolve_skill_default_enabled_for_mode(skill: &SkillInfo, mode_id: &str) -> bool {
    match skill.level {
        SkillLocation::Project => true,
        SkillLocation::User => {
            if !skill.is_builtin {
                true
            } else {
                resolve_builtin_default_enabled(&skill.dir_name, mode_id).unwrap_or(true)
            }
        }
    }
}

fn resolve_default_state_for_user_skill(skill: &SkillInfo, mode_id: &str) -> ModeSkillState {
    if !skill.is_builtin {
        return ModeSkillState {
            default_enabled: true,
            effective_enabled: true,
            reason: ModeSkillStateReason::CustomUserDefaultEnabled,
        };
    }

    let default_enabled = resolve_builtin_default_enabled(&skill.dir_name, mode_id).unwrap_or(true);
    ModeSkillState {
        default_enabled,
        effective_enabled: default_enabled,
        reason: if default_enabled {
            ModeSkillStateReason::BuiltinPolicyEnabled
        } else {
            ModeSkillStateReason::BuiltinPolicyDisabled
        },
    }
}

pub fn resolve_skill_state_for_mode(
    skill: &SkillInfo,
    mode_id: &str,
    user_overrides: &UserModeSkillOverrides,
    disabled_project_skills: &HashSet<String>,
) -> ModeSkillState {
    match skill.level {
        SkillLocation::Project => {
            let disabled = disabled_project_skills.contains(&skill.key);
            ModeSkillState {
                default_enabled: true,
                effective_enabled: !disabled,
                reason: if disabled {
                    ModeSkillStateReason::DisabledByProjectOverride
                } else {
                    ModeSkillStateReason::ProjectDefaultEnabled
                },
            }
        }
        SkillLocation::User => {
            let default_state = resolve_default_state_for_user_skill(skill, mode_id);

            if default_state.default_enabled {
                if user_overrides.disabled_skills.contains(&skill.key) {
                    return ModeSkillState {
                        default_enabled: true,
                        effective_enabled: false,
                        reason: ModeSkillStateReason::DisabledByUserOverride,
                    };
                }
            } else if user_overrides.enabled_skills.contains(&skill.key) {
                return ModeSkillState {
                    default_enabled: false,
                    effective_enabled: true,
                    reason: ModeSkillStateReason::EnabledByUserOverride,
                };
            }

            default_state
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_skill_default_enabled_for_mode, resolve_skill_state_for_mode};
    use crate::agentic::tools::implementations::skills::mode_overrides::UserModeSkillOverrides;
    use crate::agentic::tools::implementations::skills::types::{
        ModeSkillStateReason, SkillInfo, SkillLocation,
    };
    use std::collections::HashSet;

    fn builtin_skill(dir_name: &str) -> SkillInfo {
        SkillInfo {
            key: format!("user::bitfun-system::{}", dir_name),
            name: dir_name.to_string(),
            description: String::new(),
            path: format!("/tmp/{}", dir_name),
            level: SkillLocation::User,
            source_slot: "bitfun-system".to_string(),
            dir_name: dir_name.to_string(),
            is_builtin: true,
            group_key: None,
            is_shadowed: false,
            shadowed_by_key: None,
        }
    }

    fn custom_user_skill(dir_name: &str) -> SkillInfo {
        SkillInfo {
            key: format!("user::bitfun::{}", dir_name),
            name: dir_name.to_string(),
            description: String::new(),
            path: format!("/tmp/{}", dir_name),
            level: SkillLocation::User,
            source_slot: "bitfun".to_string(),
            dir_name: dir_name.to_string(),
            is_builtin: false,
            group_key: None,
            is_shadowed: false,
            shadowed_by_key: None,
        }
    }

    #[test]
    fn builtin_default_state_follows_policy() {
        let pdf = builtin_skill("pdf");
        let browser = builtin_skill("agent-browser");

        assert!(!resolve_skill_default_enabled_for_mode(&pdf, "agentic"));
        assert!(resolve_skill_default_enabled_for_mode(&browser, "agentic"));
        assert!(resolve_skill_default_enabled_for_mode(&pdf, "Cowork"));
        assert!(!resolve_skill_default_enabled_for_mode(&browser, "Cowork"));
    }

    #[test]
    fn custom_user_skills_are_enabled_by_default() {
        let custom = custom_user_skill("my-custom-skill");
        let state = resolve_skill_state_for_mode(
            &custom,
            "agentic",
            &UserModeSkillOverrides::default(),
            &HashSet::new(),
        );

        assert!(state.default_enabled);
        assert!(state.effective_enabled);
        assert_eq!(state.reason, ModeSkillStateReason::CustomUserDefaultEnabled);
    }

    #[test]
    fn overrides_apply_on_top_of_defaults() {
        let pdf = builtin_skill("pdf");
        let mut overrides = UserModeSkillOverrides::default();
        let disabled_project = HashSet::new();

        let disabled_state =
            resolve_skill_state_for_mode(&pdf, "agentic", &overrides, &disabled_project);
        assert!(!disabled_state.effective_enabled);
        assert_eq!(
            disabled_state.reason,
            ModeSkillStateReason::BuiltinPolicyDisabled
        );

        overrides.enabled_skills.push(pdf.key.clone());
        let enabled_state =
            resolve_skill_state_for_mode(&pdf, "agentic", &overrides, &disabled_project);
        assert!(enabled_state.effective_enabled);
        assert_eq!(
            enabled_state.reason,
            ModeSkillStateReason::EnabledByUserOverride
        );
    }
}
