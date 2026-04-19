//! Default built-in skill profiles per mode.

use super::mode_overrides::UserModeSkillOverrides;
use super::types::{SkillInfo, SkillLocation};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuiltinSkillProfile {
    /// Baseline state for built-in skills in this mode.
    default_enabled: bool,
    /// Built-in skill directory names whose state differs from `default_enabled`.
    overridden_skills: &'static [&'static str],
}

const ENABLE_ALL_BUILTINS: BuiltinSkillProfile = BuiltinSkillProfile {
    default_enabled: true,
    overridden_skills: &[],
};

const DISABLE_ALL_BUILTINS: BuiltinSkillProfile = BuiltinSkillProfile {
    default_enabled: false,
    overridden_skills: &[],
};

const AGENTIC_PROFILE: BuiltinSkillProfile = BuiltinSkillProfile {
    default_enabled: true,
    overridden_skills: &["docx", "pdf", "pptx", "xlsx"],
};

const COWORK_PROFILE: BuiltinSkillProfile = BuiltinSkillProfile {
    default_enabled: false,
    overridden_skills: &[
        "docx",
        "pdf",
        "pptx",
        "xlsx",
        "find-skills",
        "writing-skills",
    ],
};

const DESIGN_PROFILE: BuiltinSkillProfile = BuiltinSkillProfile {
    default_enabled: false,
    overridden_skills: &[
        "docx",
        "pdf",
        "pptx",
        "xlsx",
        "find-skills",
        "writing-skills",
    ],
};

fn builtin_profile_for_mode(mode_id: &str) -> BuiltinSkillProfile {
    match mode_id {
        "Plan" | "debug" => DISABLE_ALL_BUILTINS,
        "agentic" => AGENTIC_PROFILE,
        "Cowork" => COWORK_PROFILE,
        "Design" => DESIGN_PROFILE,
        _ => ENABLE_ALL_BUILTINS,
    }
}

pub fn is_enabled_by_default_for_mode(skill: &SkillInfo, mode_id: &str) -> bool {
    if skill.level != SkillLocation::User || !skill.is_builtin {
        return true;
    }

    let profile = builtin_profile_for_mode(mode_id);
    if profile.overridden_skills.contains(&skill.dir_name.as_str()) {
        !profile.default_enabled
    } else {
        profile.default_enabled
    }
}

pub fn is_skill_enabled_for_mode(
    skill: &SkillInfo,
    mode_id: &str,
    user_overrides: &UserModeSkillOverrides,
    disabled_project_skills: &HashSet<String>,
) -> bool {
    match skill.level {
        SkillLocation::Project => !disabled_project_skills.contains(&skill.key),
        SkillLocation::User => {
            let default_enabled = is_enabled_by_default_for_mode(skill, mode_id);

            if default_enabled {
                !user_overrides.disabled_skills.contains(&skill.key)
            } else {
                user_overrides.enabled_skills.contains(&skill.key)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_enabled_by_default_for_mode, is_skill_enabled_for_mode};
    use crate::agentic::tools::implementations::skills::mode_overrides::UserModeSkillOverrides;
    use crate::agentic::tools::implementations::skills::types::{SkillInfo, SkillLocation};
    use std::collections::HashSet;

    fn builtin_skill(dir_name: &str) -> SkillInfo {
        SkillInfo {
            key: format!("user::bitfun::{}", dir_name),
            name: dir_name.to_string(),
            description: String::new(),
            path: format!("/tmp/{}", dir_name),
            level: SkillLocation::User,
            source_slot: "bitfun".to_string(),
            dir_name: dir_name.to_string(),
            is_builtin: true,
            group_key: None,
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
        }
    }

    #[test]
    fn builtin_defaults_follow_mode_profiles() {
        let pdf = builtin_skill("pdf");
        let tdd = builtin_skill("test-driven-development");

        assert!(!is_enabled_by_default_for_mode(&pdf, "agentic"));
        assert!(is_enabled_by_default_for_mode(&tdd, "agentic"));
        assert!(is_enabled_by_default_for_mode(&pdf, "Cowork"));
        assert!(!is_enabled_by_default_for_mode(&tdd, "Cowork"));
        assert!(is_enabled_by_default_for_mode(&pdf, "Design"));
        assert!(!is_enabled_by_default_for_mode(&tdd, "Design"));
        assert!(!is_enabled_by_default_for_mode(&pdf, "Plan"));
        assert!(!is_enabled_by_default_for_mode(&tdd, "debug"));
    }

    #[test]
    fn non_builtin_user_skills_remain_enabled_by_default() {
        let custom = custom_user_skill("my-custom-skill");
        assert!(is_enabled_by_default_for_mode(&custom, "agentic"));
        assert!(is_enabled_by_default_for_mode(&custom, "Plan"));
    }

    #[test]
    fn user_overrides_apply_on_top_of_defaults() {
        let pdf = builtin_skill("pdf");
        let mut overrides = UserModeSkillOverrides::default();
        let disabled_project = HashSet::new();

        assert!(!is_skill_enabled_for_mode(
            &pdf,
            "agentic",
            &overrides,
            &disabled_project,
        ));

        overrides.enabled_skills.push(pdf.key.clone());
        assert!(is_skill_enabled_for_mode(
            &pdf,
            "agentic",
            &overrides,
            &disabled_project,
        ));
    }
}
