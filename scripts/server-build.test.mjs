import assert from 'node:assert/strict';
import path from 'node:path';
import { test } from 'node:test';

import { serverBuildPlan } from './server-build.mjs';

test('server release builds stage the plugin Host beside the binary', () => {
  const root = path.resolve('D:/workspace/bitfun-fixture');
  const plan = serverBuildPlan(root, {}, 'win32');

  assert.deepEqual(plan.cargoArgs, [
    'build',
    '--release',
    '--manifest-path',
    path.join(root, 'src', 'apps', 'server', 'Cargo.toml'),
  ]);
  assert.equal(plan.binaryPath, path.join(root, 'target', 'release', 'bitfun-server.exe'));
  assert.equal(
    plan.pluginHostDestination,
    path.join(root, 'target', 'release', 'resources', 'ext-host'),
  );
});

test('server release builds respect a configured Cargo target directory', () => {
  const root = path.resolve('D:/workspace/bitfun-fixture');
  const plan = serverBuildPlan(root, { CARGO_TARGET_DIR: 'build-output' }, 'linux');

  assert.equal(plan.binaryPath, path.join(root, 'build-output', 'release', 'bitfun-server'));
  assert.equal(
    plan.pluginHostDestination,
    path.join(root, 'build-output', 'release', 'resources', 'ext-host'),
  );
});
