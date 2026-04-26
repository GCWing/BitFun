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

use crate::miniapp::permission_policy::resolve_policy;
use crate::miniapp::types::MiniAppPermissions;
use crate::util::errors::{BitFunError, BitFunResult};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

/// Namespaces handled by the host-side dispatch (no Worker required).
const HOST_NAMESPACES: &[&str] = &["fs", "shell", "os", "net"];

/// Returns true when `method` belongs to a namespace served by the host directly.
///
/// `storage.*` is intentionally excluded: it is routed through `MiniAppManager` from the
/// command layer so it can share locking with the rest of the app.
pub fn is_host_primitive(method: &str) -> bool {
    method
        .split_once('.')
        .map(|(ns, _)| HOST_NAMESPACES.contains(&ns))
        .unwrap_or(false)
}

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
) -> BitFunResult<Value> {
    let policy = resolve_policy(perms, app_id, app_data_dir, workspace_dir, granted_paths);
    let (ns, name) = method
        .split_once('.')
        .ok_or_else(|| BitFunError::parse(format!("invalid method: {}", method)))?;
    match ns {
        "fs" => dispatch_fs(&policy, name, &params).await,
        "shell" => dispatch_shell(&policy, app_data_dir, workspace_dir, name, &params).await,
        "os" => dispatch_os(name).await,
        "net" => dispatch_net(&policy, name, &params).await,
        _ => Err(BitFunError::validation(format!(
            "unsupported host namespace: {}",
            ns
        ))),
    }
}

fn deny<S: Into<String>>(msg: S) -> BitFunError {
    BitFunError::validation(msg)
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
fn path_allowed(policy: &Value, target: &Path, mode: &str) -> bool {
    let key = if mode == "write" { "write" } else { "read" };
    let scopes = match policy
        .get("fs")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_array())
    {
        Some(a) => a,
        None => return false,
    };
    let resolved = canonicalize_best_effort(target);
    for s in scopes {
        let Some(scope_str) = s.as_str() else {
            continue;
        };
        let scope_path = PathBuf::from(scope_str);
        let scope_canon = canonicalize_best_effort(&scope_path);
        if resolved.starts_with(&scope_canon) {
            return true;
        }
    }
    false
}

fn arg_path(params: &Value, key: &str) -> BitFunResult<PathBuf> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| BitFunError::parse(format!("missing param: {}", key)))
}

async fn dispatch_fs(policy: &Value, name: &str, params: &Value) -> BitFunResult<Value> {
    // Common path arg ("path" or legacy "p").
    let path_param = params
        .get("path")
        .or_else(|| params.get("p"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from);

    let needs_write = matches!(
        name,
        "writeFile" | "mkdir" | "rm" | "appendFile" | "rename" | "copyFile"
    );

    if let Some(ref p) = path_param {
        let mode = if needs_write { "write" } else { "read" };
        if name != "access" && !path_allowed(policy, p, mode) {
            return Err(deny(format!("Path not allowed: {}", p.display())));
        }
    }

    match name {
        "readFile" => {
            let p = path_param.ok_or_else(|| BitFunError::parse("missing path"))?;
            let enc = params
                .get("encoding")
                .and_then(|v| v.as_str())
                .unwrap_or("utf8");
            let bytes = tokio::fs::read(&p)
                .await
                .map_err(|e| BitFunError::io(format!("readFile {}: {}", p.display(), e)))?;
            if enc == "base64" {
                Ok(Value::String(BASE64.encode(&bytes)))
            } else {
                Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
            }
        }
        "writeFile" => {
            let p = path_param.ok_or_else(|| BitFunError::parse("missing path"))?;
            let data = params.get("data").and_then(|v| v.as_str()).unwrap_or("");
            tokio::fs::write(&p, data)
                .await
                .map_err(|e| BitFunError::io(format!("writeFile {}: {}", p.display(), e)))?;
            Ok(Value::Null)
        }
        "readdir" => {
            let p = path_param.ok_or_else(|| BitFunError::parse("missing path"))?;
            let mut rd = tokio::fs::read_dir(&p)
                .await
                .map_err(|e| BitFunError::io(format!("readdir {}: {}", p.display(), e)))?;
            let mut out = Vec::new();
            while let Some(entry) = rd
                .next_entry()
                .await
                .map_err(|e| BitFunError::io(e.to_string()))?
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
        "stat" => {
            let p = path_param.ok_or_else(|| BitFunError::parse("missing path"))?;
            let meta = tokio::fs::metadata(&p)
                .await
                .map_err(|e| BitFunError::io(format!("stat {}: {}", p.display(), e)))?;
            Ok(json!({
                "size": meta.len(),
                "isDirectory": meta.is_dir(),
                "isFile": meta.is_file(),
            }))
        }
        "mkdir" => {
            let p = path_param.ok_or_else(|| BitFunError::parse("missing path"))?;
            let recursive = params
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            (if recursive {
                tokio::fs::create_dir_all(&p).await
            } else {
                tokio::fs::create_dir(&p).await
            })
            .map_err(|e| BitFunError::io(format!("mkdir {}: {}", p.display(), e)))?;
            Ok(Value::Null)
        }
        "rm" => {
            let p = path_param.ok_or_else(|| BitFunError::parse("missing path"))?;
            let recursive = params
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let force = params
                .get("force")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
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
                    return Err(BitFunError::io(format!("rm {}: {}", p.display(), e)));
                }
            };
            result.map_err(|e| BitFunError::io(format!("rm {}: {}", p.display(), e)))?;
            Ok(Value::Null)
        }
        "copyFile" => {
            let src = arg_path(params, "src")?;
            let dst = arg_path(params, "dst")?;
            if !path_allowed(policy, &src, "read") {
                return Err(deny(format!("src not allowed: {}", src.display())));
            }
            if !path_allowed(policy, &dst, "write") {
                return Err(deny(format!("dst not allowed: {}", dst.display())));
            }
            tokio::fs::copy(&src, &dst)
                .await
                .map_err(|e| BitFunError::io(format!("copyFile: {}", e)))?;
            Ok(Value::Null)
        }
        "rename" => {
            let oldp = arg_path(params, "oldPath")?;
            let newp = arg_path(params, "newPath")?;
            if !path_allowed(policy, &oldp, "write") {
                return Err(deny(format!("oldPath not allowed: {}", oldp.display())));
            }
            if !path_allowed(policy, &newp, "write") {
                return Err(deny(format!("newPath not allowed: {}", newp.display())));
            }
            tokio::fs::rename(&oldp, &newp)
                .await
                .map_err(|e| BitFunError::io(format!("rename: {}", e)))?;
            Ok(Value::Null)
        }
        "appendFile" => {
            use tokio::io::AsyncWriteExt;
            let p = path_param.ok_or_else(|| BitFunError::parse("missing path"))?;
            let data = params.get("data").and_then(|v| v.as_str()).unwrap_or("");
            let mut f = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&p)
                .await
                .map_err(|e| BitFunError::io(format!("appendFile open: {}", e)))?;
            f.write_all(data.as_bytes())
                .await
                .map_err(|e| BitFunError::io(format!("appendFile write: {}", e)))?;
            Ok(Value::Null)
        }
        "access" => {
            let p = path_param.ok_or_else(|| BitFunError::parse("missing path"))?;
            tokio::fs::metadata(&p)
                .await
                .map_err(|e| BitFunError::io(format!("access {}: {}", p.display(), e)))?;
            Ok(Value::Null)
        }
        other => Err(BitFunError::validation(format!(
            "unknown fs method: {}",
            other
        ))),
    }
}

async fn dispatch_shell(
    policy: &Value,
    app_data_dir: &Path,
    workspace_dir: Option<&Path>,
    name: &str,
    params: &Value,
) -> BitFunResult<Value> {
    if name != "exec" {
        return Err(BitFunError::validation(format!(
            "unknown shell method: {}",
            name
        )));
    }
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if command.is_empty() {
        return Err(BitFunError::parse("empty command"));
    }

    // Allowlist check: take the program name (basename of the first whitespace-
    // separated token, sans extension) and require it to be in `policy.shell.allow`.
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
    let first_token = command.split_whitespace().next().unwrap_or("");
    let base = Path::new(first_token)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(first_token)
        .to_lowercase();
    if !allow.is_empty() && !allow.iter().any(|a| a.to_lowercase() == base) {
        return Err(deny(format!("Command not in allowlist: {}", base)));
    }

    // cwd: explicit > workspace > appdata. Mirrors what worker_host.js gives users
    // (where process.cwd() is appDir, but the iframe always passes cwd explicitly).
    let cwd = params
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .or_else(|| workspace_dir.map(Path::to_path_buf))
        .unwrap_or_else(|| app_data_dir.to_path_buf());
    let timeout_ms = params
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(30_000);

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", &command]);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.args(["-c", &command]);
        c
    };
    cmd.current_dir(&cwd);
    // Match worker_host.js: never let git prompt for credentials, force C locale so
    // stdout parsing is deterministic.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("LC_ALL", "C");

    let output = tokio::time::timeout(Duration::from_millis(timeout_ms), cmd.output())
        .await
        .map_err(|_| BitFunError::service(format!("shell.exec timed out after {}ms", timeout_ms)))?
        .map_err(|e| BitFunError::service(format!("shell.exec spawn failed: {}", e)))?;

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
        return Err(BitFunError::service(msg));
    }

    Ok(json!({ "stdout": stdout, "stderr": stderr, "exit_code": code }))
}

async fn dispatch_os(name: &str) -> BitFunResult<Value> {
    if name != "info" {
        return Err(BitFunError::validation(format!(
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

async fn dispatch_net(policy: &Value, name: &str, params: &Value) -> BitFunResult<Value> {
    if name != "fetch" {
        return Err(BitFunError::validation(format!(
            "unknown net method: {}",
            name
        )));
    }
    let url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.is_empty() {
        return Err(BitFunError::parse("missing url"));
    }
    let parsed =
        reqwest::Url::parse(url).map_err(|e| BitFunError::parse(format!("invalid url: {}", e)))?;
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
    if !allow.is_empty()
        && !allow.iter().any(|a| a == "*")
        && !allow
            .iter()
            .any(|a| host == *a || host.ends_with(&format!(".{}", a)))
    {
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
        .map_err(|e| BitFunError::service(format!("net.fetch: {}", e)))?;
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
        .map_err(|e| BitFunError::service(format!("net.fetch read: {}", e)))?;
    Ok(json!({
        "status": status,
        "headers": Value::Object(headers_out),
        "body": body,
    }))
}
