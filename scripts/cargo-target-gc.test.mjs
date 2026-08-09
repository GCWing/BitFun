import assert from 'node:assert/strict';
import { existsSync, mkdirSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import {
  collectGcPlan,
  extractDepsArtifactHash,
  isTargetProfileBusy,
  parseGcArgs,
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

function touchFingerprint(
  profileDir,
  dirName,
  unitName,
  metadata,
  mtimeMs,
  invokedMtimeMs = mtimeMs
) {
  const fingerprintDir = join(profileDir, '.fingerprint', dirName);
  mkdirSync(fingerprintDir, { recursive: true });
  const unitPath = join(fingerprintDir, `${unitName}.json`);
  writeFileSync(unitPath, JSON.stringify(metadata));
  const invokedPath = join(fingerprintDir, 'invoked.timestamp');
  writeFileSync(invokedPath, '');
  const date = new Date(mtimeMs);
  utimesSync(unitPath, date, date);
  const invokedDate = new Date(invokedMtimeMs);
  utimesSync(invokedPath, invokedDate, invokedDate);
  utimesSync(fingerprintDir, date, date);
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

test('collectGcPlan keeps distinct Cargo units while pruning stale generations', () => {
  const { root, cleanup } = fixtureRoot();
  try {
    const profileDir = join(root, 'debug');
    const now = Date.now();
    const dayMs = 24 * 60 * 60 * 1_000;

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

    const libUnit = { target: 1, profile: 2, path: 3, compile_kind: 0 };
    touchFingerprint(
      profileDir,
      'bitfun-core-aaaaaaaaaaaaaaaa',
      'lib-bitfun_core',
      { ...libUnit, features: '["old"]' },
      now - 3 * dayMs
    );
    touchFingerprint(
      profileDir,
      'bitfun-core-bbbbbbbbbbbbbbbb',
      'lib-bitfun_core',
      { ...libUnit, features: '["latest"]' },
      now
    );
    // A fingerprint reused by Cargo stays warm through invoked.timestamp even
    // when the directory itself is old.
    touchFingerprint(
      profileDir,
      'bitfun-core-cccccccccccccccc',
      'lib-bitfun_core',
      { ...libUnit, features: '["recent"]' },
      now - 3 * dayMs,
      now - 60 * 60 * 1_000
    );
    // Test units are a distinct Cargo unit and remain independently reusable.
    touchFingerprint(
      profileDir,
      'bitfun-core-dddddddddddddddd',
      'test-lib-bitfun_core',
      { target: 1, profile: 4, path: 3, compile_kind: 0 },
      now - 4 * dayMs
    );
    // An old incomplete fingerprint is abandoned output.
    touchDir(
      join(profileDir, '.fingerprint', 'bitfun-core-eeeeeeeeeeeeeeee'),
      now - 5 * dayMs
    );

    touchFile(join(profileDir, 'deps', 'libbitfun_core-aaaaaaaaaaaaaaaa.rlib'), now);
    touchFile(join(profileDir, 'deps', 'libbitfun_core-bbbbbbbbbbbbbbbb.rlib'), now);
    touchFile(join(profileDir, 'deps', 'libbitfun_core-cccccccccccccccc.rlib'), now);
    touchFile(join(profileDir, 'deps', 'libbitfun_core-dddddddddddddddd.rlib'), now);
    touchFile(join(profileDir, 'deps', 'libbitfun_core-eeeeeeeeeeeeeeee.rlib'), now);

    touchDir(join(profileDir, 'build', 'bitfun-core-aaaaaaaaaaaaaaaa'), now);
    touchDir(join(profileDir, 'build', 'bitfun-core-bbbbbbbbbbbbbbbb'), now);
    touchDir(join(profileDir, 'build', 'bitfun-core-eeeeeeeeeeeeeeee'), now);

    const plan = collectGcPlan(profileDir, { now, fingerprintMinAgeMs: dayMs });

    assert.ok(plan.incremental.some((path) => path.endsWith('bitfun_core-oldhash1')));
    assert.ok(
      plan.incremental.some((path) =>
        path.includes(`${join('bitfun_core-newhash2', 's-old-session')}`)
      )
    );
    assert.ok(plan.fingerprint.some((path) => path.endsWith('bitfun-core-aaaaaaaaaaaaaaaa')));
    assert.ok(plan.fingerprint.some((path) => path.endsWith('bitfun-core-eeeeeeeeeeeeeeee')));
    assert.ok(!plan.fingerprint.some((path) => path.endsWith('bitfun-core-bbbbbbbbbbbbbbbb')));
    assert.ok(!plan.fingerprint.some((path) => path.endsWith('bitfun-core-cccccccccccccccc')));
    assert.ok(!plan.fingerprint.some((path) => path.endsWith('bitfun-core-dddddddddddddddd')));
    assert.ok(plan.deps.some((path) => path.endsWith('libbitfun_core-aaaaaaaaaaaaaaaa.rlib')));
    assert.ok(plan.deps.some((path) => path.endsWith('libbitfun_core-eeeeeeeeeeeeeeee.rlib')));
    assert.ok(!plan.deps.some((path) => path.endsWith('libbitfun_core-bbbbbbbbbbbbbbbb.rlib')));
    assert.ok(!plan.deps.some((path) => path.endsWith('libbitfun_core-cccccccccccccccc.rlib')));
    assert.ok(!plan.deps.some((path) => path.endsWith('libbitfun_core-dddddddddddddddd.rlib')));
    assert.ok(plan.build.some((path) => path.endsWith('bitfun-core-aaaaaaaaaaaaaaaa')));
    assert.ok(plan.build.some((path) => path.endsWith('bitfun-core-eeeeeeeeeeeeeeee')));
    assert.ok(!plan.build.some((path) => path.endsWith('bitfun-core-bbbbbbbbbbbbbbbb')));
  } finally {
    cleanup();
  }
});

test('runCargoTargetGc prunes old generations and honors dry-run', () => {
  const { root, cleanup } = fixtureRoot();
  try {
    const targetDir = join(root, 'target');
    const profileDir = join(targetDir, 'debug');
    const now = Date.now();
    touchDir(join(profileDir, 'incremental', 'bitfun_demo-old'), now - 1_000);
    touchDir(join(profileDir, 'incremental', 'bitfun_demo-new'), now);
    const unit = { target: 1, profile: 2, path: 3, compile_kind: 0 };
    touchFingerprint(
      profileDir,
      'bitfun-demo-aaaaaaaaaaaaaaaa',
      'lib-bitfun_demo',
      unit,
      now - 1_000
    );
    touchFingerprint(
      profileDir,
      'bitfun-demo-bbbbbbbbbbbbbbbb',
      'lib-bitfun_demo',
      unit,
      now
    );
    touchFile(join(profileDir, 'deps', 'libbitfun_demo-aaaaaaaaaaaaaaaa.rlib'), now - 1_000);
    touchFile(join(profileDir, 'deps', 'libbitfun_demo-bbbbbbbbbbbbbbbb.rlib'), now);
    touchFile(join(profileDir, 'deps', 'libghost-cccccccccccccccc.rlib'), now - 2_000);

    const dry = runCargoTargetGc({
      rootDir: root,
      targetDir,
      profile: 'debug',
      dryRun: true,
      fingerprintMinAgeHours: 0,
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
      fingerprintMinAgeHours: 0,
      skipIfBusy: false,
      logger: { info() {}, warn() {} },
    });
    assert.equal(live.skipped, false);
    assert.equal(existsSync(join(profileDir, 'incremental', 'bitfun_demo-old')), false);
    assert.equal(existsSync(join(profileDir, 'incremental', 'bitfun_demo-new')), true);
    assert.equal(
      existsSync(join(profileDir, '.fingerprint', 'bitfun-demo-aaaaaaaaaaaaaaaa')),
      false
    );
    assert.equal(
      existsSync(join(profileDir, 'deps', 'libbitfun_demo-aaaaaaaaaaaaaaaa.rlib')),
      false
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

test('target busy detection scopes Cargo locks to the selected profile', () => {
  const { root, cleanup } = fixtureRoot();
  try {
    const profileDir = join(root, 'target', 'debug');
    touchFile(join(profileDir, '.cargo-lock'), Date.now());

    assert.equal(
      isTargetProfileBusy({
        profileDir,
        platform: 'darwin',
        exec(command) {
          assert.equal(command, 'lsof');
          return '123\n';
        },
      }),
      true
    );

    assert.equal(
      isTargetProfileBusy({
        profileDir,
        platform: 'darwin',
        exec(command) {
          assert.equal(command, 'lsof');
          const error = new Error('no open files');
          error.status = 1;
          throw error;
        },
      }),
      false
    );
  } finally {
    cleanup();
  }
});

test('dry-run environment remains effective when CLI omits the flag', () => {
  const { root, cleanup } = fixtureRoot();
  const previous = process.env.BITFUN_TARGET_GC_DRY_RUN;
  try {
    const targetDir = join(root, 'target');
    const profileDir = join(targetDir, 'debug');
    touchDir(join(profileDir, 'incremental', 'bitfun_demo-old'), 1);
    touchDir(join(profileDir, 'incremental', 'bitfun_demo-new'), 2);
    process.env.BITFUN_TARGET_GC_DRY_RUN = '1';

    const result = runCargoTargetGc({
      rootDir: root,
      targetDir,
      profile: 'debug',
      skipIfBusy: false,
      logger: { info() {}, warn() {} },
    });

    assert.equal(parseGcArgs([]).dryRun, undefined);
    assert.equal(result.dryRun, true);
    assert.equal(existsSync(join(profileDir, 'incremental', 'bitfun_demo-old')), true);
  } finally {
    if (previous === undefined) {
      delete process.env.BITFUN_TARGET_GC_DRY_RUN;
    } else {
      process.env.BITFUN_TARGET_GC_DRY_RUN = previous;
    }
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
