//! LSP server process management
//!
//! Manages the lifecycle of a single LSP server process.

use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::{timeout, Duration};

use super::protocol::{
    create_notification, create_request, extract_result, read_message, write_message,
};
use super::types::{
    InitializeParams, InitializeResult, JsonRpcMessage, JsonRpcResponse, RuntimeType, ServerConfig,
};

/// Process crash callback type.
pub type CrashCallback = Arc<dyn Fn(String) + Send + Sync>;

/// Progress notification callback type.
/// Parameters: `(kind: "begin" | "report" | "end", token: String, percentage: Option<u32>, message: String)`.
pub type ProgressCallback = Arc<dyn Fn(String, String, Option<u32>, String) + Send + Sync>;

/// Token creation callback type.
/// Parameters: `(token: String)`.
pub type TokenCreateCallback = Arc<dyn Fn(String) + Send + Sync>;

/// Diagnostics callback type.
/// Parameters: `(uri: String, diagnostics: Vec<serde_json::Value>)`.
pub type DiagnosticsCallback = Arc<dyn Fn(String, Vec<serde_json::Value>) + Send + Sync>;

/// LSP server process.
pub struct LspServerProcess {
    /// Plugin ID.
    pub id: String,
    /// Child process.
    child: Arc<RwLock<Child>>,
    /// Standard input.
    stdin: Arc<RwLock<ChildStdin>>,
    /// Request ID counter.
    request_id: Arc<AtomicU64>,
    /// Pending requests waiting for a response.
    pending_requests: Arc<RwLock<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    /// Notification sender.
    notification_tx: mpsc::UnboundedSender<JsonRpcMessage>,
    /// Server capabilities.
    capabilities: Arc<RwLock<Option<serde_json::Value>>>,
    /// Crash callback.
    crash_callback: Option<CrashCallback>,
    /// Progress callback.
    progress_callback: Option<ProgressCallback>,
    /// Token creation callback.
    token_create_callback: Option<TokenCreateCallback>,
    /// Diagnostics callback.
    diagnostics_callback: Option<DiagnosticsCallback>,
}

impl LspServerProcess {
    /// Spawns a new LSP server process.
    pub async fn spawn(
        id: String,
        server_bin: PathBuf,
        config: &ServerConfig,
        crash_callback: Option<CrashCallback>,
        progress_callback: Option<ProgressCallback>,
        token_create_callback: Option<TokenCreateCallback>,
        diagnostics_callback: Option<DiagnosticsCallback>,
    ) -> Result<Self> {
        info!("Spawning LSP server: {} at {:?}", id, server_bin);
        debug!(
            "LSP config - args: {:?}, env: {:?}",
            config.args, config.env
        );

        if !server_bin.exists() {
            error!("LSP server binary not found: {:?}", server_bin);
            return Err(anyhow!("LSP server binary not found: {:?}", server_bin));
        }

        let runtime_type = Self::detect_runtime_type(config, &server_bin);
        debug!("Detected runtime type: {:?}", runtime_type);

        let mut cmd = Self::build_command(&runtime_type, &server_bin, config)?;

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            error!("Failed to spawn LSP server {}: {}", id, e);
            anyhow!("Failed to spawn LSP server {}: {}", id, e)
        })?;

        if let Some(pid) = child.id() {
            debug!("LSP server process started with PID: {}", pid);
        }

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdout"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stderr"))?;

        let (notification_tx, notification_rx) = mpsc::unbounded_channel();

        let process = Self {
            id: id.clone(),
            child: Arc::new(RwLock::new(child)),
            stdin: Arc::new(RwLock::new(stdin)),
            request_id: Arc::new(AtomicU64::new(1)),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            notification_tx,
            capabilities: Arc::new(RwLock::new(None)),
            crash_callback,
            progress_callback,
            token_create_callback,
            diagnostics_callback,
        };

        process.start_read_task(stdout).await;

        process.start_stderr_task(stderr).await;

        process.start_notification_task(notification_rx).await;

        info!("LSP server process spawned: {}", id);

        Ok(process)
    }

    /// Starts the message reader task.
    async fn start_read_task(&self, stdout: ChildStdout) {
        let pending_requests = self.pending_requests.clone();
        let notification_tx = self.notification_tx.clone();
        let id = self.id.clone();
        let crash_callback = self.crash_callback.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut consecutive_timeouts = 0;
            const MAX_CONSECUTIVE_TIMEOUTS: u32 = 3;

            loop {
                match timeout(Duration::from_secs(30), read_message(&mut reader)).await {
                    Ok(Ok(message)) => {
                        consecutive_timeouts = 0;

                        match &message {
                            JsonRpcMessage::Response(response) => {
                                let request_id = response.id;
                                let mut pending = pending_requests.write().await;

                                if let Some(sender) = pending.remove(&request_id) {
                                    let _ = sender.send(response.clone());
                                } else {
                                    warn!(
                                        "[{}] Received response for unknown request ID: {}",
                                        id, request_id
                                    );
                                }
                            }
                            JsonRpcMessage::Notification(_) => {
                                if let Err(e) = notification_tx.send(message) {
                                    error!("[{}] Failed to send notification: {}", id, e);
                                    break;
                                }
                            }
                            JsonRpcMessage::Request(_req) => {
                                if let Err(e) = notification_tx.send(message) {
                                    error!("[{}] Failed to send request: {}", id, e);
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        error!("[{}] Failed to read message: {}", id, e);
                        error!("[{}] This usually means the LSP server is outputting non-protocol data to stdout", id);
                        break;
                    }
                    Err(_) => {
                        consecutive_timeouts += 1;

                        if consecutive_timeouts >= MAX_CONSECUTIVE_TIMEOUTS {
                            warn!(
                                "[{}] No LSP messages for {}s (this is normal if idle)",
                                id,
                                30 * MAX_CONSECUTIVE_TIMEOUTS
                            );

                            consecutive_timeouts = 0;
                        }
                    }
                }
            }

            error!("LSP server read task ended abnormally: {}", id);

            {
                let mut pending = pending_requests.write().await;
                let count = pending.len();
                if count > 0 {
                    warn!("Dropping {} pending request(s) for server {}", count, id);
                }
                pending.clear();
            }

            if let Some(callback) = crash_callback {
                error!("Invoking crash callback - server connection lost: {}", id);
                callback(id.clone());
            }
        });
    }

    /// Starts the stderr reader task.
    ///
    /// This task continuously reads the LSP server's stderr output to prevent the pipe buffer from
    /// filling up and blocking the process.
    /// The LSP protocol specifies using stdout for protocol communication; stderr is used for the
    /// server's diagnostic logs.
    async fn start_stderr_task(&self, stderr: ChildStderr) {
        let id = self.id.clone();

        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            let mut line_count = 0;
            let mut error_count = 0;
            let mut warn_count = 0;

            let mut missing_cmake = false;
            let mut missing_spectre = false;
            let mut build_script_errors = std::collections::HashSet::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            line_count += 1;

                            let lower = trimmed.to_lowercase();

                            if lower.contains("missing dependency: cmake")
                                || (lower.contains("failed to spawn") && lower.contains("cmake"))
                            {
                                if !missing_cmake {
                                    missing_cmake = true;
                                    warn!("[{}] Missing build dependency: CMake not installed or not in PATH", id);
                                    info!("[{}] Tip: Some Rust crates require CMake to compile C/C++ code. Download: https://cmake.org/download/", id);
                                }
                                continue;
                            }

                            if lower.contains("no spectre-mitigated libs") {
                                if !missing_spectre {
                                    missing_spectre = true;
                                    warn!("[{}] Missing build dependency: MSVC Spectre mitigation libraries not installed", id);
                                    info!("[{}] Tip: Some Rust crates require MSVC Spectre libraries. Install via Visual Studio Installer", id);
                                }
                                continue;
                            }

                            if lower.contains("failed to run custom build command") {
                                if let Some(start) = trimmed.find("for `") {
                                    if let Some(end) = trimmed[start + 5..].find('`') {
                                        let package = &trimmed[start + 5..start + 5 + end];
                                        if build_script_errors.insert(package.to_string()) {
                                            warn!("[{}] Build script failed for package: {} (LSP may still work but code analysis accuracy may be affected)", id, package);
                                        }
                                    }
                                }
                                continue;
                            }

                            if lower.contains("compiling")
                                || lower.contains("building")
                                || lower.contains("cargo:rerun-if")
                            {
                                continue;
                            }

                            if lower.contains("panic") {
                                error_count += 1;
                                if error_count <= 3 {
                                    debug!("[{}] Build script panic: {}", id, trimmed);
                                }
                                continue;
                            }

                            if lower.contains("error") || lower.contains("fatal") {
                                error_count += 1;

                                if error_count <= 5 {
                                    error!("[{}] stderr: {}", id, trimmed);
                                } else if error_count % 10 == 0 {
                                    error!("[{}] stderr: ... (omitted {} errors)", id, error_count);
                                }
                            } else if lower.contains("warn") || lower.contains("warning") {
                                warn_count += 1;

                                if warn_count <= 10 {
                                    warn!("[{}] stderr: {}", id, trimmed);
                                } else if warn_count % 100 == 0 {
                                    warn!("[{}] stderr: ... (omitted {} warnings)", id, warn_count);
                                }
                            } else {
                                if line_count <= 5 || line_count % 1000 == 0 {
                                    debug!("[{}] stderr: {}", id, trimmed);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to read stderr from {}: {}", id, e);
                        break;
                    }
                }
            }

            if line_count > 0 || error_count > 0 || warn_count > 0 {
                info!(
                    "LSP server stderr task ended: {} (read {} lines, {} errors, {} warnings)",
                    id, line_count, error_count, warn_count
                );

                if !build_script_errors.is_empty() {
                    warn!("[{}] {} package(s) had build script failures, but LSP service is still running", id, build_script_errors.len());
                }

                if missing_cmake || missing_spectre {
                    info!("[{}] Tip: Installing missing dependencies may improve code analysis accuracy", id);
                }
            }
        });
    }

    /// Starts the notification handler task.
    async fn start_notification_task(
        &self,
        mut notification_rx: mpsc::UnboundedReceiver<JsonRpcMessage>,
    ) {
        let id = self.id.clone();
        let progress_callback = self.progress_callback.clone();
        let token_create_callback = self.token_create_callback.clone();
        let diagnostics_callback = self.diagnostics_callback.clone();
        let stdin = self.stdin.clone();

        tokio::spawn(async move {
            while let Some(message) = notification_rx.recv().await {
                match message {
                    JsonRpcMessage::Notification(notif) => match notif.method.as_str() {
                        "$/progress" => {
                            if let Some(params) = &notif.params {
                                let token = params
                                    .get("token")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();

                                if let Some(value) = params.get("value") {
                                    if let Some(kind) = value.get("kind").and_then(|k| k.as_str()) {
                                        match kind {
                                            "begin" => {
                                                let title = value
                                                    .get("title")
                                                    .and_then(|t| t.as_str())
                                                    .unwrap_or("");
                                                info!("[{}] Indexing started: {}", id, title);

                                                if let Some(ref callback) = progress_callback {
                                                    callback(
                                                        "begin".to_string(),
                                                        token.clone(),
                                                        Some(0),
                                                        title.to_string(),
                                                    );
                                                }
                                            }
                                            "report" => {
                                                let percentage = value
                                                    .get("percentage")
                                                    .and_then(|p| p.as_u64());
                                                let message = value
                                                    .get("message")
                                                    .and_then(|m| m.as_str())
                                                    .unwrap_or("");

                                                if let Some(ref callback) = progress_callback {
                                                    callback(
                                                        "report".to_string(),
                                                        token.clone(),
                                                        percentage.map(|p| p as u32),
                                                        message.to_string(),
                                                    );
                                                }
                                            }
                                            "end" => {
                                                let message = value
                                                    .get("message")
                                                    .and_then(|m| m.as_str())
                                                    .unwrap_or("");
                                                info!("[{}] Indexing completed: {}", id, message);

                                                if let Some(ref callback) = progress_callback {
                                                    callback(
                                                        "end".to_string(),
                                                        token.clone(),
                                                        Some(100),
                                                        message.to_string(),
                                                    );
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                        "textDocument/publishDiagnostics" => {
                            if let Some(params) = &notif.params {
                                if let Some(uri) = params.get("uri").and_then(|u| u.as_str()) {
                                    if let Some(diagnostics_arr) =
                                        params.get("diagnostics").and_then(|d| d.as_array())
                                    {
                                        let diags: Vec<serde_json::Value> = diagnostics_arr.clone();

                                        debug!(
                                            "[{}] Diagnostics: {} items for {}",
                                            id,
                                            diags.len(),
                                            uri
                                        );

                                        if let Some(callback) = &diagnostics_callback {
                                            callback(uri.to_string(), diags);
                                        }
                                    }
                                }
                            }
                        }
                        "window/logMessage" => {
                            if let Some(params) = &notif.params {
                                let msg_type =
                                    params.get("type").and_then(|t| t.as_u64()).unwrap_or(3);
                                if let Some(msg) = params.get("message").and_then(|m| m.as_str()) {
                                    match msg_type {
                                        1 => error!("[{}] Server log: {}", id, msg),
                                        2 => warn!("[{}] Server log: {}", id, msg),
                                        3 => info!("[{}] Server log: {}", id, msg),
                                        4 => debug!("[{}] Server log: {}", id, msg),
                                        _ => debug!("[{}] Server log: {}", id, msg),
                                    }
                                }
                            }
                        }
                        "window/showMessage" => {
                            if let Some(params) = &notif.params {
                                let msg_type =
                                    params.get("type").and_then(|t| t.as_u64()).unwrap_or(3);
                                if let Some(msg) = params.get("message").and_then(|m| m.as_str()) {
                                    match msg_type {
                                        1 => error!("[{}] Server message: {}", id, msg),
                                        2 => warn!("[{}] Server message: {}", id, msg),
                                        3 => info!("[{}] Server message: {}", id, msg),
                                        4 => debug!("[{}] Server message: {}", id, msg),
                                        _ => info!("[{}] Server message: {}", id, msg),
                                    }
                                }
                            }
                        }
                        _ => {}
                    },

                    JsonRpcMessage::Request(req) => match req.method.as_str() {
                        "window/workDoneProgress/create" => {
                            if let Some(params) = &req.params {
                                if let Some(token) = params.get("token") {
                                    let token_str = token.as_str().unwrap_or("unknown").to_string();

                                    if let Some(ref callback) = token_create_callback {
                                        callback(token_str);
                                    }
                                }
                            }

                            let response = super::types::JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id,
                                result: Some(serde_json::Value::Null),
                                error: None,
                            };

                            let response_message = super::types::JsonRpcMessage::Response(response);
                            let mut stdin_lock = stdin.write().await;
                            if let Err(e) =
                                super::protocol::write_message(&mut stdin_lock, &response_message)
                                    .await
                            {
                                error!(
                                    "[{}] Failed to send workDoneProgress/create response: {}",
                                    id, e
                                );
                            }
                        }
                        "client/registerCapability" => {
                            let response = super::types::JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id,
                                result: Some(serde_json::Value::Null),
                                error: None,
                            };

                            let response_message = super::types::JsonRpcMessage::Response(response);
                            let mut stdin_lock = stdin.write().await;
                            if let Err(e) =
                                super::protocol::write_message(&mut stdin_lock, &response_message)
                                    .await
                            {
                                error!(
                                    "[{}] Failed to send registerCapability response: {}",
                                    id, e
                                );
                            }
                        }
                        "workspace/configuration" => {
                            let response = super::types::JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id,
                                result: Some(serde_json::json!([])),
                                error: None,
                            };

                            let response_message = super::types::JsonRpcMessage::Response(response);
                            let mut stdin_lock = stdin.write().await;
                            if let Err(e) =
                                super::protocol::write_message(&mut stdin_lock, &response_message)
                                    .await
                            {
                                error!("[{}] Failed to send configuration response: {}", id, e);
                            }
                        }
                        _ => {
                            warn!("[{}] Unhandled server request: {}", id, req.method);

                            let response = super::types::JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: req.id,
                                result: None,
                                error: Some(super::types::JsonRpcError {
                                    code: -32601,
                                    message: format!("Method not supported: {}", req.method),
                                    data: None,
                                }),
                            };

                            let response_message = super::types::JsonRpcMessage::Response(response);
                            let mut stdin_lock = stdin.write().await;
                            if let Err(e) =
                                super::protocol::write_message(&mut stdin_lock, &response_message)
                                    .await
                            {
                                error!("[{}] Failed to send error response: {}", id, e);
                            }
                        }
                    },
                    _ => {}
                }
            }

            info!("LSP notification task ended: {}", id);
        });
    }

    /// Sends a request and waits for the response.
    pub async fn send_request(
        &self,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let method_str = method.into();

        let message = create_request(id, method_str.clone(), params);

        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(id, tx);
        }

        {
            let mut stdin = self.stdin.write().await;
            write_message(&mut stdin, &message).await?;
        }

        let response = timeout(Duration::from_secs(60), rx).await.map_err(|_| {
            error!("LSP request timeout after 60s: {}", method_str);
            anyhow!(
                "LSP request timeout (60s): {}. The LSP server may not be responding.",
                method_str
            )
        })??;

        extract_result(response)
    }

    /// Sends a notification (does not wait for a response).
    pub async fn send_notification(
        &self,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Result<()> {
        let method_str = method.into();
        let message = create_notification(method_str, params);

        let mut stdin = self.stdin.write().await;
        write_message(&mut stdin, &message).await?;

        Ok(())
    }

    /// Initializes the server.
    pub async fn initialize(&self, workspace_root: Option<String>) -> Result<InitializeResult> {
        info!("Initializing LSP server: {}", self.id);

        let root_uri = workspace_root.as_ref().map(|path| {
            if cfg!(windows) {
                format!("file:///{}", path.replace('\\', "/"))
            } else {
                format!("file://{}", path)
            }
        });

        let workspace_folders = workspace_root.as_ref().map(|root| {
            let uri = if cfg!(windows) {
                format!("file:///{}", root.replace('\\', "/"))
            } else {
                format!("file://{}", root)
            };

            let name = std::path::Path::new(root)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string();

            vec![super::types::WorkspaceFolder { uri, name }]
        });

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_path: None,
            root_uri: root_uri.clone(),
            capabilities: super::types::ClientCapabilities {
                window: Some(serde_json::json!({
                    "workDoneProgress": true,
                    "showMessage": {
                        "messageActionItem": {
                            "additionalPropertiesSupport": false
                        }
                    },
                    "showDocument": {
                        "support": true
                    }
                })),

                workspace: Some(serde_json::json!({
                    "applyEdit": true,
                    "workspaceEdit": {
                        "documentChanges": true,
                        "resourceOperations": ["create", "rename", "delete"]
                    },
                    "didChangeConfiguration": {
                        "dynamicRegistration": false
                    },
                    "didChangeWatchedFiles": {
                        "dynamicRegistration": false
                    },
                    "symbol": {
                        "dynamicRegistration": false
                    },
                    "executeCommand": {
                        "dynamicRegistration": false
                    },
                    "workspaceFolders": true,
                    "configuration": true
                })),
                text_document: Some(serde_json::json!({
                    "synchronization": {
                        "dynamicRegistration": false,
                        "didSave": true,
                        "willSave": false,
                        "willSaveWaitUntil": false
                    },
                    "completion": {
                        "dynamicRegistration": false,
                        "completionItem": {
                            "snippetSupport": true,
                            "commitCharactersSupport": false,
                            "documentationFormat": ["plaintext", "markdown"],
                            "deprecatedSupport": false,
                            "preselectSupport": false
                        },
                        "contextSupport": false
                    },
                    "hover": {
                        "dynamicRegistration": false,
                        "contentFormat": ["plaintext", "markdown"]
                    },
                    "signatureHelp": {
                        "dynamicRegistration": false,
                        "signatureInformation": {
                            "documentationFormat": ["plaintext", "markdown"]
                        }
                    },
                    "definition": {
                        "dynamicRegistration": false,
                        "linkSupport": true
                    },
                    "references": {
                        "dynamicRegistration": false
                    },
                    "documentHighlight": {
                        "dynamicRegistration": false
                    },
                    "documentSymbol": {
                        "dynamicRegistration": false,
                        "hierarchicalDocumentSymbolSupport": true
                    },
                    "codeAction": {
                        "dynamicRegistration": false,
                        "codeActionLiteralSupport": {
                            "codeActionKind": {
                                "valueSet": ["quickfix", "refactor", "refactor.extract", "refactor.inline", "refactor.rewrite", "source", "source.organizeImports"]
                            }
                        }
                    },
                    "formatting": {
                        "dynamicRegistration": false
                    },
                    "rangeFormatting": {
                        "dynamicRegistration": false
                    },
                    "rename": {
                        "dynamicRegistration": false,
                        "prepareSupport": false
                    },
                    "publishDiagnostics": {
                        "relatedInformation": true,
                        "tagSupport": {
                            "valueSet": [1, 2]
                        }
                    },
                    "inlayHint": {
                        "dynamicRegistration": false,
                        "resolveSupport": {
                            "properties": ["tooltip", "textEdits", "label.tooltip", "label.location", "label.command"]
                        }
                    }
                })),
                experimental: None,
            },

            initialization_options: Some(serde_json::json!({

                "checkOnSave": {
                    "command": "clippy"
                },
                "cargo": {
                    "allFeatures": true
                },

            })),

            workspace_folders,
        };

        let result = self
            .send_request("initialize", Some(serde_json::to_value(params)?))
            .await?;

        let init_result: InitializeResult = serde_json::from_value(result)?;

        {
            let mut caps = self.capabilities.write().await;
            *caps = Some(serde_json::to_value(&init_result.capabilities)?);
        }

        self.send_notification("initialized", Some(serde_json::json!({})))
            .await?;

        info!("LSP server initialized: {}", self.id);

        Ok(init_result)
    }

    /// Shuts down the server.
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down LSP server: {}", self.id);

        let _ = self.send_request("shutdown", None).await;

        let _ = self.send_notification("exit", None).await;

        tokio::time::sleep(Duration::from_millis(500)).await;

        let mut child = self.child.write().await;
        let _ = child.kill().await;

        info!("LSP server shut down: {}", self.id);

        Ok(())
    }

    /// Returns server capabilities.
    pub async fn get_capabilities(&self) -> Option<serde_json::Value> {
        let caps = self.capabilities.read().await;
        caps.clone()
    }

    /// Returns whether the process is still alive.
    pub async fn is_alive(&self) -> bool {
        let mut child = self.child.write().await;
        match child.try_wait() {
            Ok(Some(status)) => {
                warn!("[{}] Process has exited with status: {:?}", self.id, status);
                false
            }
            Ok(None) => true,
            Err(e) => {
                error!("[{}] Failed to check process status: {}", self.id, e);
                false
            }
        }
    }

    /// Detects the runtime type.
    fn detect_runtime_type(config: &ServerConfig, server_bin: &Path) -> RuntimeType {
        if let Some(runtime) = &config.runtime {
            debug!("Runtime explicitly specified: {}", runtime);
            return match runtime.to_lowercase().as_str() {
                "bash" | "sh" => RuntimeType::Bash,
                "node" | "nodejs" => RuntimeType::Node,
                "exe" | "executable" => RuntimeType::Executable,
                _ => {
                    warn!(
                        "Unknown runtime type '{}', defaulting to executable",
                        runtime
                    );
                    RuntimeType::Executable
                }
            };
        }

        if let Some(ext) = server_bin.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "sh" | "bash" => return RuntimeType::Bash,
                "js" | "mjs" | "cjs" => return RuntimeType::Node,
                _ => {}
            }
        }

        RuntimeType::Executable
    }

    /// Builds the command based on the runtime type.
    fn build_command(
        runtime_type: &RuntimeType,
        server_bin: &PathBuf,
        config: &ServerConfig,
    ) -> Result<tokio::process::Command> {
        match runtime_type {
            RuntimeType::Executable => {
                #[cfg(windows)]
                {
                    if let Some(ext) = server_bin.extension().and_then(|e| e.to_str()) {
                        let ext_lower = ext.to_lowercase();

                        if ext_lower == "bat" || ext_lower == "cmd" {
                            debug!(
                                "Detected batch file (.{}), extracting node command",
                                ext_lower
                            );

                            if let Ok(content) = std::fs::read_to_string(server_bin) {
                                let mut script_path: Option<PathBuf> = None;

                                for line in content.lines() {
                                    let line = line.trim();

                                    if line.starts_with("node ") || line.starts_with("node.exe ") {
                                        info!("Found node execution command: {}", line);

                                        if let Some(start_quote) = line.find('"') {
                                            if let Some(end_quote) =
                                                line[start_quote + 1..].find('"')
                                            {
                                                let path_expr = &line
                                                    [start_quote + 1..start_quote + 1 + end_quote];
                                                debug!("Extracted path expression: {}", path_expr);

                                                for prev_line in content.lines() {
                                                    let prev_line = prev_line.trim();
                                                    if prev_line.starts_with("set ")
                                                        && prev_line.contains(
                                                            path_expr
                                                                .trim_matches('%')
                                                                .split('%')
                                                                .next()
                                                                .unwrap_or(""),
                                                        )
                                                    {
                                                        if let Some(eq_pos) = prev_line.find('=') {
                                                            let value_part = &prev_line
                                                                [eq_pos + 1..]
                                                                .trim_matches('"');

                                                            if let Some(parent) =
                                                                server_bin.parent()
                                                            {
                                                                let mut resolved_path =
                                                                    parent.to_path_buf();

                                                                let rel_part = value_part
                                                                    .replace("%SCRIPT_DIR%", "");

                                                                for component in
                                                                    rel_part.split(['\\', '/'])
                                                                {
                                                                    match component {
                                                                        "" | "." => continue,
                                                                        ".." => {
                                                                            resolved_path.pop();
                                                                        }
                                                                        part => {
                                                                            resolved_path.push(part)
                                                                        }
                                                                    }
                                                                }

                                                                if resolved_path.exists() {
                                                                    script_path =
                                                                        Some(resolved_path);
                                                                    break;
                                                                } else {
                                                                    warn!("Resolved path does not exist: {:?}", resolved_path);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        break;
                                    }
                                }

                                if let Some(js_path) = script_path {
                                    let node_cmd = if cfg!(windows) { "node.exe" } else { "node" };

                                    let mut cmd =
                                        crate::util::process_manager::create_tokio_command(
                                            node_cmd,
                                        );
                                    cmd.arg(js_path);
                                    cmd.args(&config.args);
                                    cmd.envs(&config.env);
                                    return Ok(cmd);
                                }
                            }

                            error!("Failed to extract node command from bat file");
                            error!("Bat files cannot be executed directly without cmd wrapper");
                            return Err(anyhow!(
                                "Failed to parse batch file. Please check the plugin installation."
                            ));
                        }
                    }
                }

                let mut cmd = crate::util::process_manager::create_tokio_command(server_bin);
                cmd.args(&config.args);
                cmd.envs(&config.env);
                Ok(cmd)
            }
            RuntimeType::Bash => {
                #[cfg(windows)]
                {
                    let bash_paths = vec![
                        "bash.exe",
                        "C:\\Program Files\\Git\\bin\\bash.exe",
                        "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
                        "wsl.exe",
                    ];

                    let mut bash_exe = None;
                    for path in &bash_paths {
                        if crate::util::process_manager::create_command(path)
                            .arg("--version")
                            .output()
                            .is_ok()
                        {
                            bash_exe = Some(path.to_string());
                            break;
                        }
                    }

                    let bash_cmd = bash_exe.ok_or_else(|| {
                        error!(
                            "Bash not found on Windows. Searched paths: {:?}",
                            bash_paths
                        );
                        anyhow!(
                            "Bash not found on Windows. Please install Git Bash or WSL.\n\
                             - Git Bash: https://git-scm.com/download/win\n\
                             - WSL: https://docs.microsoft.com/windows/wsl/install"
                        )
                    })?;

                    let mut cmd = crate::util::process_manager::create_tokio_command(&bash_cmd);
                    cmd.arg(server_bin);
                    cmd.args(&config.args);
                    cmd.envs(&config.env);
                    Ok(cmd)
                }

                #[cfg(not(windows))]
                {
                    let mut cmd = crate::util::process_manager::create_tokio_command("bash");
                    cmd.arg(server_bin);
                    cmd.args(&config.args);
                    cmd.envs(&config.env);
                    Ok(cmd)
                }
            }
            RuntimeType::Node => {
                let node_cmd = if cfg!(windows) { "node.exe" } else { "node" };

                match crate::util::process_manager::create_command(node_cmd)
                    .arg("--version")
                    .output()
                {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Node.js not found: {}", e);
                        return Err(anyhow!(
                            "Node.js not found. Please install Node.js from https://nodejs.org/\n\
                             The LSP plugin requires Node.js to be installed and available in PATH."
                        ));
                    }
                }

                let mut cmd = crate::util::process_manager::create_tokio_command(node_cmd);
                cmd.arg(server_bin);
                cmd.args(&config.args);
                cmd.envs(&config.env);
                Ok(cmd)
            }
        }
    }
}

impl Drop for LspServerProcess {
    fn drop(&mut self) {
        debug!("Dropping LSP server process: {}", self.id);
    }
}
