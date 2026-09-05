#!/usr/bin/env node

/**
 * Runs the independent pre-bundle build pipelines in parallel:
 *   - build:web            (type-check + Vite build + revision manifest + asset verification)
 *   - prepare:mobile-web   (mobile-web install/build with mtime short-circuit)
 *   - prepare:dsh-profile  (the DeepSeek Harness bridge official desktop:build ships)
 *
 * Used as the Tauri beforeBuildCommand so the stage costs max(…) instead of
 * their sum. Any failure fails the whole script with a non-zero exit code.
 */

import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function runPrefixed(prefix, command, args) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: ROOT_DIR,
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

const codes = await Promise.all([
  runPrefixed('web', 'pnpm', ['run', 'build:web']),
  runPrefixed('mobile-web', 'pnpm', ['run', 'prepare:mobile-web']),
  // The DeepSeek Harness bridge Tauri ships as a resource. On a cold tree this
  // installs its own pinned toolchain (~30s), which still fits inside the two
  // above; it is independent of them, and of OpenBitFun's pnpm store.
  runPrefixed('dsh-profile', 'pnpm', ['run', 'prepare:dsh-profile']),
]);

const failed = codes.some((code) => code !== 0);
if (failed) {
  process.stderr.write('[frontend-build-all] frontend build failed (see output above)\n');
}
// Set the code instead of calling process.exit(): stdout is a pipe under CI and
// process.exit() would drop whatever is still queued on it.
process.exitCode = failed ? 1 : 0;
