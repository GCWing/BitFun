use bitfun_agent_runtime::agents::{
    builtin_agent_definition_specs, default_model_id_for_builtin_agent, mode_config_profile_label,
    mode_config_profile_member_mode_ids, mode_presentation_rank, resolve_mode_config_profile_id,
    resolve_subagent_availability, resolve_subagent_default_enabled,
    shared_coding_mode_user_context_policy, subagent_source_kind,
    subagent_source_presentation_rank, BuiltinAgentCategory, BuiltinSubagentExposure,
    SubAgentSource, SubagentOverrideLayers, SubagentOverrideState, SubagentSourceKind,
    SubagentStateReason, SubagentVisibilityPolicy, SHARED_CODING_MODE_CONFIG_PROFILE_ID,
    SHARED_CODING_MODE_CONFIG_PROFILE_LABEL, SHARED_CODING_MODE_IDS,
};

#[test]
fn visibility_policy_supports_public_restricted_hidden_and_denied_parents() {
    let public = SubagentVisibilityPolicy::public();
    assert!(public.can_access_from_parent(None));
    assert!(public.can_access_from_parent(Some("agentic")));

    let restricted = SubagentVisibilityPolicy::restricted(["DeepResearch"]);
    assert!(!restricted.can_access_from_parent(None));
    assert!(restricted.can_access_from_parent(Some("DeepResearch")));
    assert!(!restricted.can_access_from_parent(Some("agentic")));

    let denied = SubagentVisibilityPolicy::public().deny_for(["Team"]);
    assert!(!denied.can_access_from_parent(Some("Team")));
    assert!(denied.can_access_from_parent(Some("agentic")));

    let hidden = SubagentVisibilityPolicy::hidden(["DeepReview"]);
    assert_eq!(hidden.summary().exposure, BuiltinSubagentExposure::Hidden);
    assert!(!hidden.summary().show_in_global_registry);
    assert!(hidden.can_access_from_parent(Some("DeepReview")));
}

#[test]
fn availability_preserves_builtin_project_and_user_override_layering() {
    let builtin = resolve_subagent_availability(
        SubagentSourceKind::Builtin,
        false,
        SubagentOverrideLayers {
            project_override: Some(SubagentOverrideState::Enabled),
            user_override: Some(SubagentOverrideState::Enabled),
        },
    );
    assert!(!builtin.default_enabled);
    assert_eq!(builtin.override_state, Some(SubagentOverrideState::Enabled));
    assert_eq!(
        builtin.state_reason,
        Some(SubagentStateReason::EnabledByUserOverride)
    );

    let project = resolve_subagent_availability(
        SubagentSourceKind::Project,
        true,
        SubagentOverrideLayers {
            project_override: Some(SubagentOverrideState::Disabled),
            user_override: Some(SubagentOverrideState::Enabled),
        },
    );
    assert_eq!(
        project.override_state,
        Some(SubagentOverrideState::Disabled)
    );
    assert_eq!(
        project.state_reason,
        Some(SubagentStateReason::DisabledByProjectOverride)
    );

    let custom_default = resolve_subagent_availability(
        SubagentSourceKind::User,
        true,
        SubagentOverrideLayers::default(),
    );
    assert!(custom_default.effective_enabled);
    assert_eq!(
        custom_default.state_reason,
        Some(SubagentStateReason::CustomDefaultEnabled)
    );
}

#[test]
fn default_enabled_uses_visibility_only_for_builtin_subagents() {
    let hidden = SubagentVisibilityPolicy::hidden(["DeepReview"]);

    assert!(!resolve_subagent_default_enabled(
        SubagentSourceKind::Builtin,
        &hidden,
        Some("agentic")
    ));
    assert!(resolve_subagent_default_enabled(
        SubagentSourceKind::Builtin,
        &hidden,
        Some("DeepReview")
    ));
    assert!(resolve_subagent_default_enabled(
        SubagentSourceKind::Project,
        &hidden,
        Some("agentic")
    ));
    assert!(resolve_subagent_default_enabled(
        SubagentSourceKind::User,
        &hidden,
        Some("agentic")
    ));
}

#[test]
fn shared_coding_modes_resolve_to_the_same_config_profile() {
    for mode_id in SHARED_CODING_MODE_IDS {
        assert_eq!(
            resolve_mode_config_profile_id(mode_id).as_ref(),
            SHARED_CODING_MODE_CONFIG_PROFILE_ID
        );
    }

    assert_eq!(resolve_mode_config_profile_id("Cowork").as_ref(), "Cowork");
    assert_eq!(
        mode_config_profile_member_mode_ids(SHARED_CODING_MODE_CONFIG_PROFILE_ID),
        SHARED_CODING_MODE_IDS
    );
    assert_eq!(
        mode_config_profile_label(SHARED_CODING_MODE_CONFIG_PROFILE_ID),
        Some(SHARED_CODING_MODE_CONFIG_PROFILE_LABEL)
    );
}

#[test]
fn subagent_source_contract_preserves_runtime_kind_and_presentation_order() {
    assert_eq!(
        subagent_source_kind(Some(SubAgentSource::Builtin)),
        SubagentSourceKind::Builtin
    );
    assert_eq!(
        subagent_source_kind(Some(SubAgentSource::Project)),
        SubagentSourceKind::Project
    );
    assert_eq!(
        subagent_source_kind(Some(SubAgentSource::User)),
        SubagentSourceKind::User
    );
    assert_eq!(
        subagent_source_kind(Some(SubAgentSource::External)),
        SubagentSourceKind::External
    );
    assert_eq!(subagent_source_kind(None), SubagentSourceKind::Unspecified);

    assert_eq!(
        subagent_source_presentation_rank(Some(SubAgentSource::Builtin)),
        0
    );
    assert_eq!(
        subagent_source_presentation_rank(Some(SubAgentSource::Project)),
        1
    );
    assert_eq!(
        subagent_source_presentation_rank(Some(SubAgentSource::User)),
        2
    );
    assert_eq!(
        subagent_source_presentation_rank(Some(SubAgentSource::External)),
        3
    );
    assert_eq!(subagent_source_presentation_rank(None), 4);
}

#[test]
fn mode_presentation_and_shared_context_policy_match_existing_mode_contract() {
    assert_eq!(mode_presentation_rank("agentic"), 0);
    assert_eq!(mode_presentation_rank("Cowork"), 1);
    assert_eq!(mode_presentation_rank("Team"), 6);
    assert_eq!(mode_presentation_rank("unknown"), 99);

    assert_eq!(
        shared_coding_mode_user_context_policy().cache_scope_key(),
        "workspace_context|workspace_instructions|project_layout|memory_summary"
    );
}

#[test]
fn builtin_agent_definition_catalog_preserves_order_categories_models_and_visibility() {
    let specs = builtin_agent_definition_specs();
    let ids: Vec<_> = specs.iter().map(|spec| spec.id).collect();
    assert_eq!(
        ids,
        vec![
            "agentic",
            "Cowork",
            "debug",
            "Multitask",
            "Plan",
            "Claw",
            "DeepResearch",
            "Team",
            "ComputerUse",
            "Explore",
            "GeneralPurpose",
            "ResearchSpecialist",
            "FileFinder",
            "ReviewGeneral",
            "ReviewBusinessLogic",
            "ReviewPerformance",
            "ReviewSecurity",
            "ReviewArchitecture",
            "ReviewFrontend",
            "ReviewJudge",
            "ReviewFixer",
            "CodeReview",
            "DeepReview",
            "GenerateDoc",
            "MemoryPhase2",
        ]
    );

    assert_eq!(specs[0].category, BuiltinAgentCategory::Mode);
    assert_eq!(specs[8].category, BuiltinAgentCategory::SubAgent);
    assert_eq!(specs[21].category, BuiltinAgentCategory::SubAgent);
    assert!(specs[21]
        .visibility_policy
        .can_access_from_parent(Some("agentic")));
    assert!(!specs[21].visibility_policy.show_in_global_registry);
    assert_eq!(default_model_id_for_builtin_agent("agentic"), "auto");
    assert_eq!(default_model_id_for_builtin_agent("Explore"), "primary");
    assert_eq!(
        default_model_id_for_builtin_agent("GeneralPurpose"),
        "primary"
    );
    assert_eq!(default_model_id_for_builtin_agent("GenerateDoc"), "fast");
    assert_eq!(
        default_model_id_for_builtin_agent("MemoryPhase2"),
        "primary"
    );
    assert_eq!(
        default_model_id_for_builtin_agent("ResearchSpecialist"),
        "fast"
    );
    assert_eq!(
        default_model_id_for_builtin_agent("ReviewArchitecture"),
        "fast"
    );
    assert_eq!(default_model_id_for_builtin_agent("ReviewGeneral"), "fast");

    let computer_use = specs
        .iter()
        .find(|spec| spec.id == "ComputerUse")
        .expect("ComputerUse spec should exist");
    assert_eq!(
        computer_use.visibility_policy.summary().exposure,
        BuiltinSubagentExposure::Restricted
    );
    assert!(computer_use
        .visibility_policy
        .can_access_from_parent(Some("Claw")));
    assert!(computer_use
        .visibility_policy
        .can_access_from_parent(Some("Team")));
    assert!(!computer_use
        .visibility_policy
        .can_access_from_parent(Some("agentic")));

    let research_specialist = specs
        .iter()
        .find(|spec| spec.id == "ResearchSpecialist")
        .expect("ResearchSpecialist spec should exist");
    assert_eq!(research_specialist.default_model_id, "fast");
}

#[test]
fn shared_coding_modes_have_identical_builtin_subagent_defaults() {
    let specs = builtin_agent_definition_specs();

    for spec in specs
        .iter()
        .filter(|spec| spec.category == BuiltinAgentCategory::SubAgent)
    {
        let expected = resolve_subagent_default_enabled(
            SubagentSourceKind::Builtin,
            &spec.visibility_policy,
            Some("agentic"),
        );

        for mode_id in SHARED_CODING_MODE_IDS {
            assert_eq!(
                resolve_subagent_default_enabled(
                    SubagentSourceKind::Builtin,
                    &spec.visibility_policy,
                    Some(mode_id),
                ),
                expected,
                "builtin subagent {} differs for shared coding mode {}",
                spec.id,
                mode_id
            );
        }
    }
}
