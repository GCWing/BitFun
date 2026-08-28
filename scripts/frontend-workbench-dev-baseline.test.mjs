import assert from 'node:assert/strict';
import { mkdirSync, readFileSync, utimesSync, writeFileSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { getFrontendWorkbenchDevBaselinePlan } from './frontend-workbench-dev-baseline.mjs';

function write(root, relativePath, contents = '') {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, contents);
  return target;
}

test('desktop development prepares the FrontendWorkbench baseline before launch', () => {
  const devScript = readFileSync(new URL('./dev.cjs', import.meta.url), 'utf8');

  assert.match(devScript, /getFrontendWorkbenchDevBaselinePlan\(ROOT_DIR\)/);
  assert.match(
    devScript,
    /\['--dir', 'src\/web-ui', 'run', 'build:desktop'\]/,
  );
});

test('requests a build when the desktop frontend baseline is missing', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'bitfun-workbench-baseline-'));
  write(root, 'src/web-ui/src/main.tsx', 'export {};');

  assert.deepEqual(getFrontendWorkbenchDevBaselinePlan(root), {
    shouldBuild: true,
    reason: 'FrontendWorkbench baseline is missing',
  });
});

test('requests a rebuild when frontend source is newer than the baseline', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'bitfun-workbench-baseline-'));
  const baseline = write(root, 'dist/index.html', '<main>old</main>');
  const source = write(root, 'src/web-ui/src/main.tsx', 'export const changed = true;');
  const now = Date.now() / 1000;
  utimesSync(baseline, now - 10, now - 10);
  utimesSync(source, now, now);

  const plan = getFrontendWorkbenchDevBaselinePlan(root);
  assert.equal(plan.shouldBuild, true);
  assert.match(plan.reason, /src\/web-ui\/src\/main\.tsx/);
});

test('reuses a current baseline and ignores generated dependency output', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'bitfun-workbench-baseline-'));
  const source = write(root, 'src/web-ui/src/main.tsx', 'export {};');
  const baseline = write(root, 'dist/index.html', '<main>current</main>');
  const generated = write(root, 'design-system/packages/ui/dist/index.js', 'generated');
  const now = Date.now() / 1000;
  utimesSync(source, now - 20, now - 20);
  utimesSync(baseline, now - 10, now - 10);
  utimesSync(generated, now, now);

  assert.deepEqual(getFrontendWorkbenchDevBaselinePlan(root), {
    shouldBuild: false,
    reason: 'FrontendWorkbench baseline is current',
  });
});
