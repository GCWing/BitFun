use async_trait::async_trait;
use openbitfun_agent_runtime::native_hooks::{
    AgentHookMatcher, BuiltinHookExecutor, HookCall, HookHandler, HookHandlerResult,
    RuntimeHookErrorPolicy, RuntimeHookKind, RuntimeHookPlan, RuntimeHookRegistration,
    RuntimeHookRegistry, RuntimeHookRegistryBuildError, RuntimeHookSource,
};
use std::sync::Arc;

struct NoopBuiltin;

#[async_trait]
impl BuiltinHookExecutor for NoopBuiltin {
    async fn execute(&self, _call: &HookCall) -> HookHandlerResult {
        Default::default()
    }
}

fn builtin(plan: RuntimeHookPlan) -> RuntimeHookRegistration {
    RuntimeHookRegistration::new(
        plan,
        HookHandler::Builtin {
            executor: Arc::new(NoopBuiltin),
        },
        AgentHookMatcher::Any,
    )
}

fn plan(id: &str, source: RuntimeHookSource) -> RuntimeHookPlan {
    RuntimeHookPlan::new(id, RuntimeHookKind::SuccessfulToolPostCall, source)
}

#[test]
fn successful_tool_call_uses_stable_builtin_registration_id() {
    let registry = RuntimeHookRegistry::builder()
        .register(builtin(plan(
            "deep-review.shared-context",
            RuntimeHookSource::Builtin { priority: 0 },
        )))
        .build()
        .expect("builtin registration should build");

    assert_eq!(registry.plans()[0].id(), "deep-review.shared-context");
    assert_eq!(
        registry.plans()[0].kind(),
        &RuntimeHookKind::SuccessfulToolPostCall
    );
}

#[test]
fn runtime_hook_registry_preserves_source_order_timeout_and_error_policy() {
    let registry = RuntimeHookRegistry::builder()
        .register(builtin(
            plan("project.post-call", RuntimeHookSource::ProjectCommand)
                .with_order(20)
                .with_timeout_millis(750),
        ))
        .register(builtin(
            plan(
                "deep-review.shared-context",
                RuntimeHookSource::Builtin { priority: 0 },
            )
            .with_order(20),
        ))
        .register(builtin(
            plan("user.post-call", RuntimeHookSource::UserCommand)
                .with_order(10)
                .with_timeout_millis(250)
                .with_error_policy(RuntimeHookErrorPolicy::SkipHook),
        ))
        .build()
        .expect("hook registry should build");

    assert_eq!(
        registry
            .plans()
            .iter()
            .map(|hook| hook.id())
            .collect::<Vec<_>>(),
        vec![
            "deep-review.shared-context",
            "user.post-call",
            "project.post-call"
        ]
    );
    assert_eq!(registry.plans()[1].timeout_millis(), 250);
    assert_eq!(
        registry.plans()[1].error_policy(),
        RuntimeHookErrorPolicy::SkipHook
    );
}

#[test]
fn runtime_hook_registry_rejects_duplicate_ids() {
    let error = RuntimeHookRegistry::builder()
        .register(builtin(plan(
            "duplicate",
            RuntimeHookSource::Builtin { priority: 0 },
        )))
        .register(builtin(plan("duplicate", RuntimeHookSource::UserCommand)))
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
        .register(builtin(plan(
            "   ",
            RuntimeHookSource::Builtin { priority: 0 },
        )))
        .build()
        .expect_err("blank hook ids must not become registry keys");
    assert_eq!(empty_id_error, RuntimeHookRegistryBuildError::EmptyHookId);

    let zero_timeout_error = RuntimeHookRegistry::builder()
        .register(builtin(
            plan(
                "deep-review.shared-context",
                RuntimeHookSource::Builtin { priority: 0 },
            )
            .with_timeout_millis(0),
        ))
        .build()
        .expect_err("hook timeouts must remain explicit and non-zero");
    assert_eq!(
        zero_timeout_error,
        RuntimeHookRegistryBuildError::InvalidTimeoutMillis {
            hook_id: "deep-review.shared-context".to_string()
        }
    );
}
