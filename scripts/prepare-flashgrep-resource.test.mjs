import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { ensureFlashgrepBinary, flashgrepBinaryName } from './prepare-flashgrep-resource.mjs';

for (const [target, name] of [
  ['x86_64-apple-darwin', 'flashgrep-x86_64-apple-darwin'],
  ['aarch64-apple-darwin', 'flashgrep-aarch64-apple-darwin'],
  ['x86_64-pc-windows-msvc', 'flashgrep-x86_64-pc-windows-msvc.exe'],
  ['aarch64-pc-windows-msvc', 'flashgrep-aarch64-pc-windows-msvc.exe'],
  ['x86_64-unknown-linux-gnu', 'flashgrep-x86_64-unknown-linux-musl'],
  ['aarch64-unknown-linux-gnu', 'flashgrep-aarch64-unknown-linux-musl'],
]) {
  test(`selects ${target} independently of the build host`, () => {
    assert.equal(flashgrepBinaryName({ target, platform: 'darwin', arch: 'arm64' }), name);
  });
}

test('unsupported targets fail explicitly', () => {
  assert.throws(() => flashgrepBinaryName({ target: 'riscv64-unknown-linux-gnu' }), /Unsupported/);
  assert.throws(() => flashgrepBinaryName({ platform: 'linux', arch: 'ia32' }), /Unsupported/);
});

test('verified downloads are cached, corrupt caches are repaired, invalid downloads never replace assets', () => {
  const resourceDir = mkdtempSync(join(tmpdir(), 'flashgrep-download-'));
  const target = 'aarch64-apple-darwin';
  const name = flashgrepBinaryName({ target });
  const bytes = Buffer.from('verified binary');
  const manifest = { repo: 'wgqqqqq/flashgrep-binaries', tag: 'v0.2.16', assets: {
    [name]: { sha256: createHash('sha256').update(bytes).digest('hex'), size: bytes.length },
  } };
  let downloads = 0;
  const options = { resourceDir, target, manifest, download(url, destination) {
    downloads++;
    assert.equal(url, `https://github.com/wgqqqqq/flashgrep-binaries/releases/download/v0.2.16/${name}`);
    writeFileSync(destination, bytes);
  } };
  try {
    const path = ensureFlashgrepBinary(options);
    assert.deepEqual(readFileSync(path), bytes);
    assert.equal(ensureFlashgrepBinary(options), path);
    assert.equal(downloads, 1);
    writeFileSync(path, 'corrupt');
    ensureFlashgrepBinary(options);
    assert.equal(downloads, 2);
    writeFileSync(path, 'old content');
    assert.throws(() => ensureFlashgrepBinary({ ...options,
      download: (_, destination) => writeFileSync(destination, 'bad release'),
    }), /mismatch/);
    assert.equal(readFileSync(path, 'utf8'), 'old content');
    assert.deepEqual(readdirSync(resourceDir), [name]);
    assert.throws(() => ensureFlashgrepBinary({ ...options, manifest: { ...manifest, assets: {} } }), /Missing/);
    assert.throws(() => ensureFlashgrepBinary({ ...options,
      download: () => { throw new Error('offline'); },
    }), /offline/);
    assert.deepEqual(readdirSync(resourceDir), [name]);
  } finally {
    rmSync(resourceDir, { recursive: true, force: true });
  }
});
