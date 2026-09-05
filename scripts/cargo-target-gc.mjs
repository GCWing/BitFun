#!/usr/bin/env node
/**
 * Prune Cargo target caches after desktop:dev exits and desktop:build ends.
 *
 * Safe policy:
 * - incremental: keep latest crate root (+ latest session)
 * - .fingerprint: keep the latest generation per Cargo unit identity, plus a
 *   short grace window for recently-built feature variants
 * - deps / build: delete hashes with no remaining fingerprint directory
 *
 * Env:
 *   OPENBITFUN_TARGET_GC=0          disable
 *   OPENBITFUN_TARGET_GC_DRY_RUN=1  report only
 *   OPENBITFUN_TARGET_GC_MIN_AGE_HOURS=24
 */
import { execFileSync } from 'node:child_process';
import {
  existsSync,
  lstatSync,
  readFileSync,
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
const DEFAULT_FINGERPRINT_MIN_AGE_MS = 24 * 60 * 60 * 1_000;

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

function fingerprintActivityMtimeMs(fingerprintPath) {
  // Cargo refreshes invoked.timestamp even when a fingerprint is reused, while
  // the directory mtime can remain unchanged for days. The stamp is therefore
  // the authoritative activity signal; the directory time covers incomplete
  // fingerprints that do not have a stamp yet.
  return Math.max(
    safeStatMtimeMs(join(fingerprintPath, 'invoked.timestamp')),
    safeStatMtimeMs(fingerprintPath)
  );
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

function readFingerprintUnitIdentity(fingerprintPath, stem) {
  const jsonNames = listFiles(fingerprintPath)
    .filter((name) => name.endsWith('.json'))
    .sort();
  if (jsonNames.length === 0) {
    return null;
  }

  const units = [];
  for (const name of jsonNames) {
    try {
      const metadata = JSON.parse(readFileSync(join(fingerprintPath, name), 'utf8'));
      // These fields identify the Cargo unit itself. Intentionally exclude
      // features, dependency hashes, rustflags and config: changes to those
      // fields create a new generation of the same unit, which is precisely
      // the history this GC needs to bound.
      units.push([
        name,
        metadata.target ?? null,
        metadata.profile ?? null,
        metadata.path ?? null,
        metadata.compile_kind ?? null,
      ]);
    } catch {
      // An unreadable fingerprint may be in the middle of being written.
      // Defer to the caller's age-based incomplete-entry policy.
      return null;
    }
  }

  return JSON.stringify([stem, units]);
}

/**
 * Keep the latest generation of each distinct Cargo unit.
 *
 * A package stem alone is unsafe because Cargo can concurrently own lib,
 * test-lib, bin and build-script units. Cargo's fingerprint JSON supplies a
 * stable unit identity; feature/dependency changes are treated as generations
 * of that unit. Recently-created generations stay inside a grace window so a
 * just-finished multi-command workflow remains warm.
 */
export function planFingerprintPrune(
  fingerprintDir,
  { now = Date.now(), minAgeMs = DEFAULT_FINGERPRINT_MIN_AGE_MS } = {}
) {
  const toDelete = [];
  const keptHashes = new Set();
  const groups = new Map();

  for (const name of listDirs(fingerprintDir)) {
    const split = splitFingerprintDir(name);
    if (!split) {
      continue;
    }

    const path = join(fingerprintDir, name);
    const mtimeMs = fingerprintActivityMtimeMs(path);
    const unitIdentity = readFingerprintUnitIdentity(path, split.stem);
    if (!unitIdentity) {
      // Incomplete fingerprints are safe to remove once old; unreadable recent
      // entries may still be in flight and remain protected by the grace period.
      if (now - mtimeMs >= minAgeMs) {
        toDelete.push(path);
      } else {
        keptHashes.add(split.hash);
      }
      continue;
    }

    const entries = groups.get(unitIdentity) || [];
    entries.push({ path, hash: split.hash, mtimeMs });
    groups.set(unitIdentity, entries);
  }

  for (const entries of groups.values()) {
    entries.sort((a, b) => b.mtimeMs - a.mtimeMs);
    entries.forEach((entry, index) => {
      const isLatest = index === 0;
      const isRecent = now - entry.mtimeMs < minAgeMs;
      if (isLatest || isRecent) {
        keptHashes.add(entry.hash);
      } else {
        toDelete.push(entry.path);
      }
    });
  }

  return { toDelete, keptHashes };
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

export function planBuildOrphanPrune(buildDir, keptHashes) {
  const toDelete = [];
  for (const name of listDirs(buildDir)) {
    const split = splitFingerprintDir(name);
    if (split && !keptHashes.has(split.hash)) {
      toDelete.push(join(buildDir, name));
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
      for (const imageName of ['cargo.exe', 'rustc.exe']) {
        const out = exec('tasklist', ['/FI', `IMAGENAME eq ${imageName}`, '/NH'], {
          encoding: 'utf8',
        });
        if (out.toLowerCase().includes(imageName)) {
          return true;
        }
      }
      return false;
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

export function isTargetProfileBusy({
  profileDir,
  exec = execFileSync,
  platform = process.platform,
} = {}) {
  if (platform !== 'win32' && profileDir) {
    const lockPaths = [
      join(profileDir, '.cargo-lock'),
      join(profileDir, '.cargo-build-lock'),
      join(profileDir, '.cargo-artifact-lock'),
    ].filter((path) => existsSync(path));

    if (lockPaths.length > 0) {
      try {
        const output = exec('lsof', ['-t', ...lockPaths], { encoding: 'utf8' });
        return Boolean(String(output).trim());
      } catch (error) {
        // lsof exits 1 when none of the named files are open. That is a scoped,
        // authoritative "not busy" result even if another worktree is compiling.
        if (error?.status === 1) {
          return false;
        }
        // lsof is optional; fall through to the conservative global fallback.
      }
    }
  }

  return isCompilerBusy({ exec, platform });
}

export function collectGcPlan(
  profileDir,
  { now = Date.now(), fingerprintMinAgeMs = DEFAULT_FINGERPRINT_MIN_AGE_MS } = {}
) {
  const incrementalDir = join(profileDir, 'incremental');
  const fingerprintDir = join(profileDir, '.fingerprint');
  const depsDir = join(profileDir, 'deps');
  const buildDir = join(profileDir, 'build');

  const incremental = planIncrementalPrune(incrementalDir);
  const fingerprintPlan = planFingerprintPrune(fingerprintDir, {
    now,
    minAgeMs: fingerprintMinAgeMs,
  });
  const deps = planDepsOrphanPrune(depsDir, fingerprintPlan.keptHashes);
  const build = planBuildOrphanPrune(buildDir, fingerprintPlan.keptHashes);

  return {
    incremental,
    fingerprint: fingerprintPlan.toDelete,
    deps,
    build,
    all: [...incremental, ...fingerprintPlan.toDelete, ...deps, ...build],
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
    skipIfBusy = true,
    logger = console,
  } = options;
  const dryRun =
    options.dryRun ??
    ['1', 'true', 'yes'].includes(
      String(process.env.OPENBITFUN_TARGET_GC_DRY_RUN ?? '').toLowerCase()
    );
  const enabled =
    options.enabled ??
    !['0', 'false', 'no'].includes(
      String(process.env.OPENBITFUN_TARGET_GC ?? '1').toLowerCase()
    );
  const configuredMinAgeHours = Number(
    options.fingerprintMinAgeHours ??
      process.env.OPENBITFUN_TARGET_GC_MIN_AGE_HOURS ??
      DEFAULT_FINGERPRINT_MIN_AGE_MS / (60 * 60 * 1_000)
  );
  const fingerprintMinAgeMs =
    Number.isFinite(configuredMinAgeHours) && configuredMinAgeHours >= 0
      ? configuredMinAgeHours * 60 * 60 * 1_000
      : DEFAULT_FINGERPRINT_MIN_AGE_MS;

  if (!enabled) {
    return { skipped: true, reason: 'disabled', removed: [] };
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

  if (skipIfBusy) {
    const busyDeadline = Date.now() + 15_000;
    while (isTargetProfileBusy({ profileDir })) {
      if (Date.now() >= busyDeadline) {
        logger.info?.(`[target-gc] Skipping: Cargo still uses ${profileDir}`);
        return { skipped: true, reason: 'compiler-busy', removed: [], profileDir };
      }
      sleepMs(500);
    }
  }

  const plan = collectGcPlan(profileDir, { fingerprintMinAgeMs });
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
      build: plan.build.length,
      total: plan.all.length,
    },
  };

  if (summary.counts.total > 0) {
    logger.info?.(
      `[target-gc] ${dryRun ? 'Would remove' : 'Removed'} ${summary.counts.total} stale cache path(s) ` +
        `(incremental=${summary.counts.incremental}, fingerprint=${summary.counts.fingerprint}, ` +
        `deps=${summary.counts.deps}, build=${summary.counts.build}) ` +
        `under ${profileDir}`
    );
  } else {
    logger.info?.(`[target-gc] No stale cache paths under ${profileDir}`);
  }

  return summary;
}

export function parseGcArgs(argv) {
  const args = { profile: 'debug', triple: null, dryRun: undefined, help: false };
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
  OPENBITFUN_TARGET_GC=0           disable
  OPENBITFUN_TARGET_GC_DRY_RUN=1   dry-run
  OPENBITFUN_TARGET_GC_MIN_AGE_HOURS=24
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
