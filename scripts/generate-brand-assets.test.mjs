import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import test from 'node:test';
import { canonicalizeIcns } from './icns-container.mjs';

const GENERATED_ICNS_FILES = [
  'src/apps/desktop/icons/openbitfun-app-icon.icns',
  'OpenBitFun-Installer/src-tauri/icons/openbitfun-app-icon.icns',
];

const HARMONY_MEDIA_DIRS = [
  'src/apps/mobile/harmonyos/AppScope/resources/base/media',
  'src/apps/mobile/harmonyos/entry/src/main/resources/base/media',
];

function createChunk(type, payload) {
  const chunk = Buffer.alloc(8 + payload.length);
  chunk.write(type, 0, 4, 'ascii');
  chunk.writeUInt32BE(chunk.length, 4);
  payload.copy(chunk, 8);
  return chunk;
}

function createIcns(chunks) {
  const length = 8 + chunks.reduce((total, chunk) => total + chunk.length, 0);
  const header = Buffer.alloc(8);
  header.write('icns', 0, 4, 'ascii');
  header.writeUInt32BE(length, 4);
  return Buffer.concat([header, ...chunks], length);
}

test('ICNS canonicalization is independent of Tauri chunk order', () => {
  const chunks = [
    createChunk('ic10', Buffer.from('large')),
    createChunk('ic07', Buffer.from('small')),
    createChunk('s8mk', Buffer.from('mask')),
  ];
  const forward = canonicalizeIcns(createIcns(chunks));
  const reverse = canonicalizeIcns(createIcns([...chunks].reverse()));

  assert.deepEqual(forward, reverse);
  assert.deepEqual(canonicalizeIcns(forward), forward);
});

test('generated macOS icons use the canonical ICNS layout', () => {
  const [desktop, installer] = GENERATED_ICNS_FILES.map(filePath => readFileSync(filePath));

  assert.ok(desktop.equals(canonicalizeIcns(desktop)), 'desktop ICNS is not canonical');
  assert.ok(installer.equals(canonicalizeIcns(installer)), 'installer ICNS is not canonical');
  assert.ok(desktop.equals(installer), 'desktop and installer ICNS files differ');
});

test('HarmonyOS generated media use valid resource identifiers', () => {
  for (const directory of HARMONY_MEDIA_DIRS) {
    for (const fileName of readdirSync(directory)) {
      const resourceName = fileName.replace(/\.[^.]+$/, '');
      assert.match(
        resourceName,
        /^[a-zA-Z0-9_]+$/,
        `${directory}/${fileName} is not a valid HarmonyOS resource name`,
      );
    }
  }

  const appConfig = readFileSync('src/apps/mobile/harmonyos/AppScope/app.json5', 'utf8');
  const moduleConfig = readFileSync('src/apps/mobile/harmonyos/entry/src/main/module.json5', 'utf8');
  assert.match(appConfig, /\$media:openbitfun_app_icon/);
  assert.match(moduleConfig, /\$media:openbitfun_app_icon/);
  assert.match(moduleConfig, /\$media:openbitfun_start_window/);
});

test('ICNS canonicalization rejects malformed containers', () => {
  assert.throws(
    () => canonicalizeIcns(Buffer.from('not-an-icns')),
    /Invalid ICNS header/,
  );

  const truncated = createIcns([createChunk('ic07', Buffer.from('small'))]).subarray(0, -1);
  assert.throws(() => canonicalizeIcns(truncated), /Invalid ICNS length/);
});
