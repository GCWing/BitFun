import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, utimesSync, writeFileSync } from 'node:fs';
import { mkdtemp } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const require = createRequire(import.meta.url);
const { cleanStaleMobileWebResources, getMobileWebRebuildPlan } = require('./mobile-web-build.cjs');

function write(root, relativePath, contents = '') {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, contents);
  return target;
}

function setMtime(target, seconds) {
  utimesSync(target, seconds, seconds);
}

test('mobile-web lifecycle prepares its generated design-system package entries', () => {
  const packageJson = JSON.parse(
    readFileSync(new URL('../src/mobile-web/package.json', import.meta.url), 'utf8'),
  );
  const prepareCommand = 'pnpm --dir ../../design-system run prepare:consumer-dev';

  assert.equal(packageJson.scripts['prepare:design-system'], prepareCommand);
  assert.equal(packageJson.scripts.predev, 'pnpm run prepare:design-system');
  assert.equal(packageJson.scripts['pretype-check'], 'pnpm run prepare:design-system');
  assert.equal(packageJson.scripts.prebuild, 'pnpm run prepare:design-system');
});

test('requests a mobile-web rebuild when a theme source changes', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'openbitfun-mobile-web-build-'));
  const mobileWebDir = path.join(root, 'src/mobile-web');
  const output = write(root, 'src/mobile-web/dist/index.html', '<main>old</main>');
  const marker = write(
    root,
    'src/mobile-web/node_modules/.cache/openbitfun-mobile-web-build-marker',
    'old build\n',
  );
  const themeSource = write(
    root,
    'design-system/packages/theme-openbitfun/src/light.tokens.json',
    '{}\n',
  );
  const now = Date.now() / 1000;
  setMtime(output, now - 20);
  setMtime(marker, now - 10);
  setMtime(themeSource, now);

  const plan = getMobileWebRebuildPlan(mobileWebDir, false, root);

  assert.equal(plan.shouldBuild, true);
  assert.match(plan.reason, /design-system[\\/]packages[\\/]theme-openbitfun[\\/]src[\\/]light\.tokens\.json/);
});

test('reuses mobile-web output when only generated design-system output is newer', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'openbitfun-mobile-web-build-'));
  const mobileWebDir = path.join(root, 'src/mobile-web');
  const mobileSource = write(root, 'src/mobile-web/src/main.tsx', 'export {};\n');
  const output = write(root, 'src/mobile-web/dist/index.html', '<main>current</main>');
  const marker = write(
    root,
    'src/mobile-web/node_modules/.cache/openbitfun-mobile-web-build-marker',
    'current build\n',
  );
  const generatedTheme = write(
    root,
    'design-system/packages/theme-openbitfun/dist/index.js',
    'export {};\n',
  );
  const now = Date.now() / 1000;
  setMtime(mobileSource, now - 30);
  setMtime(output, now - 20);
  setMtime(marker, now - 10);
  setMtime(generatedTheme, now);

  assert.deepEqual(getMobileWebRebuildPlan(mobileWebDir, false, root), {
    shouldBuild: false,
    reason: 'mobile-web dist is up to date; skipping clean/install/build (use --force or OPENBITFUN_MOBILE_WEB_FORCE_BUILD=1 to rebuild)',
  });
});

test('cleans stale mobile-web resources from native and explicit target profiles', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'openbitfun-mobile-web-clean-'));
  const native = write(root, 'target/release/mobile-web/dist/index.html', 'native');
  const targeted = write(
    root,
    'target/x86_64-pc-windows-msvc/release/mobile-web/dist/index.html',
    'targeted',
  );
  const unrelated = write(root, 'target/release/frontend/dist/index.html', 'frontend');

  assert.equal(cleanStaleMobileWebResources(() => {}, root), 2);
  assert.equal(fileExists(native), false);
  assert.equal(fileExists(targeted), false);
  assert.equal(fileExists(unrelated), true);
});

function fileExists(filePath) {
  try {
    readFileSync(filePath);
    return true;
  } catch (error) {
    if (error && error.code === 'ENOENT') return false;
    throw error;
  }
}
