//! Compatibility re-export for skill availability resolution.
//!
//! The provider-neutral owner lives in `bitfun-agent-runtime`.

pub use bitfun_agent_runtime::skills::{
    normalize_user_mode_skill_overrides, resolve_skill_default_enabled_for_mode,
    resolve_skill_state_for_mode, ModeSkillState,
};

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
            source_id: "bitfun".to_string(),
            source_label: "BitFun".to_string(),
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
            source_id: "bitfun".to_string(),
            source_label: "BitFun".to_string(),
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
