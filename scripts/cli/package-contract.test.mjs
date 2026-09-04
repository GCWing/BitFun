import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');

for (const packageScript of [
  'scripts/cli/package-unix.sh',
  'scripts/cli/package-windows.ps1',
]) {
  const content = read(packageScript);
  assert.match(content, /src[\\/]apps[\\/]cli[\\/]README\.md/);
  assert.match(content, /PROJECT-README\.md/);
  assert.match(content, /THIRD_PARTY_NOTICES\.md/);
  assert.match(content, /models-dev\.LICENSE\.txt/);
  assert.match(content, /models-dev\.provenance\.json/);
  assert.match(content, /extension-host/);
  assert.match(content, /resources[\\/]ext-host/);
  assert.match(content, /extension-host\.js/);
}

for (const workflow of [
  '.github/workflows/cli-package.yml',
  '.github/workflows/cli-package-manual.yml',
  '.github/workflows/linux-binaries.yml',
  '.github/workflows/nightly-artifacts.yml',
]) {
  const content = read(workflow);
  assert.match(content, /oven-sh\/setup-bun@v2/);
  assert.match(content, /plugin Host resources/);
  assert.match(content, /plugin-host:prepare|extension-host/);
}

for (const workflow of [
  '.github/workflows/cli-package.yml',
  '.github/workflows/cli-package-manual.yml',
]) {
  const content = read(workflow);
  assert.match(content, /target-feature=\+crt-static/);
  assert.match(content, /scripts\/cli\/test-install-unix\.sh/);
  assert.match(content, /scripts\/cli\/test-install-windows\.ps1/);
}

const releaseWorkflow = read('.github/workflows/cli-package.yml');
assert.match(releaseWorkflow, /primary_binary:\s*"openbitfun"/);
assert.doesNotMatch(releaseWorkflow, /deprecated_binary|deprecated:\s*true/);
assert.match(releaseWorkflow, /post-publication/i);
assert.doesNotMatch(releaseWorkflow, /Homebrew release gate/i);
