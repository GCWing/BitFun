import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');

test('relay archive contains the runtime and admin binaries plus static assets', () => {
  const packageScript = read('scripts/relay/package-unix.sh');
  assert.match(packageScript, /bitfun-relay-server/);
  assert.match(packageScript, /relay-admin/);
  assert.match(packageScript, /src\/apps\/relay-server\/static/);
  assert.match(packageScript, /\/health/);
  assert.match(packageScript, /\.sha256/);
});

test('formal and nightly releases gate publication on Linux binaries', () => {
  const desktop = read('.github/workflows/desktop-package.yml');
  const nightly = read('.github/workflows/nightly.yml');
  const reusable = read('.github/workflows/linux-binaries.yml');

  for (const workflow of [desktop, nightly]) {
    assert.match(workflow, /uses:\s+\.\/\.github\/workflows\/linux-binaries\.yml/);
    assert.match(workflow, /needs:\s*\[[^\]]*linux-binaries[^\]]*\]/);
    assert.match(workflow, /bitfun-relay-server-\*\.tar\.gz/);
    assert.match(workflow, /bitfun-cli-\*\.tar\.gz/);
    assert.match(workflow, /linux-binaries\.json/);
  }

  assert.match(reusable, /ubuntu-24\.04-arm/);
  assert.match(reusable, /aarch64-unknown-linux-gnu/);
  assert.match(reusable, /x86_64-unknown-linux-gnu/);
  assert.match(reusable, /scripts\/relay\/package-unix\.sh/);
  assert.match(reusable, /scripts\/cli\/package-unix\.sh/);
});

test('exactly one workflow publishes the Linux CLI archives', () => {
  // Both cli-package.yml and desktop-package.yml run on `release: published`.
  // If both built Linux they would upload identical asset names concurrently,
  // and softprops/action-gh-release deletes a same-named asset before writing,
  // so the two runs can destroy each other's upload.
  const cli = read('.github/workflows/cli-package.yml');

  assert.doesNotMatch(cli, /target:\s*x86_64-unknown-linux-gnu/);
  assert.doesNotMatch(cli, /target:\s*aarch64-unknown-linux-gnu/);
  assert.doesNotMatch(cli, /uses:\s+\.\/\.github\/workflows\/linux-binaries\.yml/);

  // macOS and Windows stay owned by cli-package.yml.
  assert.match(cli, /target:\s*aarch64-apple-darwin/);
  assert.match(cli, /target:\s*x86_64-apple-darwin/);
  assert.match(cli, /target:\s*x86_64-pc-windows-msvc/);
});

test('release asset names carry no SemVer build metadata', () => {
  // GitHub rewrites `+` in stored asset filenames, which would make every URL
  // in linux-binaries.json a 404 on the nightly channel.
  const reusable = read('.github/workflows/linux-binaries.yml');
  const nightly = read('.github/workflows/nightly.yml');

  assert.match(reusable, /ASSET_VERSION="\$\{RELEASE_VERSION%%\+\*\}"/);
  assert.match(reusable, /package-unix\.sh "\$ASSET_VERSION"/);
  assert.doesNotMatch(reusable, /package-unix\.sh "\$VERSION"/);
  assert.match(nightly, /--version "\$\{NIGHTLY_VERSION%%\+\*\}"/);
});
