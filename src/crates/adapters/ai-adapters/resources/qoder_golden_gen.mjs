#!/usr/bin/env node
/**
 * Golden-baseline generator for the Qoder wasm auth signature chain.
 *
 * Replicates the official qoderclicn bundle glue (wasm-bindgen) semantics,
 * one import at a time, mirroring bundle @qoderclicn qoder_auth_wasm_bg.js
 * (bundle offsets ~427487..430600).  The Rust host imports in
 * `subscription_auth/qoder_wasm.rs` are implemented to the same semantics;
 * this script produces a byte-identical `prepareRequest` signature (headers)
 * so the Rust test can assert against it instead of guessing.
 *
 * Usage:
 *   node qoder_golden_gen.mjs <machine_id> <uid> <encrypt_user_info> <key>
 *
 * Prints JSON: { context_user_info, request: { url, path, method, authMode,
 * headers: {name:value,...} } }
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { webcrypto } from 'node:crypto';
import { TextEncoder, TextDecoder } from 'node:util';

const WASM_PATH = fileURLToPath(new URL('./qoder_auth_wasm_bg.wasm', import.meta.url));

// ---- wasm-bindgen glue state (mirrors bundle Em/S7o/HD/Np/yCA/cw/kCA) ----
const refs = [undefined, null, true, false];
const freeList = [];
function allocRef(value) {
  if (freeList.length > 0) {
    const idx = freeList.pop();
    refs[idx] = value;
    return idx;
  }
  refs.push(value);
  return refs.length - 1;
}
function freeRef(idx) {
  if (idx < 4) return;
  refs[idx] = undefined;
  freeList.push(idx);
}
function getRef(idx) { return refs[idx]; }

let memory = null;
let memoryView = null;
function mem() {
  if (!memoryView || memoryView.buffer !== memory.buffer) {
    memoryView = new Uint8Array(memory.buffer);
  }
  return memoryView;
}
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });

// ---- imports (semantics read from the official bundle) ----
const imports = {
  './qoder_auth_wasm_bg.js': {
    __wbindgen_object_drop_ref: (idx) => { freeRef(idx); },
    __wbindgen_object_clone_ref: (idx) => allocRef(getRef(idx)),
    __wbindgen_export: (excIdx) => { throw getRef(excIdx); },
    __wbg_Error_2e59b1b37a9a34c3: (ptr, len) => allocRef(new Error(readString(ptr, len))),
    __wbg___wbindgen_is_object_40c5a80572e8f9d3: (idx) => {
      const v = getRef(idx);
      return typeof v === 'object' && v !== null ? 1 : 0;
    },
    __wbg___wbindgen_is_string_b29b5c5a8065ba1a: (idx) => (typeof getRef(idx) === 'string' ? 1 : 0),
    __wbg___wbindgen_is_function_49868bde5eb1e745: (idx) => (typeof getRef(idx) === 'function' ? 1 : 0),
    __wbg___wbindgen_is_undefined_c0cca72b82b86f4d: (idx) => (getRef(idx) === undefined ? 1 : 0),
    __wbg___wbindgen_throw_81fc77679af83bc6: (ptr, len) => {
      throw new Error(readString(ptr, len));
    },
    __wbg_call_d578befcc3145dee: (_fnRef, _thisRef, _argRef) => allocRef(undefined),
    __wbg_crypto_38df2bab126b63dc: () => allocRef({}),
    __wbg_getRandomValues_c44a50d8cfdaebeb: (_objRef, arrRef) => {
      const arr = getRef(arrRef);
      webcrypto.getRandomValues(arr);
      return allocRef(arr);
    },
    __wbg_getRandomValues_d49329ff89a07af1: (ptr, len) => {
      const buf = new Uint8Array(len);
      webcrypto.getRandomValues(buf);
      mem().set(buf, ptr);
    },
    __wbg_length_0c32cb8543c8e4c8: (idx) => getRef(idx).length,
    __wbg_msCrypto_bd5a034af96bcba6: () => allocRef({}),
    __wbg_new_99cabae501c0a8a0: () => allocRef(new Map()),
    __wbg_new_with_length_9cedd08484b73942: (len) => allocRef(new Uint8Array(len)),
    __wbg_node_84ea875411254db1: () => allocRef({}),
    __wbg_now_88621c9c9a4f3ffc: () => Date.now(),
    __wbg_process_44c7a14e11e9f69e: () => allocRef({}),
    __wbg_prototypesetcall_3e05eb9545565046: (destPtr, _destLen, srcRef) => {
      mem().set(getRef(srcRef), destPtr);
    },
    __wbg_randomFillSync_6c25eac9869eb53c: (_objRef, arrRef) => {
      const arr = getRef(arrRef);
      webcrypto.getRandomValues(arr);
      return allocRef(arr);
    },
    __wbg_require_b4edbdcf3e2a1ef0: () => allocRef(undefined),
    __wbg_set_08463b1df38a7e29: (mapRef, keyRef, valueRef) => {
      const map = getRef(mapRef);
      map.set(getRef(keyRef), getRef(valueRef));
      return allocRef(map);
    },
    __wbg_static_accessor_GLOBAL_THIS_a1248013d790bf5f: () => allocRef(globalThis),
    __wbg_static_accessor_GLOBAL_f2e0f995a21329ff: () => allocRef(globalThis),
    __wbg_static_accessor_SELF_24f78b6d23f286ea: () => allocRef(globalThis),
    __wbg_static_accessor_WINDOW_59fd959c540fe405: () => allocRef(globalThis),
    __wbg_subarray_0f98d3fb634508ad: (idx, begin, end) => allocRef(getRef(idx).subarray(begin, end)),
    __wbg_versions_276b2795b1c6a219: () => allocRef({}),
    __wbindgen_cast_0000000000000001: (ptr, len) => allocRef(mem().subarray(ptr, ptr + len)),
    __wbindgen_cast_0000000000000002: (ptr, len) => allocRef(readString(ptr, len)),
  },
};

function readString(ptr, len) {
  return textDecoder.decode(mem().subarray(ptr, ptr + len));
}

// stack pointer helpers
const addToStackPointer = () => instance.exports.__wbindgen_add_to_stack_pointer(-16);
const restoreStackPointer = (sp) => instance.exports.__wbindgen_add_to_stack_pointer(16);
const readI32 = (sp, off) => new DataView(memory.buffer).getInt32(sp + off, true);
function allocString(text) {
  const bytes = textEncoder.encode(text);
  const ptr = instance.exports.__wbindgen_export2(bytes.length, 1);
  mem().set(bytes, ptr);
  return { ptr, len: bytes.length };
}
function takeStringResult(sp) {
  const ptr = readI32(sp, 0);
  const len = readI32(sp, 4);
  const err = readI32(sp, 8);
  const isErr = readI32(sp, 12);
  restoreStackPointer(sp);
  if (isErr) throw new Error(readString(ptr, len));
  return readString(ptr, len);
}

let instance;
const wasmBytes = readFileSync(WASM_PATH);
({ instance } = await WebAssembly.instantiate(wasmBytes, imports));
memory = instance.exports.memory;

// ---- high-level call chain (mirrors the CLI) ----
function qodercontextNew(machineId, cosyVersion, userInfoJson, clientMetadataJson) {
  const a = allocString(machineId);
  const g = allocString(cosyVersion);
  const B = allocString(userInfoJson);
  const l = allocString(clientMetadataJson);
  const sp = addToStackPointer();
  instance.exports.qodercontext_new(sp, a.ptr, a.len, g.ptr, g.len, B.ptr, B.len, l.ptr, l.len);
  const ptr = readI32(sp, 0);
  const err = readI32(sp, 4);
  restoreStackPointer(sp);
  if (err) throw new Error('qodercontext_new failed');
  return ptr;
}
function prepareRequest(ctxPtr, endpoint, path, method, authMode, body, headers) {
  const ep = allocString(endpoint);
  const p = allocString(path);
  const m = allocString(method);
  const am = allocString(authMode);
  const bodyArgs = body === undefined ? [0, 0] : [allocString(body).ptr, 0];
  const hdrArgs = headers === undefined ? [0, 0] : [0, 0];
  const sp = addToStackPointer();
  instance.exports.qodercontext_prepareRequest(
    sp, ctxPtr,
    ep.ptr, ep.len, p.ptr, p.len, m.ptr, m.len, am.ptr, am.len,
    bodyArgs[0], bodyArgs[1], hdrArgs[0], hdrArgs[1]
  );
  const reqPtr = readI32(sp, 0);
  const err = readI32(sp, 4);
  restoreStackPointer(sp);
  if (err) throw new Error('prepareRequest failed');
  return reqPtr;
}
function requestResultFields(reqPtr) {
  const sp = addToStackPointer();
  instance.exports.requestresult_url(sp, reqPtr);
  const urlPtr = readI32(sp, 0), urlLen = readI32(sp, 4);
  restoreStackPointer(sp);
  const url = readString(urlPtr, urlLen);

  // headers: JS object ref (Map<string,string> serialized)
  const headersRef = instance.exports.requestresult_headers(reqPtr);
  let headersJson = '';
  if (headersRef !== 0) {
    const value = getRef(headersRef);
    freeRef(headersRef);
    if (value instanceof Map) headersJson = JSON.stringify(Object.fromEntries(value));
    else if (typeof value === 'string') headersJson = value;
  }
  let headers = {};
  try { headers = JSON.parse(headersJson); } catch {}

  // body (optional)
  const sp2 = addToStackPointer();
  instance.exports.requestresult_body(sp2, reqPtr);
  const bodyPtr = readI32(sp2, 0), bodyLen = readI32(sp2, 4);
  restoreStackPointer(sp2);
  let body = null;
  if (bodyPtr !== 0) {
    body = new TextDecoder().decode(mem().subarray(bodyPtr, bodyPtr + bodyLen));
    instance.exports.__wbindgen_export4(bodyPtr, bodyLen, 1);
  }

  instance.exports.__wbg_requestresult_free(reqPtr, 0);
  return { url, headers, body };
}

// ---- main ----
const [machineId, uid, encryptUserInfo, key] = process.argv.slice(2);
if (!machineId || !uid) {
  console.error('usage: node qoder_golden_gen.mjs <machine_id> <uid> <encrypt_user_info> <key>');
  process.exit(2);
}
const userInfo = JSON.stringify({ uid, encrypt_user_info: encryptUserInfo, key });
const ctx = qodercontextNew(machineId, '1.1.23', userInfo, JSON.stringify({ client_type: 5 }));
const reqPtr = prepareRequest(ctx, 'https://gateway.qoder.com.cn', '/api/v2/model/list?Encode=1', 'GET', 'auth', undefined, undefined);
const { url, headers, body } = requestResultFields(reqPtr);
console.log(JSON.stringify({ context_user_info: userInfo, request: { url, path: '/api/v2/model/list?Encode=1', method: 'GET', authMode: 'auth', headers, body } }, null, 2));
