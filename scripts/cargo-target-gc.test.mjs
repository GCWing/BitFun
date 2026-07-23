import assert from 'node:assert/strict';
import { existsSync, mkdirSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import {
  collectGcPlan,
  extractDepsArtifactHash,
  profileFromTauriBuildArgs,
  runCargoTargetGc,
  selectStaleByMtime,
  splitFingerprintDir,
  splitIncrementalCrateDir,
  targetFromTauriBuildArgs,
} from './cargo-target-gc.mjs';

function fixtureRoot() {
  const root = join(tmpdir(), `bitfun-target-gc-${process.pid}-${Date.now()}`);
  mkdirSync(root, { recursive: true });
  return {
    root,
    cleanup: () => rmSync(root, { force: true, recursive: true }),
  };
}

function touchDir(path, mtimeMs) {
  mkdirSync(path, { recursive: true });
  const date = new Date(mtimeMs);
  utimesSync(path, date, date);
}

function touchFile(path, mtimeMs) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, 'x');
  const date = new Date(mtimeMs);
  utimesSync(path, date, date);
}

test('split helpers parse cargo cache names', () => {
  assert.deepEqual(splitIncrementalCrateDir('bitfun_core-3vwcc7dt79hqo'), {
    crate: 'bitfun_core',
    hash: '3vwcc7dt79hqo',
  });
  assert.deepEqual(splitFingerprintDir('aes-gcm-150676ea617fdfd1'), {
    stem: 'aes-gcm',
    hash: '150676ea617fdfd1',
  });
  assert.equal(extractDepsArtifactHash('libsyn-0ddb3bc374064a9b.rlib'), '0ddb3bc374064a9b');
  assert.equal(
    extractDepsArtifactHash('bitfun_core-023db5b6b08d0150.02dradpmwvb53dgn1eck0186y.rcgu.o'),
    '023db5b6b08d0150'
  );
});

test('selectStaleByMtime keeps newest entries', () => {
  const stale = selectStaleByMtime(
    [
      { path: 'a', mtimeMs: 1 },
      { path: 'b', mtimeMs: 3 },
      { path: 'c', mtimeMs: 2 },
    ],
    1
  );
  assert.deepEqual(new Set(stale), new Set(['a', 'c']));
});

test('collectGcPlan prunes incremental only and never drops live fingerprints', () => {
  const { root, cleanup } = fixtureRoot();
  try {
    const profileDir = join(root, 'debug');
    const now = Date.now();

    touchDir(join(profileDir, 'incremental', 'bitfun_core-oldhash1'), now - 3_000);
    touchDir(join(profileDir, 'incremental', 'bitfun_core-newhash2'), now);
    touchDir(
      join(profileDir, 'incremental', 'bitfun_core-newhash2', 's-old-session'),
      now - 2_000
    );
    touchDir(
      join(profileDir, 'incremental', 'bitfun_core-newhash2', 's-new-session'),
      now
    );

    // Multiple fingerprint units for the same stem can all be live for Cargo
    // (lib / build-script / feature variants). GC must not mtime-prune them.
    touchDir(join(profileDir, '.fingerprint', 'bitfun-core-aaaaaaaaaaaaaaaa'), now - 3_000);
    touchDir(join(profileDir, '.fingerprint', 'bitfun-core-bbbbbbbbbbbbbbbb'), now);
    touchDir(join(profileDir, '.fingerprint', 'syn-1111111111111111'), now - 4_000);
    touchDir(join(profileDir, '.fingerprint', 'syn-2222222222222222'), now - 2_000);
    touchDir(join(profileDir, '.fingerprint', 'syn-3333333333333333'), now);

    touchFile(join(profileDir, 'deps', 'libbitfun_core-aaaaaaaaaaaaaaaa.rlib'), now - 3_000);
    touchFile(join(profileDir, 'deps', 'libbitfun_core-bbbbbbbbbbbbbbbb.rlib'), now);
    touchFile(join(profileDir, 'deps', 'libsyn-1111111111111111.rlib'), now - 4_000);
    touchFile(join(profileDir, 'deps', 'libsyn-2222222222222222.rlib'), now - 2_000);
    touchFile(join(profileDir, 'deps', 'libsyn-3333333333333333.rlib'), now);
    // True orphan: no matching fingerprint directory remains.
    touchFile(join(profileDir, 'deps', 'liborphan-ffffffffffffffff.rlib'), now - 5_000);

    const plan = collectGcPlan(profileDir);

    assert.ok(plan.incremental.some((path) => path.endsWith('bitfun_core-oldhash1')));
    assert.ok(
      plan.incremental.some((path) =>
        path.includes(`${join('bitfun_core-newhash2', 's-old-session')}`)
      )
    );
    assert.equal(plan.fingerprint.length, 0);
    assert.ok(
      !plan.deps.some((path) => path.endsWith('libbitfun_core-aaaaaaaaaaaaaaaa.rlib'))
    );
    assert.ok(!plan.deps.some((path) => path.endsWith('libsyn-1111111111111111.rlib')));
    assert.ok(!plan.deps.some((path) => path.endsWith('libsyn-2222222222222222.rlib')));
    assert.ok(!plan.deps.some((path) => path.endsWith('libsyn-3333333333333333.rlib')));
    assert.ok(plan.deps.some((path) => path.endsWith('liborphan-ffffffffffffffff.rlib')));
  } finally {
    cleanup();
  }
});

test('runCargoTargetGc keeps fingerprint-backed deps so next cargo can reuse them', () => {
  const { root, cleanup } = fixtureRoot();
  try {
    const targetDir = join(root, 'target');
    const profileDir = join(targetDir, 'debug');
    const now = Date.now();
    touchDir(join(profileDir, 'incremental', 'bitfun_demo-old'), now - 1_000);
    touchDir(join(profileDir, 'incremental', 'bitfun_demo-new'), now);
    touchDir(join(profileDir, '.fingerprint', 'bitfun-demo-aaaaaaaaaaaaaaaa'), now - 1_000);
    touchDir(join(profileDir, '.fingerprint', 'bitfun-demo-bbbbbbbbbbbbbbbb'), now);
    touchFile(join(profileDir, 'deps', 'libbitfun_demo-aaaaaaaaaaaaaaaa.rlib'), now - 1_000);
    touchFile(join(profileDir, 'deps', 'libbitfun_demo-bbbbbbbbbbbbbbbb.rlib'), now);
    touchFile(join(profileDir, 'deps', 'libghost-cccccccccccccccc.rlib'), now - 2_000);

    const dry = runCargoTargetGc({
      rootDir: root,
      targetDir,
      profile: 'debug',
      dryRun: true,
      skipIfBusy: false,
      logger: { info() {}, warn() {} },
    });
    assert.equal(dry.dryRun, true);
    assert.ok(dry.counts.total >= 2);
    assert.ok(existsSync(join(profileDir, 'incremental', 'bitfun_demo-old')));

    const live = runCargoTargetGc({
      rootDir: root,
      targetDir,
      profile: 'debug',
      dryRun: false,
      skipIfBusy: false,
      logger: { info() {}, warn() {} },
    });
    assert.equal(live.skipped, false);
    assert.equal(existsSync(join(profileDir, 'incremental', 'bitfun_demo-old')), false);
    assert.equal(existsSync(join(profileDir, 'incremental', 'bitfun_demo-new')), true);
    assert.equal(
      existsSync(join(profileDir, '.fingerprint', 'bitfun-demo-aaaaaaaaaaaaaaaa')),
      true
    );
    assert.equal(
      existsSync(join(profileDir, 'deps', 'libbitfun_demo-aaaaaaaaaaaaaaaa.rlib')),
      true
    );
    assert.equal(
      existsSync(join(profileDir, 'deps', 'libbitfun_demo-bbbbbbbbbbbbbbbb.rlib')),
      true
    );
    assert.equal(existsSync(join(profileDir, 'deps', 'libghost-cccccccccccccccc.rlib')), false);
  } finally {
    cleanup();
  }
});

test('tauri build argv helpers resolve profile and target', () => {
  assert.equal(profileFromTauriBuildArgs(['--debug']), 'debug');
  assert.equal(profileFromTauriBuildArgs(['--profile', 'release-fast']), 'release-fast');
  assert.equal(profileFromTauriBuildArgs([]), 'release');
  assert.equal(targetFromTauriBuildArgs(['--target', 'aarch64-apple-darwin']), 'aarch64-apple-darwin');
  assert.equal(targetFromTauriBuildArgs([]), null);
});
