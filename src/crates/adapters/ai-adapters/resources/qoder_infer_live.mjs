#!/usr/bin/env node
/**
 * Live inference probe: full CLI-compatible chain
 *   exchange -> userinfo -> generate_runtime_auth_fields -> context_new
 *   -> prepareInferRequest(endpoint, body, model_key, source) -> POST
 *   -> decrypt_server_response (if encrypted)
 * Reports the raw response so we can decide what BitFun's stream layer needs.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { webcrypto } from 'node:crypto';
import { TextEncoder, TextDecoder } from 'node:util';

const WASM_PATH = fileURLToPath(new URL('./qoder_auth_wasm_bg.wasm', import.meta.url));
const refs = [undefined, null, true, false];
const freeList = [];
function allocRef(v) { if (freeList.length) { const i = freeList.pop(); refs[i] = v; return i; } refs.push(v); return refs.length - 1; }
function freeRef(i) { if (i < 4) return; refs[i] = undefined; freeList.push(i); }
function getRef(i) { return refs[i]; }
let memory, memoryView;
function mem() { if (!memoryView || memoryView.buffer !== memory.buffer) memoryView = new Uint8Array(memory.buffer); return memoryView; }
const te = new TextEncoder(), td = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
function readString(p, l) { return td.decode(mem().subarray(p, p + l)); }

const imports = {
  './qoder_auth_wasm_bg.js': {
    __wbindgen_object_drop_ref: (i) => freeRef(i),
    __wbindgen_object_clone_ref: (i) => allocRef(getRef(i)),
    __wbindgen_export: (i) => { throw getRef(i); },
    __wbg___wbindgen_is_object_40c5a80572e8f9d3: (i) => (typeof getRef(i) === 'object' && getRef(i) !== null ? 1 : 0),
    __wbg___wbindgen_is_string_b29b5c5a8065ba1a: (i) => (typeof getRef(i) === 'string' ? 1 : 0),
    __wbg___wbindgen_is_function_49868bde5eb1e745: (i) => (typeof getRef(i) === 'function' ? 1 : 0),
    __wbg___wbindgen_is_undefined_c0cca72b82b86f4d: (i) => (getRef(i) === undefined ? 1 : 0),
    __wbg___wbindgen_throw_81fc77679af83bc6: (p, l) => { throw new Error(readString(p, l)); },
    __wbg_call_d578befcc3145dee: () => allocRef(undefined),
    __wbg_crypto_38df2bab126b63dc: () => allocRef({}),
    __wbg_getRandomValues_c44a50d8cfdaebeb: (_o, a) => { const arr = getRef(a); webcrypto.getRandomValues(arr); return allocRef(arr); },
    __wbg_getRandomValues_d49329ff89a07af1: (p, l) => { const buf = new Uint8Array(l); webcrypto.getRandomValues(buf); mem().set(buf, p); },
    __wbg_length_0c32cb8543c8e4c8: (i) => getRef(i).length,
    __wbg_msCrypto_bd5a034af96bcba6: () => allocRef({}),
    __wbg_new_99cabae501c0a8a0: () => allocRef(new Map()),
    __wbg_new_with_length_9cedd08484b73942: (l) => allocRef(new Uint8Array(l)),
    __wbg_node_84ea875411254db1: () => allocRef({}),
    __wbg_now_88621c9c9a4f3ffc: () => Date.now(),
    __wbg_process_44c7a14e11e9f69e: () => allocRef({}),
    __wbg_prototypesetcall_3e05eb9545565046: (d, _l, s) => mem().set(getRef(s), d),
    __wbg_randomFillSync_6c25eac9869eb53c: (_o, a) => { const arr = getRef(a); webcrypto.getRandomValues(arr); return allocRef(arr); },
    __wbg_require_b4edbdcf3e2a1ef0: () => allocRef(undefined),
    __wbg_set_08463b1df38a7e29: (m, k, v) => { const map = getRef(m); map.set(getRef(k), getRef(v)); return allocRef(map); },
    __wbg_static_accessor_GLOBAL_THIS_a1248013d790bf5f: () => allocRef(globalThis),
    __wbg_static_accessor_GLOBAL_f2e0f995a21329ff: () => allocRef(globalThis),
    __wbg_static_accessor_SELF_24f78b6d23f286ea: () => allocRef(globalThis),
    __wbg_static_accessor_WINDOW_59fd959c540fe405: () => allocRef(globalThis),
    __wbg_subarray_0f98d3fb634508ad: (i, b, e) => allocRef(getRef(i).subarray(b, e)),
    __wbg_versions_276b2795b1c6a219: () => allocRef({}),
    __wbindgen_cast_0000000000000001: (p, l) => allocRef(mem().subarray(p, p + l)),
    __wbindgen_cast_0000000000000002: (p, l) => allocRef(readString(p, l)),
    __wbg_Error_2e59b1b37a9a34c3: (p, l) => allocRef(new Error(readString(p, l))),
  },
};

let instance;
const wasmBytes = readFileSync(WASM_PATH);
({ instance } = await WebAssembly.instantiate(wasmBytes, imports));
memory = instance.exports.memory;
const allocString = (text) => { const b = te.encode(text); const p = instance.exports.__wbindgen_export2(b.length, 1); mem().set(b, p); return { ptr: p, len: b.length }; };
const addSP = () => instance.exports.__wbindgen_add_to_stack_pointer(-16);
const restoreSP = (sp) => instance.exports.__wbindgen_add_to_stack_pointer(16);
const readI32 = (sp, off) => new DataView(memory.buffer).getInt32(sp + off, true);

function ctxNew(machineId, userInfo) {
  const a = allocString(machineId), g = allocString('1.1.23'), B = allocString(userInfo), l = allocString('{"client_type":5}');
  const sp = addSP();
  instance.exports.qodercontext_new(sp, a.ptr, a.len, g.ptr, g.len, B.ptr, B.len, l.ptr, l.len);
  const ptr = readI32(sp, 0), err = readI32(sp, 4);
  restoreSP(sp);
  if (err) throw new Error('ctxNew failed');
  return ptr;
}
function generateAuthFields(userJson) {
  const u = allocString(userJson);
  const sp = addSP();
  instance.exports.generate_runtime_auth_fields(sp, u.ptr, u.len);
  const ptr = readI32(sp, 0), len = readI32(sp, 4), err = readI32(sp, 8), isErr = readI32(sp, 12);
  restoreSP(sp);
  if (isErr) throw new Error('generate_runtime_auth_fields failed: ' + readString(ptr, len));
  return readString(ptr, len);
}
function prepareInfer(ctx, endpoint, pathOrBody, body, headers) {
  const ep = allocString(endpoint), pb = allocString(pathOrBody);
  const bodyArgs = body === undefined ? [0, 0] : [allocString(body).ptr, 0];
  const hdrArgs = headers === undefined ? [0, 0] : [0, 0];
  const sp = addSP();
  instance.exports.qodercontext_prepareInferRequest(sp, ctx, ep.ptr, ep.len, pb.ptr, pb.len, bodyArgs[0], bodyArgs[1], hdrArgs[0], hdrArgs[1]);
  const ptr = readI32(sp, 0), err = readI32(sp, 4);
  restoreSP(sp);
  if (err) throw new Error('prepareInfer failed');
  return ptr;
}
function readResult(reqPtr) {
  const sp = addSP();
  instance.exports.requestresult_url(sp, reqPtr);
  const urlPtr = readI32(sp, 0), urlLen = readI32(sp, 4);
  restoreSP(sp);
  const url = readString(urlPtr, urlLen);
  const headersRef = instance.exports.requestresult_headers(reqPtr);
  let headers = {};
  if (headersRef !== 0) { const v = getRef(headersRef); freeRef(headersRef); if (v instanceof Map) headers = Object.fromEntries(v); }
  const sp2 = addSP();
  instance.exports.requestresult_body(sp2, reqPtr);
  const bPtr = readI32(sp2, 0), bLen = readI32(sp2, 4);
  restoreSP(sp2);
  let body = null;
  if (bPtr !== 0) { body = td.decode(mem().subarray(bPtr, bPtr + bLen)); instance.exports.__wbindgen_export4(bPtr, bLen, 1); }
  instance.exports.__wbg_requestresult_free(reqPtr, 0);
  return { url, headers, body };
}
function decryptResp(cipher) {
  const c = allocString(cipher);
  const sp = addSP();
  instance.exports.decrypt_server_response(sp, c.ptr, c.len);
  const ptr = readI32(sp, 0), len = readI32(sp, 4), err = readI32(sp, 8), isErr = readI32(sp, 12);
  restoreSP(sp);
  if (isErr) throw new Error('decrypt failed: ' + readString(ptr, len));
  return readString(ptr, len);
}

const machineId = process.argv[2] || 'ad05505d-0918-4a5d-bbf5-d7acf9abdd9d';
const pat = process.env.BITFUN_QODER_PAT;
if (!pat) { console.error('BITFUN_QODER_PAT required'); process.exit(2); }

const ex = await fetch('https://openapi.qoder.com.cn/api/v1/jobToken/exchange', {
  method: 'POST', headers: { 'content-type': 'application/json' },
  body: JSON.stringify({ personal_token: pat }),
});
const exj = await ex.json();
const jobToken = exj.token || exj.access_token || exj.device_token;
console.log('exchange:', ex.status, 'token len', jobToken?.length);

const ui = await fetch('https://openapi.qoder.com.cn/api/v1/userinfo', { headers: { authorization: 'Bearer ' + jobToken } });
const uij = await ui.json();
const uid = String(uij.id || uij.user_id);
console.log('userinfo:', ui.status, 'uid', uid);
const authFields = JSON.parse(generateAuthFields(JSON.stringify({
  uid, organization_id: uij.organization_id ?? null,
  organization_tags: uij.organization_tags ?? null,
  data_policy_agreed: uij.data_policy_agreed ?? null,
})));
const ctx = ctxNew(machineId, JSON.stringify({ uid, encrypt_user_info: authFields.encrypt_user_info, key: authFields.key }));

// CLI remoteChatAsk shape: {model_config:{key,source}, messages:[...], session_id, request_id}
const modelKey = process.argv[3] || 'auto';
const ask = {
  model_config: { key: modelKey, source: 'system', format: 'openai' },
  custom_model: null,
  session_id: crypto.randomUUID(),
  request_id: crypto.randomUUID(),
  messages: [{ role: 'user', content: 'Reply with exactly: PONG' }],
  stream: true,
};
const prepared = readResult(prepareInfer(ctx, 'https://gateway.qoder.com.cn', JSON.stringify(ask), modelKey, 'system'));
console.log('signed url:', prepared.url);
console.log('signed headers:', JSON.stringify(prepared.headers).slice(0, 400));
console.log('signed body head:', prepared.body ? prepared.body.slice(0, 80) : null);

const headers = { ...prepared.headers, Accept: 'text/event-stream' };
const r = await fetch(prepared.url, { method: 'POST', headers, body: prepared.body || JSON.stringify(ask) });
console.log('infer status:', r.status, r.statusText);
const contentType = r.headers.get('content-type');
console.log('content-type:', contentType);
const raw = await r.text();
console.log('raw head:', raw.slice(0, 500).replace(/\n/g, '\\n'));
console.log('raw len:', raw.length);
console.log('raw tail:', raw.slice(-600).replace(/\n/g, '\\n'));
// Split the SSE lines and show event shapes
const lines = raw.split('\n').filter(l => l.startsWith('data:'));
console.log('data lines:', lines.length);
const shapes = lines.slice(0, 3).map(l => {
  try { const j = JSON.parse(l.slice(5)); return { keys: Object.keys(j), bodyHead: (j.body||'').slice(0,120) }; } catch { return { raw: l.slice(0,120) }; }
});
console.log('first shapes:', JSON.stringify(shapes, null, 1).slice(0, 800));
const last = lines[lines.length-1];
console.log('last data line:', last.slice(0, 300));
try { const j = JSON.parse(last.slice(5)); console.log('last body:', (j.body||'').slice(0,200)); } catch {}
fs.writeFileSync('qoder_infer_raw_sample.txt', raw);
console.log('saved qoder_infer_raw_sample.txt');
try {
  const decrypted = decryptResp(raw);
  console.log('decrypted head:', decrypted.slice(0, 500));
} catch (e) {
  console.log('decrypt failed (likely plaintext):', e.message);
}
