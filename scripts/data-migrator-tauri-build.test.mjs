import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';
import { prepareDataMigratorTauriConfig, dataMigratorEnvironment } from './data-migrator-tauri-build.mjs';
import { releaseVersion, validateReleaseTag } from './data-migrator-release.mjs';
import { prepareTauriConfig } from './desktop-tauri-build.mjs';
const ROOT = resolve(import.meta.dirname, '..');
const APP = join(ROOT, 'src/apps/data-migrator');

test('independent bundle owns version, offline assets, icons and identity', () => {
  const output = prepareDataMigratorTauriConfig(join(APP, 'tauri.conf.json'), mkdtempSync(join(tmpdir(), 'migrator-config-')));
  const config = JSON.parse(readFileSync(output, 'utf8'));
  assert.equal(config.productName, 'OpenBitFun Data Migrator');
  assert.equal(config.identifier, 'com.openbitfun.data-migrator');
  assert.equal(config.version, releaseVersion());
  assert.deepEqual(config.build, { frontendDist: 'ui' });
  assert.equal(config.plugins?.updater, undefined);
  assert.equal(config.bundle.externalBin, undefined);
  assert.ok(config.bundle.icon.every((icon) => !icon.includes('desktop') && existsSync(join(APP, icon))));
  assert.ok(config.app.security.csp.includes("connect-src 'self'"));
});

test('a Desktop product environment cannot change the migration destination identity', () => {
  const env = dataMigratorEnvironment({ OPENBITFUN_PRODUCT_ID: 'acme', OPENBITFUN_DATA_NAMESPACE: 'acme', OPENBITFUN_DESKTOP_BINARY_NAME: 'acme', CI: '1' });
  assert.equal(env.OPENBITFUN_PRODUCT_ID, 'openbitfun');
  assert.equal(env.OPENBITFUN_DATA_NAMESPACE, 'openbitfun');
  assert.equal(env.OPENBITFUN_DESKTOP_BINARY_NAME, undefined);
  assert.equal(env.CI, 'true');
});

test('Desktop package generation has no migrator payload or build hook', () => {
  const desktop = join(ROOT, 'src/apps/desktop');
  const output = prepareTauriConfig(join(desktop, 'tauri.conf.json'), { desktopDir: desktop });
  const config = JSON.parse(readFileSync(output, 'utf8'));
  assert.ok(!(config.bundle.externalBin || []).some((file) => file.includes('migrator')));
  for (const file of ['scripts/dev.cjs', 'scripts/desktop-tauri-build.mjs', 'src/apps/desktop/src/lib.rs']) {
    assert.doesNotMatch(readFileSync(join(ROOT, file), 'utf8'), /data-migrator|legacy_migration_api/);
  }
});

test('migrator dependency and command closure exclude the main app and restart handshake', () => {
  const manifest = readFileSync(join(APP, 'Cargo.toml'), 'utf8');
  assert.doesNotMatch(manifest, /^openbitfun-core\s*=|product-full|product-capabilities|plugin-runtime/m);
  const source = readFileSync(join(APP, 'src/app_state.rs'), 'utf8');
  assert.doesNotMatch(source, /HandoffStore|MigrationOnboardingStore|restart_desktop|TrustedInstallationResolver/);
  const capability = readFileSync(join(APP, 'capabilities/migrator.json'), 'utf8');
  assert.doesNotMatch(capability, /fs:|shell:|updater:|dialog:/);
});

test('independent release tags cannot be confused with main app versions', () => {
  const version = releaseVersion();
  assert.doesNotThrow(() => validateReleaseTag('data-migrator-v' + version, version));
  assert.throws(() => validateReleaseTag('v' + version, version));
  assert.throws(() => validateReleaseTag('data-migrator-v9.0.0', version));
});

test('all static UI labels and locale keys have complete translations', () => {
  const html = readFileSync(join(APP, 'ui/index.html'), 'utf8');
  const locales = JSON.parse(readFileSync(join(APP, 'ui/locales.json'), 'utf8'));
  const keys = Object.keys(locales.en).sort();
  for (const labels of Object.values(locales)) {
    assert.deepEqual(Object.keys(labels).sort(), keys);
    for (const match of html.matchAll(/data-i18n="([^"]+)"/g)) assert.ok(labels[match[1]], match[1]);
  }
});
