//! Bridge the provider-neutral native hook executor to the OpenCode RPC host.

use bitfun_agent_runtime::native_hooks::{
    AgentHookMatcher, PluginHookCall, PluginHookExecutor, PluginHookResult, RuntimeHookCommitToken,
    RuntimeHookKind, RuntimeHookPlan, RuntimeHookRegistration, RuntimeHookRegistry,
    RuntimeHookSource,
};
use bitfun_opencode_plugin_host::{PluginGenerationLease, PluginHostClient};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct PluginHostHookExecutor {
    client: PluginHostClient,
    deadline: Duration,
}

impl PluginHostHookExecutor {
    pub(crate) fn new(client: PluginHostClient) -> Self {
        Self {
            client,
            deadline: Duration::from_secs(30),
        }
    }
}

#[async_trait::async_trait]
impl PluginHookExecutor for PluginHostHookExecutor {
    async fn execute(&self, call: PluginHookCall) -> Result<PluginHookResult, String> {
        let result = self
            .client
            .call_hook(
                &PluginGenerationLease {
                    instance_id: call.instance_id.clone(),
                    generation_key: call.generation_key.clone(),
                    revision: call.revision.clone(),
                },
                &call.hook_name,
                call.input,
                call.output,
                self.deadline,
            )
            .await
            .map_err(|error| error.to_string())?;
        let input = result
            .get("input")
            .cloned()
            .ok_or_else(|| "host.hook.call response is missing input".to_string())?;
        let output = result
            .get("output")
            .cloned()
            .ok_or_else(|| "host.hook.call response is missing output".to_string())?;
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

pub(crate) fn register_plugin_hooks(
    registry: &RuntimeHookRegistry,
    workspace_scope: &str,
    client: PluginHostClient,
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
    let executor: Arc<dyn PluginHookExecutor> = Arc::new(PluginHostHookExecutor::new(client));
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
                    RuntimeHookSource::OpenCodePlugin,
                ),
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

pub(crate) fn hook_names(open_result: &Value) -> Vec<String> {
    open_result
        .get("hooks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|name| matches!(*name, "tool.execute.before" | "tool.execute.after"))
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
            client().await,
            "instance-a",
            "generation-a",
            "revision-a",
            &[],
        )
        .unwrap();

        assert!(token.is_none());
        commit_plugin_generation(&registry, "C:/workspace", token.as_ref());
        assert_eq!(
            registry.source_activation_for_workspace(
                RuntimeHookSource::OpenCodePlugin,
                Some("C:/workspace")
            ),
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
            client().await,
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
            client().await,
            "instance-a",
            "generation-a",
            "revision-a",
            &hooks,
        )
        .is_err());
        assert_eq!(
            registry.source_activation_for_workspace(
                RuntimeHookSource::OpenCodePlugin,
                Some("C:/workspace")
            ),
            RuntimeHookActivation::Ready
        );
    }
}
