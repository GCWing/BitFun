import assert from 'node:assert/strict';
import { mkdirSync, readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import {
  configureDesktopWebFontProfile,
  prepareMacOSFlashgrepForSigning,
  prepareTauriConfig,
  shouldRetryMacDmgBuild,
} from './desktop-tauri-build.mjs';
import { resolveProductDefinition } from './product-customization/resolver.mjs';
import {
  APPLE_SYSTEM_FONT_PROFILE,
  HARMONY_BUNDLED_FONT_PROFILE,
  WEB_FONT_PROFILE_ENV,
} from './web-font-profile.mjs';

const FAILED_BUILD = { status: 1 };
const DMG_ARGS = ['--target', 'x86_64-apple-darwin', '--bundles', 'app,dmg'];
const ROOT = join(import.meta.dirname, '..');

test('Desktop packaging selects the Web font profile from its target triple', () => {
  const appleEnv = { [WEB_FONT_PROFILE_ENV]: HARMONY_BUNDLED_FONT_PROFILE };
  assert.equal(
    configureDesktopWebFontProfile(
      ['--target', 'aarch64-apple-darwin'],
      { env: appleEnv, platform: 'win32' },
    ),
    APPLE_SYSTEM_FONT_PROFILE,
  );
  assert.equal(appleEnv[WEB_FONT_PROFILE_ENV], APPLE_SYSTEM_FONT_PROFILE);

  const windowsEnv = { [WEB_FONT_PROFILE_ENV]: APPLE_SYSTEM_FONT_PROFILE };
  assert.equal(
    configureDesktopWebFontProfile(
      ['--target=x86_64-pc-windows-msvc'],
      { env: windowsEnv, platform: 'darwin' },
    ),
    HARMONY_BUNDLED_FONT_PROFILE,
  );
  assert.equal(windowsEnv[WEB_FONT_PROFILE_ENV], HARMONY_BUNDLED_FONT_PROFILE);

  assert.equal(
    configureDesktopWebFontProfile([], { env: {}, platform: 'darwin' }),
    APPLE_SYSTEM_FONT_PROFILE,
  );
  assert.equal(
    configureDesktopWebFontProfile([], { env: {}, platform: 'linux' }),
    HARMONY_BUNDLED_FONT_PROFILE,
  );
});

test('release builds do not mutate DMGs after Tauri signs and notarizes them', () => {
  const source = readFileSync(join(ROOT, 'scripts', 'desktop-tauri-build.mjs'), 'utf8');
  assert.doesNotMatch(source, /patchDmgExtras/);
  assert.doesNotMatch(source, /patch-dmg-extras\.sh/);
  assert.match(source, /TAURI_BUNDLER_DMG_IGNORE_CI = 'true'/);
});

test('Desktop DMG uses the branded installer layout', () => {
  const config = JSON.parse(
    readFileSync(join(ROOT, 'src', 'apps', 'desktop', 'tauri.conf.json'), 'utf8')
  );
  assert.deepEqual(config.bundle.macOS.dmg, {
    background: 'dmg/background.png',
    windowSize: { width: 660, height: 400 },
    appPosition: { x: 180, y: 170 },
    applicationFolderPosition: { x: 480, y: 170 },
  });
});

test('Desktop builds prepare and bundle the OpenCode extension Host', () => {
  const source = readFileSync(join(ROOT, 'scripts', 'desktop-tauri-build.mjs'), 'utf8');
  assert.match(source, /preparePluginHost\(\)/);

  for (const name of ['tauri.conf.json', 'tauri.dev.conf.json']) {
    const config = JSON.parse(
      readFileSync(join(ROOT, 'src', 'apps', 'desktop', name), 'utf8')
    );
    assert.equal(
      config.bundle.resources['../extension-host/dist/extension-host.js'],
      'resources/ext-host/extension-host.js'
    );
  }
});

test('macOS release signing covers the bundled flashgrep executable', () => {
  const fixture = join(tmpdir(), `openbitfun-flashgrep-signing-${process.pid}-${Date.now()}`);
  const desktopDir = join(fixture, 'src', 'apps', 'desktop');
  const source = join(fixture, 'flashgrep-aarch64-apple-darwin');
  const calls = [];
  mkdirSync(desktopDir, { recursive: true });
  writeFileSync(source, 'test-binary');

  try {
    const signed = prepareMacOSFlashgrepForSigning(source, desktopDir, {
      platform: 'darwin',
      signingIdentity: 'Developer ID Application: Test (TEAMID)',
      spawnSync: (...args) => {
        calls.push(args);
        return { status: 0 };
      },
    });

    assert.notEqual(signed, source);
    assert.equal(readFileSync(signed, 'utf8'), 'test-binary');
    assert.deepEqual(calls[0][0], 'codesign');
    assert.deepEqual(calls[0][1], [
      '--force',
      '--sign',
      'Developer ID Application: Test (TEAMID)',
      '--options',
      'runtime',
      '--timestamp',
      signed,
    ]);
  } finally {
    rmSync(fixture, { force: true, recursive: true });
  }
});

test('unsigned and non-macOS builds keep the original flashgrep executable', () => {
  assert.equal(
    prepareMacOSFlashgrepForSigning('/tmp/flashgrep', '/tmp/desktop', {
      platform: 'darwin',
      signingIdentity: '',
    }),
    '/tmp/flashgrep',
  );
  assert.equal(
    prepareMacOSFlashgrepForSigning('/tmp/flashgrep', '/tmp/desktop', {
      platform: 'linux',
      signingIdentity: 'unused',
    }),
    '/tmp/flashgrep',
  );
});

test('macOS packaging fails when bundled flashgrep signing fails', () => {
  const fixture = join(tmpdir(), `openbitfun-flashgrep-signing-failure-${process.pid}-${Date.now()}`);
  const desktopDir = join(fixture, 'src', 'apps', 'desktop');
  const source = join(fixture, 'flashgrep-x86_64-apple-darwin');
  mkdirSync(desktopDir, { recursive: true });
  writeFileSync(source, 'test-binary');

  try {
    assert.throws(
      () => prepareMacOSFlashgrepForSigning(source, desktopDir, {
        platform: 'darwin',
        signingIdentity: 'Developer ID Application: Test (TEAMID)',
        spawnSync: () => ({ status: 1, stderr: 'identity unavailable' }),
      }),
      /Failed to sign bundled flashgrep binary: identity unavailable/,
    );
  } finally {
    rmSync(fixture, { force: true, recursive: true });
  }
});

function retryFixture() {
  const root = join(tmpdir(), `openbitfun-dmg-retry-${process.pid}-${Date.now()}`);
  const desktopDir = join(root, 'src', 'apps', 'desktop');
  const targetDir = join(root, 'target');
  const appDir = join(
    targetDir,
    'x86_64-apple-darwin',
    'release',
    'bundle',
    'macos',
    'OpenBitFun.app'
  );
  mkdirSync(desktopDir, { recursive: true });
  mkdirSync(appDir, { recursive: true });

  return {
    appDir,
    desktopDir,
    runtime: {
      cargoTargetDir: targetDir,
      githubActions: 'true',
      platform: 'darwin',
      root,
    },
    cleanup: () => rmSync(root, { force: true, recursive: true }),
  };
}

test('retries a failed GitHub Actions DMG bundle after a fresh app bundle', () => {
  const fixture = retryFixture();
  try {
    assert.equal(
      shouldRetryMacDmgBuild(
        FAILED_BUILD,
        DMG_ARGS,
        fixture.desktopDir,
        Date.now(),
        fixture.runtime
      ),
      true
    );
  } finally {
    fixture.cleanup();
  }
});

test('does not retry failures outside the narrow DMG bundling boundary', () => {
  const fixture = retryFixture();
  try {
    const cases = [
      [{ status: 0 }, DMG_ARGS, fixture.runtime],
      [FAILED_BUILD, DMG_ARGS, { ...fixture.runtime, platform: 'linux' }],
      [FAILED_BUILD, DMG_ARGS, { ...fixture.runtime, githubActions: 'false' }],
      [FAILED_BUILD, ['--no-bundle'], fixture.runtime],
      [FAILED_BUILD, ['--bundles=app'], fixture.runtime],
      [
        FAILED_BUILD,
        DMG_ARGS,
        { ...fixture.runtime, cargoTargetDir: join(fixture.runtime.root, 'missing') },
      ],
    ];
    for (const [result, args, runtime] of cases) {
      assert.equal(
        shouldRetryMacDmgBuild(result, args, fixture.desktopDir, Date.now(), runtime),
        false
      );
    }

    const staleTime = new Date(Date.now() - 60_000);
    utimesSync(fixture.appDir, staleTime, staleTime);
    assert.equal(
      shouldRetryMacDmgBuild(
        FAILED_BUILD,
        DMG_ARGS,
        fixture.desktopDir,
        Date.now(),
        fixture.runtime
      ),
      false
    );
  } finally {
    fixture.cleanup();
  }
});

test('Desktop Tauri projection consumes only the resolved member identity', () => {
  const fixture = join(tmpdir(), `openbitfun-tauri-product-${process.pid}-${Date.now()}`);
  mkdirSync(fixture, { recursive: true });
  const baseConfig = join(fixture, 'tauri.conf.json');
  writeFileSync(baseConfig, JSON.stringify({
    productName: 'OpenBitFun',
    identifier: 'com.openbitfun.desktop',
    bundle: { resources: {} },
  }));
  try {
    const resolution = resolveProductDefinition({
      rootDir: ROOT,
      productConfig: join(ROOT, 'products', 'fixtures', 'acme', 'product.jsonc'),
      member: 'desktop',
    });
    const generated = prepareTauriConfig(baseConfig, {
      desktopDir: fixture,
      flashgrepBinary: join(fixture, 'flashgrep'),
      resolution,
    });
    const config = JSON.parse(readFileSync(generated, 'utf8'));
    assert.equal(config.productName, 'Acme Desktop');
    assert.equal(config.mainBinaryName, 'acme-desktop');
    assert.equal(config.identifier, 'com.acme.desktop');
    assert.equal(config.bundle.icon, undefined);
  } finally {
    rmSync(fixture, { force: true, recursive: true });
  }
});

test('Windows updater installs NSIS packages without showing its progress window', () => {
  const fixture = join(tmpdir(), `openbitfun-tauri-updater-${process.pid}-${Date.now()}`);
  const baseConfig = join(fixture, 'tauri.conf.json');
  const updaterEnv = {
    OPENBITFUN_ENABLE_UPDATER_ARTIFACTS: process.env.OPENBITFUN_ENABLE_UPDATER_ARTIFACTS,
    OPENBITFUN_RELEASE_CHANNEL: process.env.OPENBITFUN_RELEASE_CHANNEL,
    OPENBITFUN_UPDATER_FALLBACK_ENDPOINT: process.env.OPENBITFUN_UPDATER_FALLBACK_ENDPOINT,
    OPENBITFUN_UPDATER_PRIMARY_ENDPOINT: process.env.OPENBITFUN_UPDATER_PRIMARY_ENDPOINT,
    TAURI_SIGNING_PRIVATE_KEY: process.env.TAURI_SIGNING_PRIVATE_KEY,
    TAURI_UPDATER_ENDPOINT: process.env.TAURI_UPDATER_ENDPOINT,
    TAURI_UPDATER_FALLBACK_ENDPOINT: process.env.TAURI_UPDATER_FALLBACK_ENDPOINT,
    TAURI_UPDATER_PUBKEY: process.env.TAURI_UPDATER_PUBKEY,
  };
  mkdirSync(fixture, { recursive: true });
  writeFileSync(baseConfig, JSON.stringify({ bundle: { resources: {} } }));
  process.env.OPENBITFUN_ENABLE_UPDATER_ARTIFACTS = 'true';
  process.env.TAURI_SIGNING_PRIVATE_KEY = 'test-private-key';
  process.env.TAURI_UPDATER_PUBKEY = 'test-public-key';

  try {
    const generated = prepareTauriConfig(baseConfig, {
      desktopDir: fixture,
      flashgrepBinary: join(fixture, 'flashgrep'),
    });
    const config = JSON.parse(readFileSync(generated, 'utf8'));
    assert.equal(config.plugins.updater.windows.installMode, 'quiet');
    assert.match(config.plugins.updater.endpoints[0], /releases\/latest\/download/);
  } finally {
    for (const [name, value] of Object.entries(updaterEnv)) {
      if (value === undefined) {
        delete process.env[name];
      } else {
        process.env[name] = value;
      }
    }
    rmSync(fixture, { force: true, recursive: true });
  }
});

test('beta Desktop artifacts compile and bundle only beta updater endpoints', () => {
  const fixture = join(tmpdir(), `openbitfun-tauri-beta-${process.pid}-${Date.now()}`);
  const baseConfig = join(fixture, 'tauri.conf.json');
  const names = [
    'OPENBITFUN_ENABLE_UPDATER_ARTIFACTS',
    'OPENBITFUN_RELEASE_CHANNEL',
    'OPENBITFUN_UPDATER_FALLBACK_ENDPOINT',
    'OPENBITFUN_UPDATER_PRIMARY_ENDPOINT',
    'TAURI_SIGNING_PRIVATE_KEY',
    'TAURI_UPDATER_ENDPOINT',
    'TAURI_UPDATER_FALLBACK_ENDPOINT',
    'TAURI_UPDATER_PUBKEY',
  ];
  const previous = Object.fromEntries(names.map((name) => [name, process.env[name]]));
  mkdirSync(fixture, { recursive: true });
  writeFileSync(baseConfig, JSON.stringify({ bundle: { resources: {} } }));
  process.env.OPENBITFUN_ENABLE_UPDATER_ARTIFACTS = 'true';
  process.env.OPENBITFUN_RELEASE_CHANNEL = 'beta';
  process.env.TAURI_SIGNING_PRIVATE_KEY = 'test-private-key';
  process.env.TAURI_UPDATER_PUBKEY = 'test-public-key';
  delete process.env.TAURI_UPDATER_ENDPOINT;
  delete process.env.TAURI_UPDATER_FALLBACK_ENDPOINT;

  try {
    const generated = prepareTauriConfig(baseConfig, {
      desktopDir: fixture,
      flashgrepBinary: join(fixture, 'flashgrep'),
    });
    const config = JSON.parse(readFileSync(generated, 'utf8'));
    assert.equal(
      config.plugins.updater.endpoints[0],
      'https://github.com/GCWing/OpenBitFun/releases/download/channel-beta/latest.json',
    );
    assert.equal(
      config.plugins.updater.endpoints[1],
      'https://openbitfun.com/release/beta/latest.json',
    );
    assert.equal(
      process.env.OPENBITFUN_UPDATER_PRIMARY_ENDPOINT,
      config.plugins.updater.endpoints[0],
    );
    assert.equal(
      process.env.OPENBITFUN_UPDATER_FALLBACK_ENDPOINT,
      config.plugins.updater.endpoints[1],
    );
  } finally {
    for (const [name, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
    rmSync(fixture, { force: true, recursive: true });
  }
});

test('static desktop Tauri configs do not require the DeepSeek profile at compile time', () => {
  for (const name of ['tauri.conf.json', 'tauri.dev.conf.json']) {
    const config = JSON.parse(
      readFileSync(join(ROOT, 'src', 'apps', 'desktop', name), 'utf8')
    );
    assert.equal(
      config.bundle.resources['../../../packages/dsh-acp/dist-profile'],
      undefined,
      `${name} must not list dist-profile as a compile-time resource`,
    );
  }
});

test('official packaging injects the DeepSeek profile resource', () => {
  const fixture = join(tmpdir(), `openbitfun-tauri-dsh-${process.pid}-${Date.now()}`);
  mkdirSync(fixture, { recursive: true });
  const baseConfig = join(fixture, 'tauri.conf.json');
  writeFileSync(baseConfig, JSON.stringify({
    bundle: { resources: { 'resources/worker_host.js': 'resources/worker_host.js' } },
  }));
  try {
    const generated = prepareTauriConfig(baseConfig, {
      desktopDir: fixture,
      flashgrepBinary: join(fixture, 'flashgrep'),
    });
    const config = JSON.parse(readFileSync(generated, 'utf8'));
    assert.equal(
      config.bundle.resources['../../../packages/dsh-acp/dist-profile'],
      'resources/dsh-profile',
    );
    assert.equal(
      config.bundle.resources['../../../dist'],
      'frontend/dist',
    );
    assert.equal(
      config.bundle.resources['resources/worker_host.js'],
      'resources/worker_host.js',
    );
  } finally {
    rmSync(fixture, { force: true, recursive: true });
  }
});

test('static desktop configs keep the full frontend outside Tauri embedded assets', () => {
  for (const name of ['tauri.conf.json', 'tauri.dev.conf.json']) {
    const config = JSON.parse(
      readFileSync(join(ROOT, 'src', 'apps', 'desktop', name), 'utf8')
    );
    assert.equal(config.build.frontendDist, 'bootstrap-ui');
    assert.equal(config.bundle.resources['../../../dist'], undefined);
  }
});

test('Desktop release config bundles models.dev notices and provenance', () => {
  const config = JSON.parse(
    readFileSync(join(ROOT, 'src', 'apps', 'desktop', 'tauri.conf.json'), 'utf8')
  );
  assert.equal(
    config.bundle.resources['../../../THIRD_PARTY_NOTICES.md'],
    'THIRD_PARTY_NOTICES.md'
  );
  assert.equal(
    config.bundle.resources[
      '../../crates/services/services-integrations/assets/models-dev.LICENSE.txt'
    ],
    'third-party/models.dev/LICENSE.txt'
  );
  assert.equal(
    config.bundle.resources[
      '../../crates/services/services-integrations/assets/models-dev.provenance.json'
    ],
    'third-party/models.dev/provenance.json'
  );
});
