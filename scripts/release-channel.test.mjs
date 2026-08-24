import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  compareReleaseVersions,
  resolveReleaseChannel,
  validateReleaseVersion,
} from './release-channel.mjs';
import { setBuildVersion } from './set-build-version.mjs';
import { decodeMinisignPublicKey } from './write-minisign-public-key.mjs';

const RAW_PUBLIC_KEY = `untrusted comment: minisign public key E3E0874CEC1C22C3
RWTDIhzsTIfg41w2Gwiei0zNDKaLYm9dQVpEWNQ/Ulpyt2mbS2JE1U2M`;

test('stable and beta channels resolve to isolated updater feeds', () => {
  const stable = resolveReleaseChannel('stable');
  const beta = resolveReleaseChannel('beta');
  assert.match(stable.primaryUpdaterEndpoint, /releases\/latest\/download/);
  assert.match(beta.primaryUpdaterEndpoint, /releases\/download\/channel-beta/);
  assert.equal(beta.fallbackUpdaterEndpoint, 'https://openbitfun.com/release/beta/latest.json');
  assert.notEqual(beta.primaryUpdaterEndpoint, stable.primaryUpdaterEndpoint);
});

test('channel promotion follows SemVer including beta precedence', () => {
  assert.equal(compareReleaseVersions('0.2.18-beta.2', '0.2.18-beta.1'), 1);
  assert.equal(compareReleaseVersions('0.2.18', '0.2.18-beta.9'), 1);
  assert.equal(compareReleaseVersions('0.2.19-beta.1', '0.2.18'), 1);
  assert.equal(compareReleaseVersions('0.2.18', '0.2.19-beta.1'), -1);
  assert.equal(compareReleaseVersions('0.2.18-beta.2', '0.2.18-beta.2'), 0);
});

test('release versions must match their channel', () => {
  assert.equal(validateReleaseVersion('stable', '0.2.18'), '0.2.18');
  assert.equal(validateReleaseVersion('beta', '0.2.18-beta.1'), '0.2.18-beta.1');
  assert.throws(() => validateReleaseVersion('stable', '0.2.18-beta.1'));
  assert.throws(() => validateReleaseVersion('beta', '0.2.18'));
  assert.throws(() => validateReleaseVersion('beta', '0.2.18-beta.0'));
  assert.equal(
    validateReleaseVersion('nightly', '0.2.18-nightly.20260811'),
    '0.2.18-nightly.20260811',
  );
});

test('release public key export accepts raw and legacy base64 values', () => {
  const expected = `${RAW_PUBLIC_KEY}\n`;
  assert.equal(decodeMinisignPublicKey(RAW_PUBLIC_KEY), expected);
  assert.equal(
    decodeMinisignPublicKey(Buffer.from(expected).toString('base64')),
    expected,
  );
  assert.throws(() => decodeMinisignPublicKey('not-a-key'));
});

test('build version projection updates every release-owned version file', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'bitfun-build-version-'));
  const jsonFiles = [
    'package.json',
    'package-lock.json',
    'BitFun-Installer/package.json',
    'BitFun-Installer/package-lock.json',
  ];
  for (const relative of jsonFiles) {
    const file = path.join(root, relative);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, JSON.stringify({ version: '1.0.0', packages: { '': { version: '1.0.0' } } }));
  }
  writeFixture(
    root,
    'Cargo.toml',
    `[package]
name = "build-version-fixture"
version = "1.0.0" # x-release-please-version
edition = "2021"

[dependencies]
fixture-dependency = { path = "fixture-dependency" }

[workspace]
members = []
exclude = ["fixture-dependency", "src/apps/relay-server", "BitFun-Installer/src-tauri"]
`,
  );
  writeFixture(root, 'src/lib.rs', 'pub fn fixture() {}\n');
  writeFixture(
    root,
    'fixture-dependency/Cargo.toml',
    `[package]
name = "fixture-dependency"
version = "1.0.0"
edition = "2021"
`,
  );
  writeFixture(root, 'fixture-dependency/src/lib.rs', 'pub fn fixture_dependency() {}\n');
  writeFixture(
    root,
    'src/apps/relay-server/Cargo.toml',
    'version = "1.0.0" # x-release-please-version\n',
  );
  writeFixture(root, 'BitFun-Installer/src-tauri/Cargo.toml', 'version = "1.0.0"\n');
  const initialLock = spawnSync('cargo', ['generate-lockfile'], {
    cwd: root,
    encoding: 'utf8',
  });
  assert.equal(initialLock.status, 0, initialLock.stderr);

  setBuildVersion(root, '1.1.0-beta.2');

  for (const relative of jsonFiles) {
    const data = JSON.parse(readFileSync(path.join(root, relative), 'utf8'));
    assert.equal(data.version, '1.1.0-beta.2');
    assert.equal(data.packages[''].version, '1.1.0-beta.2');
  }
  assert.match(readFileSync(path.join(root, 'Cargo.toml'), 'utf8'), /1\.1\.0-beta\.2/);
  assert.match(
    readFileSync(path.join(root, 'src/apps/relay-server/Cargo.toml'), 'utf8'),
    /1\.1\.0-beta\.2/,
  );
  const lockfile = readFileSync(path.join(root, 'Cargo.lock'), 'utf8');
  assert.match(lockfile, /name = "build-version-fixture"\nversion = "1\.1\.0-beta\.2"/);
  assert.match(lockfile, /name = "fixture-dependency"\nversion = "1\.0\.0"/);
  const lockedMetadata = spawnSync('cargo', ['metadata', '--locked', '--no-deps'], {
    cwd: root,
    encoding: 'utf8',
  });
  assert.equal(lockedMetadata.status, 0, lockedMetadata.stderr);
});

function writeFixture(root, relative, content) {
  const file = path.join(root, relative);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, content);
}
