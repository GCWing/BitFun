use bitfun_agent_runtime::prompt::{
    render_project_layout, render_prompt_environment_info, render_runtime_context_reminder,
    render_runtime_facts_reminder, render_user_context_reminder, render_workspace_context,
    PrependedPromptReminders, ProjectLayoutFacts, PromptEnvironmentFacts, PromptRelatedPath,
    RemoteExecutionHints, RuntimeContextFacts, RuntimeContextNeeds, RuntimeFactsInput,
    RuntimeShellFacts, ToolListingSections, UserContextPolicy, UserContextSection,
    WorkspaceContextFacts, WorktreeContextFacts,
};
use bitfun_core_types::{SessionExecutionTarget, SessionExecutionTargetKind, WorktreeLifecycle};

fn sample_runtime_facts_input(context_usage_ratio: Option<f32>) -> RuntimeFactsInput {
    RuntimeFactsInput {
        local_time_rfc3339: "2026-08-05T10:30:00+08:00".to_string(),
        utc_time_rfc3339: "2026-08-05T02:30:00Z".to_string(),
        weekday_name: "Wednesday".to_string(),
        weekday_number: 3,
        local_hhmm: "10:30".to_string(),
        timezone_offset: "+08:00".to_string(),
        context_usage_ratio,
        compression_preview_ratio: Some(0.9),
    }
}

#[test]
fn user_context_policy_preserves_order_and_deduplicates_sections() {
    let policy = UserContextPolicy::empty()
        .with_workspace_context()
        .with_workspace_instructions()
        .with_workspace_context()
        .with_project_layout()
        .with_memory_summary()
        .without_section(UserContextSection::ProjectLayout);

    assert_eq!(
        policy.sections,
        vec![
            UserContextSection::WorkspaceContext,
            UserContextSection::WorkspaceInstructions,
            UserContextSection::MemorySummary,
        ]
    );
    assert_eq!(
        policy.cache_scope_key(),
        "workspace_context|workspace_instructions|memory_summary"
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
        deferred_tool_listing: Some("Search: summary".to_string()),
    };

    assert!(!sections.is_empty());
    assert!(sections
        .render_skill_listing_reminder()
        .expect("skill listing should render")
        .starts_with("# Skill Listing\nA skill is a set of instructions"));
    assert!(sections.render_agent_listing_reminder().is_none());
    let deferred_tool_listing = sections
        .render_deferred_tool_listing_reminder()
        .expect("deferred tool listing should render");
    assert!(deferred_tool_listing.starts_with("# Tool Calling Guide\n"));
    assert!(deferred_tool_listing.contains("Direct tools: tools in the available tool list"));
    assert!(deferred_tool_listing.contains("Deferred tools: call them through `CallDeferredTool`"));
    assert!(deferred_tool_listing.contains(
        "Before the first call for a deferred tool whose full spec is not already available"
    ));
    assert!(deferred_tool_listing
        .contains("Once its spec is available, call `CallDeferredTool` directly"));
    assert!(deferred_tool_listing
        .contains("unless the system reports that the spec is stale or unavailable"));
    assert!(deferred_tool_listing.contains("tool_name[: optional short description]"));
    assert!(deferred_tool_listing.contains(
        "## Deferred Tool Listing\nEach entry has the form `tool_name[: optional short description]`.\n\nSearch: summary"
    ));
}

#[test]
fn prepended_prompt_reminders_keep_runtime_injection_order() {
    let reminders = PrependedPromptReminders {
        deferred_tool_listing: Some("deferred-tools".to_string()),
        skill_listing: Some("skills".to_string()),
        agent_listing: Some("agents".to_string()),
        runtime_context: Some("runtime-context".to_string()),
        runtime_facts: Some("runtime-facts".to_string()),
        user_context: Some("user-context".to_string()),
    };

    assert_eq!(
        reminders.ordered_reminders(),
        vec![
            "deferred-tools",
            "skills",
            "agents",
            "runtime-context",
            "runtime-facts",
            "user-context"
        ]
    );
    assert!(PrependedPromptReminders::default()
        .ordered_reminders()
        .is_empty());
}

#[test]
fn runtime_facts_reminder_renders_time_and_offset_facts() {
    let reminder = render_runtime_facts_reminder(&sample_runtime_facts_input(Some(0.35)));

    assert!(reminder.starts_with("[Runtime Facts]"));
    assert!(reminder.contains("当前本地时间: 2026-08-05T10:30:00+08:00（周3 Wednesday）"));
    assert!(reminder.contains("UTC 时间: 2026-08-05T02:30:00Z"));
    assert!(reminder.contains("时区偏移: +08:00"));
    assert!(reminder.contains("当前上下文占比: 35%"));
}

#[test]
fn runtime_facts_reminder_formats_usage_percent_with_rounding_and_clamping() {
    assert!(render_runtime_facts_reminder(&sample_runtime_facts_input(Some(0.35)))
        .contains("当前上下文占比: 35%"));
    assert!(render_runtime_facts_reminder(&sample_runtime_facts_input(Some(0.0)))
        .contains("当前上下文占比: 0%"));
    assert!(render_runtime_facts_reminder(&sample_runtime_facts_input(Some(0.004)))
        .contains("当前上下文占比: 0%"));
    assert!(render_runtime_facts_reminder(&sample_runtime_facts_input(Some(0.999)))
        .contains("当前上下文占比: 100%"));
    assert!(render_runtime_facts_reminder(&sample_runtime_facts_input(Some(1.5)))
        .contains("当前上下文占比: 100%"));
}

#[test]
fn runtime_facts_reminder_tiered_guidance_covers_high_usage_compression_and_normal() {
    // P-02: the 30% hallucination guardrail and compression preview lines were
    // removed by the owner ruling. The reminder now always emits the bare usage
    // percentage next to the clock; the tiered guidance text must not reappear.
    let high = render_runtime_facts_reminder(&sample_runtime_facts_input(Some(0.35)));
    assert!(high.contains("当前上下文占比: 35%"));
    assert!(!high.contains("上下文已超 30%"));
    assert!(!high.contains("即将自动压缩"));
    assert!(!high.contains("DeepSeek 峰谷定价"));

    let preview = render_runtime_facts_reminder(&sample_runtime_facts_input(Some(0.9)));
    assert!(preview.contains("当前上下文占比: 90%"));
    assert!(!preview.contains("即将自动压缩"));
    assert!(!preview.contains("上下文已超 30%"));

    let normal = render_runtime_facts_reminder(&sample_runtime_facts_input(Some(0.05)));
    assert!(normal.contains("当前上下文占比: 5%"));
    assert!(!normal.contains("上下文已超 30%"));
    assert!(!normal.contains("即将自动压缩"));
}

#[test]
fn runtime_facts_reminder_omits_usage_lines_when_ratio_is_absent() {
    let reminder = render_runtime_facts_reminder(&sample_runtime_facts_input(None));

    assert!(!reminder.contains("当前上下文占比"));
    assert!(!reminder.contains("上下文已超 30%"));
    assert!(!reminder.contains("即将自动压缩"));
    assert!(reminder.contains("当前本地时间"));
}

#[test]
fn runtime_facts_reminder_omits_compression_preview_text() {
    // P-02: the compression preview was removed by the owner ruling. Setting a
    // preview ratio (or leaving it missing) must not emit the old preview text.
    let mut input = sample_runtime_facts_input(Some(0.95));
    input.compression_preview_ratio = None;
    let reminder = render_runtime_facts_reminder(&input);
    assert!(!reminder.contains("即将自动压缩"));
    assert!(reminder.contains("当前上下文占比: 95%"));

    let mut input = sample_runtime_facts_input(Some(0.5));
    input.compression_preview_ratio = Some(0.9);
    let reminder = render_runtime_facts_reminder(&input);
    assert!(!reminder.contains("即将自动压缩"));
    assert!(reminder.contains("当前上下文占比: 50%"));
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

#[test]
fn runtime_context_renderer_preserves_local_exec_and_computer_use_guidance() {
    let reminder = render_runtime_context_reminder(&RuntimeContextFacts {
        needs: RuntimeContextNeeds::from_tool_names(["Read", "ExecCommand", "ComputerUse"]),
        host_os: "windows".to_string(),
        host_family: "windows".to_string(),
        host_arch: "x86_64".to_string(),
        remote_execution: None,
        local_shell: Some(RuntimeShellFacts {
            display_name: "PowerShell".to_string(),
            shell_type: "powershell".to_string(),
            invocation: "powershell.exe -NoLogo".to_string(),
        }),
        supports_image_understanding: None,
        inline_markdown_image_display: false,
    })
    .expect("runtime context should render");

    assert!(reminder.contains("# Runtime Context"));
    assert!(reminder.contains("## Workspace Execution"));
    assert!(reminder.contains("- Workspace file and shell tools operate on the local filesystem."));
    assert!(reminder.contains("## ExecCommand Shell"));
    assert!(reminder.contains("PowerShell (powershell)"));
    assert!(reminder.contains("prefer native PowerShell cmdlets"));
    assert!(reminder.contains("## Local Client"));
    assert!(reminder.contains("- Local BitFun client OS: windows (windows)"));
    assert!(reminder.contains("meta`/`super`"));
}

#[test]
fn runtime_context_renderer_preserves_remote_workspace_split() {
    let reminder = render_runtime_context_reminder(&RuntimeContextFacts {
        needs: RuntimeContextNeeds::from_tool_names([
            "Read",
            "ExecCommand",
            "ExecControl",
            "ComputerUse",
        ]),
        host_os: "windows".to_string(),
        host_family: "windows".to_string(),
        host_arch: "x86_64".to_string(),
        remote_execution: Some(RemoteExecutionHints {
            connection_display_name: "prod \"box\"".to_string(),
            kernel_name: "Linux".to_string(),
            hostname: "remote-host".to_string(),
        }),
        local_shell: Some(RuntimeShellFacts {
            display_name: "PowerShell".to_string(),
            shell_type: "powershell".to_string(),
            invocation: "powershell.exe".to_string(),
        }),
        supports_image_understanding: None,
        inline_markdown_image_display: false,
    })
    .expect("remote runtime context should render");

    assert!(reminder.contains("remote SSH connection \"prod 'box'\""));
    assert!(reminder.contains("Remote host: remote-host (uname/kernel: Linux)"));
    assert!(reminder.contains("ExecCommand uses the remote user's default POSIX shell"));
    assert!(!reminder.contains("## ExecControl"));
    assert!(reminder.contains("Computer use and UI automation operate on the local BitFun desktop"));
}

#[test]
fn runtime_context_renderer_adds_text_only_computer_use_guidance_for_non_visual_models() {
    let reminder = render_runtime_context_reminder(&RuntimeContextFacts {
        needs: RuntimeContextNeeds::from_tool_names(["ComputerUse"]),
        host_os: "windows".to_string(),
        host_family: "windows".to_string(),
        host_arch: "x86_64".to_string(),
        remote_execution: None,
        local_shell: None,
        supports_image_understanding: Some(false),
        inline_markdown_image_display: false,
    })
    .expect("runtime context should render");

    assert!(reminder.contains("## Local Client"));
    assert!(reminder.contains("## Computer Use Input Strategy"));
    assert!(reminder.contains("primary model does not accept image inputs"));
    assert!(reminder.contains("do not use `screenshot`"));
    assert!(reminder.contains("prefer `snapshot` then click by `@e*` ref"));
}

#[test]
fn runtime_context_renderer_omits_text_only_guidance_for_visual_or_unknown_models() {
    for supports_image_understanding in [Some(true), None] {
        let reminder = render_runtime_context_reminder(&RuntimeContextFacts {
            needs: RuntimeContextNeeds::from_tool_names(["ComputerUse"]),
            host_os: "windows".to_string(),
            host_family: "windows".to_string(),
            host_arch: "x86_64".to_string(),
            remote_execution: None,
            local_shell: None,
            supports_image_understanding,
            inline_markdown_image_display: false,
        })
        .expect("runtime context should render");

        assert!(reminder.contains("## Local Client"));
        assert!(!reminder.contains("## Computer Use Input Strategy"));
        assert!(!reminder.contains("primary model does not accept image inputs"));
    }
}

#[test]
fn runtime_context_renderer_scopes_inline_image_guidance_to_capable_surfaces() {
    let reminder = render_runtime_context_reminder(&RuntimeContextFacts {
        needs: RuntimeContextNeeds::default(),
        host_os: "linux".to_string(),
        host_family: "unix".to_string(),
        host_arch: "x86_64".to_string(),
        remote_execution: None,
        local_shell: None,
        supports_image_understanding: None,
        inline_markdown_image_display: true,
    })
    .expect("output-surface context should render without tool runtime facts");

    assert!(reminder.contains("## Chat Image Display"));
    assert!(reminder.contains("`![concise alt text](source)`"));
    assert!(reminder.contains("workspace-relative image paths"));
    assert!(reminder.contains("do not call image-analysis tools solely to display an image"));

    assert!(render_runtime_context_reminder(&RuntimeContextFacts {
        needs: RuntimeContextNeeds::default(),
        host_os: "linux".to_string(),
        host_family: "unix".to_string(),
        host_arch: "x86_64".to_string(),
        remote_execution: None,
        local_shell: None,
        supports_image_understanding: None,
        inline_markdown_image_display: false,
    })
    .is_none());
}

#[test]
fn workspace_and_user_context_renderers_preserve_section_shape() {
    let local = render_workspace_context(&WorkspaceContextFacts {
        workspace_path: "workspace/root".to_string(),
        related_paths: vec![PromptRelatedPath {
            path: "sibling\\project".to_string(),
            description: Some("docs".to_string()),
        }],
        remote_execution: None,
        worktree: None,
    });

    assert!(local.contains("## Workspace Context"));
    assert!(local.contains("- Current Working Directory: workspace/root"));
    assert!(local.contains("sibling/project"));
    assert!(local.contains("sibling/project — docs"));
    assert!(local.contains("docs"));

    let remote = render_workspace_context(&WorkspaceContextFacts {
        workspace_path: "/srv/workspace".to_string(),
        related_paths: Vec::new(),
        remote_execution: Some(RemoteExecutionHints {
            connection_display_name: "remote".to_string(),
            kernel_name: "Linux".to_string(),
            hostname: "host".to_string(),
        }),
        worktree: None,
    });
    assert!(remote.contains(
        "Workspace root (file tools, Glob, LS, ExecCommand on workspace): /srv/workspace"
    ));
    assert!(remote.contains("Execution environment: **Remote SSH**"));
    assert!(remote.contains("**Remote SSH** — connection"));

    let managed_worktree = render_workspace_context(&WorkspaceContextFacts {
        workspace_path: "/managed/BitFun-wt-1".to_string(),
        related_paths: Vec::new(),
        remote_execution: None,
        worktree: Some(WorktreeContextFacts {
            project_workspace_path: "/projects/BitFun".to_string(),
            execution_target: SessionExecutionTarget {
                kind: SessionExecutionTargetKind::ManagedWorktree,
                worktree_id: Some("wt-1".to_string()),
                root_path: "/managed/BitFun-wt-1".to_string(),
                base_ref: Some("HEAD".to_string()),
                base_commit: Some("0123456789abcdef".to_string()),
                branch: None,
                lifecycle: Some(WorktreeLifecycle::Managed),
            },
        }),
    });
    assert!(managed_worktree.contains("Managed Git worktree created for this session"));
    assert!(managed_worktree.contains("Owning project root"));
    assert!(managed_worktree.contains("/projects/BitFun"));
    assert!(managed_worktree.contains("Worktree ID: wt-1"));
    assert!(managed_worktree.contains("Worktree checkout: detached HEAD"));
    assert!(managed_worktree.contains("Worktree base commit: 0123456789abcdef"));
    assert!(managed_worktree
        .contains("Keep file, shell, and Git operations inside the workspace root above"));

    let project_layout = render_project_layout(&ProjectLayoutFacts {
        listing: "src\nCargo.toml".to_string(),
        reached_limit: true,
        max_entries: 2,
        remote: false,
    });
    assert!(project_layout.contains("showing up to 2 entries"));

    let user_context =
        render_user_context_reminder(vec![local, project_layout]).expect("context should render");
    assert!(user_context.starts_with("# User Context\nAs you answer"));
    assert!(user_context.contains("## Workspace Context"));
    assert!(user_context.contains("## Workspace Layout"));
}
