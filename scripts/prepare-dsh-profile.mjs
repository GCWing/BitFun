#!/usr/bin/env node

/**
 * Build the DeepSeek Harness bridge into the profile directory the desktop
 * bundle ships (`packages/dsh-acp/dist-profile`).
 *
 * OpenBitFun launches DeepSeek Harness as `dsh --profile openbitfun-acp`, and copies
 * that directory into the user's own dsh install on first use — see
 * `src/crates/interfaces/acp/src/client/dsh_profile.rs`. The build has to
 * happen here rather than in Cargo because it is a TypeScript compile.
 *
 * `packages/dsh-acp` is deliberately NOT a pnpm workspace member: it pins the
 * whole harness `0.1.0-rc.6` train, and making every contributor's
 * `pnpm install` fetch that is a poor trade when only the desktop bundle needs
 * it. So this installs the package from its own lockfile, here, once — unless
 * a developer already linked a local harness checkout, in which case that
 * checkout wins and no install runs.
 *
 * A failure here fails the build. An app that silently ships no bridge looks
 * exactly like an app that ships a working one until a user tries to start a
 * DeepSeek session, and that is the wrong place to find out. Set
 * `OPENBITFUN_SKIP_DSH_PROFILE=1` to opt out deliberately.
 *
 * Official `desktop:build` compiles this via `frontend:build-all`. desktop:dev
 * and cargo check do not: the profile is not a compile-time Tauri resource.
 * Local DeepSeek sessions run this script explicitly. A stamped profile whose
 * inputs have not changed is left alone — the same mtime short-circuit
 * mobile-web uses. Escape hatches: `--force` /
 * `OPENBITFUN_DSH_PROFILE_FORCE_BUILD=1`.
 */

import { spawnSync } from 'node:child_process';
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import path, { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const PACKAGE_DIR = path.join(ROOT_DIR, 'packages', 'dsh-acp');
const OUT_DIR = path.join(PACKAGE_DIR, 'dist-profile');
const STAMP_FILENAME = '.openbitfun-bridge.json';

/** One installed harness package is enough to tell a populated tree from a bare one. */
const DEPS_PROBE = path.join(PACKAGE_DIR, 'node_modules', '@deepseek-ai', 'dsh-app-boot');

const INPUT_IGNORED_DIRS = new Set([
  'node_modules',
  'lib',
  'dist-profile',
  '.sessions',
]);

/** Explains an empty dist-profile to whoever finds one in a build tree. */
const PLACEHOLDER = `This OpenBitFun build ships no DeepSeek Harness bridge.

OPENBITFUN_SKIP_DSH_PROFILE was set when this build ran, so packages/dsh-acp was
never compiled. OpenBitFun's profile resolver rejects this unstamped directory and
reports that the bridge is missing from the build.

To include it: run \`pnpm run prepare:dsh-profile\` without that variable.
`;

/**
 * Newest mtime among the profile build inputs (recursively for directories).
 * @param {string} entryPath
 * @returns {{ path: string, mtimeMs: number } | null}
 */
export function getNewestInputMtime(entryPath) {
  if (!existsSync(entryPath)) {
    return null;
  }

  const stat = lstatSync(entryPath);
  if (stat.isSymbolicLink()) {
    return null;
  }

  if (stat.isFile()) {
    return { path: entryPath, mtimeMs: stat.mtimeMs };
  }

  if (!stat.isDirectory()) {
    return null;
  }

  let newest = null;
  for (const entry of readdirSync(entryPath, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) {
      continue;
    }
    if (entry.isDirectory() && INPUT_IGNORED_DIRS.has(entry.name)) {
      continue;
    }
    const candidate = getNewestInputMtime(path.join(entryPath, entry.name));
    if (candidate && (!newest || candidate.mtimeMs > newest.mtimeMs)) {
      newest = candidate;
    }
  }

  return newest;
}

/**
 * Decide whether the shipped profile needs a rebuild.
 *
 * A directory is not enough: Tauri only needs the path to exist, but OpenBitFun's
 * resolver rejects an unstamped tree. The stamp `.openbitfun-bridge.json` is the
 * same file `scripts/build-profile.mjs` writes and ACP reads.
 *
 * @param {{
 *   packageDir?: string,
 *   outDir?: string,
 *   prepareScriptPath?: string,
 *   force?: boolean,
 * }} [options]
 * @returns {{ shouldBuild: boolean, reason: string }}
 */
export function getDshProfileRebuildPlan({
  packageDir = PACKAGE_DIR,
  outDir = path.join(packageDir, 'dist-profile'),
  prepareScriptPath = path.join(ROOT_DIR, 'scripts', 'prepare-dsh-profile.mjs'),
  force = false,
} = {}) {
  if (force) {
    return { shouldBuild: true, reason: 'Force rebuild requested for dsh-profile' };
  }

  const stampPath = path.join(outDir, STAMP_FILENAME);
  if (!existsSync(stampPath)) {
    return { shouldBuild: true, reason: 'dsh-profile stamp is missing' };
  }

  try {
    const stamp = JSON.parse(readFileSync(stampPath, 'utf8'));
    if (
      typeof stamp.profile !== 'string' ||
      stamp.profile === '' ||
      typeof stamp.content !== 'string' ||
      stamp.content === ''
    ) {
      return { shouldBuild: true, reason: 'dsh-profile stamp is incomplete' };
    }
  } catch {
    return { shouldBuild: true, reason: 'dsh-profile stamp is unreadable' };
  }

  const stampMtimeMs = statSync(stampPath).mtimeMs;
  const inputs = [
    path.join(packageDir, 'src'),
    path.join(packageDir, 'presets'),
    path.join(packageDir, 'cordis.yml'),
    path.join(packageDir, 'package.json'),
    path.join(packageDir, 'package-lock.json'),
    path.join(packageDir, 'tsconfig.build.json'),
    path.join(packageDir, 'scripts', 'build-profile.mjs'),
    prepareScriptPath,
  ];

  let newestInput = null;
  for (const input of inputs) {
    const candidate = getNewestInputMtime(input);
    if (candidate && (!newestInput || candidate.mtimeMs > newestInput.mtimeMs)) {
      newestInput = candidate;
    }
  }

  if (newestInput && newestInput.mtimeMs > stampMtimeMs) {
    return {
      shouldBuild: true,
      reason: `dsh-profile inputs changed since the last build (${path.relative(ROOT_DIR, newestInput.path)})`,
    };
  }

  return {
    shouldBuild: false,
    reason:
      'dsh-profile is up to date; skipping install/build (use --force or OPENBITFUN_DSH_PROFILE_FORCE_BUILD=1 to rebuild)',
  };
}

/**
 * Run a command in the bridge package directory.
 * @param {string} command - the executable to run.
 * @param {string[]} args - its arguments.
 * @returns {number} the exit status, with a missing status treated as a failure.
 */
function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: PACKAGE_DIR,
    shell: process.platform === 'win32',
    stdio: 'inherit',
    env: process.env,
  });
  return result.status ?? 1;
}

/**
 * Fail the build, saying what to do about it.
 * @param {string} message - what went wrong.
 * @param {number} status - the exit status to propagate.
 */
function fail(message, status) {
  process.stderr.write(`[dsh-profile] ${message}\n`);
  process.stderr.write(
    '[dsh-profile] set OPENBITFUN_SKIP_DSH_PROFILE=1 to build without the DeepSeek bridge\n',
  );
  process.exit(status === 0 ? 1 : status);
}

function main() {
  if (process.env.OPENBITFUN_SKIP_DSH_PROFILE === '1') {
    process.stdout.write('[dsh-profile] skipped (this build ships no DeepSeek bridge)\n');
    mkdirSync(OUT_DIR, { recursive: true });
    writeFileSync(path.join(OUT_DIR, 'NOT-BUILT.md'), PLACEHOLDER);
    return;
  }

  const force =
    process.argv.includes('--force') || process.env.OPENBITFUN_DSH_PROFILE_FORCE_BUILD === '1';
  const plan = getDshProfileRebuildPlan({ force });
  if (!plan.shouldBuild) {
    process.stdout.write(`[dsh-profile] ${plan.reason}\n`);
    return;
  }
  process.stdout.write(`[dsh-profile] ${plan.reason}\n`);

  if (!existsSync(DEPS_PROBE)) {
    process.stdout.write('[dsh-profile] installing the harness toolchain from package-lock.json\n');
    const installed = run('npm', ['ci', '--no-audit', '--no-fund']);
    if (installed !== 0) fail('npm ci failed', installed);
  }

  const compiled = run('npm', ['run', 'build']);
  if (compiled !== 0) fail('tsc failed', compiled);

  const packaged = run('node', ['scripts/build-profile.mjs']);
  if (packaged !== 0) fail('profile packaging failed', packaged);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
