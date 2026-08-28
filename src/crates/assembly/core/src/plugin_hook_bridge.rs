//! Bridge the provider-neutral native hook executor to the OpenCode RPC host.

use bitfun_agent_runtime::native_hooks::{
    AgentHookMatcher, PluginHookCall, PluginHookExecutor, PluginHookResult, RuntimeHookCommitToken,
    RuntimeHookErrorPolicy, RuntimeHookKind, RuntimeHookPlan, RuntimeHookRegistration,
    RuntimeHookRegistry, RuntimeHookSource,
};
use bitfun_runtime_ports::{
    HookFunctionAfterOutput, HookFunctionAfterRequest, HookFunctionBeforeRequest,
    HookFunctionGeneration, HookFunctionRuntime,
};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct PluginHostHookExecutor {
    runtime: Arc<dyn HookFunctionRuntime>,
    deadline: Duration,
}

impl PluginHostHookExecutor {
    pub(crate) fn new(runtime: Arc<dyn HookFunctionRuntime>) -> Self {
        Self {
            runtime,
            deadline: Duration::from_secs(30),
        }
    }
}

#[async_trait::async_trait]
impl PluginHookExecutor for PluginHostHookExecutor {
    async fn execute(&self, call: PluginHookCall) -> Result<PluginHookResult, String> {
        let generation = HookFunctionGeneration {
            instance_id: call.instance_id.clone(),
            generation_key: call.generation_key.clone(),
            revision: call.revision.clone(),
        };
        let input = call.input.clone();
        let output = match call.hook_name.as_str() {
            "tool.execute.before" => {
                let result = self
                    .runtime
                    .transform_tool_before(
                        HookFunctionBeforeRequest {
                            generation,
                            tool_name: string_field(&call.input, "tool"),
                            session_id: string_field(&call.input, "sessionID"),
                            call_id: string_field(&call.input, "callID"),
                            args: call.output.get("args").cloned().ok_or_else(|| {
                                "tool.execute.before input is missing args".to_string()
                            })?,
                        },
                        self.deadline,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                serde_json::json!({"args": result.args})
            }
            "tool.execute.after" => {
                let metadata = call
                    .output
                    .get("metadata")
                    .and_then(Value::as_object)
                    .cloned()
                    .ok_or_else(|| {
                        "tool.execute.after output metadata must be an object".to_string()
                    })?;
                let result = self
                    .runtime
                    .transform_tool_after(
                        HookFunctionAfterRequest {
                            generation,
                            tool_name: string_field(&call.input, "tool"),
                            session_id: string_field(&call.input, "sessionID"),
                            call_id: string_field(&call.input, "callID"),
                            args: call.input.get("args").cloned().unwrap_or(Value::Null),
                            output: HookFunctionAfterOutput {
                                title: string_field(&call.output, "title"),
                                output: string_field(&call.output, "output"),
                                metadata,
                            },
                        },
                        self.deadline,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                serde_json::to_value(result).map_err(|error| error.to_string())?
            }
            other => return Err(format!("unsupported operational plugin hook: {other}")),
        };
        Ok(PluginHookResult {
            instance_id: call.instance_id,
            generation_key: call.generation_key,
            revision: call.revision,
            hook_name: call.hook_name,
            input,
            output,
        })
    }
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
pub(crate) fn register_plugin_hooks(
    registry: &RuntimeHookRegistry,
    workspace_scope: &str,
    runtime: Arc<dyn HookFunctionRuntime>,
    instance_id: &str,
    generation_key: &str,
    revision: &str,
    hook_names: &[String],
) -> Result<Option<RuntimeHookCommitToken>, String> {
    register_plugin_hooks_with_runtime(
        registry,
        workspace_scope,
        runtime,
        instance_id,
        generation_key,
        revision,
        hook_names,
    )
}

pub(crate) fn register_plugin_hooks_with_runtime(
    registry: &RuntimeHookRegistry,
    workspace_scope: &str,
    runtime: Arc<dyn HookFunctionRuntime>,
    instance_id: &str,
    generation_key: &str,
    revision: &str,
    hook_names: &[String],
) -> Result<Option<RuntimeHookCommitToken>, String> {
    log::debug!(
        "Plugin hook registration preparing: workspace={}, instance_id={}, hook_count={}",
        workspace_scope,
        instance_id,
        hook_names.len()
    );
    let executor: Arc<dyn PluginHookExecutor> = Arc::new(PluginHostHookExecutor::new(runtime));
    let entries = hook_names
        .iter()
        .map(|hook_name| {
            let id = format!(
                "opencode:{workspace_scope}:{instance_id}:{generation_key}:{revision}:{hook_name}"
            );
            RuntimeHookRegistration::plugin(
                RuntimeHookPlan::new(
                    id,
                    RuntimeHookKind::PluginHook(hook_name.clone()),
                    RuntimeHookSource::Plugin,
                )
                // The adapter owns a 30s RPC deadline. Keep the registry's
                // outer guard slightly longer so it observes that terminal
                // error instead of dropping an in-flight Host request.
                .with_timeout_millis(31_000)
                .with_error_policy(RuntimeHookErrorPolicy::DenyTool),
                hook_name,
                instance_id,
                generation_key,
                revision,
                executor.clone(),
                AgentHookMatcher::Any,
            )
            .with_workspace_scope(workspace_scope)
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        log::debug!(
            "Plugin hook registration prepared with no dispatch hooks: workspace={}, instance_id={}",
            workspace_scope,
            instance_id
        );
        return Ok(None);
    }
    let token = match registry.register_plugin_batch(entries) {
        Ok(token) => token,
        Err(error) => {
            log::error!(
                "Plugin hook registration failed: workspace={}, instance_id={}, hook_count={}, error={}",
                workspace_scope,
                instance_id,
                hook_names.len(),
                error
            );
            return Err(error.to_string());
        }
    };
    log::info!(
        "Plugin hook registration prepared in Rust registry: workspace={}, instance_id={}, target_id={}, generation_key={}, revision={}, hook_count={}",
        workspace_scope,
        instance_id,
        token.target_id(),
        token.generation_key(),
        token.revision(),
        hook_names.len()
    );
    Ok(Some(token))
}

pub(crate) fn commit_plugin_generation(
    registry: &RuntimeHookRegistry,
    workspace_scope: &str,
    token: Option<&RuntimeHookCommitToken>,
) {
    registry.activate_plugin_batch(workspace_scope, token);
}

pub(crate) fn unregister_plugin_hooks(
    registry: &RuntimeHookRegistry,
    workspace_scope: &str,
    token: RuntimeHookCommitToken,
) {
    registry.rollback_plugin_batch(&token);
    let _ = workspace_scope;
}

pub(crate) fn withdraw_plugin_workspace(registry: &RuntimeHookRegistry, workspace_scope: &str) {
    registry.withdraw_plugin_workspace(workspace_scope);
}

pub(crate) fn hook_names(
    batch: &bitfun_runtime_ports::HookFunctionRegistrationBatch,
) -> Vec<String> {
    batch
        .hooks
        .iter()
        .map(|hook| match hook {
            bitfun_runtime_ports::HookFunctionHookKind::ToolExecuteBefore => "tool.execute.before",
            bitfun_runtime_ports::HookFunctionHookKind::ToolExecuteAfter => "tool.execute.after",
        })
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{commit_plugin_generation, register_plugin_hooks};
    use bitfun_agent_runtime::native_hooks::{
        RuntimeHookActivation, RuntimeHookRegistry, RuntimeHookSource,
    };
    use bitfun_opencode_plugin_host::JsonRpcPeer;
    use tokio::net::{TcpListener, TcpStream};

    async fn client() -> bitfun_opencode_plugin_host::PluginHostClient {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let host = tokio::spawn(async move { TcpStream::connect(address).await.unwrap() });
        let (backend, _) = listener.accept().await.unwrap();
        let _host = host.await.unwrap();
        JsonRpcPeer::start_with_capabilities(
            backend,
            1,
            1024 * 1024,
            bitfun_opencode_plugin_host::PluginHostCapabilities::all_supported(),
        )
        .client()
    }

    #[tokio::test]
    async fn empty_hook_set_is_ready_without_a_commit_token() {
        let registry = RuntimeHookRegistry::default();
        let token = register_plugin_hooks(
            &registry,
            "C:/workspace",
            bitfun_opencode_plugin_host::hook_function_runtime(client().await),
            "instance-a",
            "generation-a",
            "revision-a",
            &[],
        )
        .unwrap();

        assert!(token.is_none());
        commit_plugin_generation(&registry, "C:/workspace", token.as_ref());
        assert_eq!(
            registry
                .source_activation_for_workspace(RuntimeHookSource::Plugin, Some("C:/workspace")),
            RuntimeHookActivation::Ready
        );
    }

    #[tokio::test]
    async fn duplicate_hook_registration_preserves_active_generation() {
        let registry = RuntimeHookRegistry::default();
        let hooks = vec!["tool.execute.before".to_string()];
        let first = register_plugin_hooks(
            &registry,
            "C:/workspace",
            bitfun_opencode_plugin_host::hook_function_runtime(client().await),
            "instance-a",
            "generation-a",
            "revision-a",
            &hooks,
        )
        .unwrap();
        commit_plugin_generation(&registry, "C:/workspace", first.as_ref());
        assert!(register_plugin_hooks(
            &registry,
            "C:/workspace",
            bitfun_opencode_plugin_host::hook_function_runtime(client().await),
            "instance-a",
            "generation-a",
            "revision-a",
            &hooks,
        )
        .is_err());
        assert_eq!(
            registry
                .source_activation_for_workspace(RuntimeHookSource::Plugin, Some("C:/workspace")),
            RuntimeHookActivation::Ready
        );
    }
}
