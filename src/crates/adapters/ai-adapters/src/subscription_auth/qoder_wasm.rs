//! Embedded Qoder CN CLI authentication WASM runtime.
//!
//! The official `@qodercn-ai/qoderclicn` bundle embeds `qoder_auth_wasm_bg.wasm`
//! (CLI 1.1.23) as a base64 constant. BitFun extracts that module into
//! `resources/qoder_auth_wasm_bg.wasm` (see `scripts/qoder-wasm-extract.mjs`)
//! and instantiates it directly with a Rust host that reproduces the
//! wasm-bindgen glue the CLI generates. This gives BitFun the same
//! request signing (`prepareRequest`/`prepareInferRequest`) and response
//! decryption (`decrypt_server_response`) the official client performs, without
//! shelling out to the CLI.
//!
//! The module imports 31 wasm-bindgen glue functions from
//! `./qoder_auth_wasm_bg.js`. Only a handful have real behavior (crypto
//! random, `Date.now`); the rest manipulate JS object references that live in
//! a host-side reference table, which this implementation mirrors.
#![allow(dead_code)]
// wired up incrementally by the qoder adapter (R-QODER-03+)
// The wasmi TypedFunc signatures are fixed by the wasm module's exports; the
// host functions take many pointer pairs by construction.
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasmi::{Caller, Engine, Linker, Memory, Module, Store, TypedFunc};

/// Wasm bytes embedded from the extraction output (297,238 bytes, CLI 1.1.23).
const QODER_AUTH_WASM: &[u8] = include_bytes!("../../resources/qoder_auth_wasm_bg.wasm");

/// Number of reference-table slots the wasm-bindgen glue preallocates before
/// pushing `undefined/null/true/false`; first user object gets this index.
const JS_REF_BASE: u32 = 1024;

/// A JS-like value stored in the reference table.
#[derive(Clone)]
enum JsValue {
    Undefined,
    Null,
    Bool(bool),
    /// A `Map<string, string>` (the wasm's header map; keys/values are
    /// string refs copied into the map so later `drop_ref` calls do not
    /// invalidate them).
    Map(HashMap<String, String>),
    /// A `Uint8Array`. Two forms exist in the glue:
    /// - `MemView { ptr, len }`: a view over wasm linear memory (the glue's
    ///   `yCA(ptr, len)` helper) — reads/writes go straight to wasm memory.
    /// - `HostBuf(Arc<Mutex<Vec<u8>>>)`: a host-owned JS `Uint8Array` created
    ///   by `new Uint8Array(len)` — the buffer lives outside wasm memory.
    ///   `Arc<Mutex<_>>` (not `Rc<RefCell<_>>`) so `QoderWasm` stays `Send`,
    ///   which tauri commands require for async futures crossing threads.
    U8Array(U8ArrayData),
    /// A UTF-8 string decoded into host memory.
    Str(String),
    /// A function stub (used by `module.require`-style accessors).
    Function,
    /// A generic JS object (e.g. `crypto`, `process`).
    Object,
}

#[derive(Clone)]
enum U8ArrayData {
    MemView { ptr: u32, len: u32 },
    HostBuf(Arc<Mutex<Vec<u8>>>),
}

/// wasm-bindgen reference table with free-list allocation, mirroring the
/// CLI's `Em`/`S7o`/`HD` helpers.
#[derive(Default)]
struct RefTable {
    slots: Vec<Option<JsValue>>,
    free: Vec<u32>,
}

impl RefTable {
    fn new() -> Self {
        let mut slots = vec![None; JS_REF_BASE as usize];
        // yV.push(void 0, null, true, false)
        for value in [
            JsValue::Undefined,
            JsValue::Null,
            JsValue::Bool(true),
            JsValue::Bool(false),
        ] {
            slots.push(Some(value));
        }
        Self {
            slots,
            free: Vec::new(),
        }
    }

    /// `Em(value)` — allocate a new ref, reusing a freed slot if present.
    fn alloc(&mut self, value: JsValue) -> u32 {
        if let Some(slot) = self.free.pop() {
            self.slots[slot as usize] = Some(value);
            return slot;
        }
        self.slots.push(Some(value));
        (self.slots.len() - 1) as u32
    }

    /// `S7o(ref)` — return the ref to the free list (values below the base are
    /// primitives and are ignored).
    fn free(&mut self, reference: u32) {
        if reference < JS_REF_BASE {
            return;
        }
        if let Some(slot) = self.slots.get_mut(reference as usize) {
            *slot = None;
            self.free.push(reference);
        }
    }

    /// `Np(ref)` — resolve a ref to a JS value.
    fn get(&self, reference: u32) -> Option<&JsValue> {
        self.slots
            .get(reference as usize)
            .and_then(|slot| slot.as_ref())
    }

    /// Mutable `Np(ref)` for host-side mutation (e.g. `Map.set`).
    fn get_mut(&mut self, reference: u32) -> Option<&mut JsValue> {
        self.slots
            .get_mut(reference as usize)
            .and_then(|slot| slot.as_mut())
    }
}

/// Host state stored inside the wasmi store.
struct QoderWasmHost {
    refs: RefTable,
    memory: Option<Memory>,
}

/// A handle to a `QoderContext` instance in wasm linear memory.
#[derive(Clone, Copy)]
pub(crate) struct QoderContextPtr(pub(crate) u32);

/// The result of `prepareRequest`/`prepareInferRequest`.
pub(crate) struct PreparedRequest {
    pub(crate) url: String,
    /// Header map serialized as JSON (the wasm `headers` string is
    /// `{name: value, ...}`).
    pub(crate) headers_json: String,
    /// Optional request body (absent when the signature only produced headers).
    pub(crate) body: Option<Vec<u8>>,
}

/// Compile-time proof that `QoderWasm` (and its host state) is `Send`, which
/// tauri commands require when an async command future crosses threads. A
/// regression to `Rc`/`RefCell` here breaks the `bitfun-desktop` build, so the
/// bound is asserted at compile time.
const _: () = {
    fn assert_send<T: Send>() {}
    fn _proof() {
        assert_send::<QoderWasm>();
    }
};

impl PreparedRequest {
    /// Parses the headers JSON into a `(name, value)` list.
    pub(crate) fn headers(&self) -> Result<Vec<(String, String)>> {
        if self.headers_json.trim().is_empty() {
            return Ok(Vec::new());
        }
        let map: HashMap<String, String> = serde_json::from_str(&self.headers_json)
            .context("parse qoder wasm prepared request headers")?;
        Ok(map.into_iter().collect())
    }
}

/// The embedded wasm engine plus a store. A single instance is created once
/// per signing operation so concurrent signers do not share linear memory.
pub(crate) struct QoderWasm {
    store: Store<QoderWasmHost>,
    qodercontext_new: TypedFunc<(i32, i32, i32, i32, i32, i32, i32, i32, i32), ()>,
    qodercontext_free: TypedFunc<(i32, i32), ()>,
    qodercontext_prepare_request: TypedFunc<
        (
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
            i32,
        ),
        (),
    >,
    qodercontext_prepare_infer_request:
        TypedFunc<(i32, i32, i32, i32, i32, i32, i32, i32, i32, i32), ()>,
    qodercontext_refresh_auth_fields: TypedFunc<(i32, i32, i32, i32), ()>,
    requestresult_free: TypedFunc<(i32, i32), ()>,
    requestresult_url: TypedFunc<(i32, i32), ()>,
    requestresult_body: TypedFunc<(i32, i32), ()>,
    requestresult_headers: TypedFunc<(i32,), i32>,
    requestresult_header_count: TypedFunc<(i32,), i32>,
    decrypt_server_response: TypedFunc<(i32, i32, i32), ()>,
    model_cache_decrypt: TypedFunc<(i32, i32, i32, i32, i32), ()>,
    generate_runtime_auth_fields: TypedFunc<(i32, i32, i32), ()>,
    add_to_stack_pointer: TypedFunc<(i32,), i32>,
    /// `__wbindgen_export2` — malloc for UTF-8 string buffers.
    malloc: TypedFunc<(i32, i32), i32>,
    /// `__wbindgen_export4` — free (ptr, len, align).
    free_mem: TypedFunc<(i32, i32, i32), ()>,
    memory: Memory,
}

impl QoderWasm {
    /// Instantiates the embedded wasm with the wasm-bindgen host glue.
    pub(crate) fn instantiate() -> Result<Self> {
        // Fuel metering guards against a host-glue bug looping forever in the
        // interpreter; signing calls are short, a generous cap is fine.
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let host = QoderWasmHost {
            refs: RefTable::new(),
            memory: None,
        };
        let mut store = Store::new(&engine, host);
        store
            .add_fuel(2_000_000_000)
            .map_err(|error| anyhow!("seed qoder wasm fuel: {error:?}"))?;
        let module = Module::new(&engine, QODER_AUTH_WASM)
            .context("parse embedded qoder auth wasm module")?;
        let mut linker = <Linker<QoderWasmHost>>::new(&engine);
        register_host_imports(&mut linker)?;
        let instance = linker
            .instantiate(&mut store, &module)
            .and_then(|instance| instance.start(&mut store))
            .context("instantiate embedded qoder auth wasm module")?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .context("qoder wasm memory export")?;
        store.data_mut().memory = Some(memory);

        let qodercontext_new = instance
            .get_typed_func::<(i32, i32, i32, i32, i32, i32, i32, i32, i32), ()>(
                &mut store,
                "qodercontext_new",
            )
            .context("qoder wasm export qodercontext_new")?;
        let qodercontext_free = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "__wbg_qodercontext_free")
            .context("qoder wasm export __wbg_qodercontext_free")?;
        let qodercontext_prepare_request = instance
            .get_typed_func::<(
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
                i32,
            ), ()>(&mut store, "qodercontext_prepareRequest")
            .context("qoder wasm export qodercontext_prepareRequest")?;
        let qodercontext_prepare_infer_request = instance
            .get_typed_func::<(i32, i32, i32, i32, i32, i32, i32, i32, i32, i32), ()>(
                &mut store,
                "qodercontext_prepareInferRequest",
            )
            .context("qoder wasm export qodercontext_prepareInferRequest")?;
        let qodercontext_refresh_auth_fields = instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(
                &mut store,
                "qodercontext_refreshAuthFields",
            )
            .context("qoder wasm export qodercontext_refreshAuthFields")?;
        let requestresult_free = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "__wbg_requestresult_free")
            .context("qoder wasm export __wbg_requestresult_free")?;
        let requestresult_url = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "requestresult_url")
            .context("qoder wasm export requestresult_url")?;
        let requestresult_body = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "requestresult_body")
            .context("qoder wasm export requestresult_body")?;
        let requestresult_headers = instance
            .get_typed_func::<(i32,), i32>(&mut store, "requestresult_headers")
            .context("qoder wasm export requestresult_headers")?;
        let requestresult_header_count = instance
            .get_typed_func::<(i32,), i32>(&mut store, "requestresult_headerCount")
            .context("qoder wasm export requestresult_headerCount")?;
        let decrypt_server_response = instance
            .get_typed_func::<(i32, i32, i32), ()>(&mut store, "decrypt_server_response")
            .context("qoder wasm export decrypt_server_response")?;
        let model_cache_decrypt = instance
            .get_typed_func::<(i32, i32, i32, i32, i32), ()>(&mut store, "model_cache_decrypt")
            .context("qoder wasm export model_cache_decrypt")?;
        let generate_runtime_auth_fields = instance
            .get_typed_func::<(i32, i32, i32), ()>(&mut store, "generate_runtime_auth_fields")
            .context("qoder wasm export generate_runtime_auth_fields")?;
        let add_to_stack_pointer = instance
            .get_typed_func::<(i32,), i32>(&mut store, "__wbindgen_add_to_stack_pointer")
            .context("qoder wasm export __wbindgen_add_to_stack_pointer")?;
        let malloc = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "__wbindgen_export2")
            .context("qoder wasm export __wbindgen_export2")?;
        let free_mem = instance
            .get_typed_func::<(i32, i32, i32), ()>(&mut store, "__wbindgen_export4")
            .context("qoder wasm export __wbindgen_export4")?;

        Ok(Self {
            store,
            qodercontext_new,
            qodercontext_free,
            qodercontext_prepare_request,
            qodercontext_prepare_infer_request,
            qodercontext_refresh_auth_fields,
            requestresult_free,
            requestresult_url,
            requestresult_body,
            requestresult_headers,
            requestresult_header_count,
            decrypt_server_response,
            model_cache_decrypt,
            generate_runtime_auth_fields,
            add_to_stack_pointer,
            malloc,
            free_mem,
            memory,
        })
    }

    /// Allocates a UTF-8 string in wasm linear memory and returns `(ptr, len)`.
    fn write_string(&mut self, text: &str) -> Result<(i32, i32)> {
        let bytes = text.as_bytes();
        let len = bytes.len() as i32;
        let ptr = self
            .malloc
            .call(&mut self.store, (len, 1))
            .context("qoder wasm malloc for string")?;
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .map_err(|error| anyhow!("qoder wasm write string bytes: {error:?}"))?;
        Ok((ptr, len))
    }

    /// Reads a UTF-8 string from wasm linear memory and frees the buffer the
    /// way the glue does (`__wbindgen_export4(ptr, len, 1)`).
    fn read_string(&mut self, ptr: i32, len: i32) -> Result<String> {
        let mut buf = vec![0u8; len as usize];
        self.memory
            .read(&self.store, ptr as usize, &mut buf)
            .map_err(|error| anyhow!("qoder wasm read string at {ptr} len {len}: {error:?}"))?;
        let text = String::from_utf8(buf).context("qoder wasm returned non-UTF-8 string")?;
        self.free_mem
            .call(&mut self.store, (ptr, len, 1))
            .context("qoder wasm free string buffer")?;
        Ok(text)
    }

    /// Reads the result of a `Result<String, JsValue>` ABI: `(ptr, len, err, is_err)`.
    fn read_string_result(&mut self, ptr: i32, len: i32, err: i32, is_err: i32) -> Result<String> {
        if is_err != 0 {
            let error = self.read_js_error(err)?;
            return Err(anyhow!("qoder wasm returned error: {error}"));
        }
        self.read_string(ptr, len)
    }

    /// `HD(err_ref)` — resolve a JS Error object to its message.
    fn read_js_error(&mut self, reference: i32) -> Result<String> {
        let reference = reference as u32;
        let value = self
            .store
            .data()
            .refs
            .get(reference)
            .cloned()
            .ok_or_else(|| anyhow!("qoder wasm error reference {reference} missing"))?;
        let text = match value {
            JsValue::Str(text) => text,
            _ => "qoder wasm js error".to_string(),
        };
        self.store.data_mut().refs.free(reference);
        Ok(text)
    }

    /// Reads an i32 from the stack-pointer scratch region.
    fn read_stack_i32(&mut self, base: usize, offset: usize) -> Result<i32> {
        let mut buf = [0u8; 4];
        self.memory
            .read(&self.store, base + offset, &mut buf)
            .map_err(|error| anyhow!("qoder wasm read stack offset {offset}: {error:?}"))?;
        Ok(i32::from_le_bytes(buf))
    }

    /// Creates a `QoderContext` in wasm memory.
    pub(crate) fn context_new(
        &mut self,
        machine_id: &str,
        cosy_version: &str,
        user_info_json: &str,
        client_metadata_json: &str,
    ) -> Result<QoderContextPtr> {
        let (a_ptr, a_len) = self.write_string(machine_id)?;
        let (b_ptr, b_len) = self.write_string(cosy_version)?;
        let (c_ptr, c_len) = self.write_string(user_info_json)?;
        let (d_ptr, d_len) = self.write_string(client_metadata_json)?;

        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.qodercontext_new
            .call(
                &mut self.store,
                (
                    stack, a_ptr, a_len, b_ptr, b_len, c_ptr, c_len, d_ptr, d_len,
                ),
            )
            .context("qoder wasm qodercontext_new")?;
        let ptr = self.read_stack_i32(stack as usize, 0)?;
        let err = self.read_stack_i32(stack as usize, 4)?;
        let is_err = self.read_stack_i32(stack as usize, 8)?;
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;
        if is_err != 0 {
            let message = self.read_js_error(err)?;
            return Err(anyhow!("qoder wasm qodercontext_new failed: {message}"));
        }
        Ok(QoderContextPtr(ptr as u32))
    }

    /// `prepareRequest(endpoint, path, method, auth_mode, body?, headers?)`.
    pub(crate) fn prepare_request(
        &mut self,
        ctx: QoderContextPtr,
        endpoint: &str,
        path: &str,
        method: &str,
        auth_mode: &str,
        body: Option<&str>,
        headers: Option<&str>,
    ) -> Result<PreparedRequest> {
        let (e_ptr, e_len) = self.write_string(endpoint)?;
        let (p_ptr, p_len) = self.write_string(path)?;
        let (m_ptr, m_len) = self.write_string(method)?;
        let (a_ptr, a_len) = self.write_string(auth_mode)?;
        let (b_ptr, b_len) = match body {
            Some(text) => self.write_string(text)?,
            None => (0, 0),
        };
        let (h_ptr, h_len) = match headers {
            Some(text) => self.write_string(text)?,
            None => (0, 0),
        };

        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.qodercontext_prepare_request
            .call(
                &mut self.store,
                (
                    stack,
                    ctx.0 as i32,
                    e_ptr,
                    e_len,
                    p_ptr,
                    p_len,
                    m_ptr,
                    m_len,
                    a_ptr,
                    a_len,
                    b_ptr,
                    b_len,
                    h_ptr,
                    h_len,
                ),
            )
            .context("qoder wasm prepareRequest")?;
        let ptr = self.read_stack_i32(stack as usize, 0)?;
        let err = self.read_stack_i32(stack as usize, 4)?;
        let is_err = self.read_stack_i32(stack as usize, 8)?;
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;
        if is_err != 0 {
            let message = self.read_js_error(err)?;
            return Err(anyhow!("qoder prepareRequest failed: {message}"));
        }
        self.read_request_result(ptr as u32)
    }

    /// `prepareInferRequest(endpoint, path_or_body, body?, headers?)`.
    pub(crate) fn prepare_infer_request(
        &mut self,
        ctx: QoderContextPtr,
        endpoint: &str,
        path_or_body: &str,
        body: Option<&str>,
        headers: Option<&str>,
    ) -> Result<PreparedRequest> {
        let (e_ptr, e_len) = self.write_string(endpoint)?;
        let (p_ptr, p_len) = self.write_string(path_or_body)?;
        let (b_ptr, b_len) = match body {
            Some(text) => self.write_string(text)?,
            None => (0, 0),
        };
        let (h_ptr, h_len) = match headers {
            Some(text) => self.write_string(text)?,
            None => (0, 0),
        };

        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.qodercontext_prepare_infer_request
            .call(
                &mut self.store,
                (
                    stack,
                    ctx.0 as i32,
                    e_ptr,
                    e_len,
                    p_ptr,
                    p_len,
                    b_ptr,
                    b_len,
                    h_ptr,
                    h_len,
                ),
            )
            .context("qoder wasm prepareInferRequest")?;
        let ptr = self.read_stack_i32(stack as usize, 0)?;
        let err = self.read_stack_i32(stack as usize, 4)?;
        let is_err = self.read_stack_i32(stack as usize, 8)?;
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;
        if is_err != 0 {
            let message = self.read_js_error(err)?;
            return Err(anyhow!("qoder prepareInferRequest failed: {message}"));
        }
        self.read_request_result(ptr as u32)
    }

    /// Reads the `RequestResult` struct: url string, body bytes, headers JSON,
    /// and frees the struct and its string buffers.
    fn read_request_result(&mut self, ptr: u32) -> Result<PreparedRequest> {
        // url getter
        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.requestresult_url
            .call(&mut self.store, (stack, ptr as i32))
            .context("qoder wasm requestresult_url")?;
        let url_ptr = self.read_stack_i32(stack as usize, 0)?;
        let url_len = self.read_stack_i32(stack as usize, 4)?;
        let url = self.read_string(url_ptr, url_len)?;
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;

        // body getter
        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.requestresult_body
            .call(&mut self.store, (stack, ptr as i32))
            .context("qoder wasm requestresult_body")?;
        let body_ptr = self.read_stack_i32(stack as usize, 0)?;
        let body_len = self.read_stack_i32(stack as usize, 4)?;
        let body = if body_ptr != 0 {
            let mut bytes = vec![0u8; body_len as usize];
            self.memory
                .read(&self.store, body_ptr as usize, &mut bytes)
                .map_err(|error| anyhow!("qoder wasm read request body: {error:?}"))?;
            self.free_mem
                .call(&mut self.store, (body_ptr, body_len, 1))
                .context("qoder wasm free request body")?;
            Some(bytes)
        } else {
            None
        };
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;

        // headers getter — returns a JS object ref (JSON string wrapped in JS).
        let headers_ref = self
            .requestresult_headers
            .call(&mut self.store, (ptr as i32,))
            .context("qoder wasm requestresult_headers")?;
        let headers_json = if headers_ref != 0 {
            let value = self.store.data().refs.get(headers_ref as u32).cloned();
            self.store.data_mut().refs.free(headers_ref as u32);
            match value {
                Some(JsValue::Str(text)) => text,
                // The wasm returns a JS Map<string, string>; entries were
                // copied into the map by `__wbg_set` so they survive drop_ref.
                Some(JsValue::Map(map)) => serde_json::to_string(&map).unwrap_or_default(),
                _ => String::new(),
            }
        } else {
            String::new()
        };

        self.requestresult_free
            .call(&mut self.store, (ptr as i32, 0))
            .context("qoder wasm requestresult_free")?;

        Ok(PreparedRequest {
            url,
            headers_json,
            body,
        })
    }

    /// `decrypt_server_response(ciphertext) -> Result<String>`.
    pub(crate) fn decrypt_server_response(&mut self, ciphertext: &str) -> Result<String> {
        let (c_ptr, c_len) = self.write_string(ciphertext)?;
        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.decrypt_server_response
            .call(&mut self.store, (stack, c_ptr, c_len))
            .context("qoder wasm decrypt_server_response")?;
        let ptr = self.read_stack_i32(stack as usize, 0)?;
        let len = self.read_stack_i32(stack as usize, 4)?;
        let err = self.read_stack_i32(stack as usize, 8)?;
        let is_err = self.read_stack_i32(stack as usize, 12)?;
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;
        self.read_string_result(ptr, len, err, is_err)
    }

    /// `model_cache_decrypt(ciphertext, key) -> Result<String>`.
    pub(crate) fn model_cache_decrypt(&mut self, ciphertext: &str, key: &str) -> Result<String> {
        let (c_ptr, c_len) = self.write_string(ciphertext)?;
        let (k_ptr, k_len) = self.write_string(key)?;
        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.model_cache_decrypt
            .call(&mut self.store, (stack, c_ptr, c_len, k_ptr, k_len))
            .context("qoder wasm model_cache_decrypt")?;
        let ptr = self.read_stack_i32(stack as usize, 0)?;
        let len = self.read_stack_i32(stack as usize, 4)?;
        let err = self.read_stack_i32(stack as usize, 8)?;
        let is_err = self.read_stack_i32(stack as usize, 12)?;
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;
        self.read_string_result(ptr, len, err, is_err)
    }

    /// `generate_runtime_auth_fields(user_json) -> Result<String>`.
    ///
    /// Generates the `encrypt_user_info` + `key` pair the QoderContext needs
    /// for real gateway signatures. Input mirrors the CLI's
    /// `regenerateRuntimeFields`: `{uid, organization_id, organization_tags,
    /// data_policy_agreed}`.
    pub(crate) fn generate_runtime_auth_fields(&mut self, user_json: &str) -> Result<String> {
        let (u_ptr, u_len) = self.write_string(user_json)?;
        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.generate_runtime_auth_fields
            .call(&mut self.store, (stack, u_ptr, u_len))
            .context("qoder wasm generate_runtime_auth_fields")?;
        let ptr = self.read_stack_i32(stack as usize, 0)?;
        let len = self.read_stack_i32(stack as usize, 4)?;
        let err = self.read_stack_i32(stack as usize, 8)?;
        let is_err = self.read_stack_i32(stack as usize, 12)?;
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;
        self.read_string_result(ptr, len, err, is_err)
    }

    /// `qodercontext_refreshAuthFields(uid)`.
    pub(crate) fn refresh_auth_fields(&mut self, ctx: QoderContextPtr, uid: &str) -> Result<()> {
        let (u_ptr, u_len) = self.write_string(uid)?;
        let stack = self
            .add_to_stack_pointer
            .call(&mut self.store, (-16,))
            .context("qoder wasm stack pointer reserve")?;
        self.qodercontext_refresh_auth_fields
            .call(&mut self.store, (stack, ctx.0 as i32, u_ptr, u_len))
            .context("qoder wasm refreshAuthFields")?;
        let err = self.read_stack_i32(stack as usize, 0)?;
        let is_err = self.read_stack_i32(stack as usize, 4)?;
        self.add_to_stack_pointer
            .call(&mut self.store, (16,))
            .context("qoder wasm stack pointer restore")?;
        if is_err != 0 {
            let message = self.read_js_error(err)?;
            return Err(anyhow!("qoder refreshAuthFields failed: {message}"));
        }
        Ok(())
    }
}

/// Registers the 31 wasm-bindgen host imports.
fn register_host_imports(linker: &mut Linker<QoderWasmHost>) -> Result<()> {
    const WASM_BINDGEN_JS: &str = "./qoder_auth_wasm_bg.js";

    macro_rules! host_func {
        ($name:literal, $f:expr) => {
            linker
                .func_wrap(WASM_BINDGEN_JS, $name, $f)
                .with_context(|| format!("register qoder wasm import {}", $name))?;
        };
    }

    // Object reference table management.
    host_func!(
        "__wbindgen_object_drop_ref",
        |mut caller: Caller<'_, QoderWasmHost>, reference: i32| {
            caller.data_mut().refs.free(reference as u32);
        }
    );
    host_func!("__wbindgen_object_clone_ref", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                               reference: i32|
     -> i32 {
        // Clone keeps the same object alive: alloc a new ref pointing at the
        // same underlying JS value.
        let value = caller.data().refs.get(reference as u32).cloned();
        let value = value.unwrap_or(JsValue::Undefined);
        caller.data_mut().refs.alloc(value) as i32
    });
    host_func!(
        "__wbg___wbindgen_is_object_40c5a80572e8f9d3",
        |caller: Caller<'_, QoderWasmHost>, reference: i32| -> i32 {
            matches!(
                caller.data().refs.get(reference as u32),
                Some(JsValue::Object) | Some(JsValue::Map(_)) | Some(JsValue::U8Array(_))
            ) as i32
        }
    );
    host_func!(
        "__wbg___wbindgen_is_string_b29b5c5a8065ba1a",
        |caller: Caller<'_, QoderWasmHost>, reference: i32| -> i32 {
            matches!(
                caller.data().refs.get(reference as u32),
                Some(JsValue::Str(_))
            ) as i32
        }
    );
    host_func!(
        "__wbg___wbindgen_is_function_49868bde5eb1e745",
        |caller: Caller<'_, QoderWasmHost>, reference: i32| -> i32 {
            matches!(
                caller.data().refs.get(reference as u32),
                Some(JsValue::Function)
            ) as i32
        }
    );
    host_func!(
        "__wbg___wbindgen_is_undefined_c0cca72b82b86f4d",
        |caller: Caller<'_, QoderWasmHost>, reference: i32| -> i32 {
            matches!(
                caller.data().refs.get(reference as u32),
                Some(JsValue::Undefined)
            ) as i32
        }
    );

    // Static accessors returning object refs.
    host_func!(
        "__wbg_static_accessor_GLOBAL_THIS_a1248013d790bf5f",
        |mut caller: Caller<'_, QoderWasmHost>| -> i32 {
            caller.data_mut().refs.alloc(JsValue::Object) as i32
        }
    );
    host_func!(
        "__wbg_static_accessor_SELF_24f78b6d23f286ea",
        |mut caller: Caller<'_, QoderWasmHost>| -> i32 {
            caller.data_mut().refs.alloc(JsValue::Object) as i32
        }
    );
    host_func!(
        "__wbg_static_accessor_GLOBAL_f2e0f995a21329ff",
        |mut caller: Caller<'_, QoderWasmHost>| -> i32 {
            caller.data_mut().refs.alloc(JsValue::Object) as i32
        }
    );
    host_func!(
        "__wbg_static_accessor_WINDOW_59fd959c540fe405",
        |mut caller: Caller<'_, QoderWasmHost>| -> i32 {
            caller.data_mut().refs.alloc(JsValue::Object) as i32
        }
    );

    // Object property accessors (crypto/process/versions/node/msCrypto).
    host_func!("__wbg_crypto_38df2bab126b63dc", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                 _reference: i32|
     -> i32 {
        caller.data_mut().refs.alloc(JsValue::Object) as i32
    });
    host_func!("__wbg_process_44c7a14e11e9f69e", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                  _reference: i32|
     -> i32 {
        caller.data_mut().refs.alloc(JsValue::Object) as i32
    });
    host_func!("__wbg_versions_276b2795b1c6a219", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                   _reference: i32|
     -> i32 {
        caller.data_mut().refs.alloc(JsValue::Object) as i32
    });
    host_func!("__wbg_node_84ea875411254db1", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                               _reference: i32|
     -> i32 {
        caller.data_mut().refs.alloc(JsValue::Object) as i32
    });
    host_func!("__wbg_msCrypto_bd5a034af96bcba6", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                   _reference: i32|
     -> i32 {
        caller.data_mut().refs.alloc(JsValue::Object) as i32
    });

    // module.require — returns undefined in a non-Node host.
    host_func!("__wbg_require_b4edbdcf3e2a1ef0", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >|
     -> i32 {
        caller.data_mut().refs.alloc(JsValue::Undefined) as i32
    });

    // Date.now()
    host_func!("__wbg_now_88621c9c9a4f3ffc", |_caller: Caller<
        '_,
        QoderWasmHost,
    >|
     -> wasmi::core::F64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as f64)
            .unwrap_or(0.0);
        wasmi::core::F64::from(now)
    });

    // new Map()
    host_func!("__wbg_new_99cabae501c0a8a0", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >|
     -> i32 {
        caller.data_mut().refs.alloc(JsValue::Map(HashMap::new())) as i32
    });
    // map.set(k, v) -> Map (glue: `Em(Np(A).set(Np(e), Np(t)))` — a NEW ref).
    // Key/value string refs are resolved and copied into the map so later
    // `__wbindgen_object_drop_ref` calls cannot invalidate stored entries.
    host_func!("__wbg_set_08463b1df38a7e29", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                              map_ref: i32,
                                              key_ref: i32,
                                              value_ref: i32|
     -> i32 {
        let key = caller.js_string(key_ref);
        let value = caller.js_string(value_ref);
        let new_map = match caller.data_mut().refs.get_mut(map_ref as u32) {
            Some(JsValue::Map(map)) => {
                map.insert(key.clone(), value.clone());
                JsValue::Map(map.clone())
            }
            _ => {
                let mut map = HashMap::new();
                map.insert(key, value);
                JsValue::Map(map)
            }
        };
        caller.data_mut().refs.alloc(new_map) as i32
    });

    // new Uint8Array(len) — host-backed buffer.
    host_func!(
        "__wbg_new_with_length_9cedd08484b73942",
        |mut caller: Caller<'_, QoderWasmHost>, len: i32| -> i32 {
            caller
                .data_mut()
                .refs
                .alloc(JsValue::U8Array(U8ArrayData::HostBuf(Arc::new(
                    Mutex::new(vec![0u8; len as usize]),
                )))) as i32
        }
    );
    // obj.length
    host_func!("__wbg_length_0c32cb8543c8e4c8", |caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                 reference: i32|
     -> i32 {
        match caller.data().refs.get(reference as u32) {
            Some(JsValue::U8Array(U8ArrayData::MemView { len, .. })) => *len as i32,
            Some(JsValue::U8Array(U8ArrayData::HostBuf(buffer))) => {
                buffer.lock().map(|data| data.len() as i32).unwrap_or(0)
            }
            Some(JsValue::Str(text)) => text.len() as i32,
            _ => 0,
        }
    });
    // arr.subarray(begin, end) -> Uint8Array (shares the same host buffer view)
    host_func!("__wbg_subarray_0f98d3fb634508ad", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                   reference: i32,
                                                   begin: i32,
                                                   end: i32|
     -> i32 {
        let begin_u = begin as u32;
        let len = (end - begin).max(0) as u32;
        let value = match caller.data().refs.get(reference as u32) {
            Some(JsValue::U8Array(U8ArrayData::HostBuf(buffer))) => {
                let slice = {
                    let data = buffer
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let b = (begin as usize).min(data.len());
                    let e = (end as usize).min(data.len());
                    data[b..e].to_vec()
                };
                JsValue::U8Array(U8ArrayData::HostBuf(Arc::new(Mutex::new(slice))))
            }
            Some(JsValue::U8Array(U8ArrayData::MemView { ptr, .. })) => {
                JsValue::U8Array(U8ArrayData::MemView {
                    ptr: ptr.saturating_add(begin_u),
                    len,
                })
            }
            _ => JsValue::U8Array(U8ArrayData::MemView { ptr: 0, len: 0 }),
        };
        caller.data_mut().refs.alloc(value) as i32
    });
    // Uint8Array.prototype.set.call(dest, src) — copy src bytes into dest.
    // The glue calls this as `set.call(yCA(A, e), Np(t))`: dest is a memory
    // view (ptr, len) passed positionally as two i32s, src is a ref.
    host_func!(
        "__wbg_prototypesetcall_3e05eb9545565046",
        |mut caller: Caller<'_, QoderWasmHost>, dest_ptr: i32, _dest_len: i32, src_ref: i32| {
            let bytes = match caller.data().refs.get(src_ref as u32) {
                Some(JsValue::U8Array(U8ArrayData::HostBuf(buffer))) => {
                    buffer.lock().map(|data| data.clone()).unwrap_or_default()
                }
                Some(JsValue::U8Array(U8ArrayData::MemView { ptr, len })) => caller
                    .data()
                    .memory
                    .map(|memory| memory.data(&caller))
                    .map(|data| data[*ptr as usize..(*ptr + *len) as usize].to_vec())
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            if let Some(memory) = caller.data().memory {
                let _ = memory.write(&mut caller, dest_ptr as usize, &bytes);
            }
        }
    );

    // crypto.getRandomValues(bytes) — (ptr, len) variant writes wasm memory.
    host_func!(
        "__wbg_getRandomValues_d49329ff89a07af1",
        |caller: Caller<'_, QoderWasmHost>, ptr: i32, len: i32| {
            fill_random_memory(caller, ptr as usize, len as usize);
        }
    );
    // crypto.getRandomValues(arr) — Uint8Array ref variant fills host buffer.
    host_func!(
        "__wbg_getRandomValues_c44a50d8cfdaebeb",
        |caller: Caller<'_, QoderWasmHost>, _reference: i32, arr_ref: i32| {
            match caller.data().refs.get(arr_ref as u32) {
                Some(JsValue::U8Array(U8ArrayData::HostBuf(buffer))) => fill_random_buffer(buffer),
                Some(JsValue::U8Array(U8ArrayData::MemView { ptr, len })) => {
                    let (ptr, len) = (*ptr, *len);
                    fill_random_memory(caller, ptr as usize, len as usize);
                }
                _ => {}
            }
        }
    );
    // node crypto.randomFillSync(bytes) — Uint8Array ref, fill host buffer.
    host_func!(
        "__wbg_randomFillSync_6c25eac9869eb53c",
        |caller: Caller<'_, QoderWasmHost>, _reference: i32, arr_ref: i32| {
            match caller.data().refs.get(arr_ref as u32) {
                Some(JsValue::U8Array(U8ArrayData::HostBuf(buffer))) => fill_random_buffer(buffer),
                Some(JsValue::U8Array(U8ArrayData::MemView { ptr, len })) => {
                    let (ptr, len) = (*ptr, *len);
                    fill_random_memory(caller, ptr as usize, len as usize);
                }
                _ => {}
            }
        }
    );

    // fn.call(thisArg, arg) -> JS value
    host_func!("__wbg_call_d578befcc3145dee", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                               _fn_ref: i32,
                                               _this_ref: i32,
                                               _arg_ref: i32|
     -> i32 {
        caller.data_mut().refs.alloc(JsValue::Undefined) as i32
    });

    // new Error(message_ptr, message_len) -> Error ref
    host_func!("__wbg_Error_2e59b1b37a9a34c3", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                ptr: i32,
                                                len: i32|
     -> i32 {
        let message = caller
            .data()
            .memory
            .map(|memory| memory.data(&caller))
            .map(|data| {
                String::from_utf8_lossy(&data[ptr as usize..(ptr + len) as usize]).to_string()
            })
            .unwrap_or_default();
        caller.data_mut().refs.alloc(JsValue::Str(message)) as i32
    });

    // throw new Error(message) — traps the interpreter with a host error.
    host_func!(
        "__wbg___wbindgen_throw_81fc77679af83bc6",
        |caller: Caller<'_, QoderWasmHost>, ptr: i32, len: i32| -> () {
            let message = caller
                .data()
                .memory
                .map(|memory| memory.data(&caller))
                .map(|data| {
                    String::from_utf8_lossy(&data[ptr as usize..(ptr + len) as usize]).to_string()
                })
                .unwrap_or_default();
            panic!("qoder wasm host trap: {message}");
        }
    );

    // __wbindgen_export(exception_ref) — the CLI's `c1A` wrapper signals a JS
    // exception to the wasm side. Surface it loudly.
    host_func!("__wbindgen_export", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                     exception_ref: i32|
     -> () {
        let message = caller
            .data()
            .refs
            .get(exception_ref as u32)
            .cloned()
            .map(|value| match value {
                JsValue::Str(text) => text,
                _ => "js exception".to_string(),
            })
            .unwrap_or_else(|| "js exception".to_string());
        caller.data_mut().refs.free(exception_ref as u32);
        panic!("qoder wasm js exception: {message}");
    });

    // wasm-bindgen casts: (ptr,len) -> Uint8Array / String
    host_func!("__wbindgen_cast_0000000000000001", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                    ptr: i32,
                                                    len: i32|
     -> i32 {
        caller
            .data_mut()
            .refs
            .alloc(JsValue::U8Array(U8ArrayData::MemView {
                ptr: ptr as u32,
                len: len as u32,
            })) as i32
    });
    host_func!("__wbindgen_cast_0000000000000002", |mut caller: Caller<
        '_,
        QoderWasmHost,
    >,
                                                    ptr: i32,
                                                    len: i32|
     -> i32 {
        let text = caller
            .data()
            .memory
            .map(|memory| memory.data(&caller))
            .map(|data| {
                String::from_utf8_lossy(&data[ptr as usize..(ptr + len) as usize]).to_string()
            })
            .unwrap_or_default();
        caller.data_mut().refs.alloc(JsValue::Str(text)) as i32
    });

    Ok(())
}

/// Extension helpers for a wasmi `Caller` over the Qoder host state.
trait QoderCaller {
    fn js_string(&self, reference: i32) -> String;
}

impl QoderCaller for Caller<'_, QoderWasmHost> {
    fn js_string(&self, reference: i32) -> String {
        match self.data().refs.get(reference as u32) {
            Some(JsValue::Str(text)) => text.clone(),
            Some(JsValue::Bool(value)) => value.to_string(),
            _ => String::new(),
        }
    }
}

/// Fills `len` bytes at `ptr` in wasm linear memory with CSPRNG data.
fn fill_random_memory(mut caller: Caller<'_, QoderWasmHost>, ptr: usize, len: usize) {
    let mut buf = vec![0u8; len];
    if let Err(error) = getrandom::getrandom(&mut buf) {
        panic!("qoder wasm getrandom failed: {error}");
    }
    if let Some(memory) = caller.data().memory {
        if let Err(error) = memory.write(&mut caller, ptr, &buf) {
            panic!("qoder wasm write random bytes failed: {error}");
        }
    }
}

/// Fills a host-backed Uint8Array buffer with CSPRNG data.
fn fill_random_buffer(buffer: &Arc<Mutex<Vec<u8>>>) {
    let mut data = buffer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Err(error) = getrandom::getrandom(&mut data) {
        panic!("qoder wasm getrandom failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_wasm_has_expected_size() {
        assert_eq!(QODER_AUTH_WASM.len(), 297_238);
    }

    #[test]
    fn embedded_wasm_starts_with_wasm_magic() {
        assert_eq!(
            &QODER_AUTH_WASM[..8],
            &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn decrypt_without_context_errors_quickly() {
        let mut runtime = QoderWasm::instantiate().expect("wasm instantiates");
        // 诊断：纯字符串函数是否死循环（不构造 context）
        let decrypted = runtime
            .decrypt_server_response("not-valid-ciphertext")
            .expect_err("invalid ciphertext must error, not loop");
        assert!(decrypted.to_string().contains("error"));
    }

    /// Decrypts the CLI's on-disk model catalog (`model_cache_decrypt`) using
    /// the uid as the key — validates the cache path (侦察 §九 方案 B) works
    /// with the embedded wasm, and that the catalog JSON contains real models.
    #[test]
    fn decrypt_cli_model_cache_catalog() {
        let cache_path = std::env::var("BITFUN_QODER_CACHE").unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|home| {
                    home.join(".qoder-cn")
                        .join(".models")
                        .join("019f1805-ee0a-7b5c-b319-5804d853c6b0")
                        .join("catalog-v6")
                })
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        let Ok(ciphertext) = std::fs::read_to_string(&cache_path) else {
            eprintln!("SKIP: qoder model cache not present at {cache_path:?}");
            return;
        };
        let mut runtime = QoderWasm::instantiate().expect("wasm instantiates");
        let decrypted = runtime
            .model_cache_decrypt(&ciphertext, "019f1805-ee0a-7b5c-b319-5804d853c6b0")
            .expect("model cache decrypt");
        eprintln!("decrypted head: {}", &decrypted[..decrypted.len().min(300)]);
        let parsed: serde_json::Value = serde_json::from_str(&decrypted).expect("cache JSON");
        // The catalog is keyed by scene ("chat"); the models array lives there.
        let models = parsed
            .get("chat")
            .or_else(|| parsed.get("models"))
            .or_else(|| parsed.get("data"))
            .or_else(|| parsed.as_array().map(|_| &parsed));
        assert!(models.is_some(), "cache contains a model list");
        let count = match models {
            Some(serde_json::Value::Array(items)) => items.len(),
            _ => 0,
        };
        eprintln!("model cache decrypted OK, entries: {count}");
        assert!(count > 0, "decrypted cache contains models");
    }

    #[test]
    fn instantiate_wasm_with_host_imports() {
        let mut runtime = QoderWasm::instantiate().expect("wasm instantiates");
        let ctx = runtime
            .context_new(
                "machine-id-test",
                "1.1.23",
                r#"{"uid":"u-test","encrypt_user_info":"","key":""}"#,
                r#"{"client_type":5}"#,
            )
            .expect("qodercontext_new");
        let prepared = runtime
            .prepare_request(
                ctx,
                "https://gateway.qoder.com.cn",
                "/api/v2/model/list?Encode=1",
                "GET",
                "auth",
                None,
                None,
            )
            .expect("prepareRequest");
        assert!(prepared.url.starts_with("https://gateway.qoder.com.cn"));
        assert!(
            !prepared.headers_json.is_empty(),
            "signature headers present"
        );
        let headers = prepared.headers().expect("headers parse");
        let names: Vec<&str> = headers.iter().map(|(name, _)| name.as_str()).collect();
        assert!(
            names
                .iter()
                .any(|name| name.contains("Cosy-MachineId") || name.contains("Cosy-MachineToken")),
            "signature includes Cosy machine headers, got {names:?}"
        );
    }

    /// Golden-baseline contract: the embedded wasm must produce the exact same
    /// signature headers as the official qoderclicn glue for identical input.
    /// The baseline (`resources/qoder_golden.json`) was generated with
    /// `resources/qoder_golden_gen.mjs` (node + replicated glue semantics read
    /// from the official bundle). Every header except the two volatile values
    /// (Cosy-Date, and the Authorization JWT whose payload carries a random
    /// requestId) must match byte-for-byte; the volatile pair is validated
    /// structurally instead.
    #[test]
    fn prepare_request_matches_golden_signature() {
        let golden = include_str!("../../resources/qoder_golden.json");
        let golden: serde_json::Value = serde_json::from_str(golden).expect("golden JSON parses");
        let golden_headers: HashMap<String, String> =
            serde_json::from_value(golden["request"]["headers"].clone())
                .expect("golden headers are a string map");

        let mut runtime = QoderWasm::instantiate().expect("wasm instantiates");
        let ctx = runtime
            .context_new(
                "ad05505d-0918-4a5d-bbf5-d7acf9abdd9d",
                "1.1.23",
                r#"{"uid":"test-uid-0001","encrypt_user_info":"enc-test","key":"key-test"}"#,
                r#"{"client_type":5}"#,
            )
            .expect("qodercontext_new");
        let prepared = runtime
            .prepare_request(
                ctx,
                "https://gateway.qoder.com.cn",
                "/api/v2/model/list?Encode=1",
                "GET",
                "auth",
                None,
                None,
            )
            .expect("prepareRequest");
        let headers: HashMap<String, String> = prepared
            .headers()
            .expect("headers parse")
            .into_iter()
            .collect();

        assert_eq!(prepared.url, golden["request"]["url"].as_str().unwrap());

        // Structural check for the Authorization JWT: `Bearer COSY.` + 3
        // base64 segments. Header must be `{"version":"v1","requestId":<uuid>,
        // "info":<encrypt_user_info>,"cosyVersion":"1.1.23","ideVersion":""}`.
        fn b64_decode(seg: &str) -> Vec<u8> {
            let mut s = seg.to_string();
            while !s.len().is_multiple_of(4) {
                s.push('=');
            }
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .unwrap_or_default()
        }
        let auth = headers.get("Authorization").expect("Authorization header");
        let rest = auth
            .strip_prefix("Bearer COSY.")
            .expect("COSY bearer prefix");
        // COSY bearer is `payload.signature` (two segments, no header).
        let segments: Vec<&str> = rest.split('.').collect();
        assert_eq!(segments.len(), 2, "COSY bearer has 2 segments: {auth}");
        let payload: serde_json::Value =
            serde_json::from_slice(&b64_decode(segments[0])).expect("JWT payload JSON");
        assert_eq!(payload["version"], "v1");
        assert_eq!(payload["info"], "enc-test");
        assert_eq!(payload["cosyVersion"], "1.1.23");
        assert_eq!(payload["ideVersion"], "");
        assert!(
            payload["requestId"]
                .as_str()
                .is_some_and(|id| id.len() == 36),
            "requestId is a UUID"
        );
        assert_eq!(segments[1].len(), 32, "signature is 32 hex chars");

        // Every other header must match byte-for-byte. Cosy-Date is a live
        // epoch-seconds timestamp (the golden file's copy ages out), so it is
        // validated against the current clock with a generous skew window.
        let mut mismatches = Vec::new();
        for (name, expected) in &golden_headers {
            if name == "Authorization" {
                continue;
            }
            let actual = headers.get(name);
            match actual {
                None => mismatches.push(format!("{name}: missing in Rust output")),
                Some(actual) if name == "Cosy-Date" => {
                    let a: i64 = actual.parse().unwrap_or(0);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    if (a - now).abs() > 300 {
                        mismatches.push(format!("{name}: not near current time ({a})"));
                    }
                }
                Some(actual) if actual != expected => {
                    mismatches.push(format!("{name}: {expected:?} != {actual:?}"));
                }
                Some(_) => {}
            }
        }
        for name in headers.keys() {
            if !golden_headers.contains_key(name) {
                mismatches.push(format!("{name}: extra header in Rust output"));
            }
        }
        assert!(
            mismatches.is_empty(),
            "golden signature mismatch:\n{}",
            mismatches.join("\n")
        );
    }

    /// Real-gateway PoC: with `BITFUN_QODER_PAT` set, exchange the PAT for a
    /// job token, sign the model-list request with the embedded wasm, fetch it,
    /// and decrypt the response. Succeeds only when the gateway accepts the
    /// wasm signature (200 + decrypted JSON, not a 503 ALB reject).
    #[tokio::test]
    async fn poc_real_gateway_model_list_with_wasm_signature() {
        let Ok(pat) = std::env::var("BITFUN_QODER_PAT") else {
            eprintln!("SKIP: BITFUN_QODER_PAT not set");
            return;
        };
        // 1. PAT -> job token (exchange)
        let client = reqwest::Client::new();
        let exchange: serde_json::Value = client
            .post("https://openapi.qoder.com.cn/api/v1/jobToken/exchange")
            .json(&serde_json::json!({ "personal_token": pat }))
            .send()
            .await
            .expect("exchange request")
            .error_for_status()
            .expect("exchange ok")
            .json()
            .await
            .expect("exchange json");
        let job_token = exchange
            .get("token")
            .and_then(|v| v.as_str())
            .expect("job token present")
            .to_string();
        // userinfo to get uid
        let userinfo: serde_json::Value = client
            .get("https://openapi.qoder.com.cn/api/v1/userinfo")
            .bearer_auth(&job_token)
            .send()
            .await
            .expect("userinfo request")
            .error_for_status()
            .expect("userinfo ok")
            .json()
            .await
            .expect("userinfo json");
        let uid = userinfo
            .get("id")
            .or_else(|| userinfo.get("user_id"))
            .and_then(|v| v.as_str())
            .expect("uid present")
            .to_string();

        // 2. QoderContext with runtime auth fields. The CLI's
        // `regenerateRuntimeFields` calls wasm `generate_runtime_auth_fields`
        // to derive `encrypt_user_info` + `key`; the QoderContext then signs
        // real gateway requests ("sign" auth mode).
        let machine_id = std::env::var("BITFUN_QODER_MACHINE_ID")
            .unwrap_or_else(|_| "poc-machine-id".to_string());
        let mut runtime = QoderWasm::instantiate().expect("wasm instantiates");
        let auth_fields: serde_json::Value = serde_json::from_str(
            &runtime
                .generate_runtime_auth_fields(
                    &serde_json::json!({
                        "uid": uid,
                        "organization_id": userinfo.get("organization_id").cloned().unwrap_or(serde_json::Value::Null),
                        "organization_tags": userinfo.get("organization_tags").cloned().unwrap_or(serde_json::Value::Null),
                        "data_policy_agreed": userinfo.get("data_policy_agreed").cloned().unwrap_or(serde_json::Value::Null),
                    })
                    .to_string(),
                )
                .expect("generate_runtime_auth_fields"),
        )
        .expect("auth fields JSON");
        let encrypt_user_info = auth_fields
            .get("encrypt_user_info")
            .and_then(|v| v.as_str())
            .expect("encrypt_user_info")
            .to_string();
        let key = auth_fields
            .get("key")
            .and_then(|v| v.as_str())
            .expect("key")
            .to_string();
        let user_info = serde_json::json!({
            "uid": uid,
            "encrypt_user_info": encrypt_user_info,
            "key": key,
        })
        .to_string();
        let ctx = runtime
            .context_new(&machine_id, "1.1.23", &user_info, r#"{"client_type":5}"#)
            .expect("qodercontext_new");

        // 3. Sign the model list request (wasm path). The CLI's
        // `listModelsFromRemote` uses authMode "auth" (logged-in context
        // signature) for the model list, not "sign" (anonymous).
        let prepared = runtime
            .prepare_request(
                ctx,
                "https://gateway.qoder.com.cn",
                "/api/v2/model/list?Encode=1",
                "GET",
                "auth",
                None,
                None,
            )
            .expect("prepareRequest");

        // 4. Send the wasm-signed request as-is (COSY Authorization +
        // Cosy-* signature headers, exactly what the CLI sends).
        let headers = prepared.headers().expect("headers parse");
        let mut request = client.get(&prepared.url);
        for (name, value) in &headers {
            request = request.header(name, value);
        }
        let resp = request.send().await.expect("signed model list request");
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        eprintln!("PoC wasm-signed status: {status}, body: {body:?}");
        if status == reqwest::StatusCode::OK {
            // CLI `NsA`: decrypt_server_response, falling back to the raw
            // text when the payload is plaintext (model list is not always
            // encrypted). Mirror that: plaintext body is valid.
            let decrypted = match runtime.decrypt_server_response(&body) {
                Ok(d) => d,
                Err(_) => body.clone(),
            };
            eprintln!("PoC wasm-signed decrypted OK, len: {}", decrypted.len());
            assert!(decrypted.contains("\"chat\""));
            return;
        }
        // Fallback: retry with the machine token as Cosy-MachineToken (the
        // CLI's `_ka` anonymous path), keeping the wasm signature headers.
        let mut request = client.get(&prepared.url);
        for (name, value) in &headers {
            if name == "Cosy-MachineToken" {
                request = request.header(name, "P1gAhUXEJPv7B1QTjZsv2UoTjzQStj40FyzZJ_GPs8NcmMqxnmgUCpEdJO89u2fSS24-wZm4-ZAeOkmTQ4h9vPHS");
            } else {
                request = request.header(name, value);
            }
        }
        let resp = request.send().await.expect("signed model list retry");
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        eprintln!("PoC wasm-signed+mt status: {status}, body: {body:?}");
        assert!(
            status == reqwest::StatusCode::OK,
            "wasm-signed request must reach the gateway (200), got {status}: {}",
            &body[..body.len().min(200)]
        );

        // 5. Decrypt the response
        let decrypted = runtime.decrypt_server_response(&body).expect("decrypt");
        let parsed: serde_json::Value = serde_json::from_str(&decrypted).expect("decrypted JSON");
        eprintln!(
            "PoC decrypted keys: {:?}",
            parsed.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
        assert!(!decrypted.is_empty(), "decrypted response non-empty");
    }
}
