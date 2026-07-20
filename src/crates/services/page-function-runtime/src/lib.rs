//! Embedded Page Function runtime backed by rquickjs.
//!
//! User workers define a global `fetch(request, env)` that returns
//! `{ status, headers, body }` (or a Promise of that shape). Host bindings
//! expose KV / DB / BLOBS / ASSETS / PAGE via a JSON host-call bridge.
//! No arbitrary outbound network. Async handlers may only await microtasks;
//! host bindings are synchronous.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rquickjs::promise::MaybePromise;
use rquickjs::{Context, Ctx, Function, Object, Runtime};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
pub const WORKER_ENTRY_PATH: &str = "server/worker.js";

#[derive(Debug, Error)]
pub enum PageFunctionError {
    #[error("runtime init failed: {0}")]
    Init(String),
    #[error("worker evaluation failed: {0}")]
    Eval(String),
    #[error("fetch handler missing or invalid")]
    MissingFetch,
    #[error("fetch handler failed: {0}")]
    Handler(String),
    #[error("execution timed out after {0:?}")]
    Timeout(Duration),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchRequest {
    pub method: String,
    pub url: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMeta {
    pub username: String,
    pub slug: String,
    pub version_id: String,
    pub visibility: String,
}

impl Default for PageMeta {
    fn default() -> Self {
        Self {
            username: "user".into(),
            slug: "site".into(),
            version_id: "v1".into(),
            visibility: "public".into(),
        }
    }
}

/// Host capabilities injected into the worker `env` object.
pub trait PageHost: Send + Sync {
    fn kv_get(&self, key: &str) -> Result<Option<String>, String>;
    fn kv_put(&self, key: &str, value: &str) -> Result<(), String>;
    fn kv_delete(&self, key: &str) -> Result<bool, String>;
    fn kv_list(&self) -> Result<Vec<String>, String>;

    fn db_execute(&self, sql: &str, params_json: &str) -> Result<String, String>;
    fn db_query(&self, sql: &str, params_json: &str) -> Result<String, String>;

    fn blob_put(&self, blob_id: &str, content_type: &str, data_b64: &str) -> Result<(), String>;
    fn blob_get(&self, blob_id: &str) -> Result<Option<(String, String)>, String>;
    fn blob_delete(&self, blob_id: &str) -> Result<bool, String>;

    fn assets_get(&self, path: &str) -> Result<Option<(String, Vec<u8>)>, String>;

    fn page_meta(&self) -> PageMeta;
}

/// Run a page worker `fetch` handler to completion (or timeout).
pub fn run_fetch(
    worker_source: &str,
    request: &FetchRequest,
    host: Arc<dyn PageHost>,
    timeout: Duration,
) -> Result<FetchResponse, PageFunctionError> {
    let started = Instant::now();
    let runtime = Runtime::new().map_err(|e| PageFunctionError::Init(e.to_string()))?;
    runtime.set_memory_limit(16 * 1024 * 1024);
    runtime.set_max_stack_size(256 * 1024);

    let context = Context::full(&runtime).map_err(|e| PageFunctionError::Init(e.to_string()))?;

    let json_out: String = context.with(|ctx| {
        if started.elapsed() > timeout {
            return Err(PageFunctionError::Timeout(timeout));
        }

        ctx.eval::<(), _>(worker_source)
            .map_err(|e| PageFunctionError::Eval(format!("{e}")))?;

        // Ensure fetch exists before wrapping.
        let globals = ctx.globals();
        let _: Function = globals
            .get("fetch")
            .map_err(|_| PageFunctionError::MissingFetch)?;

        // Serialize response in JS to avoid RefCell re-entrancy when reading objects.
        // Support both sync returns and Promise / async function fetch.
        ctx.eval::<(), _>(
            r#"
            globalThis.__bitfun_normalize = function(r) {
              if (typeof r === "string") {
                return JSON.stringify({ status: 200, headers: { "content-type": "text/plain; charset=utf-8" }, body: r });
              }
              var headers = {};
              if (r && r.headers) {
                var keys = Object.keys(r.headers);
                for (var i = 0; i < keys.length; i++) {
                  headers[String(keys[i]).toLowerCase()] = String(r.headers[keys[i]]);
                }
              }
              if (!headers["content-type"]) {
                headers["content-type"] = "text/plain; charset=utf-8";
              }
              return JSON.stringify({
                status: (r && r.status) ? r.status : 200,
                headers: headers,
                body: (r && r.body != null) ? String(r.body) : ""
              });
            };
            globalThis.__bitfun_invoke = function(req, env) {
              var r = fetch(req, env);
              if (r && typeof r.then === "function") {
                return r.then(globalThis.__bitfun_normalize);
              }
              return globalThis.__bitfun_normalize(r);
            };
            "#,
        )
        .map_err(|e| PageFunctionError::Eval(format!("wrap fetch: {e}")))?;

        let invoke: Function = globals
            .get("__bitfun_invoke")
            .map_err(|e| PageFunctionError::Init(e.to_string()))?;

        let env = build_env_object(&ctx, host)?;
        let req_obj = request_to_object(&ctx, request)?;
        let maybe: MaybePromise = invoke
            .call((req_obj, env))
            .map_err(|e| PageFunctionError::Handler(format!("{e}")))?;

        // Drive QuickJS microtasks until the (maybe) promise settles, respecting timeout.
        let out = loop {
            if started.elapsed() > timeout {
                return Err(PageFunctionError::Timeout(timeout));
            }
            match maybe.result::<String>() {
                Some(Ok(s)) => break s,
                Some(Err(e)) => {
                    return Err(PageFunctionError::Handler(format!("async fetch failed: {e}")));
                }
                None => {
                    if !ctx.execute_pending_job() {
                        return Err(PageFunctionError::Handler(
                            "async fetch returned a pending promise with no runnable jobs \
                             (host bindings are synchronous; do not await external I/O)"
                                .into(),
                        ));
                    }
                }
            }
        };
        Ok(out)
    })?;

    serde_json::from_str::<FetchResponse>(&json_out)
        .map_err(|e| PageFunctionError::Handler(format!("invalid fetch response JSON: {e}")))
}

fn build_env_object<'js>(
    ctx: &Ctx<'js>,
    host: Arc<dyn PageHost>,
) -> Result<Object<'js>, PageFunctionError> {
    let host_fn = Function::new(
        ctx.clone(),
        move |op: String, a: String, b: String, c: String| -> String {
            host_call(host.as_ref(), &op, &a, &b, &c)
        },
    )
    .map_err(|e| PageFunctionError::Init(e.to_string()))?;

    let factory_src = r#"
        (function(hostCall) {
          var meta = JSON.parse(hostCall("page_meta","","",""));
          return {
            KV: {
              get: function(k) {
                var r = JSON.parse(hostCall("kv_get", String(k), "", ""));
                return r.v;
              },
              put: function(k, v) {
                JSON.parse(hostCall("kv_put", String(k), String(v), ""));
              },
              delete: function(k) {
                return JSON.parse(hostCall("kv_delete", String(k), "", "")).ok;
              },
              list: function() {
                return JSON.parse(hostCall("kv_list", "", "", "")).keys;
              }
            },
            DB: {
              execute: function(sql, params) {
                return hostCall("db_execute", String(sql), params ? JSON.stringify(params) : "[]", "");
              },
              query: function(sql, params) {
                return hostCall("db_query", String(sql), params ? JSON.stringify(params) : "[]", "");
              }
            },
            BLOBS: {
              put: function(id, contentType, dataB64) {
                return JSON.parse(hostCall("blob_put", String(id), String(contentType||"application/octet-stream"), String(dataB64))).ok;
              },
              get: function(id) {
                var r = JSON.parse(hostCall("blob_get", String(id), "", ""));
                return r.found ? { contentType: r.contentType, data: r.data } : null;
              },
              delete: function(id) {
                return JSON.parse(hostCall("blob_delete", String(id), "", "")).ok;
              }
            },
            ASSETS: {
              fetch: function(path) {
                return JSON.parse(hostCall("assets_fetch", String(path), "", ""));
              }
            },
            PAGE: meta
          };
        })
    "#;

    let factory: Function = ctx
        .eval(factory_src)
        .map_err(|e| PageFunctionError::Init(format!("env factory: {e}")))?;
    let built: Object = factory
        .call((host_fn,))
        .map_err(|e| PageFunctionError::Init(format!("env build: {e}")))?;
    Ok(built)
}

fn host_call(host: &dyn PageHost, op: &str, a: &str, b: &str, c: &str) -> String {
    match op {
        "kv_get" => match host.kv_get(a) {
            Ok(Some(v)) => format!(r#"{{"v":{}}}"#, json_str(&v)),
            Ok(None) => r#"{"v":null}"#.into(),
            Err(e) => format!(r#"{{"v":null,"error":{}}}"#, json_str(&e)),
        },
        "kv_put" => match host.kv_put(a, b) {
            Ok(()) => r#"{"ok":true}"#.into(),
            Err(e) => format!(r#"{{"ok":false,"error":{}}}"#, json_str(&e)),
        },
        "kv_delete" => match host.kv_delete(a) {
            Ok(ok) => format!(r#"{{"ok":{ok}}}"#),
            Err(_) => r#"{"ok":false}"#.into(),
        },
        "kv_list" => match host.kv_list() {
            Ok(keys) => format!(
                r#"{{"keys":{}}}"#,
                serde_json::to_string(&keys).unwrap_or_else(|_| "[]".into())
            ),
            Err(_) => r#"{"keys":[]}"#.into(),
        },
        "db_execute" => host
            .db_execute(a, b)
            .unwrap_or_else(|e| format!(r#"{{"ok":false,"error":{}}}"#, json_str(&e))),
        "db_query" => host
            .db_query(a, b)
            .unwrap_or_else(|e| format!(r#"{{"ok":false,"error":{}}}"#, json_str(&e))),
        "blob_put" => match host.blob_put(a, b, c) {
            Ok(()) => r#"{"ok":true}"#.into(),
            Err(e) => format!(r#"{{"ok":false,"error":{}}}"#, json_str(&e)),
        },
        "blob_get" => match host.blob_get(a) {
            Ok(Some((ct, data))) => format!(
                r#"{{"found":true,"contentType":{},"data":{}}}"#,
                json_str(&ct),
                json_str(&data)
            ),
            Ok(None) => r#"{"found":false}"#.into(),
            Err(e) => format!(r#"{{"found":false,"error":{}}}"#, json_str(&e)),
        },
        "blob_delete" => match host.blob_delete(a) {
            Ok(ok) => format!(r#"{{"ok":{ok}}}"#),
            Err(_) => r#"{"ok":false}"#.into(),
        },
        "assets_fetch" => match host.assets_get(a) {
            Ok(Some((ct, bytes))) => {
                let body = String::from_utf8_lossy(&bytes);
                format!(
                    r#"{{"status":200,"headers":{{"content-type":{}}},"body":{}}}"#,
                    json_str(&ct),
                    json_str(&body)
                )
            }
            Ok(None) => r#"{"status":404,"headers":{},"body":"Not Found"}"#.into(),
            Err(e) => format!(r#"{{"status":500,"headers":{{}},"body":{}}}"#, json_str(&e)),
        },
        "page_meta" => serde_json::to_string(&host.page_meta()).unwrap_or_else(|_| "{}".into()),
        _ => r#"{"error":"unknown op"}"#.into(),
    }
}

fn request_to_object<'js>(
    ctx: &Ctx<'js>,
    request: &FetchRequest,
) -> Result<Object<'js>, PageFunctionError> {
    let obj = Object::new(ctx.clone()).map_err(|e| PageFunctionError::Init(e.to_string()))?;
    obj.set("method", request.method.as_str())
        .map_err(|e| PageFunctionError::Init(e.to_string()))?;
    obj.set("url", request.url.as_str())
        .map_err(|e| PageFunctionError::Init(e.to_string()))?;
    obj.set("path", request.path.as_str())
        .map_err(|e| PageFunctionError::Init(e.to_string()))?;
    if let Some(body) = &request.body {
        obj.set("body", body.as_str())
            .map_err(|e| PageFunctionError::Init(e.to_string()))?;
    }
    let headers = Object::new(ctx.clone()).map_err(|e| PageFunctionError::Init(e.to_string()))?;
    for (k, v) in &request.headers {
        let _ = headers.set(k.as_str(), v.as_str());
    }
    obj.set("headers", headers)
        .map_err(|e| PageFunctionError::Init(e.to_string()))?;
    Ok(obj)
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// In-memory host for unit tests.
pub struct MemoryPageHost {
    pub kv: Mutex<HashMap<String, String>>,
    pub meta: PageMeta,
    pub assets: Mutex<HashMap<String, Vec<u8>>>,
}

impl Default for MemoryPageHost {
    fn default() -> Self {
        Self {
            kv: Mutex::new(HashMap::new()),
            meta: PageMeta::default(),
            assets: Mutex::new(HashMap::new()),
        }
    }
}

impl PageHost for MemoryPageHost {
    fn kv_get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.kv.lock().unwrap().get(key).cloned())
    }
    fn kv_put(&self, key: &str, value: &str) -> Result<(), String> {
        self.kv
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }
    fn kv_delete(&self, key: &str) -> Result<bool, String> {
        Ok(self.kv.lock().unwrap().remove(key).is_some())
    }
    fn kv_list(&self) -> Result<Vec<String>, String> {
        let mut keys: Vec<_> = self.kv.lock().unwrap().keys().cloned().collect();
        keys.sort();
        Ok(keys)
    }
    fn db_execute(&self, _sql: &str, _params_json: &str) -> Result<String, String> {
        Ok(r#"{"ok":true,"changes":0}"#.into())
    }
    fn db_query(&self, _sql: &str, _params_json: &str) -> Result<String, String> {
        Ok(r#"{"ok":true,"rows":[]}"#.into())
    }
    fn blob_put(&self, _blob_id: &str, _content_type: &str, _data_b64: &str) -> Result<(), String> {
        Ok(())
    }
    fn blob_get(&self, _blob_id: &str) -> Result<Option<(String, String)>, String> {
        Ok(None)
    }
    fn blob_delete(&self, _blob_id: &str) -> Result<bool, String> {
        Ok(false)
    }
    fn assets_get(&self, path: &str) -> Result<Option<(String, Vec<u8>)>, String> {
        Ok(self
            .assets
            .lock()
            .unwrap()
            .get(path)
            .map(|b| ("text/html; charset=utf-8".into(), b.clone())))
    }
    fn page_meta(&self) -> PageMeta {
        self.meta.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_returns_body_and_uses_kv() {
        let host = Arc::new(MemoryPageHost::default());
        host.kv_put("name", "world").unwrap();
        let worker = r#"
            function fetch(request, env) {
                var name = env.KV.get("name") || "anon";
                return { status: 200, headers: { "content-type": "text/plain" }, body: "hello " + name };
            }
        "#;
        let resp = run_fetch(
            worker,
            &FetchRequest {
                method: "GET".into(),
                url: "https://example/p/u/s/".into(),
                path: "/".into(),
                headers: HashMap::new(),
                body: None,
            },
            host,
            DEFAULT_TIMEOUT,
        )
        .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "hello world");
    }

    #[test]
    fn missing_fetch_errors() {
        let host = Arc::new(MemoryPageHost::default());
        let err = run_fetch(
            "var x = 1;",
            &FetchRequest {
                method: "GET".into(),
                url: "/".into(),
                path: "/".into(),
                headers: HashMap::new(),
                body: None,
            },
            host,
            DEFAULT_TIMEOUT,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            PageFunctionError::MissingFetch | PageFunctionError::Eval(_)
        ));
    }

    #[test]
    fn async_fetch_promise_is_awaited() {
        let host = Arc::new(MemoryPageHost::default());
        host.kv_put("name", "async").unwrap();
        let worker = r#"
            async function fetch(request, env) {
                var name = env.KV.get("name") || "anon";
                return { status: 200, headers: { "content-type": "application/json" }, body: JSON.stringify({ hello: name }) };
            }
        "#;
        let resp = run_fetch(
            worker,
            &FetchRequest {
                method: "GET".into(),
                url: "https://example/p/u/s/api/hello".into(),
                path: "/api/hello".into(),
                headers: HashMap::new(),
                body: None,
            },
            host,
            DEFAULT_TIMEOUT,
        )
        .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, r#"{"hello":"async"}"#);
        assert_eq!(
            resp.headers.get("content-type").map(String::as_str),
            Some("application/json")
        );
    }
}
