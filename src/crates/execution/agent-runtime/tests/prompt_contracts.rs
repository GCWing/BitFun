use bitfun_agent_runtime::prompt::{
    render_prompt_environment_info, PrependedPromptReminders, PromptEnvironmentFacts,
    ToolListingSections, UserContextPolicy, UserContextSection,
};

#[test]
fn user_context_policy_preserves_order_and_deduplicates_sections() {
    let policy = UserContextPolicy::empty()
        .with_workspace_context()
        .with_workspace_instructions()
        .with_workspace_context()
        .with_project_layout()
        .without_section(UserContextSection::ProjectLayout)
        .with_workspace_memory_files();

    assert_eq!(
        policy.sections,
        vec![
            UserContextSection::WorkspaceContext,
            UserContextSection::WorkspaceInstructions,
            UserContextSection::WorkspaceMemoryFiles,
        ]
    );
    assert_eq!(
        policy.cache_scope_key(),
        "workspace_context|workspace_instructions|workspace_memory_files"
    );
}

#[test]
fn user_context_policy_default_and_empty_scope_are_empty() {
    assert_eq!(UserContextPolicy::default(), UserContextPolicy::empty());
    assert!(UserContextPolicy::default().sections.is_empty());
    assert_eq!(UserContextPolicy::empty().cache_scope_key(), "empty");
}

#[test]
fn tool_listing_sections_render_only_present_sections() {
    let sections = ToolListingSections {
        skill_listing: Some("skill-a\nskill-b".to_string()),
        agent_listing: None,
        collapsed_tool_listing: Some("Search: summary".to_string()),
    };

    assert!(!sections.is_empty());
    assert!(sections
        .render_skill_listing_reminder()
        .expect("skill listing should render")
        .starts_with("# Skill Listing\nThe following skills are available"));
    assert!(sections.render_agent_listing_reminder().is_none());
    assert!(sections
        .render_collapsed_tool_listing_reminder()
        .expect("collapsed tool listing should render")
        .starts_with("# Collapsed Tool Listing\n"));
}

#[test]
fn prepended_prompt_reminders_keep_runtime_injection_order() {
    let reminders = PrependedPromptReminders {
        collapsed_tool_listing: Some("collapsed-tools".to_string()),
        skill_listing: Some("skills".to_string()),
        agent_listing: Some("agents".to_string()),
        runtime_context: Some("runtime-context".to_string()),
        user_context: Some("user-context".to_string()),
    };

    assert_eq!(
        reminders.ordered_reminders(),
        vec![
            "collapsed-tools",
            "skills",
            "agents",
            "runtime-context",
            "user-context"
        ]
    );
    assert!(PrependedPromptReminders::default()
        .ordered_reminders()
        .is_empty());
}

#[test]
fn prompt_environment_info_preserves_local_and_remote_guidance() {
    let local = render_prompt_environment_info(PromptEnvironmentFacts {
        host_os: "windows",
        host_family: "windows",
        host_arch: "x86_64",
        remote_execution_active: false,
    });
    assert!(local.contains("- Operating System: windows (windows)"));
    assert!(local.contains("Computer use / `key_chord`"));
    assert!(local.contains("PowerShell"));

    let remote = render_prompt_environment_info(PromptEnvironmentFacts {
        host_os: "linux",
        host_family: "unix",
        host_arch: "aarch64",
        remote_execution_active: true,
    });
    assert!(remote.contains("- Local BitFun client OS: linux (unix)"));
    assert!(remote.contains("applies to Computer use / UI automation"));
    assert!(remote.contains("Local client architecture: aarch64"));
}
