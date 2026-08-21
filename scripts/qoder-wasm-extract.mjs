// Extracts the Qoder CN CLI authentication WASM from the official bundle.
//
// The `@qodercn-ai/qoderclicn` package embeds the wasm as a base64 constant
// (`u7o="..."`) inside `bundle/qoderclicn.js`. This script decodes it back
// into `ai-adapters/resources/qoder_auth_wasm_bg.wasm` and verifies the wasm
// magic so BitFun can instantiate the signing/decryption module directly.
//
// Rerunnable: `node scripts/qoder-wasm-extract.mjs [--bundle <path>]`.
// The bundle path defaults to the npm global install of @qodercn-ai/qoderclicn.

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const DEFAULT_BUNDLE = 'C:/Users/Administrator/AppData/Roaming/npm/node_modules/@qodercn-ai/qoderclicn/bundle/qoderclicn.js';
const OUT = resolve(root, 'src/crates/adapters/ai-adapters/resources/qoder_auth_wasm_bg.wasm');
const EXPECTED_BYTES = 297_238;
const WASM_MAGIC = '0061736d01000000';

function parseArgs(argv) {
  const args = { bundle: DEFAULT_BUNDLE, out: OUT };
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--bundle') args.bundle = argv[i + 1];
    if (argv[i] === '--out') args.out = argv[i + 1];
  }
  return args;
}

const { bundle, out } = parseArgs(process.argv.slice(2));

let source;
try {
  source = readFileSync(bundle, 'utf8');
} catch (error) {
  console.error(`[FAIL] cannot read bundle ${bundle}: ${error.message}`);
  process.exit(1);
}

const marker = 'u7o="';
const index = source.indexOf(marker);
if (index < 0) {
  console.error('[FAIL] u7o constant not found in bundle; the wasm embedding layout may have changed');
  process.exit(1);
}
const start = index + marker.length;
const end = source.indexOf('"', start);
const decoded = Buffer.from(source.slice(start, end), 'base64');
const magic = decoded.subarray(0, 8).toString('hex');

console.log(`bundle:      ${bundle}`);
console.log(`u7o offset:  ${index}`);
console.log(`wasm bytes:  ${decoded.length}`);
console.log(`magic:       ${magic}`);

if (magic !== WASM_MAGIC) {
  console.error('[FAIL] decoded payload is not a wasm module (magic mismatch)');
  process.exit(1);
}
if (decoded.length !== EXPECTED_BYTES) {
  console.error(`[FAIL] wasm size ${decoded.length} != expected ${EXPECTED_BYTES}; the CLI version may have changed`);
  process.exit(1);
}

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, decoded);
console.log(`[OK] wrote  ${out}`);
