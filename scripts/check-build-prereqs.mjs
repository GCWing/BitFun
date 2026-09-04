#!/usr/bin/env node

/**
 * Build prerequisite preflight check.
 *
 * Detects missing build prerequisites that cause confusing cargo/pnpm failures:
 *
 * - Root node_modules missing → pnpm scripts fail with "node_modules missing,
 *   did you mean to install?"
 * - src/mobile-web/dist missing → cargo check -p openbitfun-desktop and
 *   cargo check --workspace fail with "resource path '../../mobile-web/dist'
 *   doesn't exist" in the openbitfun-desktop build script
 * - OpenCode extension Host dist missing → Desktop, CLI, and app-server
 *   builds cannot bundle or launch the Bun plugin Host resources
 * - sherpa-onnx prebuilt libs missing → sherpa-onnx-sys build script attempts
 *   a network download from GitHub that fails on poor connectivity
 *
 * Usage:
 *   node scripts/check-build-prereqs.mjs          # check only
 *   node scripts/check-build-prereqs.mjs --fix    # attempt to fix missing prereqs
 *
 * When cargo check or pnpm build fails with a confusing error about missing
 * resources or sherpa-onnx download failures, run this check first.
 */

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SCRIPT_DIR = __dirname;
const DEFAULT_ROOT = join(SCRIPT_DIR, '..');
const ROOT_DIR = process.env.OPENBITFUN_BUILD_PREREQS_TEST_ROOT || DEFAULT_ROOT;
const FIX = process.argv.includes('--fix');

// --- Check logic (extracted for re-use and testing) ---

function runChecks(rootDir) {
  const errors = [];
  const warnings = [];

  // --- Check 1: Root node_modules ---
  if (!existsSync(join(rootDir, 'node_modules'))) {
    errors.push({
      name: 'root node_modules',
      message: 'Root node_modules is missing. pnpm scripts will not work.',
      fix: ['pnpm', 'install'],
    });
  }

  // --- Check 2: mobile-web dist (required by openbitfun-desktop build script) ---
  if (!existsSync(join(rootDir, 'src', 'mobile-web', 'dist', 'index.html'))) {
    errors.push({
      name: 'mobile-web dist',
      message:
        "src/mobile-web/dist is missing. The openbitfun-desktop Tauri build script references '../../mobile-web/dist' as a resource, so 'cargo check -p openbitfun-desktop' and 'cargo check --workspace' will fail with \"resource path doesn't exist\".",
      fix: ['pnpm', 'run', 'prepare:mobile-web'],
    });
  }

  // --- Check 3: OpenCode extension Host dist (product runtime resource) ---
  const pluginHostDist = join(
    rootDir,
    'src',
    'apps',
    'extension-host',
    'dist',
  );
  const pluginHostEntries = [join(pluginHostDist, 'extension-host.js')];
  if (pluginHostEntries.some((entry) => !existsSync(entry))) {
    errors.push({
      name: 'OpenCode extension Host dist',
      message:
        'src/apps/extension-host/dist is missing the Bun Host entry. Desktop, CLI, and app-server builds require this plugin Host runtime resource.',
      fix: ['pnpm', 'run', 'plugin-host:prepare'],
    });
  }

  // --- Check 4: sherpa-onnx prebuilt libs ---
  // sherpa-onnx-sys build.rs auto-detects target/sherpa-onnx-prebuilt/<version>/lib/
  // and returns immediately without downloading. Only warn for the first-build
  // scenario where no prebuilt cache exists yet.
  const sherpaLibDirEnv = process.env.SHERPA_ONNX_LIB_DIR;
  const sherpaPrebuiltDir = join(rootDir, 'target', 'sherpa-onnx-prebuilt');

  if (sherpaLibDirEnv) {
    if (!existsSync(sherpaLibDirEnv)) {
      warnings.push({
        name: 'sherpa-onnx env var',
        message: `SHERPA_ONNX_LIB_DIR is set to "${sherpaLibDirEnv}" but the path does not exist.`,
      });
    }
  } else if (!existsSync(sherpaPrebuiltDir)) {
    warnings.push({
      name: 'sherpa-onnx',
      message:
        'No prebuilt sherpa-onnx cache found. The first build will download from GitHub. If connectivity is poor and the download fails, you can set SHERPA_ONNX_ARCHIVE_DIR to a directory containing the pre-downloaded archive.',
    });
  }
  // If prebuilt dir exists, build.rs auto-detects it — no warning needed.

  return { errors, warnings };
}

function collectPendingFixes(errors) {
  return errors
    .filter((e) => e.fix)
    .map((e) => ({ name: e.name, fix: e.fix }));
}

function reportResults({ errors, warnings }) {
  if (errors.length > 0) {
    console.error('Build prerequisite check failed:\n');
    for (const e of errors) {
      console.error(`  [FAIL] ${e.name}: ${e.message}`);
      if (e.fix) {
        console.error(`         Fix: ${e.fix.join(' ')}`);
      }
    }
    console.error();
  }

  if (warnings.length > 0) {
    console.warn('Build prerequisite warnings:\n');
    for (const w of warnings) {
      console.warn(`  [WARN] ${w.name}: ${w.message}`);
    }
    console.warn();
  }
}

function runFixes(pendingFixes, rootDir) {
  let allSucceeded = true;
  for (const { name, fix } of pendingFixes) {
    const [cmd, ...args] = fix;
    console.log(`$ ${fix.join(' ')}`);
    try {
      if (process.platform === 'win32') {
        execFileSync(
          process.env.ComSpec || 'cmd.exe',
          ['/d', '/s', '/c', fix.join(' ')],
          { stdio: 'inherit', cwd: rootDir },
        );
      } else {
        execFileSync(cmd, args, { stdio: 'inherit', cwd: rootDir });
      }
    } catch {
      console.error(`Fix command failed: ${fix.join(' ')}\n`);
      allSucceeded = false;
    }
    console.log();
  }
  return allSucceeded;
}

// --- Main ---

const firstResult = runChecks(ROOT_DIR);

if (firstResult.errors.length === 0 && firstResult.warnings.length === 0) {
  console.log('Build prerequisite check passed.');
  process.exit(0);
}

reportResults(firstResult);

if (firstResult.errors.length > 0) {
  const pendingFixes = collectPendingFixes(firstResult.errors);

  if (FIX && pendingFixes.length > 0) {
    console.log('Attempting fixes...\n');
    const allSucceeded = runFixes(pendingFixes, ROOT_DIR);

    if (allSucceeded) {
      console.log('Re-checking prerequisites...\n');
      const secondResult = runChecks(ROOT_DIR);
      reportResults(secondResult);

      if (secondResult.errors.length === 0) {
        console.log('All errors resolved after fix.');
        process.exit(0);
      }
      console.error('Some errors remain after fix.');
      process.exit(1);
    }
    console.error('Some fix attempts failed. See errors above.');
    process.exit(1);
  }

  console.error(
    'Run with --fix to attempt automatic fixes for missing prerequisites.',
  );
  process.exit(1);
}

// Only warnings, no errors
process.exit(0);
