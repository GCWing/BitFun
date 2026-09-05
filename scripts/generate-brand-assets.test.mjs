import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import test from 'node:test';
import sharp from 'sharp';
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

test('brand exports provide decodable transparent PNGs at every advertised size', async () => {
  const sizes = [16, 24, 32, 48, 64, 96, 128, 192, 256, 512, 1024, 2048];
  for (const size of sizes) {
    for (const treatment of ['mark-dark', 'mark-light', 'app-icon']) {
      const image = sharp(`assets/brand/exports/openbitfun-${treatment}-${size}.png`);
      const metadata = await image.metadata();
      assert.equal(metadata.width, size);
      assert.equal(metadata.height, size);
      assert.equal(metadata.hasAlpha, true);
      const { data, info } = await image.raw().toBuffer({ resolveWithObject: true });
      // Tiny rounded-square icons can have partial edge coverage in the corner.
      assert.ok(data[info.channels - 1] <= 16, 'corner must remain transparent apart from antialiasing');
    }
  }
  assert.deepEqual(
    readFileSync('assets/brand/exports/openbitfun-mark.svg'),
    readFileSync('assets/brand/source/openbitfun-mark.svg'),
  );
});

test('Windows ICO frames contain the size-specific app PNGs', async () => {
  const ico = readFileSync('src/apps/desktop/icons/openbitfun-app-icon.ico');
  assert.equal(ico.readUInt16LE(0), 0);
  assert.equal(ico.readUInt16LE(2), 1);
  const sizes = [];
  for (let index = 0; index < ico.readUInt16LE(4); index++) {
    const entry = 6 + index * 16;
    const size = ico[entry] || 256;
    const length = ico.readUInt32LE(entry + 8);
    const offset = ico.readUInt32LE(entry + 12);
    assert.ok(offset + length <= ico.length);
    const frame = ico.subarray(offset, offset + length);
    assert.deepEqual(frame, readFileSync(`assets/brand/exports/openbitfun-app-icon-${size}.png`));
    const metadata = await sharp(frame).metadata();
    assert.equal(metadata.width, size);
    assert.equal(metadata.height, size);
    sizes.push(size);
  }
  assert.deepEqual(sizes.sort((a, b) => a - b), [16, 24, 32, 48, 64, 256]);
  assert.deepEqual(ico, readFileSync('OpenBitFun-Installer/src-tauri/icons/openbitfun-app-icon.ico'));
});

test('small icons retain a bright rim around the entire silhouette', async () => {
  for (const size of [16, 24, 32, 48, 64]) {
    const { data, info } = await sharp(`assets/brand/exports/openbitfun-app-icon-${size}.png`)
      .raw().toBuffer({ resolveWithObject: true });
    const sectors = Array(12).fill(0);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const dx = (x + 0.5) / size - 0.5;
        const dy = (y + 0.5) / size - 0.5;
        const radius = Math.hypot(dx, dy);
        if (radius < 0.32 || radius > 0.44) continue;
        const sector = Math.floor((Math.atan2(dy, dx) + Math.PI) * 12 / (2 * Math.PI)) % 12;
        sectors[sector] = Math.max(sectors[sector], data[(y * size + x) * info.channels]);
      }
    }
    assert.ok(sectors.every(value => value >= 200), `${size}px rim has a dim sector: ${sectors}`);
  }
});

test('menu-bar template remains transparent and visible at its logical size', async () => {
  const { data, info } = await sharp('src/apps/desktop/icons/openbitfun-tray-template.png')
    .resize(16, 16).raw().toBuffer({ resolveWithObject: true });
  assert.equal(info.channels, 4);
  assert.equal(data[3], 0, 'template must not include the app background');
  assert.equal(data[(8 * 16 + 8) * 4 + 3], 0, 'center must remain open');
  let opaquePixels = 0;
  for (let offset = 3; offset < data.length; offset += 4) {
    if (data[offset] >= 200) opaquePixels++;
  }
  assert.ok(opaquePixels >= 24, 'template loses its silhouette at menu-bar size');
});

test('browser entry points reference generated optical favicons', () => {
  for (const [htmlPath, assetDir] of [
    ['src/web-ui/index.html', 'src/web-ui/public/brand'],
    ['src/mobile-web/index.html', 'src/mobile-web/public/brand'],
    ['src/apps/relay-server/static/index.html', 'src/apps/relay-server/static/brand'],
    ['OpenBitFun-Installer/index.html', 'OpenBitFun-Installer/src/assets'],
  ]) {
    const html = readFileSync(htmlPath, 'utf8');
    for (const size of [16, 32]) {
      assert.ok(html.includes(`sizes="${size}x${size}"`));
      const name = `openbitfun-app-icon-${size}.png`;
      assert.ok(html.includes(name));
      assert.deepEqual(readFileSync(`${assetDir}/${name}`), readFileSync(`assets/brand/exports/${name}`));
    }
  }
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
