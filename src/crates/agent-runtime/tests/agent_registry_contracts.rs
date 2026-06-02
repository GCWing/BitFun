use bitfun_agent_runtime::agents::{
    resolve_subagent_availability, resolve_subagent_default_enabled, BuiltinSubagentExposure,
    SubagentOverrideLayers, SubagentOverrideState, SubagentSourceKind, SubagentStateReason,
    SubagentVisibilityPolicy,
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
    assert_eq!(builtin.default_enabled, false);
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
