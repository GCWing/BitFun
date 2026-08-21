// Verifies the embedded Qoder auth wasm resource in ai-adapters:
//   - file exists with the expected size (297,238 bytes for CLI 1.1.23)
//   - wasm magic 0061736d01000000
//   - module exports include the qodercontext_* signing entry points
//   - module imports stay within the wasm-bindgen glue set (no fs/net I/O)
//
// Usage: `node scripts/qoder-wasm-verify.mjs`

import { readFileSync, statSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const WASM = resolve(root, 'src/crates/adapters/ai-adapters/resources/qoder_auth_wasm_bg.wasm');
const EXPECTED_BYTES = 297_238;
const WASM_MAGIC = '0061736d01000000';
const REQUIRED_EXPORTS = [
  'qodercontext_new',
  'qodercontext_prepareRequest',
  'qodercontext_prepareInferRequest',
  'qodercontext_refreshAuthFields',
  'decrypt_server_response',
  'model_cache_decrypt',
  'requestresult_url',
  'requestresult_body',
  'requestresult_headers',
  'requestresult_headerCount',
];
// wasm-bindgen glue imports. fs/net would appear here; their absence is what
// makes the module safe to embed and instantiate from Rust.
const ALLOWED_IMPORT_MODULE = './qoder_auth_wasm_bg.js';

let failures = 0;
const fail = (message) => {
  console.error(`[FAIL] ${message}`);
  failures += 1;
};

try {
  const stat = statSync(WASM);
  if (stat.size !== EXPECTED_BYTES) {
    fail(`size ${stat.size} != expected ${EXPECTED_BYTES}`);
  } else {
    console.log(`size:        ${stat.size}`);
  }
} catch (error) {
  fail(`cannot stat ${WASM}: ${error.message}`);
  process.exit(1);
}

const bytes = readFileSync(WASM);
const magic = bytes.subarray(0, 8).toString('hex');
console.log(`magic:       ${magic}`);
if (magic !== WASM_MAGIC) {
  fail('wasm magic mismatch');
}

const module = new WebAssembly.Module(bytes);
const exports = WebAssembly.Module.exports(module).map((entry) => entry.name);
const imports = WebAssembly.Module.imports(module);
for (const name of REQUIRED_EXPORTS) {
  if (!exports.includes(name)) {
    fail(`missing export ${name}`);
  }
}
console.log(`exports:     ${exports.length} (qodercontext_* entry points present)`);

for (const imp of imports) {
  if (imp.module !== ALLOWED_IMPORT_MODULE) {
    fail(`unexpected import module ${imp.module}::${imp.name}`);
  }
}
console.log(`imports:     ${imports.length} (all wasm-bindgen glue)`);

if (failures > 0) {
  process.exit(1);
}
console.log('[OK] qoder auth wasm resource verified');
