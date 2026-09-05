import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

assert.equal(process.platform, 'darwin', 'native AX fixture requires macOS');
const root = fileURLToPath(new URL('../', import.meta.url));
const dir = await mkdtemp(join(tmpdir(), 'openbitfun-ax-context-'));
const run = (cmd, args, env = process.env) => new Promise((resolve, reject) => {
  const child = spawn(cmd, args, { cwd: root, stdio: 'inherit', windowsHide: true, env });
  child.on('error', reject);
  child.on('close', code => code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`)));
});
let fixture;
try {
  const executable = join(dir, 'ax-fixture');
  await run('swiftc', [join(root, 'scripts/fixtures/computer-use-ax.swift'), '-o', executable]);
  fixture = spawn(executable, [], { cwd: root, stdio: ['ignore', 'pipe', 'inherit'], windowsHide: true });
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('AX fixture did not become ready')), 15000);
    fixture.on('error', error => { clearTimeout(timeout); reject(error); });
    fixture.on('exit', code => { clearTimeout(timeout); reject(new Error(`AX fixture exited ${code}`)); });
    fixture.stdout.on('data', chunk => { if (String(chunk).includes('READY')) { clearTimeout(timeout); resolve(); } });
  });
  await run('cargo', ['test', '-p', 'openbitfun-desktop', '--lib', 'native_ax_fixture_round_trips_tree_and_cached_targets', '--', '--ignored'], {
    ...process.env, OPENBITFUN_AX_FIXTURE_PID: String(fixture.pid),
  });
} finally {
  if (fixture && fixture.exitCode === null) {
    await new Promise(resolve => { fixture.once('exit', resolve); fixture.kill('SIGTERM'); });
  }
  await rm(dir, { recursive: true, force: true });
}
