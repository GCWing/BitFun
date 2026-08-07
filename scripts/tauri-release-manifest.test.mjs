import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '..');

test('release version metadata is synchronized', () => {
  const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
  const result = run('scripts/verify-release-version-sync.mjs', ['--version', version]);
  assert.equal(result.status, 0, result.stderr);
});

test('prepares a versioned custom Windows installer asset', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-manual-installer-'));
  const assets = path.join(temp, 'assets', 'nested');
  const out = path.join(temp, 'manual');
  fs.mkdirSync(assets, { recursive: true });
  fs.writeFileSync(path.join(assets, 'bitfun-installer.exe'), 'installer');

  const result = run('scripts/prepare-windows-installer-asset.mjs', [
    '--assets-dir', path.join(temp, 'assets'),
    '--version', '1.2.3',
    '--out-dir', out,
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    fs.readFileSync(path.join(out, 'BitFun_1.2.3_windows-x86_64-installer.exe'), 'utf8'),
    'installer'
  );
});

test('latest.json keeps the updater URL separate from the manual installer URL', () => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-latest-manual-'));
  const updater = path.join(temp, 'updater');
  const manual = path.join(temp, 'manual');
  const out = path.join(temp, 'latest.json');
  fs.mkdirSync(updater, { recursive: true });
  fs.mkdirSync(manual, { recursive: true });

  const updaterName = 'BitFun_1.2.3_windows-x86_64-setup.exe';
  fs.writeFileSync(path.join(updater, updaterName), 'setup');
  fs.writeFileSync(path.join(updater, `${updaterName}.sig`), 'inline-updater-signature');
  const installerName = 'BitFun_1.2.3_windows-x86_64-installer.exe';
  fs.writeFileSync(path.join(manual, installerName), 'installer');
  fs.writeFileSync(path.join(manual, `${installerName}.sig`), 'detached-signature');

  const generated = run('scripts/generate-tauri-latest-json.mjs', [
    '--assets-dir', updater,
    '--manual-assets-dir', manual,
    '--version', '1.2.3',
    '--tag', 'v1.2.3',
    '--repo', 'GCWing/BitFun',
    '--out', out,
    '--required-platforms', 'windows-x86_64',
  ]);
  assert.equal(generated.status, 0, generated.stderr);

  const manifest = JSON.parse(fs.readFileSync(out, 'utf8'));
  assert.match(manifest.platforms['windows-x86_64'].url, /-setup\.exe$/);
  assert.match(manifest.manual_installers['windows-x86_64'].url, /-installer\.exe$/);
  assert.equal(
    manifest.manual_installers['windows-x86_64'].signature_url,
    `${manifest.manual_installers['windows-x86_64'].url}.sig`
  );

  const verified = run('scripts/verify-tauri-latest-json.mjs', [
    '--manifest', out,
    '--version', '1.2.3',
    '--required-platforms', 'windows-x86_64',
    '--required-manual-platforms', 'windows-x86_64',
  ]);
  assert.equal(verified.status, 0, verified.stderr);
});

function run(script, args) {
  return spawnSync(process.execPath, [script, ...args], {
    cwd: root,
    encoding: 'utf8',
  });
}
