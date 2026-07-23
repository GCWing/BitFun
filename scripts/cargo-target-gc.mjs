#!/usr/bin/env node
/**
 * Prune Cargo target caches after desktop:dev exits and desktop:build ends.
 *
 * Safe policy:
 * - incremental: keep latest crate root (+ latest session)
 * - .fingerprint: never mtime-prune (Cargo needs multiple concurrent units)
 * - deps: delete only hashes with no remaining fingerprint directory
 *
 * Env:
 *   BITFUN_TARGET_GC=0          disable
 *   BITFUN_TARGET_GC_DRY_RUN=1  report only
 */
import { execFileSync } from 'node:child_process';
import {
  existsSync,
  lstatSync,
  readdirSync,
  rmSync,
  statSync,
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DEFAULT_ROOT = join(__dirname, '..');
const FINGERPRINT_HASH_RE = /^(.+)-([0-9a-f]{16})$/;
const DEPS_HASH_RE = /^.+?-([0-9a-f]{16})(?:[.-]|$)/;
const SESSION_DIR_RE = /^s-/;

export function splitIncrementalCrateDir(name) {
  const idx = name.lastIndexOf('-');
  if (idx <= 0 || idx === name.length - 1) {
    return null;
  }
  return {
    crate: name.slice(0, idx),
    hash: name.slice(idx + 1),
  };
}

export function splitFingerprintDir(name) {
  const match = FINGERPRINT_HASH_RE.exec(name);
  if (!match) {
    return null;
  }
  return {
    stem: match[1],
    hash: match[2],
  };
}

export function extractDepsArtifactHash(filename) {
  const match = DEPS_HASH_RE.exec(filename);
  return match ? match[1] : null;
}

function safeStatMtimeMs(path) {
  try {
    return statSync(path).mtimeMs;
  } catch {
    return 0;
  }
}

function listDirs(dir) {
  if (!existsSync(dir)) {
    return [];
  }
  return readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !entry.isSymbolicLink())
    .map((entry) => entry.name);
}

function listFiles(dir) {
  if (!existsSync(dir)) {
    return [];
  }
  return readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && !entry.isSymbolicLink())
    .map((entry) => entry.name);
}

function removePath(path, dryRun, removed) {
  removed.push(path);
  if (!dryRun) {
    rmSync(path, { force: true, recursive: true });
  }
}

/**
 * Keep the newest `keep` entries by mtime; return paths to delete.
 */
export function selectStaleByMtime(entries, keep) {
  if (keep < 1) {
    throw new Error('keep must be >= 1');
  }
  if (entries.length <= keep) {
    return [];
  }
  const sorted = [...entries].sort((a, b) => b.mtimeMs - a.mtimeMs);
  return sorted.slice(keep).map((entry) => entry.path);
}

export function planIncrementalPrune(incrementalDir, { keepSessions = 1 } = {}) {
  const toDelete = [];
  const groups = new Map();

  for (const name of listDirs(incrementalDir)) {
    const split = splitIncrementalCrateDir(name);
    if (!split) {
      continue;
    }
    const path = join(incrementalDir, name);
    const list = groups.get(split.crate) || [];
    list.push({ path, mtimeMs: safeStatMtimeMs(path), name });
    groups.set(split.crate, list);
  }

  for (const entries of groups.values()) {
    toDelete.push(...selectStaleByMtime(entries, 1));
  }

  const keptRoots = listDirs(incrementalDir)
    .map((name) => join(incrementalDir, name))
    .filter((path) => !toDelete.includes(path));

  for (const root of keptRoots) {
    const sessions = listDirs(root)
      .filter((name) => SESSION_DIR_RE.test(name) && !name.endsWith('-working'))
      .map((name) => ({
        path: join(root, name),
        mtimeMs: safeStatMtimeMs(join(root, name)),
      }));
    toDelete.push(...selectStaleByMtime(sessions, keepSessions));

    // Drop abandoned working dirs older than a few seconds if unlocked leftovers remain.
    for (const name of listDirs(root)) {
      if (!name.endsWith('-working')) {
        continue;
      }
      const path = join(root, name);
      if (Date.now() - safeStatMtimeMs(path) > 60_000) {
        toDelete.push(path);
      }
    }
  }

  return toDelete;
}

/**
 * Collect fingerprint metadata hashes that still exist on disk.
 *
 * Important: do NOT delete fingerprint directories by "keep newest N per stem".
 * Cargo routinely keeps multiple concurrent units for one package (lib,
 * build-script, feature variants). mtime is not a valid liveness signal, and
 * deleting a still-referenced fingerprint + its deps forces a cold rebuild of
 * that crate (and often a large share of the graph) on the next desktop:dev.
 */
export function planFingerprintPrune(fingerprintDir) {
  const keptHashes = new Set();
  for (const name of listDirs(fingerprintDir)) {
    const split = splitFingerprintDir(name);
    if (split) {
      keptHashes.add(split.hash);
    }
  }
  return { toDelete: [], keptHashes };
}

export function planDepsOrphanPrune(depsDir, keptHashes) {
  const toDelete = [];
  for (const name of listFiles(depsDir)) {
    const hash = extractDepsArtifactHash(name);
    if (!hash) {
      continue;
    }
    if (!keptHashes.has(hash)) {
      toDelete.push(join(depsDir, name));
    }
  }
  // Also remove empty-looking unit directories if any exist under deps.
  for (const name of listDirs(depsDir)) {
    const hash = extractDepsArtifactHash(name);
    if (hash && !keptHashes.has(hash)) {
      toDelete.push(join(depsDir, name));
    }
  }
  return toDelete;
}

export function resolveProfileDir(targetDir, { profile = 'debug', triple = null } = {}) {
  if (triple) {
    return join(targetDir, triple, profile);
  }
  return join(targetDir, profile);
}

function sleepMs(ms) {
  if (process.platform === 'win32') {
    // ping waits about 1s per iteration with -n 2; good enough for retry backoff.
    try {
      execFileSync('ping', ['127.0.0.1', '-n', '2'], { stdio: 'ignore' });
    } catch {
      // ignore
    }
    return;
  }
  try {
    execFileSync('sleep', [String(Math.max(0.1, ms / 1000))], { stdio: 'ignore' });
  } catch {
    // ignore
  }
}

export function isCompilerBusy({ exec = execFileSync, platform = process.platform } = {}) {
  try {
    if (platform === 'win32') {
      const out = exec(
        'cmd.exe',
        ['/d', '/s', '/c', 'tasklist /FI "IMAGENAME eq cargo.exe" & tasklist /FI "IMAGENAME eq rustc.exe"'],
        { encoding: 'utf8' }
      );
      return /\bcargo\.exe\b/i.test(out) || /\brustc\.exe\b/i.test(out);
    }
    const cargo = exec('pgrep', ['-x', 'cargo'], { encoding: 'utf8' }).trim();
    if (cargo) {
      return true;
    }
  } catch {
    // pgrep exit 1 => no match
  }
  try {
    if (platform !== 'win32') {
      const rustc = exec('pgrep', ['-x', 'rustc'], { encoding: 'utf8' }).trim();
      return Boolean(rustc);
    }
  } catch {
    // no rustc
  }
  return false;
}

export function collectGcPlan(profileDir) {
  const incrementalDir = join(profileDir, 'incremental');
  const fingerprintDir = join(profileDir, '.fingerprint');
  const depsDir = join(profileDir, 'deps');

  const incremental = planIncrementalPrune(incrementalDir);
  const fingerprintPlan = planFingerprintPrune(fingerprintDir);
  const deps = planDepsOrphanPrune(depsDir, fingerprintPlan.keptHashes);

  return {
    incremental,
    fingerprint: fingerprintPlan.toDelete,
    deps,
    all: [...incremental, ...fingerprintPlan.toDelete, ...deps],
  };
}

export function runCargoTargetGc(options = {}) {
  const {
    rootDir = DEFAULT_ROOT,
    targetDir = process.env.CARGO_TARGET_DIR
      ? resolve(rootDir, process.env.CARGO_TARGET_DIR)
      : join(rootDir, 'target'),
    profile = 'debug',
    triple = null,
    dryRun = ['1', 'true', 'yes'].includes(
      String(process.env.BITFUN_TARGET_GC_DRY_RUN || options.dryRun || '').toLowerCase()
    ),
    enabled = !['0', 'false', 'no'].includes(
      String(process.env.BITFUN_TARGET_GC ?? '1').toLowerCase()
    ),
    skipIfBusy = true,
    logger = console,
  } = options;

  if (!enabled) {
    return { skipped: true, reason: 'disabled', removed: [] };
  }

  if (skipIfBusy) {
    const busyDeadline = Date.now() + 15_000;
    while (isCompilerBusy()) {
      if (Date.now() >= busyDeadline) {
        logger.info?.('[target-gc] Skipping: cargo/rustc still running');
        return { skipped: true, reason: 'compiler-busy', removed: [] };
      }
      sleepMs(500);
    }
  }

  const profileDir = resolveProfileDir(targetDir, { profile, triple });
  if (!existsSync(profileDir)) {
    return { skipped: true, reason: 'missing-profile-dir', removed: [], profileDir };
  }

  // Refuse to operate on unexpected paths.
  try {
    if (!lstatSync(profileDir).isDirectory()) {
      return { skipped: true, reason: 'not-a-directory', removed: [], profileDir };
    }
  } catch {
    return { skipped: true, reason: 'stat-failed', removed: [], profileDir };
  }

  const plan = collectGcPlan(profileDir);
  const removed = [];
  for (const path of plan.all) {
    try {
      removePath(path, dryRun, removed);
    } catch (error) {
      logger.warn?.(
        `[target-gc] Failed to remove ${path}: ${error.message || String(error)}`
      );
    }
  }

  const summary = {
    skipped: false,
    dryRun,
    profileDir,
    removed,
    counts: {
      incremental: plan.incremental.length,
      fingerprint: plan.fingerprint.length,
      deps: plan.deps.length,
      total: plan.all.length,
    },
  };

  if (summary.counts.total > 0) {
    logger.info?.(
      `[target-gc] ${dryRun ? 'Would remove' : 'Removed'} ${summary.counts.total} stale cache path(s) ` +
        `(incremental=${summary.counts.incremental}, fingerprint=${summary.counts.fingerprint}, deps=${summary.counts.deps}) ` +
        `under ${profileDir}`
    );
  } else {
    logger.info?.(`[target-gc] No stale cache paths under ${profileDir}`);
  }

  return summary;
}

export function parseGcArgs(argv) {
  const args = { profile: 'debug', triple: null, dryRun: false, help: false };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else if (arg === '--dry-run') {
      args.dryRun = true;
    } else if (arg === '--profile') {
      args.profile = argv[i + 1] || args.profile;
      i += 1;
    } else if (arg.startsWith('--profile=')) {
      args.profile = arg.slice('--profile='.length);
    } else if (arg === '--target') {
      args.triple = argv[i + 1] || null;
      i += 1;
    } else if (arg.startsWith('--target=')) {
      args.triple = arg.slice('--target='.length);
    }
  }
  return args;
}

function printHelp() {
  console.log(`Usage: node scripts/cargo-target-gc.mjs [--profile debug] [--target TRIPLE] [--dry-run]

Prune stale Cargo incremental / fingerprint / deps caches for one profile.

Environment:
  BITFUN_TARGET_GC=0           disable
  BITFUN_TARGET_GC_DRY_RUN=1   dry-run
`);
}

export function profileFromTauriBuildArgs(args) {
  if (args.includes('--debug')) {
    return 'debug';
  }
  const inline = args.find((arg) => arg.startsWith('--profile='));
  if (inline) {
    return inline.slice('--profile='.length);
  }
  const idx = args.indexOf('--profile');
  if (idx >= 0 && args[idx + 1]) {
    return args[idx + 1];
  }
  return 'release';
}

export function targetFromTauriBuildArgs(args) {
  const inline = args.find((arg) => arg.startsWith('--target='));
  if (inline) {
    return inline.slice('--target='.length);
  }
  const idx = args.indexOf('--target');
  if (idx >= 0 && args[idx + 1]) {
    return args[idx + 1];
  }
  return null;
}

export function runGcBestEffort(options = {}) {
  try {
    return runCargoTargetGc(options);
  } catch (error) {
    const logger = options.logger || console;
    logger.warn?.(
      `[target-gc] Skipped due to error: ${error.message || String(error)}`
    );
    return { skipped: true, reason: 'error', error, removed: [] };
  }
}

const isMain = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
  const args = parseGcArgs(process.argv.slice(2));
  if (args.help) {
    printHelp();
    process.exit(0);
  }
  const result = runCargoTargetGc({
    profile: args.profile,
    triple: args.triple,
    dryRun: args.dryRun,
  });
  process.exit(result.skipped && result.reason === 'error' ? 1 : 0);
}
