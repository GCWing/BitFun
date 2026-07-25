import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const repoRoot = path.resolve(import.meta.dirname, '..');

test('generates GitHub URLs for both Linux CLI and Relay architectures', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-linux-manifest-'));
  const assets = path.join(temp, 'assets');
  const out = path.join(temp, 'linux-binaries.json');
  fs.mkdirSync(assets);

  for (const target of [
    'x86_64-unknown-linux-gnu',
    'aarch64-unknown-linux-gnu',
  ]) {
    for (const filename of [
      `bitfun-cli-1.2.3-${target}.tar.gz`,
      `bitfun-relay-server-${target}.tar.gz`,
    ]) {
      fs.writeFileSync(path.join(assets, filename), '');
      fs.writeFileSync(path.join(assets, `${filename}.sha256`), '');
    }
  }

  const result = spawnSync(
    process.execPath,
    [
      'scripts/generate-linux-binaries-manifest.mjs',
      '--assets-dir',
      assets,
      '--version',
      '1.2.3',
      '--tag',
      'v1.2.3',
      '--repo',
      'GCWing/BitFun',
      '--out',
      out,
    ],
    { cwd: repoRoot, encoding: 'utf8' }
  );
  assert.equal(result.status, 0, result.stderr);

  const manifest = JSON.parse(fs.readFileSync(out, 'utf8'));
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.platforms.linux_x86_64, undefined);
  assert.match(
    manifest.platforms['linux-x86_64'].cli.url,
    /releases\/download\/v1\.2\.3\/bitfun-cli-1\.2\.3-x86_64/
  );
  assert.match(
    manifest.platforms['linux-aarch64'].relay.sha256Url,
    /bitfun-relay-server-aarch64-unknown-linux-gnu\.tar\.gz\.sha256$/
  );
});

test('rejects versions whose build metadata GitHub would rewrite in asset names', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-linux-manifest-meta-'));
  const assets = path.join(temp, 'assets');
  const out = path.join(temp, 'linux-binaries.json');
  fs.mkdirSync(assets);

  const version = '1.2.3-nightly.20260724+abc1234';
  for (const target of ['x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu']) {
    for (const filename of [
      `bitfun-cli-${version}-${target}.tar.gz`,
      `bitfun-relay-server-${target}.tar.gz`,
    ]) {
      fs.writeFileSync(path.join(assets, filename), '');
      fs.writeFileSync(path.join(assets, `${filename}.sha256`), '');
    }
  }

  const result = spawnSync(
    process.execPath,
    [
      'scripts/generate-linux-binaries-manifest.mjs',
      '--assets-dir',
      assets,
      '--version',
      version,
      '--tag',
      'nightly',
      '--repo',
      'GCWing/BitFun',
      '--out',
      out,
    ],
    { cwd: repoRoot, encoding: 'utf8' }
  );
  assert.notEqual(result.status, 0, 'build metadata must not reach a release asset name');
  assert.match(result.stderr, /not preserved verbatim by GitHub/);
  assert.equal(fs.existsSync(out), false);
});

test('openbitfun sync mirrors both products and their checksums', () => {
  const syncScript = fs.readFileSync(
    path.join(repoRoot, 'scripts/openbitfun-release-sync.sh'),
    'utf8'
  );

  assert.match(syncScript, /linux-binaries\.json/);
  assert.match(syncScript, /for product in \("cli", "relay"\)/);
  assert.match(syncScript, /for key in \("url", "sha256Url"\)/);
  assert.match(syncScript, /OPENBITFUN_BASE_URL/);
  assert.match(syncScript, /WEBSITE_RELEASE_DIR.*linux-binaries\.json/);
});
