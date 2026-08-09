import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '..');
const expectedVersion = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;

for (const [buildEnv, isDev] of [['production', false], ['development', true]]) {
  test(`generates ${buildEnv} version metadata explicitly`, () => {
    const outputRoot = fs.mkdtempSync(path.join(os.tmpdir(), `bitfun-version-${buildEnv}-`));
    const result = run(['--build-env', buildEnv, '--output-root', outputRoot]);
    assert.equal(result.status, 0, result.stderr);
    const generated = JSON.parse(
      fs.readFileSync(path.join(outputRoot, 'src/web-ui/public/version.json'), 'utf8')
    );
    assert.equal(generated.version, expectedVersion);
    assert.equal(generated.buildEnv, buildEnv);
    assert.equal(generated.isDev, isDev);
  });
}

test('fails instead of reusing stale metadata when build environment is missing', () => {
  const outputRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-version-missing-'));
  const result = run(['--output-root', outputRoot]);
  assert.notEqual(result.status, 0);
  assert.match(`${result.stdout}\n${result.stderr}`, /Expected --build-env/);
});

function run(args) {
  return spawnSync(process.execPath, ['scripts/generate-version.cjs', ...args], {
    cwd: root,
    encoding: 'utf8',
  });
}
