import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { installerCommand } from './install-cli.mjs';

const windows = installerCommand('win32');
assert.equal(windows.command, 'powershell.exe');
assert.deepEqual(windows.args.slice(0, 4), [
  '-NoProfile',
  '-ExecutionPolicy',
  'Bypass',
  '-File',
]);
assert.equal(path.basename(windows.args.at(-1)), 'install.ps1');

for (const platform of ['darwin', 'linux']) {
  const unix = installerCommand(platform);
  assert.equal(unix.command, 'bash');
  assert.equal(path.basename(unix.args.at(-1)), 'install.sh');
}

assert.throws(() => installerCommand('aix'), /unsupported platform/i);

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const installPowerShell = fs.readFileSync(
  path.join(repoRoot, 'src/apps/cli/install.ps1'),
  'utf8',
);
const installShell = fs.readFileSync(
  path.join(repoRoot, 'src/apps/cli/install.sh'),
  'utf8',
);
const cliReadme = fs.readFileSync(
  path.join(repoRoot, 'src/apps/cli/README.md'),
  'utf8',
);

for (const content of [installPowerShell, installShell, cliReadme]) {
  assert.match(content, /open a new terminal/i);
}
assert.doesNotMatch(installShell, /Sourced ~\/\.(?:bashrc|zshrc) for this session/);
