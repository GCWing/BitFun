use crate::define_readonly_subagent;

define_readonly_subagent!(
    ExploreAgent,
    "Explore",
    "Explore",
    r#"Read-only subagent for **wide** codebase exploration. Prefer search-first workflows: use Grep and Glob to narrow the space, then Read the small set of relevant files. Use LS only sparingly to confirm directory shape after search has narrowed the target. Do **not** use for narrow tasks: a known path, a single class/symbol lookup, one obvious Grep pattern, or reading a handful of files — the main agent should handle those directly. When calling, set thoroughness in the prompt: "quick", "medium", or "very thorough"."#,
    "explore_agent",
    &["Grep", "Glob", "Read", "LS"]
);

#[cfg(test)]
mod tests {
    use super::ExploreAgent;
    use crate::agentic::agents::Agent;

    #[test]
    fn uses_search_first_default_tool_order() {
        let agent = ExploreAgent::new();
        assert_eq!(
            agent.default_tools(),
            vec![
                "Grep".to_string(),
                "Glob".to_string(),
                "Read".to_string(),
                "LS".to_string(),
            ]
        );
    }

    #[test]
    fn always_uses_default_prompt_template() {
        let agent = ExploreAgent::new();
        assert_eq!(agent.prompt_template_name(Some("gpt-5.1")), "explore_agent");
        assert_eq!(
            agent.prompt_template_name(Some("claude-sonnet-4")),
            "explore_agent"
        );
        assert_eq!(agent.prompt_template_name(None), "explore_agent");
    }
}
