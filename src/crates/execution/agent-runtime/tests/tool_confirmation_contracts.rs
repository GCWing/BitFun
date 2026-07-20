use bitfun_agent_runtime::tool_confirmation::{
    resolve_confirmation_failure, resolve_confirmation_wait_result, resolve_tool_confirmation_gate,
    resolve_tool_confirmation_plan, resolve_tool_confirmation_policy_gate, ConfirmationFailureKind,
    ToolConfirmationContextPolicy, ToolConfirmationGateFacts, ToolConfirmationGatePlan,
    ToolConfirmationOutcome, ToolConfirmationPlan, ToolConfirmationPolicyGateFacts,
    ToolConfirmationRequestFacts, ToolConfirmationWaitResult,
};
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn confirmation_gate_preserves_skip_policy_precedence() {
    assert_eq!(
        resolve_tool_confirmation_gate(ToolConfirmationGateFacts {
            global_skip_tool_confirmation: true,
            context_skip_tool_confirmation: false,
            any_tool_needs_permission: true,
        }),
        ToolConfirmationGatePlan::SkipByPolicy
    );
    assert_eq!(
        resolve_tool_confirmation_gate(ToolConfirmationGateFacts {
            global_skip_tool_confirmation: false,
            context_skip_tool_confirmation: true,
            any_tool_needs_permission: true,
        }),
        ToolConfirmationGatePlan::SkipByPolicy
    );
}

#[test]
fn invocation_require_policy_overrides_the_global_skip_default() {
    let plan = resolve_tool_confirmation_policy_gate(ToolConfirmationPolicyGateFacts {
        global_skip_tool_confirmation: true,
        context_policy: ToolConfirmationContextPolicy::Require,
        any_tool_needs_permission: true,
    });

    assert_eq!(plan, ToolConfirmationGatePlan::AwaitPermissionedTool);
    assert!(plan.confirm_before_run());
}

#[test]
fn confirmation_gate_requires_confirmation_only_for_permissioned_tools() {
    let permissioned = resolve_tool_confirmation_gate(ToolConfirmationGateFacts {
        global_skip_tool_confirmation: false,
        context_skip_tool_confirmation: false,
        any_tool_needs_permission: true,
    });

    assert_eq!(
        permissioned,
        ToolConfirmationGatePlan::AwaitPermissionedTool
    );
    assert!(permissioned.confirm_before_run());

    let readonly = resolve_tool_confirmation_gate(ToolConfirmationGateFacts {
        global_skip_tool_confirmation: false,
        context_skip_tool_confirmation: false,
        any_tool_needs_permission: false,
    });

    assert_eq!(readonly, ToolConfirmationGatePlan::SkipNoPermissionedTool);
    assert!(!readonly.confirm_before_run());
}

#[test]
fn confirmation_plan_requires_permission_only_when_both_flags_are_true() {
    let plan = resolve_tool_confirmation_plan(ToolConfirmationRequestFacts {
        confirm_before_run: true,
        tool_needs_permission: true,
        confirmation_timeout_secs: Some(30),
        now: UNIX_EPOCH + Duration::from_secs(1_000),
    });

    assert_eq!(
        plan,
        ToolConfirmationPlan::Await {
            timeout_at: UNIX_EPOCH + Duration::from_secs(1_030),
            timeout_secs: Some(30),
        }
    );

    assert_eq!(
        resolve_tool_confirmation_plan(ToolConfirmationRequestFacts {
            confirm_before_run: false,
            tool_needs_permission: true,
            confirmation_timeout_secs: Some(30),
            now: UNIX_EPOCH + Duration::from_secs(1_000),
        }),
        ToolConfirmationPlan::Skip
    );
    assert_eq!(
        resolve_tool_confirmation_plan(ToolConfirmationRequestFacts {
            confirm_before_run: true,
            tool_needs_permission: false,
            confirmation_timeout_secs: None,
            now: UNIX_EPOCH + Duration::from_secs(1_000),
        }),
        ToolConfirmationPlan::Skip
    );
}

#[test]
fn confirmation_plan_preserves_legacy_no_timeout_one_year_deadline() {
    assert_eq!(
        resolve_tool_confirmation_plan(ToolConfirmationRequestFacts {
            confirm_before_run: true,
            tool_needs_permission: true,
            confirmation_timeout_secs: None,
            now: UNIX_EPOCH + Duration::from_secs(1_000),
        }),
        ToolConfirmationPlan::Await {
            timeout_at: UNIX_EPOCH + Duration::from_secs(31_537_000),
            timeout_secs: None,
        }
    );
}

#[test]
fn confirmation_failure_mapping_preserves_legacy_reasons_and_errors() {
    assert_eq!(
        resolve_confirmation_failure(ToolConfirmationOutcome::Confirmed),
        None
    );

    let rejected = resolve_confirmation_failure(ToolConfirmationOutcome::Rejected {
        reason: "unsafe".to_string(),
    })
    .expect("rejection should produce failure");
    assert_eq!(rejected.kind, ConfirmationFailureKind::Rejected);
    assert_eq!(rejected.state_reason, "User rejected: unsafe");
    assert_eq!(rejected.error_message, "Tool was rejected by user: unsafe");
    assert_eq!(rejected.rejection_instruction.as_deref(), Some("unsafe"));

    let default_rejected = resolve_confirmation_failure(ToolConfirmationOutcome::Rejected {
        reason: "User rejected".to_string(),
    })
    .expect("rejection should produce failure");
    assert_eq!(default_rejected.kind, ConfirmationFailureKind::Rejected);
    assert_eq!(default_rejected.rejection_instruction, None);

    let closed = resolve_confirmation_failure(ToolConfirmationOutcome::ChannelClosed)
        .expect("closed channel should produce failure");
    assert_eq!(closed.kind, ConfirmationFailureKind::ChannelClosed);
    assert_eq!(closed.state_reason, "Confirmation channel closed");
    assert_eq!(closed.error_message, "Confirmation channel closed");

    let timeout = resolve_confirmation_failure(ToolConfirmationOutcome::Timeout {
        tool_name: "Bash".to_string(),
    })
    .expect("timeout should produce failure");
    assert_eq!(timeout.kind, ConfirmationFailureKind::Timeout);
    assert_eq!(timeout.state_reason, "Confirmation timeout");
    assert_eq!(timeout.error_message, "Confirmation timeout: Bash");
    assert_eq!(timeout.rejection_instruction, None);
}

#[test]
fn confirmation_wait_result_mapping_preserves_legacy_timeout_and_rejection() {
    assert_eq!(
        resolve_confirmation_wait_result(ToolConfirmationWaitResult::Confirmed, "Bash"),
        ToolConfirmationOutcome::Confirmed
    );
    assert_eq!(
        resolve_confirmation_wait_result(
            ToolConfirmationWaitResult::Rejected("unsafe".to_string()),
            "Edit"
        ),
        ToolConfirmationOutcome::Rejected {
            reason: "unsafe".to_string(),
        }
    );
    assert_eq!(
        resolve_confirmation_wait_result(ToolConfirmationWaitResult::ChannelClosed, "Read"),
        ToolConfirmationOutcome::ChannelClosed
    );
    assert_eq!(
        resolve_confirmation_wait_result(ToolConfirmationWaitResult::TimedOut, "Bash"),
        ToolConfirmationOutcome::Timeout {
            tool_name: "Bash".to_string(),
        }
    );
}
