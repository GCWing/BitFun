#!/usr/bin/env node

/**
 * Regenerates TypeScript bindings from the Rust schema, then runs the web-ui
 * type-check (tsc --noEmit) and the Vite production build in parallel. The
 * latter two are independent: Vite transpiles with esbuild and never consults
 * tsc, so serializing them only added wall-clock time.
 *
 * The `gen:types` step (which requires a Rust toolchain) runs serially first so
 * the Vite build picks up fresh bindings. Local frontend-only iteration can
 * skip it with `pnpm --dir src/web-ui build`. Both child processes must succeed;
 * if either fails the script exits with a non-zero code (after letting the
 * sibling finish so its output is not lost).
 */

import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function runPrefixed(prefix, command, args, cwd) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd,
      shell: process.platform === 'win32',
      stdio: ['ignore', 'pipe', 'pipe'],
      env: process.env,
    });

    const forward = (stream, out) => {
      let buffered = '';
      stream.on('data', (chunk) => {
        buffered += chunk.toString();
        const lines = buffered.split(/\r?\n/);
        buffered = lines.pop() ?? '';
        for (const line of lines) {
          out.write(`[${prefix}] ${line}\n`);
        }
      });
      stream.on('end', () => {
        if (buffered.trim() !== '') {
          out.write(`[${prefix}] ${buffered}\n`);
        }
      });
    };

    forward(child.stdout, process.stdout);
    forward(child.stderr, process.stderr);

    child.on('error', (error) => {
      process.stderr.write(`[${prefix}] failed to start: ${error.message}\n`);
      resolve(1);
    });
    child.on('close', (code) => {
      resolve(code ?? 1);
    });
  });
}

// Step 1: regenerate TypeScript bindings from the Rust schema (serial, must
// finish before the Vite build so the frontend picks up fresh types). This is
// the only step that requires a working Rust toolchain; local frontend-only
// iteration can skip it with `pnpm --dir src/web-ui build`.
const genTypesCode = await runPrefixed(
  'gen-types',
  'pnpm',
  ['--dir', 'src/web-ui', 'run', 'gen:types'],
  ROOT_DIR,
);
if (genTypesCode !== 0) {
  process.stderr.write('[build-web-parallel] gen:types failed (see output above)\n');
  process.exitCode = 1;
  process.exit();
}

// Step 2: build the independently published design-system packages once before
// the product type-check and Vite build consume their public `dist` exports.
const designSystemCode = await runPrefixed(
  'design-system',
  'pnpm',
  ['--dir', 'design-system', 'run', 'build:packages'],
  ROOT_DIR,
);
if (designSystemCode !== 0) {
  process.stderr.write('[build-web-parallel] design-system package build failed (see output above)\n');
  process.exitCode = 1;
  process.exit();
}

// Step 3: type-check and Vite build run in parallel. Use the raw tools here so
// the web-ui package lifecycle hooks do not rebuild the same design-system
// artifacts concurrently while TypeScript is reading them.
const tasks = [
  runPrefixed('type-check', 'pnpm', ['--dir', 'src/web-ui', 'exec', 'tsc', '--noEmit'], ROOT_DIR),
  runPrefixed('vite-build', 'pnpm', ['--dir', 'src/web-ui', 'exec', 'vite', 'build'], ROOT_DIR),
];

const buildCodes = await Promise.all(tasks);
const failed = buildCodes.some((code) => code !== 0);
if (failed) {
  process.stderr.write('[build-web-parallel] build:web failed (see output above)\n');
}
// Set the code instead of calling process.exit(): stdout is a pipe under CI and
// process.exit() would drop whatever is still queued on it.
process.exitCode = failed ? 1 : 0;
