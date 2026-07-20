//! Compatibility re-export for skill catalog facts.
//!
//! The provider-neutral owner lives in `bitfun-agent-runtime`.

pub use bitfun_agent_runtime::skills::builtin_skill_group_key;

#[cfg(test)]
mod tests {
    use super::builtin_skill_group_key;
    use crate::agentic::tools::implementations::skills::builtin::builtin_skill_dir_names;

    #[test]
    fn builtin_skill_groups_match_expected_sets() {
        assert_eq!(builtin_skill_group_key("docx"), Some("office"));
        assert_eq!(builtin_skill_group_key("pdf"), Some("office"));
        assert_eq!(builtin_skill_group_key("ppt-design"), Some("office"));
        assert_eq!(builtin_skill_group_key("pptx"), Some("office"));
        assert_eq!(builtin_skill_group_key("xlsx"), Some("office"));
        assert_eq!(builtin_skill_group_key("find-skills"), Some("meta"));
        assert_eq!(builtin_skill_group_key("miniapp-dev"), Some("miniapp"));
        assert_eq!(builtin_skill_group_key("writing-skills"), Some("meta"));
        assert_eq!(
            builtin_skill_group_key("agent-browser"),
            Some("computer-use")
        );
        assert_eq!(builtin_skill_group_key("agent-eval-canvas"), Some("canvas"));
        assert_eq!(builtin_skill_group_key("gstack-review"), Some("gstack"));
        assert_eq!(builtin_skill_group_key("unknown-skill"), None);
    }

    #[test]
    fn runtime_catalog_covers_all_embedded_builtin_skills() {
        for dir_name in builtin_skill_dir_names() {
            assert!(
                builtin_skill_group_key(dir_name).is_some(),
                "Missing built-in skill catalog entry for '{}'",
                dir_name
            );
        }
    }
}
