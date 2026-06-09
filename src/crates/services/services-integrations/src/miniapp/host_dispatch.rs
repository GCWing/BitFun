//! Host-side dispatch for MiniApp framework primitives (`shell.exec`, `fs.*`, `os.info`,
//! `net.fetch`).
//!
//! Why this exists
//! ---------------
//! The original MiniApp design routed every `app.*` call through a Bun/Node Worker
//! (`resources/worker_host.js`). That gives apps a real V8 sandbox for arbitrary
//! `worker.js` code, but it forces every app — even ones that just want to shell out
//! to `git` — to depend on having Bun or Node installed and a worker runtime online.
//!
//! With this module the host can serve framework-primitive RPCs directly from Rust,
//! so MiniApps that only use `app.shell.exec` / `app.fs.*` / `app.net.fetch` can run
//! with `permissions.node.enabled = false` and no JS Worker at all.
//!
//! Routing rules (must match `useMiniAppBridge.ts`):
//! - `worker.call` for methods in `fs.*`, `shell.*`, `os.*`, `net.*` always go through
//!   the host. User `worker.js` cannot override these names anymore in node-disabled mode.
//! - All other methods (custom user RPCs and `storage.*`) keep going through the worker
//!   pool when the app has `node.enabled = true`. `storage.*` is served by the manager
//!   directly from the Tauri command layer regardless of node.enabled.
//!
//! Permission enforcement here mirrors `worker_host.js` exactly so the security
//! contract is identical regardless of the routing path.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
pub use bitfun_product_domains::miniapp::host_routing::is_host_primitive;
use bitfun_product_domains::miniapp::host_routing::{
    command_basename_allowed, command_basename_for_allowlist, fs_policy_scopes,
    fs_resolved_path_allowed, host_allowed_by_allowlist, plan_fs_host_call,
    plan_fs_legacy_path_check, plan_shell_host_call, shell_exec_default_env, split_host_method,
    FsAccessMode, MiniAppFsHostCallPlan, MiniAppHostPlanError, MiniAppHostPlanErrorKind,
};
use bitfun_product_domains::miniapp::permission_policy::resolve_policy;
use bitfun_product_domains::miniapp::types::MiniAppPermissions;
use serde_json::{json, Value};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniAppHostDispatchErrorKind {
    Parse,
    Validation,
    Io,
    Service,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiniAppHostDispatchError {
    kind: MiniAppHostDispatchErrorKind,
    message: String,
}

impl MiniAppHostDispatchError {
    pub fn parse(message: impl Into<String>) -> Self {
        Self {
            kind: MiniAppHostDispatchErrorKind::Parse,
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            kind: MiniAppHostDispatchErrorKind::Validation,
            message: message.into(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self {
            kind: MiniAppHostDispatchErrorKind::Io,
            message: message.into(),
        }
    }

    pub fn service(message: impl Into<String>) -> Self {
        Self {
            kind: MiniAppHostDispatchErrorKind::Service,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> MiniAppHostDispatchErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for MiniAppHostDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MiniAppHostDispatchError {}

pub type MiniAppHostDispatchResult<T> = Result<T, MiniAppHostDispatchError>;

/// Dispatch a framework-primitive RPC on the host.
///
/// `perms` and the path arguments are used to build a permission policy with the
/// same shape `worker_host.js` consumes, then the namespace-specific handler is
/// invoked.
pub async fn dispatch_host(
    perms: &MiniAppPermissions,
    app_id: &str,
    app_data_dir: &Path,
    workspace_dir: Option<&Path>,
    granted_paths: &[PathBuf],
    method: &str,
    params: Value,
) -> MiniAppHostDispatchResult<Value> {
    let policy = resolve_policy(perms, app_id, app_data_dir, workspace_dir, granted_paths);
    let (ns, name) = split_host_method(method)
        .ok_or_else(|| MiniAppHostDispatchError::parse(format!("invalid method: {}", method)))?;
    match ns {
        "fs" => dispatch_fs(&policy, name, &params).await,
        "shell" => dispatch_shell(&policy, app_data_dir, workspace_dir, name, &params).await,
        "os" => dispatch_os(name).await,
        "net" => dispatch_net(&policy, name, &params).await,
        _ => Err(MiniAppHostDispatchError::validation(format!(
            "unsupported host namespace: {}",
            ns
        ))),
    }
}

fn deny<S: Into<String>>(msg: S) -> MiniAppHostDispatchError {
    MiniAppHostDispatchError::validation(msg)
}

/// Resolve a path to its canonical form. If the path itself doesn't exist (e.g.
/// `writeFile` to a brand new file), walk up to the closest existing parent,
/// canonicalize that, then re-append the remaining tail. Falls back to the
/// lexical input when nothing along the chain exists.
fn canonicalize_best_effort(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    let mut tail = PathBuf::new();
    let mut cur: PathBuf = p.to_path_buf();
    while let Some(parent) = cur.parent().map(Path::to_path_buf) {
        if parent.as_os_str().is_empty() {
            break;
        }
        if let Some(name) = cur.file_name() {
            let mut new_tail = PathBuf::from(name);
            new_tail.push(&tail);
            tail = new_tail;
        }
        if let Ok(c) = parent.canonicalize() {
            return c.join(tail);
        }
        cur = parent;
    }
    p.to_path_buf()
}

/// A target path is allowed when its canonicalized form starts with one of the
/// canonicalized scope roots. Mirrors the worker_host.js check, but uses real
/// canonicalization so e.g. `/tmp/foo` on macOS (`/private/tmp/foo`) matches a
/// `/tmp` scope after both sides resolve symlinks.
fn path_allowed(policy: &Value, target: &Path, mode: FsAccessMode) -> bool {
    let scopes = fs_policy_scopes(policy, mode);
    if scopes.is_empty() {
        return false;
    }
    let resolved = canonicalize_best_effort(target);
    let resolved_scopes = scopes
        .into_iter()
        .map(PathBuf::from)
        .map(|scope| canonicalize_best_effort(&scope));
    fs_resolved_path_allowed(&resolved, resolved_scopes)
}

fn host_plan_error(error: MiniAppHostPlanError) -> MiniAppHostDispatchError {
    match error.kind() {
        MiniAppHostPlanErrorKind::Parse => {
            MiniAppHostDispatchError::parse(error.message().to_string())
        }
        MiniAppHostPlanErrorKind::Validation => {
            MiniAppHostDispatchError::validation(error.message().to_string())
        }
    }
}

fn resolve_shell_program(command: &str) -> PathBuf {
    let has_path_separator = command.contains('/') || command.contains('\\');
    if has_path_separator {
        return PathBuf::from(command);
    }

    which::which(command).unwrap_or_else(|_| PathBuf::from(command))
}

async fn dispatch_fs(
    policy: &Value,
    name: &str,
    params: &Value,
) -> MiniAppHostDispatchResult<Value> {
    let legacy_path_check = plan_fs_legacy_path_check(name, params);
    if let Some(check) = &legacy_path_check {
        if !path_allowed(policy, &check.path, check.mode) {
            return Err(deny(check.denied_message()));
        }
    }

    let plan = plan_fs_host_call(name, params).map_err(host_plan_error)?;
    for check in plan.path_checks() {
        if legacy_path_check
            .as_ref()
            .is_some_and(|legacy_check| legacy_check == &check)
        {
            continue;
        }
        if !path_allowed(policy, &check.path, check.mode) {
            return Err(deny(check.denied_message()));
        }
    }

    match plan {
        MiniAppFsHostCallPlan::ReadFile {
            path: p,
            encoding_base64,
        } => {
            let bytes = tokio::fs::read(&p).await.map_err(|e| {
                MiniAppHostDispatchError::io(format!("readFile {}: {}", p.display(), e))
            })?;
            if encoding_base64 {
                Ok(Value::String(BASE64.encode(&bytes)))
            } else {
                Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
            }
        }
        MiniAppFsHostCallPlan::WriteFile { path: p, data } => {
            tokio::fs::write(&p, data).await.map_err(|e| {
                MiniAppHostDispatchError::io(format!("writeFile {}: {}", p.display(), e))
            })?;
            Ok(Value::Null)
        }
        MiniAppFsHostCallPlan::ReadDir { path: p } => {
            let mut rd = tokio::fs::read_dir(&p).await.map_err(|e| {
                MiniAppHostDispatchError::io(format!("readdir {}: {}", p.display(), e))
            })?;
            let mut out = Vec::new();
            while let Some(entry) = rd
                .next_entry()
                .await
                .map_err(|e| MiniAppHostDispatchError::io(e.to_string()))?
            {
                let ft = entry.file_type().await.ok();
                out.push(json!({
                    "name": entry.file_name().to_string_lossy(),
                    "path": entry.path().to_string_lossy(),
                    "isDirectory": ft.map(|t| t.is_dir()).unwrap_or(false),
                }));
            }
            Ok(Value::Array(out))
        }
        MiniAppFsHostCallPlan::Stat { path: p } => {
            let meta = tokio::fs::metadata(&p).await.map_err(|e| {
                MiniAppHostDispatchError::io(format!("stat {}: {}", p.display(), e))
            })?;
            Ok(json!({
                "size": meta.len(),
                "isDirectory": meta.is_dir(),
                "isFile": meta.is_file(),
            }))
        }
        MiniAppFsHostCallPlan::Mkdir { path: p, recursive } => {
            (if recursive {
                tokio::fs::create_dir_all(&p).await
            } else {
                tokio::fs::create_dir(&p).await
            })
            .map_err(|e| MiniAppHostDispatchError::io(format!("mkdir {}: {}", p.display(), e)))?;
            Ok(Value::Null)
        }
        MiniAppFsHostCallPlan::Rm {
            path: p,
            recursive,
            force,
        } => {
            let result = match tokio::fs::metadata(&p).await {
                Ok(m) if m.is_dir() => {
                    if recursive {
                        tokio::fs::remove_dir_all(&p).await
                    } else {
                        tokio::fs::remove_dir(&p).await
                    }
                }
                Ok(_) => tokio::fs::remove_file(&p).await,
                Err(e) => {
                    if force {
                        return Ok(Value::Null);
                    }
                    return Err(MiniAppHostDispatchError::io(format!(
                        "rm {}: {}",
                        p.display(),
                        e
                    )));
                }
            };
            result
                .map_err(|e| MiniAppHostDispatchError::io(format!("rm {}: {}", p.display(), e)))?;
            Ok(Value::Null)
        }
        MiniAppFsHostCallPlan::CopyFile { src, dst } => {
            tokio::fs::copy(&src, &dst)
                .await
                .map_err(|e| MiniAppHostDispatchError::io(format!("copyFile: {}", e)))?;
            Ok(Value::Null)
        }
        MiniAppFsHostCallPlan::Rename {
            old_path: oldp,
            new_path: newp,
        } => {
            tokio::fs::rename(&oldp, &newp)
                .await
                .map_err(|e| MiniAppHostDispatchError::io(format!("rename: {}", e)))?;
            Ok(Value::Null)
        }
        MiniAppFsHostCallPlan::AppendFile { path: p, data } => {
            use tokio::io::AsyncWriteExt;
            let mut f = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&p)
                .await
                .map_err(|e| MiniAppHostDispatchError::io(format!("appendFile open: {}", e)))?;
            f.write_all(data.as_bytes())
                .await
                .map_err(|e| MiniAppHostDispatchError::io(format!("appendFile write: {}", e)))?;
            Ok(Value::Null)
        }
        MiniAppFsHostCallPlan::Access { path: p } => {
            tokio::fs::metadata(&p).await.map_err(|e| {
                MiniAppHostDispatchError::io(format!("access {}: {}", p.display(), e))
            })?;
            Ok(Value::Null)
        }
    }
}

async fn dispatch_shell(
    policy: &Value,
    app_data_dir: &Path,
    workspace_dir: Option<&Path>,
    name: &str,
    params: &Value,
) -> MiniAppHostDispatchResult<Value> {
    // Two input shapes are supported:
    //   1. `{ command: "git status" }` — runs through the platform shell (sh -c / cmd /C).
    //   2. `{ args: ["git", "rev-parse", "--is-inside-work-tree"] }` — spawns the program
    //      directly with no shell. This is the cross-platform safe form: callers no longer
    //      need to worry about per-shell quoting (single quotes from sh do not work under
    //      cmd.exe on Windows, which previously broke `builtin-coding-selfie` git scans).
    let plan =
        plan_shell_host_call(name, params, workspace_dir, app_data_dir).map_err(host_plan_error)?;

    // Allowlist check: take the program name (basename of the first token, sans
    // extension) and require it to be in `policy.shell.allow`.
    let allow: Vec<String> = policy
        .get("shell")
        .and_then(|v| v.get("allow"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let base = command_basename_for_allowlist(&plan.first_token);
    if !command_basename_allowed(&allow, &base) {
        return Err(deny(format!("Command not in allowlist: {}", base)));
    }

    let mut cmd = if let Some(argv) = plan.argv.as_ref() {
        let program = resolve_shell_program(&argv[0]);
        let mut c =
            bitfun_services_core::process_manager::create_tokio_command(program.as_os_str());
        if argv.len() > 1 {
            c.args(&argv[1..]);
        }
        c
    } else {
        #[cfg(target_os = "windows")]
        {
            let mut c = bitfun_services_core::process_manager::create_tokio_command("cmd");
            c.args(["/C", &plan.command]);
            c
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut c = bitfun_services_core::process_manager::create_tokio_command("sh");
            c.args(["-c", &plan.command]);
            c
        }
    };
    cmd.current_dir(&plan.cwd);
    // Match worker_host.js: never let git prompt for credentials, force C locale so
    // stdout parsing is deterministic.
    for (key, value) in shell_exec_default_env() {
        cmd.env(key, value);
    }

    let output = tokio::time::timeout(Duration::from_millis(plan.timeout_ms), cmd.output())
        .await
        .map_err(|_| {
            MiniAppHostDispatchError::service(format!(
                "shell.exec timed out after {}ms",
                plan.timeout_ms
            ))
        })?
        .map_err(|e| {
            MiniAppHostDispatchError::service(format!("shell.exec spawn failed: {}", e))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);

    if !output.status.success() {
        // Mirror worker_host.js (which uses Node `execAsync`, rejecting on non-zero
        // exit with stderr in the message).
        let msg = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            format!("shell.exec exit {}", code)
        };
        return Err(MiniAppHostDispatchError::service(msg));
    }

    Ok(json!({ "stdout": stdout, "stderr": stderr, "exit_code": code }))
}

async fn dispatch_os(name: &str) -> MiniAppHostDispatchResult<Value> {
    if name != "info" {
        return Err(MiniAppHostDispatchError::validation(format!(
            "unknown os method: {}",
            name
        )));
    }
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    };
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    Ok(json!({
        "platform": platform,
        "homedir": dirs::home_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "tmpdir": std::env::temp_dir().to_string_lossy(),
        "cpus": cpus,
        // memory stats are not available without an extra crate; report 0 for parity
        // with `os.totalmem()` semantics ("unknown") rather than failing the call.
        "totalmem": 0u64,
        "freemem": 0u64,
    }))
}

async fn dispatch_net(
    policy: &Value,
    name: &str,
    params: &Value,
) -> MiniAppHostDispatchResult<Value> {
    if name != "fetch" {
        return Err(MiniAppHostDispatchError::validation(format!(
            "unknown net method: {}",
            name
        )));
    }
    let url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.is_empty() {
        return Err(MiniAppHostDispatchError::parse("missing url"));
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| MiniAppHostDispatchError::parse(format!("invalid url: {}", e)))?;
    let host = parsed.host_str().unwrap_or("").to_string();

    let allow: Vec<String> = policy
        .get("net")
        .and_then(|v| v.get("allow"))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if !host_allowed_by_allowlist(&allow, &host) {
        return Err(deny(format!("Domain not in allowlist: {}", host)));
    }

    let method = params
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET");
    let client = reqwest::Client::new();
    let req_method = reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut req = client.request(req_method, url);
    if let Some(headers) = params.get("headers").and_then(|v| v.as_object()) {
        for (k, v) in headers {
            if let Some(vs) = v.as_str() {
                req = req.header(k, vs);
            }
        }
    }
    if let Some(body) = params.get("body").and_then(|v| v.as_str()) {
        req = req.body(body.to_string());
    }

    let res = req
        .send()
        .await
        .map_err(|e| MiniAppHostDispatchError::service(format!("net.fetch: {}", e)))?;
    let status = res.status().as_u16();
    let mut headers_out = serde_json::Map::new();
    for (k, v) in res.headers() {
        if let Ok(vs) = v.to_str() {
            headers_out.insert(k.as_str().to_string(), Value::String(vs.to_string()));
        }
    }
    let body = res
        .text()
        .await
        .map_err(|e| MiniAppHostDispatchError::service(format!("net.fetch read: {}", e)))?;
    Ok(json!({
        "status": status,
        "headers": Value::Object(headers_out),
        "body": body,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitfun_product_domains::miniapp::types::{MiniAppPermissions, ShellPermissions};

    #[test]
    fn command_basename_allows_windows_git_executable_paths() {
        assert_eq!(
            command_basename_for_allowlist(r"C:\Program Files\Git\cmd\git.exe"),
            "git"
        );
        assert_eq!(command_basename_for_allowlist("git.exe"), "git");
        assert_eq!(command_basename_for_allowlist("/usr/bin/git"), "git");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn host_shell_exec_runs_git_with_workspace_cwd() {
        let workspace_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let perms = MiniAppPermissions {
            shell: Some(ShellPermissions {
                allow: Some(vec!["git".to_string()]),
            }),
            ..Default::default()
        };

        let result = dispatch_host(
            &perms,
            "builtin-coding-selfie",
            workspace_dir,
            Some(workspace_dir),
            &[],
            "shell.exec",
            json!({
                "args": ["git", "rev-parse", "--is-inside-work-tree"],
                "cwd": workspace_dir.to_string_lossy(),
                "timeout": 8000,
            }),
        )
        .await
        .expect("git rev-parse should run in the repository workspace");

        assert_eq!(
            result
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim(),
            "true"
        );
    }
}
