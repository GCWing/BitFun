//! Local process owner for provider-neutral JavaScript tool workers.

use async_trait::async_trait;
use bitfun_runtime_ports::{
    PortError, PortErrorKind, PortResult, ScriptToolInvokeRequest, ScriptToolInvokeResponse,
    ScriptToolLoadRequest, ScriptToolLoadResponse, ScriptToolRuntime,
    ScriptToolRuntimeAvailability,
};
use bitfun_services_core::process_tree::ProcessTreeChild;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::sync::{oneshot, watch, Mutex, OwnedSemaphorePermit, RwLock, Semaphore};

const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const CANCEL_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_millis(500);
const MAX_PROTOCOL_FRAME_BYTES: usize = 8 * 1024 * 1024;

// The wrapper creates `responseToken` before evaluating the target, keeps it out
// of normal request input and uses it to reject accidental stdout collisions.
// The target module runs in a separate VM realm to avoid accidental mutation of
// common wrapper globals. Neither mechanism authenticates a hostile module:
// Node VM contexts are not a security boundary, and approved code still controls
// its target process with the disclosed filesystem, network, child-process and
// environment capabilities under the current user.
const WORKER_SOURCE: &str = r#"
import readline from "node:readline";
import vm from "node:vm";
import { randomUUID } from "node:crypto";

const parse = JSON.parse.bind(JSON);
const stringify = JSON.stringify.bind(JSON);
const protocolWrite = process.stdout.write.bind(process.stdout);
const reallyExit = process.reallyExit.bind(process);
const trustedSetTimeout = setTimeout;
const responseToken = randomUUID();
const maxOutputBytes = 1024 * 1024;
const targets = new Map();
const operations = new Map();

function write(value) {
  protocolWrite(`${stringify(value)}\n`);
}

write({ kind: "ready", token: responseToken });

function processFacade() {
  const blockedControlModules = new Set([
    "inspector", "node:inspector", "module", "node:module", "process", "node:process",
    "vm", "node:vm", "worker_threads", "node:worker_threads",
  ]);
  return Object.freeze({
    argv: Object.freeze([...process.argv]),
    arch: process.arch,
    env: process.env,
    platform: process.platform,
    version: process.version,
    versions: process.versions,
    cwd: process.cwd.bind(process),
    chdir: process.chdir.bind(process),
    exit: process.exit.bind(process),
    getBuiltinModule(name) {
      if (blockedControlModules.has(name)) {
        throw new Error(`builtin module '${name}' is unavailable inside the script tool worker`);
      }
      return process.getBuiltinModule(name);
    },
  });
}

function moduleContext() {
  const stderrConsole = Object.freeze({
    debug: console.error.bind(console),
    error: console.error.bind(console),
    info: console.error.bind(console),
    log: console.error.bind(console),
    warn: console.error.bind(console),
  });
  const sandbox = {
    AbortController,
    AbortSignal,
    Blob,
    Buffer,
    clearImmediate,
    clearInterval,
    clearTimeout,
    console: stderrConsole,
    crypto: globalThis.crypto,
    fetch: globalThis.fetch,
    FormData: globalThis.FormData,
    Headers: globalThis.Headers,
    process: processFacade(),
    queueMicrotask,
    Request: globalThis.Request,
    Response: globalThis.Response,
    setImmediate,
    setInterval,
    setTimeout,
    structuredClone,
    TextDecoder,
    TextEncoder,
    URL,
    URLSearchParams,
  };
  sandbox.global = sandbox;
  sandbox.globalThis = sandbox;
  return vm.createContext(sandbox, { name: "bitfun-script-tool" });
}

function normalizeSchema(rawSchema, path) {
  if (!rawSchema || typeof rawSchema !== "object" || Array.isArray(rawSchema)) {
    throw Object.assign(new Error(`schema for '${path}' must be an object`), { code: "INVALID_REQUEST" });
  }
  const schema = { ...rawSchema };
  const hasDefault = Object.hasOwn(schema, "__default")
    || (Object.hasOwn(schema, "default") && typeof schema.default !== "function");
  const optional = schema.__optional === true || hasDefault;
  if (Object.hasOwn(schema, "__default")) schema.default = schema.__default;
  delete schema.__default;
  delete schema.__optional;
  for (const [name, value] of Object.entries(schema)) {
    if (typeof value === "function") delete schema[name];
  }

  if (schema.type === "object") {
    const rawProperties = schema.properties ?? {};
    if (!rawProperties || typeof rawProperties !== "object" || Array.isArray(rawProperties)) {
      throw Object.assign(new Error(`schema properties for '${path}' must be an object`), { code: "INVALID_REQUEST" });
    }
    const properties = Object.create(null);
    const required = new Set(
      Array.isArray(schema.required)
        ? schema.required.filter((name) => typeof name === "string" && Object.hasOwn(rawProperties, name))
        : [],
    );
    for (const [name, child] of Object.entries(rawProperties)) {
      const normalized = normalizeSchema(child, `${path}.${name}`);
      properties[name] = normalized.schema;
      if (normalized.optional) required.delete(name);
      else required.add(name);
    }
    schema.properties = properties;
    schema.required = [...required];
  } else if (schema.type === "array" && schema.items !== undefined) {
    schema.items = normalizeSchema(schema.items, `${path}[]`).schema;
  }
  return { schema, optional };
}

function schemaForArgs(args) {
  if (!args || typeof args !== "object" || Array.isArray(args)) {
    throw Object.assign(new Error("tool args must be an object"), { code: "INVALID_REQUEST" });
  }
  return normalizeSchema(
    { type: "object", properties: args, additionalProperties: false },
    "args",
  ).schema;
}

function validationError(value, schema, path) {
  if (!schema || typeof schema !== "object") return null;
  if (Array.isArray(schema.enum) && !schema.enum.some((item) => stringify(item) === stringify(value))) {
    return `${path} must be one of the declared enum values`;
  }
  switch (schema.type) {
    case "object": {
      if (!value || typeof value !== "object" || Array.isArray(value)) return `${path} must be an object`;
      const properties = schema.properties ?? {};
      for (const required of schema.required ?? []) {
        if (!Object.hasOwn(value, required)) return `${path}.${required} is required`;
      }
      if (schema.additionalProperties === false) {
        for (const key of Object.keys(value)) {
          if (!Object.hasOwn(properties, key)) return `${path}.${key} is not allowed`;
        }
      }
      for (const [key, child] of Object.entries(properties)) {
        if (Object.hasOwn(value, key)) {
          const error = validationError(value[key], child, `${path}.${key}`);
          if (error) return error;
        }
      }
      break;
    }
    case "array":
      if (!Array.isArray(value)) return `${path} must be an array`;
      for (let index = 0; index < value.length; index += 1) {
        const error = validationError(value[index], schema.items, `${path}[${index}]`);
        if (error) return error;
      }
      break;
    case "string": if (typeof value !== "string") return `${path} must be a string`; break;
    case "number": if (typeof value !== "number" || !Number.isFinite(value)) return `${path} must be a number`; break;
    case "integer": if (!Number.isInteger(value)) return `${path} must be an integer`; break;
    case "boolean": if (typeof value !== "boolean") return `${path} must be a boolean`; break;
    default: break;
  }
  if (typeof schema.minLength === "number" && value.length < schema.minLength) return `${path} is too short`;
  if (typeof schema.maxLength === "number" && value.length > schema.maxLength) return `${path} is too long`;
  if (typeof schema.minItems === "number" && value.length < schema.minItems) return `${path} has too few items`;
  if (typeof schema.maxItems === "number" && value.length > schema.maxItems) return `${path} has too many items`;
  if (typeof schema.minimum === "number" && value < schema.minimum) return `${path} is below minimum`;
  if (typeof schema.maximum === "number" && value > schema.maximum) return `${path} is above maximum`;
  return null;
}

function materializeDefaults(value, schema) {
  if (!schema || typeof schema !== "object") return value;
  if (schema.type === "object" && value && typeof value === "object" && !Array.isArray(value)) {
    const result = { ...value };
    for (const [key, child] of Object.entries(schema.properties ?? {})) {
      if (!Object.hasOwn(result, key) && Object.hasOwn(child, "default") && typeof child.default !== "function") {
        result[key] = structuredClone(child.default);
      }
      if (Object.hasOwn(result, key)) result[key] = materializeDefaults(result[key], child);
    }
    return result;
  }
  if (schema.type === "array" && Array.isArray(value)) {
    return value.map((item) => materializeDefaults(item, schema.items));
  }
  return value;
}

async function load(message) {
  if (targets.has(message.targetId)) targets.delete(message.targetId);
  const context = moduleContext();
  const module = new vm.SourceTextModule(message.moduleSource, {
    context,
    identifier: message.moduleUrl,
    initializeImportMeta(meta) { meta.url = message.moduleUrl; },
  });
  await module.link((specifier) => {
    throw Object.assign(new Error(`unsupported module import '${specifier}'`), { code: "INVALID_REQUEST" });
  });
  await module.evaluate();
  const tools = [];
  const exportsByName = new Map();
  for (const expected of message.expectedTools) {
    const definition = module.namespace[expected.exportName];
    if (!definition || typeof definition !== "object" || typeof definition.execute !== "function") {
      throw Object.assign(new Error(`export '${expected.exportName}' is not a tool definition`), { code: "INVALID_REQUEST" });
    }
    const inputSchema = schemaForArgs(definition.args ?? {});
    exportsByName.set(expected.exportName, { definition, inputSchema });
    tools.push({
      exportName: expected.exportName,
      name: expected.toolName,
      description: typeof definition.description === "string" ? definition.description : "",
      inputSchema,
    });
  }
  targets.set(message.targetId, { revision: message.revision, exportsByName });
  return { targetId: message.targetId, revision: message.revision, tools };
}

async function invoke(message) {
  const target = targets.get(message.targetId);
  if (!target || target.revision !== message.revision) {
    throw Object.assign(new Error("tool target is not loaded at the requested revision"), { code: "TARGET_NOT_FOUND" });
  }
  const loadedExport = target.exportsByName.get(message.exportName);
  if (!loadedExport) {
    throw Object.assign(new Error("tool export is not loaded"), { code: "TARGET_NOT_FOUND" });
  }
  if (operations.has(message.operationId)) {
    throw Object.assign(new Error("tool operation id is already active"), { code: "INVALID_REQUEST" });
  }
  const argumentsWithDefaults = materializeDefaults(message.arguments, loadedExport.inputSchema);
  const inputError = validationError(argumentsWithDefaults, loadedExport.inputSchema, "arguments");
  if (inputError) {
    throw Object.assign(new Error(inputError), { code: "INVALID_REQUEST" });
  }
  const controller = new AbortController();
  operations.set(message.operationId, { targetId: message.targetId, controller });
  try {
    const output = await loadedExport.definition.execute(argumentsWithDefaults, {
      directory: message.workspaceRoot ?? process.cwd(),
      worktree: message.worktreeRoot ?? message.workspaceRoot ?? process.cwd(),
      sessionID: message.sessionId,
      abort: controller.signal,
    });
    const text = typeof output === "string"
      ? output
      : output && typeof output.output === "string"
        ? output.output
        : null;
    if (text === null) {
      throw Object.assign(new Error("external tools must return a string or an object with a string output"), { code: "INVALID_REQUEST" });
    }
    if (Buffer.byteLength(text, "utf8") > maxOutputBytes) {
      throw Object.assign(new Error("tool output exceeds the worker response limit"), { code: "INVALID_REQUEST" });
    }
    return { output: text };
  } catch (error) {
    if (controller.signal.aborted) error.workerKind = "cancelled";
    throw error;
  } finally {
    operations.delete(message.operationId);
  }
}

async function cancel(message) {
  const operation = operations.get(message.operationId);
  if (operation?.targetId !== message.targetId) return {};
  operation.controller.abort();
  while (operations.has(message.operationId)) {
    await new Promise((resolve) => trustedSetTimeout(resolve, 10));
  }
  return {};
}

async function dispose(message) {
  for (const [operationId, operation] of operations) {
    if (operation.targetId === message.targetId) operation.controller.abort();
    operations.delete(operationId);
  }
  targets.delete(message.targetId);
  return {};
}

function errorKind(error, message) {
  if (error?.workerKind === "cancelled") return "cancelled";
  const operation = message?.operationId ? operations.get(message.operationId) : null;
  if (operation?.controller.signal.aborted) return "cancelled";
  if (error?.code === "TARGET_NOT_FOUND") return "not_found";
  if (error?.code === "INVALID_REQUEST") return "invalid_request";
  return "backend";
}

async function handle(message) {
  switch (message.type) {
    case "load": return load(message);
    case "invoke": return invoke(message);
    case "cancel": return cancel(message);
    case "dispose": return dispose(message);
    default: throw Object.assign(new Error("unknown worker request"), { code: "INVALID_REQUEST" });
  }
}

async function respond(message) {
  try {
    const result = await handle(message);
    write({ kind: "complete", token: responseToken, id: message.id, ok: true, result });
  } catch (error) {
    write({
      kind: "complete",
      token: responseToken,
      id: message.id,
      ok: false,
      errorKind: errorKind(error, message),
      error: error instanceof Error ? error.message.slice(0, 4096) : "script tool request failed",
    });
  }
  if (message.type === "dispose") reallyExit(0);
}

const input = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
input.on("line", (line) => {
  let message;
  try {
    message = parse(line);
  } catch {
    reallyExit(1);
    return;
  }
  void respond(message);
});
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerFrame {
    kind: String,
    token: String,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error_kind: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

struct PendingRequest {
    sender: oneshot::Sender<PortResult<Value>>,
}

async fn read_protocol_frame(
    reader: &mut BufReader<ChildStdout>,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if frame.is_empty() {
                return Ok(None);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "script tool worker ended inside a protocol frame",
            ));
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.unwrap_or(available.len());
        if frame.len().saturating_add(take) > MAX_PROTOCOL_FRAME_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "script tool worker protocol frame exceeded the size limit",
            ));
        }
        frame.extend_from_slice(&available[..take]);
        reader.consume(take + usize::from(newline.is_some()));
        if newline.is_some() {
            return Ok(Some(frame));
        }
    }
}

struct NodeWorker {
    stdin: Mutex<ChildStdin>,
    child: Mutex<ProcessTreeChild>,
    pending: Arc<Mutex<HashMap<u64, PendingRequest>>>,
    next_request_id: AtomicU64,
    response_token: String,
    invoke_gate: Arc<Semaphore>,
    exit_state: watch::Receiver<bool>,
}

impl NodeWorker {
    async fn spawn(executable: &PathBuf, working_directory: &str) -> PortResult<Arc<Self>> {
        let mut command = Command::new(executable);
        command
            .arg("--experimental-vm-modules")
            .arg("--input-type=module")
            .arg("--eval")
            .arg(WORKER_SOURCE)
            .current_dir(working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH");
        let mut child = ProcessTreeChild::spawn(&mut command)
            .await
            .map_err(|error| {
                PortError::new(
                    PortErrorKind::NotAvailable,
                    format!("failed to start JavaScript tool worker: {error}"),
                )
            })?;
        let stdin = child
            .take_stdin()
            .ok_or_else(|| PortError::new(PortErrorKind::Backend, "worker stdin is unavailable"))?;
        let stdout = child.take_stdout().ok_or_else(|| {
            PortError::new(PortErrorKind::Backend, "worker stdout is unavailable")
        })?;
        let stderr = child.take_stderr().ok_or_else(|| {
            PortError::new(PortErrorKind::Backend, "worker stderr is unavailable")
        })?;
        let mut reader = BufReader::new(stdout);
        let ready = tokio::time::timeout(REQUEST_TIMEOUT, read_protocol_frame(&mut reader))
            .await
            .map_err(|_| {
                PortError::new(
                    PortErrorKind::Timeout,
                    "script tool worker startup timed out",
                )
            })?
            .map_err(|error| PortError::new(PortErrorKind::Backend, error.to_string()))?
            .ok_or_else(|| {
                PortError::new(
                    PortErrorKind::NotAvailable,
                    "script tool worker exited during startup",
                )
            })?;
        let ready = serde_json::from_slice::<WorkerFrame>(&ready).map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("script tool worker returned an invalid startup frame: {error}"),
            )
        })?;
        if ready.kind != "ready" || ready.token.len() < 16 || ready.token.len() > 128 {
            return Err(PortError::new(
                PortErrorKind::Backend,
                "script tool worker returned an invalid startup token",
            ));
        }
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (exit_sender, exit_state) = watch::channel(false);
        let worker = Arc::new(Self {
            stdin: Mutex::new(stdin),
            child: Mutex::new(child),
            pending: pending.clone(),
            next_request_id: AtomicU64::new(1),
            response_token: ready.token,
            invoke_gate: Arc::new(Semaphore::new(1)),
            exit_state,
        });

        let weak_worker = Arc::downgrade(&worker);
        let response_token = worker.response_token.clone();
        tokio::spawn(async move {
            let mut rejected_frame_bytes = 0usize;
            loop {
                let frame = match read_protocol_frame(&mut reader).await {
                    Ok(Some(frame)) => frame,
                    Ok(None) => break,
                    Err(error) => {
                        log::warn!("Script tool worker protocol closed: {}", error);
                        if let Some(worker) = weak_worker.upgrade() {
                            let _ = worker.terminate().await;
                        }
                        break;
                    }
                };
                let response = match serde_json::from_slice::<WorkerFrame>(&frame) {
                    Ok(response) => response,
                    Err(_) => {
                        rejected_frame_bytes = rejected_frame_bytes.saturating_add(frame.len());
                        if rejected_frame_bytes <= 1024 * 1024 {
                            continue;
                        }
                        log::warn!("Script tool worker exceeded the rejected stdout budget");
                        if let Some(worker) = weak_worker.upgrade() {
                            let _ = worker.terminate().await;
                        }
                        break;
                    }
                };
                if response.kind != "complete" || response.token != response_token {
                    rejected_frame_bytes = rejected_frame_bytes.saturating_add(frame.len());
                    if rejected_frame_bytes <= 1024 * 1024 {
                        continue;
                    }
                    log::warn!("Script tool worker exceeded the unauthenticated stdout budget");
                    if let Some(worker) = weak_worker.upgrade() {
                        let _ = worker.terminate().await;
                    }
                    break;
                }
                let Some(id) = response.id else {
                    continue;
                };
                let Some(request) = pending.lock().await.remove(&id) else {
                    continue;
                };
                let result = if response.ok {
                    Ok(response.result)
                } else {
                    Err(PortError::new(
                        port_error_kind(response.error_kind.as_deref()),
                        response
                            .error
                            .unwrap_or_else(|| "script tool worker request failed".to_string()),
                    ))
                };
                let _ = request.sender.send(result);
            }
            for (_, request) in pending.lock().await.drain() {
                let _ = request.sender.send(Err(PortError::new(
                    PortErrorKind::NotAvailable,
                    "script tool worker exited",
                )));
            }
            let _ = exit_sender.send(true);
        });
        tokio::spawn(async move {
            let mut stderr = stderr;
            if let Ok(bytes) = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await {
                if bytes > 0 {
                    log::debug!("Script tool worker emitted {} stderr bytes", bytes);
                }
            }
        });
        Ok(worker)
    }

    async fn request(&self, request_type: &str, payload: Value) -> PortResult<Value> {
        self.request_with_timeout(request_type, payload, REQUEST_TIMEOUT)
            .await
    }

    async fn request_with_timeout(
        &self,
        request_type: &str,
        payload: Value,
        timeout: std::time::Duration,
    ) -> PortResult<Value> {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let mut message = match payload {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        message.insert("id".to_string(), Value::from(id));
        message.insert("type".to_string(), Value::from(request_type));
        let encoded = serde_json::to_vec(&Value::Object(message)).map_err(|error| {
            PortError::new(
                PortErrorKind::InvalidRequest,
                format!("failed to encode worker request: {error}"),
            )
        })?;
        if encoded.len() > MAX_PROTOCOL_FRAME_BYTES {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "script tool request exceeds the protocol limit",
            ));
        }
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .await
            .insert(id, PendingRequest { sender });
        let write_result = async {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(&encoded).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await
        }
        .await;
        if let Err(error) = write_result {
            self.pending.lock().await.remove(&id);
            return Err(PortError::new(
                PortErrorKind::NotAvailable,
                format!("failed to write worker request: {error}"),
            ));
        }
        match tokio::time::timeout(timeout, receiver).await {
            Ok(response) => response.map_err(|_| {
                PortError::new(PortErrorKind::NotAvailable, "worker response was dropped")
            })?,
            Err(_) => {
                self.pending.lock().await.remove(&id);
                let _ = self.terminate().await;
                Err(PortError::new(
                    PortErrorKind::Timeout,
                    "script tool worker timed out and was terminated",
                ))
            }
        }
    }

    async fn terminate(&self) -> PortResult<()> {
        let mut child = self.child.lock().await;
        if matches!(child.try_wait(), Ok(Some(_))) {
            return Ok(());
        }
        child
            .terminate(CANCEL_GRACE_PERIOD)
            .await
            .map(|_| ())
            .map_err(|error| {
                PortError::new(
                    PortErrorKind::Backend,
                    format!("failed to stop script tool worker: {error}"),
                )
            })
    }

    async fn is_running(&self) -> bool {
        matches!(self.child.lock().await.try_wait(), Ok(None))
    }

    async fn wait_for_exit(&self) {
        let mut state = self.exit_state.clone();
        while !*state.borrow() {
            if state.changed().await.is_err() {
                break;
            }
        }
    }

    async fn dispose(&self, target_id: &str) -> PortResult<()> {
        let _ = self
            .request_with_timeout(
                "dispose",
                serde_json::json!({ "targetId": target_id }),
                CANCEL_GRACE_PERIOD,
            )
            .await;
        self.terminate().await
    }
}

struct InvocationDropGuard {
    worker: Option<Arc<NodeWorker>>,
    permit: Option<OwnedSemaphorePermit>,
    target_id: String,
    operation_id: String,
}

impl InvocationDropGuard {
    fn new(
        worker: Arc<NodeWorker>,
        permit: OwnedSemaphorePermit,
        target_id: String,
        operation_id: String,
    ) -> Self {
        Self {
            worker: Some(worker),
            permit: Some(permit),
            target_id,
            operation_id,
        }
    }

    fn disarm(mut self) {
        self.worker.take();
        self.permit.take();
    }
}

impl Drop for InvocationDropGuard {
    fn drop(&mut self) {
        let Some(worker) = self.worker.take() else {
            return;
        };
        let permit = self.permit.take();
        let target_id = std::mem::take(&mut self.target_id);
        let operation_id = std::mem::take(&mut self.operation_id);
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            log::error!("Could not terminate a dropped script tool invocation outside Tokio");
            return;
        };
        runtime.spawn(async move {
            let cancelled = worker
                .request_with_timeout(
                    "cancel",
                    serde_json::json!({
                        "targetId": target_id,
                        "operationId": operation_id,
                    }),
                    CANCEL_GRACE_PERIOD,
                )
                .await;
            if cancelled.is_err() {
                let _ = worker.terminate().await;
            }
            drop(permit);
        });
    }
}

fn port_error_kind(kind: Option<&str>) -> PortErrorKind {
    match kind {
        Some("not_found") => PortErrorKind::NotFound,
        Some("invalid_request") => PortErrorKind::InvalidRequest,
        Some("cancelled") => PortErrorKind::Cancelled,
        Some("timeout") => PortErrorKind::Timeout,
        _ => PortErrorKind::Backend,
    }
}

pub struct NodeScriptToolRuntime {
    executable: RwLock<Option<PathBuf>>,
    workers: RwLock<HashMap<String, Arc<NodeWorker>>>,
    load_gate: Mutex<()>,
}

impl Default for NodeScriptToolRuntime {
    fn default() -> Self {
        Self::discover()
    }
}

impl NodeScriptToolRuntime {
    pub fn discover() -> Self {
        Self {
            executable: RwLock::new(which::which("node").ok()),
            workers: RwLock::new(HashMap::new()),
            load_gate: Mutex::new(()),
        }
    }

    async fn remember_discovered_executable(&self, discovered: Option<PathBuf>) -> Option<PathBuf> {
        if let Some(executable) = self.executable.read().await.clone() {
            return Some(executable);
        }
        let discovered = discovered?;
        let mut executable = self.executable.write().await;
        if executable.is_none() {
            *executable = Some(discovered);
        }
        executable.clone()
    }

    async fn resolve_executable(&self) -> Option<PathBuf> {
        if let Some(executable) = self.executable.read().await.clone() {
            return Some(executable);
        }
        self.remember_discovered_executable(which::which("node").ok())
            .await
    }

    async fn evict_worker(&self, target_id: &str, worker: &Arc<NodeWorker>) {
        let mut workers = self.workers.write().await;
        if workers
            .get(target_id)
            .is_some_and(|current| Arc::ptr_eq(current, worker))
        {
            workers.remove(target_id);
        }
        drop(workers);
        let _ = worker.terminate().await;
    }
}

#[async_trait]
impl ScriptToolRuntime for NodeScriptToolRuntime {
    async fn availability(&self) -> ScriptToolRuntimeAvailability {
        match self.resolve_executable().await {
            Some(executable) => match probe_node_version(&executable).await {
                Ok(version) => ScriptToolRuntimeAvailability::Available {
                    executable: executable.to_string_lossy().into_owned(),
                    version,
                },
                Err(reason) => ScriptToolRuntimeAvailability::Unavailable { reason },
            },
            None => ScriptToolRuntimeAvailability::Unavailable {
                reason: "BitFun could not find Node.js for external tools".to_string(),
            },
        }
    }

    async fn is_loaded(&self, target_id: &str) -> bool {
        let Some(worker) = self.workers.read().await.get(target_id).cloned() else {
            return false;
        };
        if worker.is_running().await {
            return true;
        }
        self.evict_worker(target_id, &worker).await;
        false
    }

    async fn wait_until_unloaded(&self, target_id: &str) -> PortResult<()> {
        let worker = self
            .workers
            .read()
            .await
            .get(target_id)
            .cloned()
            .ok_or_else(|| {
                PortError::new(PortErrorKind::NotFound, "script tool target is not loaded")
            })?;
        worker.wait_for_exit().await;
        let mut workers = self.workers.write().await;
        if !workers
            .get(target_id)
            .is_some_and(|current| Arc::ptr_eq(current, &worker))
        {
            return Err(PortError::new(
                PortErrorKind::NotFound,
                "script tool target generation was replaced or disposed",
            ));
        }
        workers.remove(target_id);
        Ok(())
    }

    async fn load(&self, request: ScriptToolLoadRequest) -> PortResult<ScriptToolLoadResponse> {
        let _guard = self.load_gate.lock().await;
        let executable = self.resolve_executable().await.ok_or_else(|| {
            PortError::new(PortErrorKind::NotAvailable, "Node.js is not available")
        })?;
        if request.target_id.is_empty()
            || request.revision.is_empty()
            || request.expected_tools.is_empty()
        {
            return Err(PortError::new(
                PortErrorKind::InvalidRequest,
                "script tool load request is incomplete",
            ));
        }
        if let Some(previous) = self.workers.write().await.remove(&request.target_id) {
            previous.dispose(&request.target_id).await?;
        }
        let worker = NodeWorker::spawn(&executable, &request.working_directory).await?;
        let payload = serde_json::to_value(&request).map_err(|error| {
            PortError::new(
                PortErrorKind::InvalidRequest,
                format!("failed to encode tool target: {error}"),
            )
        })?;
        let response = match worker.request("load", payload).await {
            Ok(response) => response,
            Err(error) => {
                let _ = worker.dispose(&request.target_id).await;
                return Err(error);
            }
        };
        let response =
            serde_json::from_value::<ScriptToolLoadResponse>(response).map_err(|error| {
                PortError::new(
                    PortErrorKind::Backend,
                    format!("invalid script tool load response: {error}"),
                )
            })?;
        self.workers.write().await.insert(request.target_id, worker);
        Ok(response)
    }

    async fn invoke(
        &self,
        request: ScriptToolInvokeRequest,
    ) -> PortResult<ScriptToolInvokeResponse> {
        let worker = self
            .workers
            .read()
            .await
            .get(&request.target_id)
            .cloned()
            .ok_or_else(|| {
                PortError::new(PortErrorKind::NotFound, "script tool target is not loaded")
            })?;
        let permit = Arc::clone(&worker.invoke_gate)
            .try_acquire_owned()
            .map_err(|_| {
                PortError::new(
                    PortErrorKind::NotAvailable,
                    "script tool target is already running another invocation",
                )
            })?;
        let target_id = request.target_id.clone();
        let operation_id = request.operation_id.clone();
        let drop_guard =
            InvocationDropGuard::new(Arc::clone(&worker), permit, target_id.clone(), operation_id);
        let payload = serde_json::to_value(request).map_err(|error| {
            PortError::new(
                PortErrorKind::InvalidRequest,
                format!("failed to encode tool invocation: {error}"),
            )
        })?;
        let response = worker.request("invoke", payload).await;
        drop_guard.disarm();
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                if matches!(
                    error.kind,
                    PortErrorKind::Timeout | PortErrorKind::NotAvailable | PortErrorKind::NotFound
                ) {
                    self.evict_worker(&target_id, &worker).await;
                }
                return Err(error);
            }
        };
        serde_json::from_value(response).map_err(|error| {
            PortError::new(
                PortErrorKind::Backend,
                format!("invalid script tool invocation response: {error}"),
            )
        })
    }

    async fn cancel(&self, target_id: &str, operation_id: &str) -> PortResult<()> {
        let worker = self
            .workers
            .read()
            .await
            .get(target_id)
            .cloned()
            .ok_or_else(|| {
                PortError::new(PortErrorKind::NotFound, "script tool target is not loaded")
            })?;
        let result = worker
            .request_with_timeout(
                "cancel",
                serde_json::json!({
                    "targetId": target_id,
                    "operationId": operation_id,
                }),
                CANCEL_GRACE_PERIOD,
            )
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(error)
                if matches!(
                    error.kind,
                    PortErrorKind::Timeout | PortErrorKind::NotAvailable
                ) =>
            {
                self.evict_worker(target_id, &worker).await;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    async fn dispose(&self, target_id: &str) -> PortResult<()> {
        let Some(worker) = self.workers.write().await.remove(target_id) else {
            return Ok(());
        };
        worker.dispose(target_id).await
    }
}

async fn probe_node_version(executable: &PathBuf) -> Result<String, String> {
    let mut command = Command::new(executable);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH");
    let mut child = ProcessTreeChild::spawn(&mut command)
        .await
        .map_err(|_| "BitFun could not start Node.js for external tools".to_string())?;
    let stdout = child
        .take_stdout()
        .ok_or_else(|| "Node.js version output was unavailable".to_string())?;
    let probe = async move {
        let mut output = Vec::with_capacity(65);
        stdout
            .take(65)
            .read_to_end(&mut output)
            .await
            .map_err(|_| "Node.js version output could not be read".to_string())?;
        let status = child
            .wait()
            .await
            .map_err(|_| "Node.js version probe failed".to_string())?;
        Ok::<_, String>((status, output))
    };
    let (status, output) = tokio::time::timeout(std::time::Duration::from_secs(2), probe)
        .await
        .map_err(|_| "Node.js version probe timed out".to_string())??;
    if !status.success() {
        return Err("Node.js version probe failed".to_string());
    }
    let version = String::from_utf8(output)
        .map_err(|_| "Node.js returned an invalid version".to_string())?
        .trim()
        .to_string();
    if version.is_empty() || version.len() > 64 || !version.starts_with('v') {
        return Err("Node.js returned an invalid version".to_string());
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::NodeScriptToolRuntime;
    use std::path::PathBuf;
    use tokio::sync::{Mutex, RwLock};

    #[tokio::test]
    async fn missing_node_path_can_recover_without_replacing_the_runtime() {
        let runtime = NodeScriptToolRuntime {
            executable: RwLock::new(None),
            workers: RwLock::new(Default::default()),
            load_gate: Mutex::new(()),
        };
        let discovered = PathBuf::from("C:/Program Files/nodejs/node.exe");

        assert_eq!(
            runtime
                .remember_discovered_executable(Some(discovered.clone()))
                .await,
            Some(discovered.clone())
        );
        assert_eq!(*runtime.executable.read().await, Some(discovered));
    }
}
