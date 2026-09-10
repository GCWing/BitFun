import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { resolveDevServerPorts } from './dev-server-ports.mjs';
import { prepareSherpaDev } from './prepare-sherpa-dev.mjs';

test('HTTP and HMR ports share one contract and reject conflicting overrides', () => {
  assert.deepEqual(resolveDevServerPorts({}), { port: 1422, hmrPort: 1421 });
  assert.deepEqual(resolveDevServerPorts({ OPENBITFUN_DEV_PORT: '1432' }), { port: 1432, hmrPort: 1431 });
  assert.deepEqual(resolveDevServerPorts({ OPENBITFUN_DEV_PORT: '1432', OPENBITFUN_DEV_HMR_PORT: '1440' }), { port: 1432, hmrPort: 1440 });
  for (const port of ['abc', '0', '65536', '1.5']) {
    assert.throws(() => resolveDevServerPorts({ OPENBITFUN_DEV_PORT: port }), /ports/);
  }
  assert.throws(() => resolveDevServerPorts({ OPENBITFUN_DEV_PORT: '1432', OPENBITFUN_DEV_HMR_PORT: '1432' }), /distinct/);
});

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'sherpa-dev-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  writeFileSync(join(root, 'Cargo.lock'), '[[package]]\nname = "sherpa-onnx-sys"\nversion = "1.13.4"\n');
  return root;
}
const target = 'aarch64-apple-darwin';
const archive = 'sherpa-onnx-v1.13.4-osx-arm64-static-lib.tar.bz2';

test('explicit library and archive paths are preserved', () => {
  for (const key of ['SHERPA_ONNX_LIB_DIR', 'SHERPA_ONNX_ARCHIVE_DIR']) {
    const env = { [key]: '/explicit' };
    prepareSherpaDev('/unused', env, { run: () => assert.fail('must not run commands') });
    assert.deepEqual(env, { [key]: '/explicit' });
  }
});

test('a worktree reuses the matching main checkout library without downloading', t => {
  const root = fixture(t);
  const main = join(root, 'main');
  const library = join(main, 'target/sherpa-onnx-prebuilt', archive.replace('.tar.bz2', ''), 'lib');
  mkdirSync(library, { recursive: true });
  writeFileSync(join(library, 'libsherpa-onnx-c-api.a'), 'fixture');
  const env = {};
  prepareSherpaDev(root, env, { target, run(command) {
    assert.equal(command, 'git');
    return { status: 0, stdout: join(main, '.git') };
  } });
  assert.equal(env.SHERPA_ONNX_LIB_DIR, library);
});

test('downloads the locked archive through curl and reuses it on the next run', t => {
  const root = fixture(t);
  let downloads = 0;
  const run = (command, args, options) => {
    if (command === 'git') return { status: 1 };
    assert.match(command, /^curl(?:\.exe)?$/);
    assert.equal(options.windowsHide, true);
    assert.equal(args.at(-1), `https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.4/${archive}`);
    writeFileSync(args[args.indexOf('--output') + 1], 'archive fixture');
    downloads++;
    return { status: 0 };
  };
  for (let i = 0; i < 2; i++) {
    const env = {};
    prepareSherpaDev(root, env, { target, run });
    assert.equal(readFileSync(join(env.SHERPA_ONNX_ARCHIVE_DIR, archive), 'utf8'), 'archive fixture');
  }
  assert.equal(downloads, 1);
});

test('failed downloads are not published as reusable archives', t => {
  const root = fixture(t);
  const env = {};
  assert.throws(() => prepareSherpaDev(root, env, { target, run(command) {
    return command === 'git' ? { status: 1 } : { status: 22, stderr: 'download failed' };
  } }), /download failed/);
  assert.equal(env.SHERPA_ONNX_ARCHIVE_DIR, undefined);
});
