use bitfun_agent_runtime::post_call_hooks::{
    successful_tool_post_call_hooks, RuntimeHookErrorPolicy, RuntimeHookKind, RuntimeHookPlan,
    RuntimeHookRegistry, RuntimeHookRegistryBuildError,
};

#[test]
fn successful_tool_call_routes_to_shared_context_measurement_hook() {
    assert_eq!(
        successful_tool_post_call_hooks(),
        [RuntimeHookKind::DeepReviewSharedContextToolUse]
    );
}

#[test]
fn runtime_hook_registry_preserves_order_timeout_and_error_policy() {
    let registry = RuntimeHookRegistry::builder()
        .register(
            RuntimeHookPlan::new(
                "deep-review.shared-context",
                RuntimeHookKind::DeepReviewSharedContextToolUse,
            )
            .with_order(20)
            .with_timeout_millis(750)
            .with_error_policy(RuntimeHookErrorPolicy::RecordWarning),
        )
        .register(
            RuntimeHookPlan::new("audit.post-call", RuntimeHookKind::SuccessfulToolPostCall)
                .with_order(10)
                .with_timeout_millis(250)
                .with_error_policy(RuntimeHookErrorPolicy::SkipHook),
        )
        .build()
        .expect("hook registry should build");

    assert_eq!(
        registry
            .hooks()
            .iter()
            .map(|hook| hook.id())
            .collect::<Vec<_>>(),
        vec!["audit.post-call", "deep-review.shared-context"]
    );
    assert_eq!(registry.hooks()[0].timeout_millis(), 250);
    assert_eq!(
        registry.hooks()[0].error_policy(),
        RuntimeHookErrorPolicy::SkipHook
    );
}

#[test]
fn runtime_hook_registry_rejects_duplicate_ids() {
    let error = RuntimeHookRegistry::builder()
        .register(RuntimeHookPlan::new(
            "duplicate",
            RuntimeHookKind::SuccessfulToolPostCall,
        ))
        .register(RuntimeHookPlan::new(
            "duplicate",
            RuntimeHookKind::DeepReviewSharedContextToolUse,
        ))
        .build()
        .expect_err("duplicate hook ids must not be silently accepted");

    assert_eq!(
        error,
        RuntimeHookRegistryBuildError::DuplicateHookId {
            hook_id: "duplicate".to_string()
        }
    );
}

#[test]
fn runtime_hook_registry_rejects_unstable_ids_and_zero_timeouts() {
    let empty_id_error = RuntimeHookRegistry::builder()
        .register(RuntimeHookPlan::new(
            "   ",
            RuntimeHookKind::SuccessfulToolPostCall,
        ))
        .build()
        .expect_err("blank hook ids must not become registry keys");

    assert_eq!(empty_id_error, RuntimeHookRegistryBuildError::EmptyHookId);

    let zero_timeout_error = RuntimeHookRegistry::builder()
        .register(
            RuntimeHookPlan::new(
                "deep-review.shared-context",
                RuntimeHookKind::DeepReviewSharedContextToolUse,
            )
            .with_timeout_millis(0),
        )
        .build()
        .expect_err("hook timeouts must remain explicit and non-zero");

    assert_eq!(
        zero_timeout_error,
        RuntimeHookRegistryBuildError::InvalidTimeoutMillis {
            hook_id: "deep-review.shared-context".to_string()
        }
    );
}
