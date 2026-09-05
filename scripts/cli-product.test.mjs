import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';

import { cliBuildPlan, stagePluginHostResources } from './cli-product.mjs';
import { resolveProductDefinition } from './product-customization/resolver.mjs';

const ROOT = resolve(import.meta.dirname, '..');
const ACME = join(ROOT, 'products', 'fixtures', 'acme', 'product.jsonc');

test('CLI stages only the supported plugin Host entry', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'openbitfun-cli-plugin-host-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const source = join(root, 'source');
  const destination = join(root, 'destination');
  mkdirSync(source);
  writeFileSync(join(source, 'extension-host.js'), 'current');
  writeFileSync(join(source, 'stale-runtime.js'), 'stale');

  stagePluginHostResources(destination, source);

  assert.deepEqual(readdirSync(destination), ['extension-host.js']);
});

test('CLI uses the shared resolver and stages the internal binary under the member name', () => {
  const resolution = resolveProductDefinition({ rootDir: ROOT, productConfig: ACME, member: 'cli' });
  const plan = cliBuildPlan(resolution, 'build', ['--locked'], 'win32');

  assert.deepEqual(plan.cargoArgs.slice(0, 2), ['build', '--manifest-path']);
  assert.ok(plan.cargoArgs.includes('--locked'));
  assert.ok(plan.internalBinaryPath.endsWith('openbitfun.exe'));
  assert.ok(plan.stagedBinaryPath.endsWith('acme.exe'));
  assert.ok(plan.stagedPluginHostPath.endsWith(join('resources', 'ext-host')));
  assert.equal(plan.environment.OPENBITFUN_PRODUCT_DISPLAY_NAME, 'Acme CLI');
});

test('CLI dev forwards runtime arguments after the Cargo delimiter', () => {
  const resolution = resolveProductDefinition({ rootDir: ROOT, member: 'cli' });
  const plan = cliBuildPlan(resolution, 'dev', ['--', 'health'], 'linux');
  assert.deepEqual(plan.cargoArgs.slice(-2), ['--', 'health']);
});

test('CLI build stages a standard cross-target artifact from the Cargo target subdirectory', () => {
  const resolution = resolveProductDefinition({ rootDir: ROOT, productConfig: ACME, member: 'cli' });
  const plan = cliBuildPlan(
    resolution,
    'build',
    ['--target', 'aarch64-unknown-linux-gnu'],
    'linux',
  );

  assert.ok(plan.internalBinaryPath.endsWith(
    join('aarch64-unknown-linux-gnu', 'release', 'openbitfun'),
  ));
});

test('CLI build uses CARGO_BUILD_TARGET when no explicit target is forwarded', { concurrency: false }, () => {
  const previousTarget = process.env.CARGO_BUILD_TARGET;
  process.env.CARGO_BUILD_TARGET = 'x86_64-unknown-linux-gnu';
  try {
    const resolution = resolveProductDefinition({ rootDir: ROOT, member: 'cli' });
    const plan = cliBuildPlan(resolution, 'build', [], 'linux');
    assert.ok(plan.internalBinaryPath.endsWith(
      join('x86_64-unknown-linux-gnu', 'release', 'openbitfun'),
    ));
  } finally {
    if (previousTarget === undefined) delete process.env.CARGO_BUILD_TARGET;
    else process.env.CARGO_BUILD_TARGET = previousTarget;
  }
});

test('CLI build rejects custom target specs before resolving an artifact path', { concurrency: false }, () => {
  const resolution = resolveProductDefinition({ rootDir: ROOT, member: 'cli' });
  assert.throws(
    () => cliBuildPlan(resolution, 'build', ['--target=../custom-target.json'], 'linux'),
    (error) => error?.code === 'unsupported_cli_build_target',
  );
  assert.throws(
    () => cliBuildPlan(resolution, 'build', ['--target', 'custom-target.json'], 'linux'),
    (error) => error?.code === 'unsupported_cli_build_target',
  );
  const previousTarget = process.env.CARGO_BUILD_TARGET;
  process.env.CARGO_BUILD_TARGET = 'custom-target.json';
  try {
    assert.throws(
      () => cliBuildPlan(resolution, 'build', [], 'linux'),
      (error) => error?.code === 'unsupported_cli_build_target',
    );
  } finally {
    if (previousTarget === undefined) delete process.env.CARGO_BUILD_TARGET;
    else process.env.CARGO_BUILD_TARGET = previousTarget;
  }
});

test('CLI build derives the executable suffix from an explicit target OS', () => {
  const resolution = resolveProductDefinition({ rootDir: ROOT, productConfig: ACME, member: 'cli' });
  const windowsPlan = cliBuildPlan(
    resolution,
    'build',
    ['--target=x86_64-pc-windows-gnu'],
    'linux',
  );
  const linuxPlan = cliBuildPlan(
    resolution,
    'build',
    ['--target=x86_64-unknown-linux-gnu'],
    'win32',
  );

  assert.ok(windowsPlan.internalBinaryPath.endsWith(join('release', 'openbitfun.exe')));
  assert.ok(windowsPlan.stagedBinaryPath.endsWith('acme.exe'));
  assert.ok(linuxPlan.internalBinaryPath.endsWith(join('release', 'openbitfun')));
  assert.ok(linuxPlan.stagedBinaryPath.endsWith('acme'));
});

test('CLI build rejects Cargo target directory overrides that bypass product isolation', () => {
  const resolution = resolveProductDefinition({ rootDir: ROOT, productConfig: ACME, member: 'cli' });
  for (const forwardArgs of [
    ['--target-dir', 'target/other'],
    ['--target-dir=target/other'],
    ['--config', 'build.target-dir="target/other"'],
    ['--config=build.target-dir="target/other"'],
  ]) {
    assert.throws(
      () => cliBuildPlan(resolution, 'build', forwardArgs, 'linux'),
      (error) => error?.code === 'unsupported_cli_target_dir_override',
    );
  }
});
