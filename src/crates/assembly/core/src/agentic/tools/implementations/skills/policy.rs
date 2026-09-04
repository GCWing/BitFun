//! Compatibility re-export for mode-aware built-in skill policy.
//!
//! The provider-neutral owner lives in `openbitfun-agent-runtime`.

pub use openbitfun_agent_runtime::skills::resolve_builtin_default_enabled;

#[cfg(test)]
mod tests {
    use super::resolve_builtin_default_enabled;

    #[test]
    fn builtin_defaults_follow_mode_policies() {
        assert_eq!(
            resolve_builtin_default_enabled("ppt-design", "agentic"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("ppt-design", "Cowork"),
            Some(true)
        );
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "agentic"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "Cowork"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("find-skills", "DeepResearch"),
            Some(true)
        );
        assert_eq!(
            resolve_builtin_default_enabled("ppt-design", "DeepResearch"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "Claw"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("ppt-design", "Claw"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "coding_shared"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("ppt-design", "coding_shared"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("ppt-design", "Other"),
            Some(false)
        );
        for mode_id in ["agentic", "Claw", "Creative", "Cowork", "DeepResearch"] {
            assert_eq!(
                resolve_builtin_default_enabled("openbitfun-canvas", mode_id),
                Some(false),
                "Canvas skills must stay opt-in for mode {mode_id}"
            );
        }
    }

    #[test]
    fn unknown_builtins_return_none() {
        assert_eq!(resolve_builtin_default_enabled("not-real", "agentic"), None);
    }
}
