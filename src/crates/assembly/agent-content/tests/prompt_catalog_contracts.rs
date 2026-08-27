use bitfun_agent_content::{
    agent_prompt, agent_prompt_names,
    insights::{
        AREAS, AT_A_GLANCE, FACET_EXTRACTION, FRICTION, FUN_ENDING, HORIZON, INTERACTION_STYLE,
        SUGGESTIONS, WINS,
    },
    memories::PHASE1_SYSTEM,
    EMBEDDED_PROMPTS,
};

const CATALOG_PROMPT_SOURCES: &[(&str, &[u8])] = &[
    (
        "agentic_mode",
        include_bytes!("../prompts/agents/agentic_mode.md"),
    ),
    (
        "agentic_mode_first_entry_reminder",
        include_bytes!("../prompts/agents/agentic_mode_first_entry_reminder.md"),
    ),
    (
        "claw_mode",
        include_bytes!("../prompts/agents/claw_mode.md"),
    ),
    (
        "code_review",
        include_bytes!("../prompts/agents/code_review.md"),
    ),
    (
        "computer_use_mode",
        include_bytes!("../prompts/agents/computer_use_mode.md"),
    ),
    (
        "cowork_mode",
        include_bytes!("../prompts/agents/cowork_mode.md"),
    ),
    (
        "debug_mode_first_entry_reminder",
        include_bytes!("../prompts/agents/debug_mode_first_entry_reminder.md"),
    ),
    (
        "debug_mode_ongoing_reminder",
        include_bytes!("../prompts/agents/debug_mode_ongoing_reminder.md"),
    ),
    (
        "deep_research_agent",
        include_bytes!("../prompts/agents/deep_research_agent.md"),
    ),
    (
        "deep_review_agent",
        include_bytes!("../prompts/agents/deep_review_agent.md"),
    ),
    (
        "explore_agent",
        include_bytes!("../prompts/agents/explore_agent.md"),
    ),
    (
        "file_finder_agent",
        include_bytes!("../prompts/agents/file_finder_agent.md"),
    ),
    (
        "general_purpose_agent",
        include_bytes!("../prompts/agents/general_purpose_agent.md"),
    ),
    (
        "generate_doc_agent",
        include_bytes!("../prompts/agents/generate_doc_agent.md"),
    ),
    (
        "init_agents_md",
        include_bytes!("../prompts/shared/init_agents_md.md"),
    ),
    (
        "minimal-harness-v1",
        include_bytes!("../prompts/agents/minimal-harness-v1.md"),
    ),
    (
        "multitask_mode_first_entry_reminder",
        include_bytes!("../prompts/agents/multitask_mode_first_entry_reminder.md"),
    ),
    (
        "multitask_mode_ongoing_reminder",
        include_bytes!("../prompts/agents/multitask_mode_ongoing_reminder.md"),
    ),
    (
        "phase1_system",
        include_bytes!("../prompts/memories/phase1_system.md"),
    ),
    (
        "phase2_system",
        include_bytes!("../prompts/memories/phase2_system.md"),
    ),
    (
        "plan_mode_first_entry_reminder",
        include_bytes!("../prompts/agents/plan_mode_first_entry_reminder.md"),
    ),
    (
        "plan_mode_ongoing_reminder",
        include_bytes!("../prompts/agents/plan_mode_ongoing_reminder.md"),
    ),
    (
        "research_specialist_agent",
        include_bytes!("../prompts/agents/research_specialist_agent.md"),
    ),
    (
        "review_fixer_agent",
        include_bytes!("../prompts/agents/review_fixer_agent.md"),
    ),
    (
        "review_quality_gate_agent",
        include_bytes!("../prompts/agents/review_quality_gate_agent.md"),
    ),
    (
        "review_worker_agent",
        include_bytes!("../prompts/agents/review_worker_agent.md"),
    ),
    (
        "team_mode",
        include_bytes!("../prompts/agents/team_mode.md"),
    ),
];

fn generated_rust_source_bytes(source: &[u8]) -> Vec<u8> {
    std::str::from_utf8(source)
        .expect("built-in Agent prompts must be UTF-8")
        .replace("\r\n", "\n")
        .into_bytes()
}

#[test]
fn agent_prompt_catalog_preserves_every_stable_key() {
    let mut names = agent_prompt_names();
    names.sort_unstable();
    let expected_names: Vec<_> = CATALOG_PROMPT_SOURCES
        .iter()
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(names, expected_names);
    assert_eq!(EMBEDDED_PROMPTS.len(), CATALOG_PROMPT_SOURCES.len());

    for (name, source) in CATALOG_PROMPT_SOURCES {
        let owned = agent_prompt(name).unwrap_or_else(|| panic!("missing owner prompt: {name}"));
        assert_eq!(
            owned.as_bytes(),
            generated_rust_source_bytes(source),
            "generated catalog bytes changed for {name}"
        );
    }

    assert_eq!(agent_prompt("unknown_prompt"), None);
}

#[test]
fn insights_prompt_constants_preserve_all_nine_non_empty_templates() {
    let prompts = [
        (
            "facet_extraction",
            FACET_EXTRACTION,
            include_bytes!("../prompts/insights/facet_extraction.md") as &[u8],
        ),
        (
            "suggestions",
            SUGGESTIONS,
            include_bytes!("../prompts/insights/suggestions.md") as &[u8],
        ),
        (
            "areas",
            AREAS,
            include_bytes!("../prompts/insights/areas.md") as &[u8],
        ),
        (
            "wins",
            WINS,
            include_bytes!("../prompts/insights/wins.md") as &[u8],
        ),
        (
            "friction",
            FRICTION,
            include_bytes!("../prompts/insights/friction.md") as &[u8],
        ),
        (
            "interaction_style",
            INTERACTION_STYLE,
            include_bytes!("../prompts/insights/interaction_style.md") as &[u8],
        ),
        (
            "at_a_glance",
            AT_A_GLANCE,
            include_bytes!("../prompts/insights/at_a_glance.md") as &[u8],
        ),
        (
            "horizon",
            HORIZON,
            include_bytes!("../prompts/insights/horizon.md") as &[u8],
        ),
        (
            "fun_ending",
            FUN_ENDING,
            include_bytes!("../prompts/insights/fun_ending.md") as &[u8],
        ),
    ];

    assert_eq!(prompts.len(), 9);
    for (name, prompt, source) in prompts {
        assert_eq!(
            prompt.as_bytes(),
            source,
            "direct include bytes changed for {name}"
        );
    }
}

#[test]
fn memory_phase1_prompt_preserves_direct_include_bytes() {
    assert_eq!(
        PHASE1_SYSTEM.as_bytes(),
        include_bytes!("../prompts/memories/phase1_system.md")
    );
}
