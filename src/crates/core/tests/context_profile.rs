use bitfun_core::agentic::context_profile::{
    ContextProfile, ContextProfilePolicy, ModelCapabilityProfile,
};
use bitfun_core::agentic::session::MicrocompactConfig;

#[test]
fn context_profile_maps_long_running_agents_to_long_task_profile() {
    for agent_type in [
        "agentic",
        "DeepReview",
        "DeepResearch",
        "ComputerUse",
        "Team",
        "ReviewFrontend",
        "ReviewSecurity",
    ] {
        assert_eq!(
            ContextProfile::for_agent_type(agent_type),
            ContextProfile::LongTask,
            "{agent_type} should use the long-task profile"
        );
    }
}

#[test]
fn context_profile_maps_conversation_agents_to_conversation_profile() {
    for agent_type in ["Cowork", "Plan", "Claw", "unknown-custom-agent"] {
        assert_eq!(
            ContextProfile::for_agent_type(agent_type),
            ContextProfile::Conversation,
            "{agent_type} should use the conversation profile"
        );
    }
}

#[test]
fn context_profile_review_custom_subagents_can_be_promoted_to_long_task_profile() {
    assert_eq!(
        ContextProfile::for_agent_context("legal-domain-reviewer", true),
        ContextProfile::LongTask
    );
    assert_eq!(
        ContextProfile::for_agent_context("legal-domain-reviewer", false),
        ContextProfile::Conversation
    );
}

#[test]
fn context_profile_long_task_policy_preserves_current_context_defaults() {
    let policy = ContextProfilePolicy::for_agent_context(
        "DeepReview",
        false,
        ModelCapabilityProfile::Standard,
    );
    let default_microcompact = MicrocompactConfig::default();

    assert_eq!(policy.profile, ContextProfile::LongTask);
    assert_eq!(
        policy.microcompact_config().keep_recent,
        default_microcompact.keep_recent
    );
    assert!(
        (policy.microcompact_config().trigger_ratio - default_microcompact.trigger_ratio).abs()
            < f32::EPSILON
    );
    assert_eq!(policy.compression_contract_limit, 8);
    assert_eq!(policy.subagent_concurrency_cap, 5);
    assert_eq!(policy.repeated_tool_signature_threshold, 3);
    assert_eq!(policy.consecutive_failed_command_threshold, 2);
}

#[test]
fn context_profile_conversation_policy_keeps_more_recent_chat_context() {
    let policy =
        ContextProfilePolicy::for_agent_context("Cowork", false, ModelCapabilityProfile::Standard);

    assert_eq!(policy.profile, ContextProfile::Conversation);
    assert_eq!(policy.microcompact_config().keep_recent, 12);
    assert!((policy.microcompact_config().trigger_ratio - 0.65).abs() < f32::EPSILON);
    assert_eq!(policy.compression_contract_limit, 4);
    assert_eq!(policy.subagent_concurrency_cap, 2);
    assert_eq!(policy.repeated_tool_signature_threshold, 4);
    assert_eq!(policy.consecutive_failed_command_threshold, 3);
}

#[test]
fn context_profile_weak_model_override_shortens_contract_and_caps_fanout() {
    let standard = ContextProfilePolicy::for_agent_context(
        "DeepReview",
        false,
        ModelCapabilityProfile::Standard,
    );
    let weak =
        ContextProfilePolicy::for_agent_context("DeepReview", false, ModelCapabilityProfile::Weak);

    assert_eq!(weak.profile, ContextProfile::LongTask);
    assert!(weak.compression_contract_limit < standard.compression_contract_limit);
    assert!(weak.subagent_concurrency_cap < standard.subagent_concurrency_cap);
    assert!(weak.repeated_tool_signature_threshold < standard.repeated_tool_signature_threshold);
    assert_eq!(weak.compression_contract_limit, 4);
    assert_eq!(weak.subagent_concurrency_cap, 2);
    assert_eq!(weak.repeated_tool_signature_threshold, 2);
}

#[test]
fn context_profile_model_capability_profile_only_marks_explicit_weak_models() {
    assert_eq!(
        ModelCapabilityProfile::from_model_id(Some("claude-3-haiku")),
        ModelCapabilityProfile::Weak
    );
    assert_eq!(
        ModelCapabilityProfile::from_model_id(Some("gpt-5.4-mini")),
        ModelCapabilityProfile::Weak
    );
    assert_eq!(
        ModelCapabilityProfile::from_model_id(Some("fast")),
        ModelCapabilityProfile::Standard,
        "configured model slots should not be treated as weak before resolving"
    );
    assert_eq!(
        ModelCapabilityProfile::from_model_id(None),
        ModelCapabilityProfile::Standard
    );
}

#[test]
fn context_profile_configured_subagent_concurrency_is_capped_by_policy() {
    let long_task = ContextProfilePolicy::for_agent_context(
        "DeepReview",
        false,
        ModelCapabilityProfile::Standard,
    );
    let conversation =
        ContextProfilePolicy::for_agent_context("Cowork", false, ModelCapabilityProfile::Standard);

    assert_eq!(long_task.effective_subagent_max_concurrency(64), 5);
    assert_eq!(long_task.effective_subagent_max_concurrency(3), 3);
    assert_eq!(conversation.effective_subagent_max_concurrency(64), 2);
    assert_eq!(conversation.effective_subagent_max_concurrency(1), 1);
}

#[test]
fn context_profile_subagent_policy_combines_parent_workload_and_child_model() {
    let policy = ContextProfilePolicy::for_subagent_context_and_models(
        "custom-security-reviewer",
        true,
        Some("claude-3-haiku"),
        Some("DeepReview"),
        false,
        Some("gpt-5"),
    );

    assert_eq!(policy.profile, ContextProfile::LongTask);
    assert_eq!(policy.compression_contract_limit, 4);
    assert_eq!(policy.subagent_concurrency_cap, 2);
    assert_eq!(policy.repeated_tool_signature_threshold, 2);
}

#[test]
fn context_profile_subagent_policy_inherits_parent_long_task_when_child_is_plain() {
    let policy = ContextProfilePolicy::for_subagent_context_and_models(
        "Explore",
        false,
        None,
        Some("DeepReview"),
        false,
        Some("gpt-5"),
    );

    assert_eq!(policy.profile, ContextProfile::LongTask);
    assert_eq!(policy.subagent_concurrency_cap, 5);
}
