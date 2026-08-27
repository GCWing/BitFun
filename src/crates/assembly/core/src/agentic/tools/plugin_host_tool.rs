use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use async_trait::async_trait;
use bitfun_events::ToolExecutionProgressInfo;
use bitfun_runtime_ports::{
    PluginRuntimeInvocationPort, PluginToolCancellationRequest, PluginToolInvocationRequest,
    PortErrorKind,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

#[derive(Clone)]
struct PluginHostToolRoute {
    invoker: Arc<dyn PluginRuntimeInvocationPort>,
    instance_id: String,
    generation_key: String,
    revision: String,
    registration_id: String,
    description: String,
    parameters: Value,
    allowed_runtime_agent_keys: BTreeSet<String>,
}

#[derive(Clone)]
struct PluginToolExecutionRoute {
    session_id: String,
    dialog_turn_id: String,
    agent: String,
    tool_name: String,
    generation_key: String,
    revision: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginToolMetadataParams {
    #[serde(rename = "instanceID")]
    instance_id: String,
    #[serde(rename = "generationKey")]
    generation_key: Option<String>,
    revision: Option<String>,
    #[serde(rename = "executionID")]
    execution_id: String,
    title: Option<String>,
    metadata: Option<Map<String, Value>>,
}

fn executions() -> &'static dashmap::DashMap<(String, String), PluginToolExecutionRoute> {
    static EXECUTIONS: OnceLock<dashmap::DashMap<(String, String), PluginToolExecutionRoute>> =
        OnceLock::new();
    EXECUTIONS.get_or_init(dashmap::DashMap::new)
}

struct PluginHostToolMux {
    id: String,
    hook_registry: bitfun_agent_runtime::native_hooks::RuntimeHookRegistry,
    routes: RwLock<BTreeMap<(String, String), PluginHostToolRoute>>,
}

impl PluginHostToolMux {
    fn new(
        id: String,
        hook_registry: bitfun_agent_runtime::native_hooks::RuntimeHookRegistry,
    ) -> Self {
        Self {
            id,
            hook_registry,
            routes: RwLock::new(BTreeMap::new()),
        }
    }
    fn set_route(&self, workspace_scope: String, route: PluginHostToolRoute) {
        let generation_key = route.generation_key.clone();
        self.routes
            .write()
            .expect("plugin tool route lock poisoned")
            .insert((workspace_scope, generation_key), route);
    }
    fn remove_route(&self, workspace_scope: &str, generation_key: &str) -> bool {
        self.routes
            .write()
            .expect("plugin tool route lock poisoned")
            .remove(&(workspace_scope.to_string(), generation_key.to_string()));
        self.routes
            .read()
            .expect("plugin tool route lock poisoned")
            .is_empty()
    }
    fn routes_for_scope(&self, workspace_scope: &str) -> Vec<PluginHostToolRoute> {
        if self.hook_registry.source_activation_for_workspace(
            bitfun_agent_runtime::native_hooks::RuntimeHookSource::Plugin,
            Some(workspace_scope),
        ) != bitfun_agent_runtime::native_hooks::RuntimeHookActivation::Ready
        {
            return Vec::new();
        }
        self.routes
            .read()
            .expect("plugin tool route lock poisoned")
            .iter()
            .filter(|((scope, _), _)| scope == workspace_scope)
            .map(|(_, route)| route)
            .cloned()
            .collect()
    }
    fn route_for(&self, context: Option<&ToolUseContext>) -> Option<PluginHostToolRoute> {
        let context = context?;
        let runtime_agent_key = context.agent_type.as_deref()?;
        let scope = context
            .workspace_root()
            .and_then(crate::plugin_host::canonical_plugin_workspace_scope)?;
        self.routes_for_scope(&scope)
            .into_iter()
            .find(|route| route.allowed_runtime_agent_keys.contains(runtime_agent_key))
    }
}

#[async_trait]
impl Tool for PluginHostToolMux {
    fn name(&self) -> &str {
        &self.id
    }
    async fn description(&self) -> BitFunResult<String> {
        Ok(self
            .routes
            .read()
            .expect("plugin tool route lock poisoned")
            .values()
            .next()
            .map(|r| r.description.clone())
            .unwrap_or_default())
    }
    async fn description_with_context(
        &self,
        context: Option<&ToolUseContext>,
    ) -> BitFunResult<String> {
        Ok(self
            .route_for(context)
            .map(|r| r.description)
            .unwrap_or_default())
    }
    fn short_description(&self) -> String {
        self.id.clone()
    }
    fn input_schema(&self) -> Value {
        self.routes
            .read()
            .expect("plugin tool route lock poisoned")
            .values()
            .next()
            .map(|r| r.parameters.clone())
            .unwrap_or_else(|| serde_json::json!({"type":"object"}))
    }
    async fn input_schema_for_model_with_context(&self, context: Option<&ToolUseContext>) -> Value {
        self.route_for(context)
            .map(|r| r.parameters)
            .unwrap_or_else(|| serde_json::json!({"type":"object"}))
    }
    fn dynamic_provider_id(&self) -> Option<&str> {
        Some("opencode-plugin")
    }
    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> BitFunResult<Vec<crate::agentic::tools::framework::PermissionIntent>> {
        Ok(vec![
            crate::agentic::tools::framework::PermissionIntent::new(
                "custom_tool",
                vec![self.id.clone()],
            ),
        ])
    }
    async fn is_available_in_context(&self, context: Option<&ToolUseContext>) -> bool {
        self.route_for(context).is_some()
    }
    async fn call_impl(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> BitFunResult<Vec<ToolResult>> {
        let route = self.route_for(Some(context)).ok_or_else(|| {
            BitFunError::service("OpenCode plugin tool is not registered for this workspace")
        })?;
        let execution_id = context
            .tool_call_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let rpc_context = serde_json::json!({
            "sessionID": context.session_id.clone().unwrap_or_default(),
            "messageID": context.dialog_turn_id.clone().unwrap_or_default(),
            "agent": context.agent_type.clone().unwrap_or_default(),
            "callID": context.tool_call_id,
        });
        let execution_key = (route.instance_id.clone(), execution_id.clone());
        executions().insert(
            execution_key.clone(),
            PluginToolExecutionRoute {
                session_id: context.session_id.clone().unwrap_or_default(),
                dialog_turn_id: context.dialog_turn_id.clone().unwrap_or_default(),
                agent: context.agent_type.clone().unwrap_or_default(),
                tool_name: self.id.clone(),
                generation_key: route.generation_key.clone(),
                revision: route.revision.clone(),
            },
        );
        let call = route.invoker.invoke_tool(
            PluginToolInvocationRequest {
                instance_id: route.instance_id.clone(),
                generation_key: route.generation_key.clone(),
                revision: route.revision.clone(),
                execution_id: execution_id.clone(),
                registration_id: route.registration_id.clone(),
                args: input.clone(),
                context: rpc_context,
            },
            Duration::from_secs(120),
        );
        let result = if let Some(token) = context.cancellation_token() {
            tokio::select! {
                value = call => value,
                _ = token.cancelled() => {
                    let cancelled = match route.invoker.cancel_tool(
                        PluginToolCancellationRequest {
                            instance_id: route.instance_id.clone(),
                            generation_key: route.generation_key.clone(),
                            revision: route.revision.clone(),
                            execution_id: execution_id.clone(),
                            reason: Some("cancelled".to_string()),
                        },
                        Duration::from_secs(5),
                    ).await {
                        Ok(cancelled) => cancelled,
                        Err(error) => {
                            crate::plugin_host::fault_configured_plugin_host(
                                "plugin tool cancellation failed",
                            ).await;
                            return Err(BitFunError::OutcomeUnknown(error.to_string()));
                        }
                    };
                    executions().remove(&execution_key);
                    return if cancelled {
                        Err(BitFunError::Cancelled("OpenCode plugin tool cancelled".to_string()))
                    } else {
                        crate::plugin_host::fault_configured_plugin_host(
                            "plugin tool cancellation was not confirmed",
                        )
                        .await;
                        Err(BitFunError::OutcomeUnknown("OpenCode plugin tool cancellation was not confirmed".to_string()))
                    };
                }
            }
        } else {
            call.await
        };
        executions().remove(&execution_key);
        let result = result.map_err(|error| match error.kind {
            // A timed-out/cancelled side-effecting plugin call may still have
            // completed in the Host. Preserve this distinction so the tool
            // pipeline never retries it as a transient service failure.
            PortErrorKind::OutcomeUnknown => BitFunError::OutcomeUnknown(format!(
                "OpenCode plugin tool '{}' outcome is unknown: {}",
                self.id, error.message
            )),
            PortErrorKind::Cancelled => BitFunError::Cancelled(error.message),
            PortErrorKind::Timeout => BitFunError::Timeout(error.message),
            PortErrorKind::PermissionDenied => BitFunError::Validation(error.message),
            _ => BitFunError::service(format!(
                "OpenCode plugin tool '{}' failed: {error}",
                self.id
            )),
        });
        if matches!(result, Err(BitFunError::OutcomeUnknown(_))) {
            crate::plugin_host::fault_configured_plugin_host(
                "plugin tool invocation outcome is unknown",
            )
            .await;
        }
        let result = result?;
        if result
            .get("attachments")
            .and_then(Value::as_array)
            .is_some_and(|attachments| !attachments.is_empty())
        {
            return Err(BitFunError::service("unsupported_tool_attachment"));
        }
        let (data, assistant) = match result {
            Value::String(value) => (Value::String(value.clone()), Some(value)),
            Value::Object(object) => {
                let output = object.get("output").cloned().unwrap_or(Value::Null);
                let assistant = object
                    .get("output")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                (
                    Value::Object(object),
                    assistant.or_else(|| Some(output.to_string())),
                )
            }
            other => (other.clone(), Some(other.to_string())),
        };
        Ok(vec![ToolResult::ok(data, assistant)])
    }
}

pub(crate) async fn handle_tool_metadata(
    params: Value,
) -> Result<Value, bitfun_opencode_plugin_host::RpcHandlerError> {
    let params: PluginToolMetadataParams = serde_json::from_value(params).map_err(|error| {
        bitfun_opencode_plugin_host::RpcHandlerError::new(
            -32602,
            format!("invalid backend.tool.metadata params: {error}"),
        )
    })?;
    let route = executions()
        .get(&(params.instance_id, params.execution_id.clone()))
        .map(|entry| entry.clone())
        .ok_or_else(|| {
            bitfun_opencode_plugin_host::RpcHandlerError::new(
                -32004,
                "plugin tool execution is no longer active",
            )
        })?;
    validate_reverse_generation(
        params.generation_key.as_deref(),
        params.revision.as_deref(),
        &route,
    )?;
    let progress_message = params
        .title
        .unwrap_or_else(|| Value::Object(params.metadata.unwrap_or_default()).to_string());
    crate::infrastructure::events::emit_global_event(
        crate::infrastructure::events::BackendEvent::ToolExecutionProgress(
            ToolExecutionProgressInfo {
                tool_use_id: params.execution_id,
                tool_name: route.tool_name,
                progress_message,
                percentage: None,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            },
        ),
    )
    .await
    .map_err(|error| {
        bitfun_opencode_plugin_host::RpcHandlerError::new(
            -32603,
            format!("failed to publish plugin tool metadata: {error}"),
        )
    })?;
    Ok(serde_json::json!({}))
}

pub(crate) async fn handle_tool_ask(
    params: Value,
) -> Result<Value, bitfun_opencode_plugin_host::RpcHandlerError> {
    let instance_id = params
        .get("instanceID")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            bitfun_opencode_plugin_host::RpcHandlerError::new(
                -32602,
                "backend.tool.ask instanceID is missing",
            )
        })?;
    let execution_id = params
        .get("executionID")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            bitfun_opencode_plugin_host::RpcHandlerError::new(
                -32602,
                "backend.tool.ask executionID is missing",
            )
        })?;
    let route = executions()
        .get(&(instance_id.to_string(), execution_id.to_string()))
        .map(|entry| entry.clone())
        .ok_or_else(|| {
            bitfun_opencode_plugin_host::RpcHandlerError::new(
                -32004,
                "plugin tool execution is no longer active",
            )
        })?;
    validate_reverse_generation(
        params.get("generationKey").and_then(Value::as_str),
        params.get("revision").and_then(Value::as_str),
        &route,
    )?;
    let instance = crate::plugin_host::plugin_host_instance_by_id(instance_id)
        .await
        .ok_or_else(|| {
            bitfun_opencode_plugin_host::RpcHandlerError::new(
                -32004,
                "plugin instance is unavailable",
            )
        })?;
    if instance.generation_key != route.generation_key || instance.revision != route.revision {
        return Err(bitfun_opencode_plugin_host::RpcHandlerError::new(
            -32004,
            "plugin tool generation is no longer active",
        ));
    }
    let permission = params
        .get("permission")
        .and_then(Value::as_str)
        .unwrap_or("custom_tool");
    let mut patterns = params
        .get("patterns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let permission_action = if permission == route.tool_name {
        if patterns.is_empty() {
            patterns.push(route.tool_name.clone());
        }
        "custom_tool"
    } else {
        permission
    };
    let always = params
        .get("always")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let metadata = params
        .get("metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let policy = crate::agentic::agents::get_agent_registry()
        .get_agent_tool_policy(&route.agent, Some(&instance.directory))
        .await;
    let evaluator = bitfun_runtime_ports::PermissionEvaluator::case_sensitive();
    if patterns.iter().any(|resource| {
        evaluator.evaluate_constraint_resource(
            permission_action,
            resource,
            &policy.permission_constraints,
        ) == bitfun_runtime_ports::PermissionEffect::Deny
    }) {
        return Err(bitfun_opencode_plugin_host::RpcHandlerError::new(
            -32003,
            "plugin tool permission denied by the active agent policy",
        ));
    }
    // The pipeline already admitted the exact plugin Tool through the same
    // custom_tool intent before entering the Host. Do not ask twice when the
    // plugin repeats that declaration through context.ask().
    if permission_action == "custom_tool"
        && patterns.iter().all(|resource| resource == &route.tool_name)
    {
        return Ok(serde_json::json!({}));
    }
    let manager = crate::product_runtime::core_permission_request_manager()
        .map_err(|error| bitfun_opencode_plugin_host::RpcHandlerError::new(-32603, error))?;
    let mut pending = manager
        .register_batch_for_turn(
            vec![bitfun_runtime_ports::PermissionRequest {
                request_id: uuid::Uuid::new_v4().to_string(),
                round_id: route.dialog_turn_id.clone(),
                order: 0,
                tool_call_id: Some(execution_id.to_string()),
                project_path: Some(instance.canonical_directory.clone()),
                project_id: instance.project_id,
                session_id: route.session_id,
                agent_id: route.agent,
                action: permission_action.to_string(),
                resources: patterns,
                save_resources: always,
                source: bitfun_runtime_ports::PermissionRequestSource {
                    kind: bitfun_runtime_ports::PermissionRequestSourceKind::Extension,
                    identity: instance_id.to_string(),
                },
                delegation: None,
                display_metadata: metadata,
            }],
            route.dialog_turn_id,
        )
        .await
        .map_err(|error| {
            bitfun_opencode_plugin_host::RpcHandlerError::new(-32603, error.to_string())
        })?;
    let pending = pending
        .pop()
        .expect("single permission batch must return one receiver");
    match pending.wait().await {
        bitfun_agent_runtime::permission::PermissionWaitOutcome::Replied(
            bitfun_runtime_ports::PermissionReply::Once
            | bitfun_runtime_ports::PermissionReply::Always,
        ) => Ok(serde_json::json!({})),
        bitfun_agent_runtime::permission::PermissionWaitOutcome::Replied(
            bitfun_runtime_ports::PermissionReply::Reject { feedback },
        ) => Err(bitfun_opencode_plugin_host::RpcHandlerError::new(
            -32003,
            feedback.unwrap_or_else(|| "plugin tool permission denied".to_string()),
        )),
        bitfun_agent_runtime::permission::PermissionWaitOutcome::Cancelled { reason } => Err(
            bitfun_opencode_plugin_host::RpcHandlerError::new(-32003, reason),
        ),
    }
}

fn validate_reverse_generation(
    generation_key: Option<&str>,
    revision: Option<&str>,
    route: &PluginToolExecutionRoute,
) -> Result<(), bitfun_opencode_plugin_host::RpcHandlerError> {
    if generation_key == Some(route.generation_key.as_str())
        && revision == Some(route.revision.as_str())
    {
        Ok(())
    } else {
        Err(bitfun_opencode_plugin_host::RpcHandlerError::new(
            -32004,
            "plugin tool reverse RPC generation lease does not match the active execution",
        ))
    }
}

fn muxes() -> &'static RwLock<BTreeMap<String, Arc<PluginHostToolMux>>> {
    static MUXES: OnceLock<RwLock<BTreeMap<String, Arc<PluginHostToolMux>>>> = OnceLock::new();
    MUXES.get_or_init(|| RwLock::new(BTreeMap::new()))
}

pub(crate) async fn register_workspace_tool(
    workspace_scope: &str,
    workspace_root: &std::path::Path,
    invoker: Arc<dyn PluginRuntimeInvocationPort>,
    instance_id: &str,
    generation_key: &str,
    revision: &str,
    registration_id: &str,
    id: &str,
    description: &str,
    parameters: Value,
    config_fingerprint: &str,
    allowed_runtime_agent_keys: BTreeSet<String>,
) {
    let mux = {
        let mut muxes = muxes().write().expect("plugin tool mux lock poisoned");
        if let Some(mux) = muxes.get(id) {
            mux.clone()
        } else {
            let mux = Arc::new(PluginHostToolMux::new(
                id.to_string(),
                crate::native_hooks::runtime_hook_registry(),
            ));
            muxes.insert(id.to_string(), mux.clone());
            mux
        }
    };
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    hasher.update([0]);
    hasher.update(description.as_bytes());
    hasher.update([0]);
    hasher.update(serde_json::to_vec(&parameters).unwrap_or_default());
    hasher.update([0]);
    hasher.update(config_fingerprint.as_bytes());
    let content_version = format!("sha256:{}", hex::encode(hasher.finalize()));
    mux.set_route(
        workspace_scope.to_string(),
        PluginHostToolRoute {
            invoker,
            instance_id: instance_id.to_string(),
            generation_key: generation_key.to_string(),
            revision: revision.to_string(),
            registration_id: registration_id.to_string(),
            description: description.to_string(),
            parameters,
            allowed_runtime_agent_keys,
        },
    );
    crate::external_tools::register_live_external_tool_candidate(
        workspace_root,
        mux,
        "opencode-plugin",
        content_version,
    )
    .await;
}

pub(crate) async fn unregister_workspace_tools(
    workspace_scope: &str,
    workspace_root: &std::path::Path,
    names: &[String],
    generation_key: &str,
) {
    for name in names {
        let mux = muxes()
            .read()
            .expect("plugin tool mux lock poisoned")
            .get(name)
            .cloned();
        let Some(mux) = mux else {
            continue;
        };
        if mux.remove_route(workspace_scope, generation_key) {
            muxes()
                .write()
                .expect("plugin tool mux lock poisoned")
                .remove(name);
        }
        if mux.routes_for_scope(workspace_scope).is_empty() {
            crate::external_tools::unregister_live_external_tool_candidate(
                workspace_root,
                name,
                "opencode-plugin",
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        executions, handle_tool_ask, handle_tool_metadata, PluginHostToolMux, PluginHostToolRoute,
        PluginToolExecutionRoute,
    };
    use crate::agentic::tools::framework::ToolUseContext;
    use bitfun_opencode_plugin_host::JsonRpcPeer;
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::path::PathBuf;
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

    fn route(
        client: bitfun_opencode_plugin_host::PluginHostClient,
        instance_id: &str,
    ) -> PluginHostToolRoute {
        let instance_id = instance_id.to_string();
        PluginHostToolRoute {
            invoker: bitfun_opencode_plugin_host::invocation_port(
                client,
                instance_id.clone(),
                "generation-test",
                "revision-test",
            ),
            instance_id: instance_id.clone(),
            generation_key: "generation-test".to_string(),
            revision: "revision-test".to_string(),
            registration_id: format!("registration-{instance_id}"),
            description: format!("description-{instance_id}"),
            parameters: json!({"type": "object", "title": instance_id}),
            allowed_runtime_agent_keys: BTreeSet::from(["plugin-agent".to_string()]),
        }
    }

    fn context(workspace_root: &str, runtime_agent_key: &str) -> ToolUseContext {
        crate::agentic::tools::tool_context_runtime::build_tool_description_context(
            runtime_agent_key,
            Some(&crate::agentic::WorkspaceBinding::new(
                None,
                PathBuf::from(workspace_root),
            )),
            None,
            None,
            None,
            None,
            None,
            &Default::default(),
            &Default::default(),
        )
    }

    #[tokio::test]
    async fn same_named_tool_routes_are_isolated_by_workspace() {
        let client = client().await;
        let registry = bitfun_agent_runtime::native_hooks::RuntimeHookRegistry::default();
        for scope in ["C:/workspace-a", "D:/workspace-b"] {
            registry.set_source_activation_for_workspace(
                bitfun_agent_runtime::native_hooks::RuntimeHookSource::Plugin,
                Some(scope),
                bitfun_agent_runtime::native_hooks::RuntimeHookActivation::Ready,
            );
        }
        let mux = PluginHostToolMux::new("shared-tool".to_string(), registry);
        mux.set_route(
            "C:/workspace-a".to_string(),
            route(client.clone(), "instance-a"),
        );
        mux.set_route("D:/workspace-b".to_string(), route(client, "instance-b"));

        assert_eq!(
            mux.routes_for_scope("C:/workspace-a")
                .pop()
                .unwrap()
                .instance_id,
            "instance-a"
        );
        assert_eq!(
            mux.routes_for_scope("D:/workspace-b")
                .pop()
                .unwrap()
                .instance_id,
            "instance-b"
        );
        assert!(!mux.remove_route("C:/workspace-a", "generation-test"));
        assert!(mux.routes_for_scope("C:/workspace-a").is_empty());
        assert_eq!(
            mux.routes_for_scope("D:/workspace-b")
                .pop()
                .unwrap()
                .instance_id,
            "instance-b"
        );
        assert!(mux.remove_route("D:/workspace-b", "generation-test"));
    }

    #[tokio::test]
    async fn tool_route_honors_the_shared_workspace_activation_gate() {
        use bitfun_agent_runtime::native_hooks::{RuntimeHookActivation, RuntimeHookSource};

        let registry = bitfun_agent_runtime::native_hooks::RuntimeHookRegistry::default();
        let mux = PluginHostToolMux::new("gated-tool".to_string(), registry.clone());
        mux.set_route(
            "C:/workspace-gated".to_string(),
            route(client().await, "instance-gated"),
        );
        registry.set_source_activation_for_workspace(
            RuntimeHookSource::Plugin,
            Some("C:/workspace-gated"),
            RuntimeHookActivation::Unavailable,
        );

        assert!(mux.routes_for_scope("C:/workspace-gated").is_empty());

        registry.set_source_activation_for_workspace(
            RuntimeHookSource::Plugin,
            Some("C:/workspace-gated"),
            RuntimeHookActivation::Ready,
        );
        assert!(!mux.routes_for_scope("C:/workspace-gated").is_empty());
        registry.clear_source_workspace(RuntimeHookSource::Plugin, "C:/workspace-gated");
    }

    #[tokio::test]
    async fn tool_route_requires_the_exact_generation_agent_key() {
        use bitfun_agent_runtime::native_hooks::{RuntimeHookActivation, RuntimeHookSource};

        let workspace = std::env::current_dir().expect("absolute workspace");
        let scope = crate::plugin_host::canonical_plugin_workspace_scope(&workspace)
            .expect("canonical workspace scope");
        let generation_a_agent = "external_subagent_runtime:opencode-plugin:generation-a-agent";
        let generation_b_agent = "external_subagent_runtime:opencode-plugin:generation-b-agent";
        let registry = bitfun_agent_runtime::native_hooks::RuntimeHookRegistry::default();
        registry.set_source_activation_for_workspace(
            RuntimeHookSource::Plugin,
            Some(&scope),
            RuntimeHookActivation::Ready,
        );
        let mux = PluginHostToolMux::new("generation-tool".to_string(), registry.clone());
        let client = client().await;
        let mut route_a = route(client.clone(), "instance-a");
        route_a.generation_key = "generation-a".to_string();
        route_a.allowed_runtime_agent_keys = BTreeSet::from([generation_a_agent.to_string()]);
        let mut route_b = route(client, "instance-b");
        route_b.generation_key = "generation-b".to_string();
        route_b.allowed_runtime_agent_keys = BTreeSet::from([generation_b_agent.to_string()]);
        mux.set_route(scope.clone(), route_a);
        mux.set_route(scope.clone(), route_b);

        let workspace_text = workspace.to_string_lossy();
        assert_eq!(
            mux.route_for(Some(&context(&workspace_text, generation_a_agent)))
                .expect("generation A route")
                .instance_id,
            "instance-a"
        );
        assert_eq!(
            mux.route_for(Some(&context(&workspace_text, generation_b_agent)))
                .expect("generation B route")
                .instance_id,
            "instance-b"
        );
        assert!(mux
            .route_for(Some(&context(
                &workspace_text,
                "external_subagent_runtime:other-provider:agent"
            )))
            .is_none());
        assert!(mux
            .route_for(Some(&context(&workspace_text, "Agentic")))
            .is_none());

        registry.clear_source_workspace(RuntimeHookSource::Plugin, &scope);
    }

    #[tokio::test]
    async fn ask_for_missing_execution_route_is_rejected() {
        let error = handle_tool_ask(json!({
            "instanceID": "missing-instance",
            "executionID": "missing-execution"
        }))
        .await
        .unwrap_err();

        assert_eq!(error.code, -32004);
    }

    #[tokio::test]
    async fn metadata_for_missing_execution_route_is_rejected() {
        let error = handle_tool_metadata(json!({
            "instanceID": "missing-instance",
            "executionID": "missing-execution",
            "title": "progress"
        }))
        .await
        .unwrap_err();

        assert_eq!(error.code, -32004);
    }

    #[tokio::test]
    async fn metadata_for_active_execution_is_published_as_progress() {
        let instance_id = format!("metadata-instance-{}", std::process::id());
        let execution_id = format!("metadata-execution-{}", std::process::id());
        let key = (instance_id.clone(), execution_id.clone());
        executions().insert(
            key.clone(),
            PluginToolExecutionRoute {
                session_id: "session-a".to_string(),
                dialog_turn_id: "turn-a".to_string(),
                agent: "agentic".to_string(),
                tool_name: "plugin-tool".to_string(),
                generation_key: "generation-a".to_string(),
                revision: "revision-a".to_string(),
            },
        );

        let result = handle_tool_metadata(json!({
            "instanceID": instance_id,
            "generationKey": "generation-a",
            "revision": "revision-a",
            "executionID": execution_id,
            "title": "Reading README.md",
            "metadata": {"path": "README.md"}
        }))
        .await;
        executions().remove(&key);

        assert_eq!(result.unwrap(), json!({}));
    }
}
