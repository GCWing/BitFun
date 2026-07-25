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
