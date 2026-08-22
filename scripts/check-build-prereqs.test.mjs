import assert from 'node:assert/strict';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
);
const scriptPath = path.join(repoRoot, 'scripts/check-build-prereqs.mjs');

function createTestRoot({
  nodeModules = false,
  mobileWebDist = false,
  pluginHostDist = false,
  sherpaOnnx = null,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'bitfun-build-prereqs-'));

  if (nodeModules) {
    mkdirSync(path.join(root, 'node_modules'), { recursive: true });
  }

  if (mobileWebDist) {
    const distDir = path.join(root, 'src', 'mobile-web', 'dist');
    mkdirSync(distDir, { recursive: true });
    writeFileSync(path.join(distDir, 'index.html'), '<html></html>');
  }

  if (pluginHostDist) {
    const distDir = path.join(
      root,
      'src',
      'apps',
      'extension-host',
      'dist',
    );
    mkdirSync(distDir, { recursive: true });
    writeFileSync(path.join(distDir, 'extension-host.js'), '');
  }

  if (sherpaOnnx) {
    for (const version of sherpaOnnx) {
      const libDir = path.join(
        root,
        'target',
        'sherpa-onnx-prebuilt',
        version,
        'lib',
      );
      mkdirSync(libDir, { recursive: true });
      writeFileSync(path.join(libDir, 'libsherpa-onnx-c-api.a'), '');
    }
  }

  return root;
}

function createFakePnpm() {
  const binDir = mkdtempSync(path.join(tmpdir(), 'bitfun-fake-pnpm-'));
  const fakePnpmPath = path.join(binDir, 'fake-pnpm.cjs');
  writeFileSync(
    fakePnpmPath,
    `
const { mkdirSync, writeFileSync } = require('fs');
const args = process.argv.slice(2);
if (args[0] === 'install') {
  mkdirSync('node_modules', { recursive: true });
} else if (args[0] === 'run' && args[1] === 'prepare:mobile-web') {
  mkdirSync('src/mobile-web/dist', { recursive: true });
  writeFileSync('src/mobile-web/dist/index.html', '<html></html>');
} else if (args[0] === 'run' && args[1] === 'plugin-host:prepare') {
  mkdirSync('src/apps/extension-host/dist', { recursive: true });
  writeFileSync('src/apps/extension-host/dist/extension-host.js', '');
}
`,
  );
  if (process.platform === 'win32') {
    writeFileSync(
      path.join(binDir, 'pnpm.cmd'),
      `@echo off\r\n"${process.execPath}" "%~dp0fake-pnpm.cjs" %*\r\n`,
    );
  } else {
    const pnpmPath = path.join(binDir, 'pnpm');
    writeFileSync(
      pnpmPath,
      `#!/usr/bin/env node\nrequire('./fake-pnpm.cjs');\n`,
    );
    chmodSync(pnpmPath, 0o755);
  }
  return binDir;
}

function runCheck(root, { fix = false, extraPath = null, sherpaEnv = null } = {}) {
  const env = {
    ...process.env,
    BITFUN_BUILD_PREREQS_TEST_ROOT: root,
  };
  if (extraPath) {
    env.PATH = `${extraPath}${path.delimiter}${env.PATH || ''}`;
  }
  if (sherpaEnv !== null) {
    if (sherpaEnv === '') {
      delete env.SHERPA_ONNX_LIB_DIR;
    } else {
      env.SHERPA_ONNX_LIB_DIR = sherpaEnv;
    }
  }

  const args = fix ? [scriptPath, '--fix'] : [scriptPath];

  return spawnSync(process.execPath, args, {
    env,
    encoding: 'utf8',
  });
}

test('passes when all prerequisites are present (including sherpa-onnx prebuilt)', (t) => {
  const root = createTestRoot({
    nodeModules: true,
    mobileWebDist: true,
    pluginHostDist: true,
    sherpaOnnx: ['sherpa-onnx-v1.13.4-osx-arm64-static-lib'],
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root, { sherpaEnv: '' });

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /Build prerequisite check passed/);
  assert.doesNotMatch(result.stderr, /\[WARN\]/);
});

test('fails when root node_modules is missing', (t) => {
  const root = createTestRoot({
    mobileWebDist: true,
    pluginHostDist: true,
    sherpaOnnx: ['sherpa-onnx-v1.13.4-osx-arm64-static-lib'],
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root, { sherpaEnv: '' });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /\[FAIL\] root node_modules/);
  assert.match(result.stderr, /Fix: pnpm install/);
});

test('fails when mobile-web dist is missing', (t) => {
  const root = createTestRoot({
    nodeModules: true,
    pluginHostDist: true,
    sherpaOnnx: ['sherpa-onnx-v1.13.4-osx-arm64-static-lib'],
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root, { sherpaEnv: '' });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /\[FAIL\] mobile-web dist/);
  assert.match(result.stderr, /Fix: pnpm run prepare:mobile-web/);
});

test('does not require the DeepSeek profile for cargo check', (t) => {
  const root = createTestRoot({
    nodeModules: true,
    mobileWebDist: true,
    pluginHostDist: true,
    sherpaOnnx: ['sherpa-onnx-v1.13.4-osx-arm64-static-lib'],
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root, { sherpaEnv: '' });

  assert.equal(result.status, 0);
  assert.doesNotMatch(result.stderr, /dsh bridge profile/);
  assert.doesNotMatch(result.stderr, /prepare:dsh-profile/);
});

test('fails when OpenCode extension Host dist is missing', (t) => {
  const root = createTestRoot({
    nodeModules: true,
    mobileWebDist: true,
    sherpaOnnx: ['sherpa-onnx-v1.13.4-osx-arm64-static-lib'],
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root, { sherpaEnv: '' });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /\[FAIL\] OpenCode extension Host dist/);
  assert.match(result.stderr, /Fix: pnpm run plugin-host:prepare/);
});

test('warns when sherpa-onnx prebuilt dir does not exist (first build)', (t) => {
  const root = createTestRoot({
    nodeModules: true,
    mobileWebDist: true,
    pluginHostDist: true,
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root, { sherpaEnv: '' });

  assert.equal(result.status, 0);
  assert.match(result.stderr, /\[WARN\] sherpa-onnx/);
  assert.match(result.stderr, /first build will download from GitHub/);
});

test('exits with error code when both errors and warnings are present', (t) => {
  const root = createTestRoot({ mobileWebDist: false });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root, { sherpaEnv: '' });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /\[FAIL\]/);
  assert.match(result.stderr, /\[WARN\] sherpa-onnx/);
});

test('--fix runs fix commands, re-verifies, and exits 0 when errors resolved', (t) => {
  const binDir = createFakePnpm();
  const root = createTestRoot();
  t.after(() => {
    rmSync(root, { recursive: true, force: true });
    rmSync(binDir, { recursive: true, force: true });
  });

  const result = runCheck(root, { fix: true, extraPath: binDir, sherpaEnv: '' });

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /Attempting fixes/);
  assert.match(result.stdout, /\$ pnpm install/);
  assert.match(result.stdout, /\$ pnpm run prepare:mobile-web/);
  assert.match(result.stdout, /\$ pnpm run plugin-host:prepare/);
  assert.doesNotMatch(result.stdout, /prepare:dsh-profile/);
  assert.match(result.stdout, /Re-checking prerequisites/);
  assert.match(result.stdout, /All errors resolved/);
});
