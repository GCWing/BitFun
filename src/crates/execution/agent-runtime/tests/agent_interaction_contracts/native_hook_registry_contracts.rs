use async_trait::async_trait;
use bitfun_agent_runtime::native_hooks::{
    AgentHookEngine, AgentHookMatcher, PluginHookCall, PluginHookExecutor,
    PluginHookGenerationIdentity, PluginHookResult, RuntimeHookActivation, RuntimeHookKind,
    RuntimeHookPlan, RuntimeHookRegistration, RuntimeHookRegistry, RuntimeHookSource,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakePluginExecutor {
    calls: Mutex<Vec<PluginHookCall>>,
}

#[async_trait]
impl PluginHookExecutor for FakePluginExecutor {
    async fn execute(&self, mut call: PluginHookCall) -> Result<PluginHookResult, String> {
        self.calls.lock().unwrap().push(call.clone());
        call.output
            .as_array_mut()
            .expect("array output")
            .push(serde_json::Value::String(call.instance_id.clone()));
        Ok(PluginHookResult {
            instance_id: call.instance_id,
            generation_key: call.generation_key,
            revision: call.revision,
            hook_name: call.hook_name,
            input: call.input,
            output: call.output,
        })
    }
}

fn plugin_registration(
    executor: Arc<FakePluginExecutor>,
    id: &str,
    instance_id: &str,
) -> RuntimeHookRegistration {
    plugin_registration_for_generation(executor, id, instance_id, "generation-1", "rev-1")
}

fn plugin_registration_for_generation(
    executor: Arc<FakePluginExecutor>,
    id: &str,
    instance_id: &str,
    generation_key: &str,
    revision: &str,
) -> RuntimeHookRegistration {
    RuntimeHookRegistration::plugin(
        RuntimeHookPlan::new(
            id,
            RuntimeHookKind::PluginHook("tool.execute.before".to_string()),
            RuntimeHookSource::Plugin,
        ),
        "tool.execute.before",
        instance_id,
        generation_key,
        revision,
        executor,
        AgentHookMatcher::Any,
    )
    .with_workspace_scope("C:/workspace")
}

#[test]
fn activation_gate_hides_plugin_snapshot_until_ready() {
    let registry = RuntimeHookRegistry::default();
    let token = registry
        .register_plugin_batch(vec![plugin_registration(
            Arc::new(FakePluginExecutor::default()),
            "hook.a",
            "plugin-a",
        )])
        .unwrap();
    registry.set_source_activation(RuntimeHookSource::Plugin, RuntimeHookActivation::Preparing);
    assert!(registry
        .registrations_for_workspace(
            RuntimeHookKind::PluginHook("tool.execute.before".to_string()),
            Some("C:/workspace"),
        )
        .is_empty());
    registry.activate_plugin_batch("C:/workspace", Some(&token));
    assert_eq!(
        registry
            .registrations_for_workspace(
                RuntimeHookKind::PluginHook("tool.execute.before".to_string()),
                Some("C:/workspace"),
            )
            .len(),
        1
    );
}

#[test]
fn plugin_batch_commit_token_rolls_back_only_its_instance() {
    let executor = Arc::new(FakePluginExecutor::default());
    let registry = RuntimeHookRegistry::default();
    let token = registry
        .register_plugin_batch(vec![plugin_registration(
            executor.clone(),
            "hook.a",
            "plugin-a",
        )])
        .unwrap();
    registry
        .register_plugin_batch(vec![plugin_registration(executor, "hook.b", "plugin-b")])
        .unwrap();

    registry.rollback_plugin_batch(&token);

    assert_eq!(
        registry
            .plans()
            .iter()
            .map(|plan| plan.id())
            .collect::<Vec<_>>(),
        vec!["hook.b"]
    );
}

#[tokio::test]
async fn plugin_dispatch_invokes_each_handler_and_carries_output_forward() {
    let executor = Arc::new(FakePluginExecutor::default());
    let registry = RuntimeHookRegistry::default();
    let token = registry
        .register_plugin_batch(vec![
            plugin_registration(executor.clone(), "hook-a", "plugin-a"),
            plugin_registration(executor.clone(), "hook-b", "plugin-a"),
            plugin_registration(executor.clone(), "hook-c", "plugin-a"),
        ])
        .unwrap();
    registry.activate_plugin_batch("C:/workspace", Some(&token));

    let result = AgentHookEngine::with_registry(registry)
        .dispatch_plugin_hook(
            Some("C:/workspace"),
            "tool.execute.before",
            serde_json::json!({"tool": "read"}),
            serde_json::json!([]),
        )
        .await;

    assert_eq!(result.executed_handlers, 3);
    assert!(result.warnings.is_empty());
    assert_eq!(
        result.output,
        serde_json::json!(["plugin-a", "plugin-a", "plugin-a"])
    );
    let calls = executor.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1].output, serde_json::json!(["plugin-a"]));
    assert_eq!(calls[2].output, serde_json::json!(["plugin-a", "plugin-a"]));
}

#[tokio::test]
async fn plugin_dispatch_isolated_by_canonical_workspace_scope() {
    let executor = Arc::new(FakePluginExecutor::default());
    let registry = RuntimeHookRegistry::default();
    let token_a = registry
        .register_plugin_batch(vec![plugin_registration(
            executor.clone(),
            "hook-a",
            "plugin-a",
        )])
        .unwrap();
    let token_b = registry
        .register_plugin_batch(vec![plugin_registration(
            executor.clone(),
            "hook-b",
            "plugin-b",
        )
        .with_workspace_scope("D:/workspace")])
        .unwrap();
    registry.activate_plugin_batch("C:/workspace", Some(&token_a));
    registry.activate_plugin_batch("D:/workspace", Some(&token_b));

    let engine = AgentHookEngine::with_registry(registry);
    let a = engine
        .dispatch_plugin_hook(
            Some("C:/workspace"),
            "tool.execute.before",
            serde_json::json!({}),
            serde_json::json!([]),
        )
        .await;
    let b = engine
        .dispatch_plugin_hook(
            Some("D:/workspace"),
            "tool.execute.before",
            serde_json::json!({}),
            serde_json::json!([]),
        )
        .await;

    assert_eq!(a.executed_handlers, 1);
    assert_eq!(a.output, serde_json::json!(["plugin-a"]));
    assert_eq!(b.executed_handlers, 1);
    assert_eq!(b.output, serde_json::json!(["plugin-b"]));
}

#[tokio::test]
async fn plugin_dispatch_uses_only_the_active_generation() {
    let executor = Arc::new(FakePluginExecutor::default());
    let registry = RuntimeHookRegistry::default();
    let old_token = registry
        .register_plugin_batch(vec![plugin_registration_for_generation(
            executor.clone(),
            "hook-old",
            "plugin-old",
            "generation-old",
            "rev-old",
        )])
        .unwrap();
    let new_token = registry
        .register_plugin_batch(vec![plugin_registration_for_generation(
            executor,
            "hook-new",
            "plugin-new",
            "generation-new",
            "rev-new",
        )])
        .unwrap();

    registry.activate_plugin_batch("C:/workspace", Some(&old_token));
    let engine = AgentHookEngine::with_registry(registry.clone());
    let old = engine
        .dispatch_plugin_hook(
            Some("C:/workspace"),
            "tool.execute.before",
            serde_json::json!({}),
            serde_json::json!([]),
        )
        .await;
    assert_eq!(old.output, serde_json::json!(["plugin-old"]));

    registry.activate_plugin_batch("C:/workspace", Some(&new_token));
    let old_generation = PluginHookGenerationIdentity {
        instance_id: "plugin-old".to_string(),
        generation_key: "generation-old".to_string(),
        revision: "rev-old".to_string(),
    };
    let old_turn = engine
        .dispatch_plugin_hook_for_generation(
            Some("C:/workspace"),
            Some(&old_generation),
            "tool.execute.before",
            serde_json::json!({}),
            serde_json::json!([]),
        )
        .await;
    assert_eq!(old_turn.output, serde_json::json!(["plugin-old"]));

    registry.rollback_plugin_batch(&old_token);
    let new = engine
        .dispatch_plugin_hook(
            Some("C:/workspace"),
            "tool.execute.before",
            serde_json::json!({}),
            serde_json::json!([]),
        )
        .await;
    assert_eq!(new.output, serde_json::json!(["plugin-new"]));
}
