// ─────────────────────────────────────────────────────────────────────────────
// PPT Live — single-pass bundle build
//
// This is the ONLY build script for PPT Live. It produces dist/ui.bundle.js,
// which is the sole JS artifact loaded at runtime (see builtin.rs → ui_js).
//
// IMPORTANT: `pnpm run desktop:dev` does NOT rebuild this bundle.
//   desktop:dev provides Vite HMR for the *web-ui* frontend and auto-rebuild
//   for *Rust* changes, but PPT Live's JS is a pre-built static asset embedded
//   via `include_str!` at Rust compile time. You must run this script manually
//   after editing any PPT Live JS source.
//
// Usage (from the ppt-live directory):
//   cd src/crates/contracts/product-domains/src/miniapp/builtin/assets/ppt-live
//   node build-bitfun.mjs
//
// Or from repo root:
//   node src/crates/contracts/product-domains/src/miniapp/builtin/assets/ppt-live/build-bitfun.mjs
//
// Output:  dist/ui.bundle.js   (readable, unminified — this is an open-source project)
//
// Build chain:
//   ui.js  →  src/*.js  →  npm deps (pptxgenjs, pdf-lib, jszip)
//   All imports are resolved and inlined by esbuild in a single pass.
//
// After rebuilding, bump the version in meta.json, bundle.json, and builtin.rs
// (all three must match), then run `cargo check -p bitfun-product-domains`.
// Restart desktop:dev to pick up the new bundle (Rust include_str! re-reads on
// recompile, which desktop:dev triggers automatically for .rs changes — but
// since only the embedded JS content changed, a touch to builtin.rs or a Rust
// recompile may be needed to force include_str! to refresh).
//
// The post-build replaceAll fixes a pdf-lib template literal that would
// otherwise produce invalid trailing-whitespace in the bundled output.
// ─────────────────────────────────────────────────────────────────────────────
import { build } from 'esbuild';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const appDir = path.dirname(fileURLToPath(import.meta.url));
const outfile = path.join(appDir, 'dist', 'ui.bundle.js');

await build({
  entryPoints: [path.join(appDir, 'ui.js')],
  outfile,
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: false,
  legalComments: 'none',
});

const bundledSource = await readFile(outfile, 'utf8');
await writeFile(outfile, bundledSource.replaceAll('` \n`', '" \\n"'));
