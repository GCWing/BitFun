//! Hook dispatch: run matching command handlers for one lifecycle event.
//!
//! Process interface (Codex-compatible):
//! - The JSON payload is written to the handler's stdin.
//! - Exit code 0: stdout is interpreted as a JSON decision document when it
//!   parses; otherwise, for events where plain stdout is context
//!   (SessionStart, UserPromptSubmit, SubagentStart), the text becomes
//!   model-visible context.
//! - Exit code 2: the event is blocked; stderr provides the blocking reason.
//! - Any other exit code, spawn failure, or timeout: a non-blocking warning.

use super::call::{HookCall, HookCallPayload};
use super::handler::{HookHandler, HookHandlerResult, PluginHookCall};
use super::kind::RuntimeHookKind;
use super::output::{non_empty, AgentHookOutcome, RawHookOutput};
use super::payload::AgentHookPayload;
use super::registry::RuntimeHookRegistry;
use super::settings::{AgentHookEvent, AgentHookHandler, AgentHookSettings};
use log::{debug, warn};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Cap for a single model-visible hook text (reason or context). Larger
/// output is truncated with a marker, mirroring the Codex output budget.
pub const MAX_HOOK_MODEL_OUTPUT_BYTES: usize = 10_000;

/// Cap for captured process output retained in memory.
const MAX_CAPTURED_OUTPUT_BYTES: usize = 1024 * 1024;

/// Executes configured hooks for agent lifecycle events.
#[derive(Debug, Clone, Default)]
pub struct AgentHookEngine {
    registry: RuntimeHookRegistry,
    settings: Option<Arc<AgentHookSettings>>,
}

impl AgentHookEngine {
    pub fn new(settings: AgentHookSettings) -> Self {
        let registry = RuntimeHookRegistry::default();
        registry
            .register_batch(settings.registrations())
            .expect("parsed hook settings must produce valid registrations");
        Self {
            registry,
            settings: Some(Arc::new(settings)),
        }
    }

    pub fn with_registry(registry: RuntimeHookRegistry) -> Self {
        Self {
            registry,
            settings: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.registry.plans().is_empty()
    }

    pub fn has_rules(&self, event: AgentHookEvent) -> bool {
        self.has_rules_for_workspace(event, None)
    }

    pub fn has_rules_for_workspace(
        &self,
        event: AgentHookEvent,
        workspace_scope: Option<&str>,
    ) -> bool {
        !self
            .registry
            .registrations_for_workspace(RuntimeHookKind::Lifecycle(event), workspace_scope)
            .is_empty()
    }

    pub fn settings(&self) -> &AgentHookSettings {
        self.settings
            .as_deref()
            .expect("engine was constructed from a runtime hook registry")
    }

    pub fn registry(&self) -> &RuntimeHookRegistry {
        &self.registry
    }

    /// Run every matching handler for the payload's event, sequentially in
    /// configuration order (user layers before project layers), and fold
    /// their results into one [`AgentHookOutcome`].
    pub async fn dispatch(&self, payload: &AgentHookPayload, cwd: &Path) -> AgentHookOutcome {
        self.dispatch_for_workspace(payload, cwd, None).await
    }

    pub async fn dispatch_for_workspace(
        &self,
        payload: &AgentHookPayload,
        cwd: &Path,
        workspace_scope: Option<&str>,
    ) -> AgentHookOutcome {
        let event = payload.event();
        let mut outcome = AgentHookOutcome::default();
        let registrations = self
            .registry
            .registrations_for_workspace(RuntimeHookKind::Lifecycle(event), workspace_scope);
        if registrations.is_empty() {
            return outcome;
        }
        let matcher_value = payload.event.matcher_value();
        let payload_json = payload.to_json().to_string();
        let call = lifecycle_call(payload, cwd);
        for registration in registrations.iter() {
            if !registration.matcher.matches(matcher_value) {
                continue;
            }
            outcome.executed_handlers += 1;
            let finalized = match &registration.handler {
                HookHandler::Command(handler) => {
                    self.run_and_apply(event, handler, &payload_json, cwd, &mut outcome)
                        .await
                }
                HookHandler::Builtin { executor } => {
                    apply_handler_result(executor.execute(&call).await, &mut outcome)
                }
                HookHandler::Plugin {
                    executor,
                    instance_id,
                    hook_name,
                    generation_key,
                    revision,
                    ..
                } => {
                    let result = tokio::time::timeout(
                        Duration::from_millis(registration.plan.timeout_millis()),
                        executor.execute(PluginHookCall {
                            instance_id: instance_id.clone(),
                            workspace_scope: registration
                                .workspace_scope
                                .clone()
                                .unwrap_or_default(),
                            generation_key: generation_key.clone(),
                            revision: revision.clone(),
                            hook_name: hook_name.clone(),
                            input: payload.to_json(),
                            output: Value::Object(Default::default()),
                        }),
                    )
                    .await;
                    if let Err(error) = result.unwrap_or_else(|_| Err("timed out".to_string())) {
                        outcome.warnings.push(format!(
                            "Plugin hook '{}' for {event} failed: {error}",
                            registration.plan.id()
                        ));
                    }
                    false
                }
            };
            if finalized {
                break;
            }
        }
        outcome
    }

    /// Execute one provider hook snapshot and carry input/output mutations
    /// forward in registry order.
    pub async fn dispatch_plugin_hook(
        &self,
        workspace_scope: Option<&str>,
        hook_name: &str,
        input: Value,
        output: Value,
    ) -> PluginHookDispatchResult {
        self.dispatch_plugin_hook_for_generation(workspace_scope, None, hook_name, input, output)
            .await
    }

    pub async fn dispatch_plugin_hook_for_generation(
        &self,
        workspace_scope: Option<&str>,
        generation: Option<&super::handler::PluginHookGenerationIdentity>,
        hook_name: &str,
        mut input: Value,
        mut output: Value,
    ) -> PluginHookDispatchResult {
        let kind = RuntimeHookKind::PluginHook(hook_name.to_string());
        let registrations = match (workspace_scope, generation) {
            (Some(workspace_scope), Some(generation)) => self
                .registry
                .registrations_for_plugin_generation(kind, workspace_scope, generation),
            _ => self
                .registry
                .registrations_for_workspace(kind, workspace_scope),
        };
        let mut result = PluginHookDispatchResult::default();
        for registration in registrations.iter() {
            let HookHandler::Plugin {
                executor,
                instance_id,
                hook_name,
                generation_key,
                revision,
                ..
            } = &registration.handler
            else {
                continue;
            };
            result.executed_handlers += 1;
            let invocation = executor.execute(PluginHookCall {
                instance_id: instance_id.clone(),
                workspace_scope: registration.workspace_scope.clone().unwrap_or_default(),
                generation_key: generation_key.clone(),
                revision: revision.clone(),
                hook_name: hook_name.clone(),
                input: input.clone(),
                output: output.clone(),
            });
            match tokio::time::timeout(
                Duration::from_millis(registration.plan.timeout_millis()),
                invocation,
            )
            .await
            {
                Ok(Ok(transformed)) => {
                    if transformed.instance_id != *instance_id
                        || transformed.generation_key != *generation_key
                        || transformed.revision != *revision
                        || transformed.hook_name != *hook_name
                    {
                        result.warnings.push(format!(
                            "Plugin hook '{}' returned a mismatched generation lease",
                            registration.plan.id()
                        ));
                        continue;
                    }
                    input = transformed.input;
                    output = transformed.output;
                }
                Ok(Err(error)) => result.warnings.push(format!(
                    "Plugin hook '{}' failed: {error}",
                    registration.plan.id()
                )),
                Err(_) => result.warnings.push(format!(
                    "Plugin hook '{}' timed out after {}ms",
                    registration.plan.id(),
                    registration.plan.timeout_millis()
                )),
            }
        }
        result.input = input;
        result.output = output;
        result
    }

    pub async fn dispatch_call(
        &self,
        workspace_scope: Option<&str>,
        call: &HookCall,
    ) -> HookHandlerResult {
        let registrations = self
            .registry
            .registrations_for_workspace(call.kind.clone(), workspace_scope);
        let mut result = HookHandlerResult::default();
        for registration in registrations.iter() {
            if let HookHandler::Builtin { executor } = &registration.handler {
                merge_handler_result(executor.execute(call).await, &mut result);
            }
        }
        result
    }

    /// Run one handler and fold its result into `outcome`. Returns `true`
    /// when the dispatch is finalized (blocked or denied) and remaining
    /// handlers must not run.
    async fn run_and_apply(
        &self,
        event: AgentHookEvent,
        handler: &AgentHookHandler,
        payload_json: &str,
        cwd: &Path,
        outcome: &mut AgentHookOutcome,
    ) -> bool {
        let command = handler.effective_command();
        let timeout = handler.effective_timeout(event);
        debug!(
            "Running agent hook: event={}, command={}, timeout_ms={}",
            event,
            command,
            timeout.as_millis()
        );
        let run = run_hook_command(command, payload_json, cwd, timeout).await;
        match run {
            HookCommandRun::SpawnFailed(error) => {
                outcome.warnings.push(format!(
                    "Hook '{command}' for {event} could not be started: {error}"
                ));
                false
            }
            HookCommandRun::TimedOut => {
                outcome.warnings.push(format!(
                    "Hook '{command}' for {event} timed out after {}s and was killed",
                    timeout.as_secs()
                ));
                false
            }
            HookCommandRun::Completed {
                exit_code,
                stdout,
                stderr,
            } => match exit_code {
                Some(0) => match serde_json::from_str::<RawHookOutput>(stdout.trim()) {
                    Ok(output) => outcome.apply_output(output),
                    Err(_) => {
                        let text = stdout.trim();
                        if !text.is_empty() && event.plain_stdout_is_context() {
                            outcome.additional_context.push(truncate_model_output(text));
                        }
                        false
                    }
                },
                Some(2) => {
                    let reason = non_empty(Some(stderr)).unwrap_or_else(|| {
                        format!("Hook '{command}' blocked this {event} event (exit code 2).")
                    });
                    if outcome.block_reason.is_none() {
                        outcome.block_reason = Some(truncate_model_output(&reason));
                    }
                    true
                }
                Some(code) => {
                    outcome.warnings.push(format!(
                        "Hook '{command}' for {event} exited with non-blocking code {code}"
                    ));
                    false
                }
                None => {
                    outcome.warnings.push(format!(
                        "Hook '{command}' for {event} was terminated by a signal"
                    ));
                    false
                }
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginHookDispatchResult {
    pub input: Value,
    pub output: Value,
    pub warnings: Vec<String>,
    pub executed_handlers: usize,
}

impl Default for PluginHookDispatchResult {
    fn default() -> Self {
        Self {
            input: Value::Null,
            output: Value::Null,
            warnings: Vec::new(),
            executed_handlers: 0,
        }
    }
}

fn lifecycle_call(payload: &AgentHookPayload, cwd: &Path) -> HookCall {
    HookCall {
        kind: RuntimeHookKind::Lifecycle(payload.event()),
        cwd: cwd.to_path_buf(),
        session_id: Some(payload.common.session_id.clone()),
        turn_id: payload.common.turn_id.clone(),
        workspace_root: Some(cwd.to_path_buf()),
        is_remote: false,
        model: Some(payload.common.model.clone()),
        bypass_permissions: matches!(
            payload.common.permission_mode,
            super::payload::AgentHookPermissionMode::BypassPermissions
        ),
        payload: HookCallPayload::Lifecycle(payload.event.clone()),
    }
}

fn apply_handler_result(result: HookHandlerResult, outcome: &mut AgentHookOutcome) -> bool {
    outcome.warnings.extend(result.warnings);
    outcome.additional_context.extend(result.additional_context);
    if outcome.block_reason.is_none() {
        outcome.block_reason = result.block_reason;
    }
    outcome.is_blocked()
}

fn merge_handler_result(result: HookHandlerResult, outcome: &mut HookHandlerResult) {
    outcome.warnings.extend(result.warnings);
    outcome.additional_context.extend(result.additional_context);
    if outcome.block_reason.is_none() {
        outcome.block_reason = result.block_reason;
    }
}

enum HookCommandRun {
    Completed {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    TimedOut,
    SpawnFailed(String),
}

async fn run_hook_command(
    command: &str,
    payload_json: &str,
    cwd: &Path,
    timeout: Duration,
) -> HookCommandRun {
    let mut process = if cfg!(windows) {
        let mut process = Command::new("cmd");
        process.arg("/C").arg(command);
        process
    } else {
        let mut process = Command::new("sh");
        process.arg("-c").arg(command);
        process
    };
    if let Some(cwd) = existing_dir(cwd) {
        process.current_dir(cwd);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        process.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => return HookCommandRun::SpawnFailed(error.to_string()),
    };
    let mut stdin = child.stdin.take();

    // The stdin write must be inside the timeout and must not be awaited to
    // completion before the child is reaped: a handler that never reads stdin
    // blocks the write once the payload exceeds the pipe buffer, and a
    // handler that exits early makes the write fail with EPIPE. Driving the
    // write concurrently with `wait_with_output` covers both, and the whole
    // interaction is bounded by one timeout.
    let interaction = async {
        let write = async {
            if let Some(mut stdin) = stdin.take() {
                let _ = stdin.write_all(payload_json.as_bytes()).await;
                let _ = stdin.shutdown().await;
            }
        };
        let (_, output) = tokio::join!(write, child.wait_with_output());
        output
    };

    match tokio::time::timeout(timeout, interaction).await {
        Ok(Ok(output)) => HookCommandRun::Completed {
            exit_code: output.status.code(),
            stdout: bounded_lossy_string(output.stdout),
            stderr: bounded_lossy_string(output.stderr),
        },
        Ok(Err(error)) => HookCommandRun::SpawnFailed(error.to_string()),
        // `kill_on_drop` reaps the child when the timeout drops the future.
        Err(_) => HookCommandRun::TimedOut,
    }
}

fn existing_dir(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        warn!(
            "Hook working directory does not exist; running without it: {}",
            path.display()
        );
        None
    }
}

fn bounded_lossy_string(mut bytes: Vec<u8>) -> String {
    if bytes.len() > MAX_CAPTURED_OUTPUT_BYTES {
        bytes.truncate(MAX_CAPTURED_OUTPUT_BYTES);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Truncate model-visible hook text to the output budget, preserving UTF-8.
pub(crate) fn truncate_model_output(text: &str) -> String {
    if text.len() <= MAX_HOOK_MODEL_OUTPUT_BYTES {
        return text.to_string();
    }
    let mut end = MAX_HOOK_MODEL_OUTPUT_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[hook output truncated]", &text[..end])
}
