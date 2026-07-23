use super::keys::normalize_skill_keys;
use super::policy::resolve_builtin_default_enabled;
use super::types::{ModeSkillStateReason, SkillInfo, SkillLocation};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserModeSkillOverrides {
    pub disabled_skills: Vec<String>,
    pub enabled_skills: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeSkillState {
    pub default_enabled: bool,
    pub effective_enabled: bool,
    pub reason: ModeSkillStateReason,
}

pub fn normalize_user_mode_skill_overrides(
    disabled_skills: Vec<String>,
    enabled_skills: Vec<String>,
) -> UserModeSkillOverrides {
    let disabled_skills = normalize_skill_keys(disabled_skills);
    let disabled_set: HashSet<String> = disabled_skills.iter().cloned().collect();
    let mut enabled_skills = normalize_skill_keys(enabled_skills);
    enabled_skills.retain(|key| !disabled_set.contains(key));

    UserModeSkillOverrides {
        disabled_skills,
        enabled_skills,
    }
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
