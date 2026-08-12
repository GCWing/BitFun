import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { collectPortableEntries, main } from './package-windows-assets.mjs';

function makeFakeReleaseDir() {
  const dir = mkdtempSync(join(tmpdir(), 'pwa-assets-'));
  writeFileSync(join(dir, 'bitfun-desktop.exe'), 'fake exe bytes');
  writeFileSync(join(dir, 'THIRD_PARTY_NOTICES.md'), '# notices');
  writeFileSync(join(dir, 'bitfun_desktop.pdb'), 'debug symbols');
  writeFileSync(join(dir, 'bitfun-desktop.d'), 'dep file');
  writeFileSync(join(dir, '.cargo-lock'), '');
  mkdirSync(join(dir, 'mobile-web', 'dist'), { recursive: true });
  writeFileSync(join(dir, 'mobile-web', 'dist', 'index.html'), '<html></html>');
  mkdirSync(join(dir, 'resources'), { recursive: true });
  writeFileSync(join(dir, 'resources', 'worker_host.js'), 'worker');
  mkdirSync(join(dir, 'third-party', 'models.dev'), { recursive: true });
  writeFileSync(join(dir, 'third-party', 'models.dev', 'LICENSE.txt'), 'license');
  // Non-runtime build dirs that must be excluded.
  mkdirSync(join(dir, 'deps'));
  mkdirSync(join(dir, 'build'));
  mkdirSync(join(dir, 'incremental'));
  mkdirSync(join(dir, '.fingerprint'));
  return dir;
}

test('collectPortableEntries includes exe, notice, and runtime dirs only', () => {
  const dir = makeFakeReleaseDir();
  try {
    const entries = collectPortableEntries(dir, 'bitfun-desktop.exe');
    const names = entries
      .map((entry) => entry.replace(dir, '').replace(/\\/g, '/'))
      .sort();
    assert.deepEqual(names, [
      '/THIRD_PARTY_NOTICES.md',
      '/bitfun-desktop.exe',
      '/mobile-web',
      '/resources',
      '/third-party',
    ]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('collectPortableEntries excludes pdb/d/cargo-lock/build dirs', () => {
  const dir = makeFakeReleaseDir();
  try {
    const entries = collectPortableEntries(dir, 'bitfun-desktop.exe');
    const flat = JSON.stringify(entries);
    for (const excluded of ['bitfun_desktop.pdb', 'bitfun-desktop.d', '.cargo-lock', 'deps', 'build', 'incremental', '.fingerprint']) {
      assert.ok(!flat.includes(excluded), `must exclude ${excluded}`);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('collectPortableEntries returns empty for empty release dir', () => {
  const dir = mkdtempSync(join(tmpdir(), 'pwa-empty-'));
  try {
    const entries = collectPortableEntries(dir, 'bitfun-desktop.exe');
    assert.deepEqual(entries, []);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('main refuses to delete a non-release-asset out-dir (d8-P2-1)', async () => {
  const root = mkdtempSync(join(tmpdir(), 'pwa-outguard-'));
  const outDir = join(root, 'danger');
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, 'keep.txt'), 'unrelated file');
  const releaseDir = makeFakeReleaseDir();
  const installer = join(root, 'bitfun-desktop-installer.exe');
  writeFileSync(installer, 'installer bytes');
  try {
    await assert.rejects(
      main([
        '--installer', installer,
        '--app-release-dir', releaseDir,
        '--version', '0.2.16',
        '--out-dir', outDir,
      ]),
      /contains unexpected file/,
    );
    // The unrelated file must survive.
    assert.equal(readdirSync(outDir).includes('keep.txt'), true);
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(releaseDir, { recursive: true, force: true });
  }
});

test('main allows re-using a clean release-assets out-dir (d8-P2-1)', async () => {
  const root = mkdtempSync(join(tmpdir(), 'pwa-reuse-'));
  const outDir = join(root, 'assets');
  mkdirSync(outDir, { recursive: true });
  // Old assets from a previous run are allowed.
  writeFileSync(join(outDir, 'BitFun_0.2.15_windows-x86_64-portable.zip'), 'old zip');
  writeFileSync(join(outDir, 'SHA256SUMS'), 'old sums');
  const releaseDir = makeFakeReleaseDir();
  const installer = join(root, 'bitfun-desktop-installer.exe');
  writeFileSync(installer, 'installer bytes');
  try {
    await main([
      '--installer', installer,
      '--app-release-dir', releaseDir,
      '--version', '0.2.16',
      '--out-dir', outDir,
    ]);
    // Old assets replaced by the new run.
    assert.equal(readdirSync(outDir).includes('BitFun_0.2.15_windows-x86_64-portable.zip'), false);
    assert.equal(readdirSync(outDir).some((n) => n.startsWith('BitFun_0.2.16_')), true);
  } finally {
    rmSync(root, { recursive: true, force: true });
    rmSync(releaseDir, { recursive: true, force: true });
  }
});
