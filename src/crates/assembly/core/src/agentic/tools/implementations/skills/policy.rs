//! Compatibility re-export for mode-aware built-in skill policy.
//!
//! The provider-neutral owner lives in `bitfun-agent-runtime`.

pub use bitfun_agent_runtime::skills::resolve_builtin_default_enabled;

#[cfg(test)]
mod tests {
    use super::resolve_builtin_default_enabled;

    #[test]
    fn builtin_defaults_follow_mode_policies() {
        assert_eq!(
            resolve_builtin_default_enabled("pdf", "agentic"),
            Some(false)
        );
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
            Some(true)
        );
        assert_eq!(resolve_builtin_default_enabled("pdf", "Cowork"), Some(true));
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "Cowork"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("gstack-review", "Team"),
            Some(true)
        );
        assert_eq!(resolve_builtin_default_enabled("pdf", "Team"), Some(false));
        assert_eq!(
            resolve_builtin_default_enabled("miniapp-dev", "Team"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("find-skills", "DeepResearch"),
            Some(true)
        );
        assert_eq!(
            resolve_builtin_default_enabled("pdf", "DeepResearch"),
            Some(false)
        );
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "Claw"),
            Some(true)
        );
        assert_eq!(resolve_builtin_default_enabled("pdf", "Claw"), Some(false));
        assert_eq!(
            resolve_builtin_default_enabled("agent-browser", "coding_shared"),
            Some(true)
        );
        assert_eq!(
            resolve_builtin_default_enabled("pdf", "coding_shared"),
            Some(false)
        );
        assert_eq!(resolve_builtin_default_enabled("pdf", "Other"), Some(false));
    }

    #[test]
    fn unknown_builtins_return_none() {
        assert_eq!(resolve_builtin_default_enabled("not-real", "agentic"), None);
    }
}
