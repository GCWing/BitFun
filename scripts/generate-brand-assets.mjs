import { execFileSync } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';
import { canonicalizeIcns } from './icns-container.mjs';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(SCRIPT_DIR, '..');
const SOURCE_DIR = path.join(ROOT_DIR, 'assets', 'brand', 'source');
const SOURCE_MARKS = {
  dark: path.join(SOURCE_DIR, 'openbitfun-mark-dark.png'),
  light: path.join(SOURCE_DIR, 'openbitfun-mark-light.png'),
};

const BRAND_SIZE = 512;
const APP_ICON_SIZE = 1024;
const APP_ICON_CORNER_RADIUS = 96;
const APP_MARK_LIFT = 28;
const ANDROID_FOREGROUND_SCALE = 0.66;

const ANDROID_DENSITIES = {
  mdpi: { icon: 48, adaptive: 108 },
  hdpi: { icon: 72, adaptive: 162 },
  xhdpi: { icon: 96, adaptive: 216 },
  xxhdpi: { icon: 144, adaptive: 324 },
  xxxhdpi: { icon: 192, adaptive: 432 },
};

const DESKTOP_HICOLOR_SIZES = [16, 32, 48, 64, 96, 128, 256, 512];

const LEGACY_APPLICATION_ASSETS = [
  'src/apps/desktop/icons/Logo-ICON.png',
  'src/apps/desktop/icons/icon.png',
  'src/apps/desktop/icons/icon.ico',
  'src/apps/desktop/icons/icon.icns',
  'src/apps/desktop/icons/Square30x30Logo.png',
  'src/apps/desktop/icons/Square44x44Logo.png',
  'src/apps/desktop/icons/Square71x71Logo.png',
  'src/apps/desktop/icons/Square89x89Logo.png',
  'src/apps/desktop/icons/Square107x107Logo.png',
  'src/apps/desktop/icons/Square142x142Logo.png',
  'src/apps/desktop/icons/Square150x150Logo.png',
  'src/apps/desktop/icons/Square284x284Logo.png',
  'src/apps/desktop/icons/Square310x310Logo.png',
  'src/apps/desktop/icons/StoreLogo.png',
  'src/web-ui/public/Logo-ICON.png',
  'src/web-ui/public/Logo-ICON-128.png',
  'src/web-ui/public/OpenBitFun-Logo.png',
  'src/mobile-web/src/assets/Logo-ICON.png',
  'OpenBitFun-Installer/src/Logo-ICON.png',
  'OpenBitFun-Installer/src-tauri/icons/icon.png',
  'OpenBitFun-Installer/src-tauri/icons/icon.ico',
  'OpenBitFun-Installer/src-tauri/icons/icon.icns',
  'src/apps/mobile/harmonyos/AppScope/resources/base/media/openbitfun_icon.png',
  'src/apps/mobile/harmonyos/AppScope/resources/base/media/background.png',
  'src/apps/mobile/harmonyos/AppScope/resources/base/media/foreground.png',
  'src/apps/mobile/harmonyos/AppScope/resources/base/media/layered_image.json',
  'src/apps/mobile/harmonyos/AppScope/resources/base/media/openbitfun-app-icon.png',
  'src/apps/mobile/harmonyos/entry/src/main/resources/base/media/openbitfun_icon.png',
  'src/apps/mobile/harmonyos/entry/src/main/resources/base/media/background.png',
  'src/apps/mobile/harmonyos/entry/src/main/resources/base/media/foreground.png',
  'src/apps/mobile/harmonyos/entry/src/main/resources/base/media/layered_image.json',
  'src/apps/mobile/harmonyos/entry/src/main/resources/base/media/startIcon.png',
  'src/apps/mobile/harmonyos/entry/src/main/resources/base/media/openbitfun-app-icon.png',
  'src/apps/mobile/harmonyos/entry/src/main/resources/base/media/openbitfun-start-window.png',
  'src/apps/mobile/ios/OpenBitFun/Resources.xcassets/AppIcon.appiconset/openbitfun_icon.png',
  'src/apps/mobile/ios/OpenBitFun/Resources.xcassets/OpenBitFunLogo.imageset',
  'src/apps/relay-server/static/assets/Logo-ICON-BOaKcXgO.png',
];

const outputPath = (...segments) => path.join(ROOT_DIR, ...segments);

async function writePng(filePath, buffer) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, buffer);
}

async function normalizePng(input) {
  return sharp(input)
    .ensureAlpha()
    .resize({ width: BRAND_SIZE, height: BRAND_SIZE, fit: 'contain', kernel: 'lanczos3' })
    .png({ compressionLevel: 9, adaptiveFiltering: true })
    .toBuffer();
}

async function resizePng(input, size) {
  return sharp(input)
    .resize({ width: size, height: size, fit: 'contain', kernel: 'lanczos3' })
    .png({ compressionLevel: 9, adaptiveFiltering: true })
    .toBuffer();
}

async function createApplicationMark(lightMark) {
  const { data, info } = await sharp(lightMark)
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });
  const channels = info.channels;
  const whiteMark = Buffer.alloc(data.length);

  for (let index = 0; index < data.length; index += channels) {
    const sourceTone = data[index];
    const liftedTone = Math.min(255, sourceTone + APP_MARK_LIFT);
    whiteMark[index] = liftedTone;
    whiteMark[index + 1] = liftedTone;
    whiteMark[index + 2] = liftedTone;
    whiteMark[index + 3] = data[index + 3];
  }

  return sharp(whiteMark, {
    raw: { width: info.width, height: info.height, channels: 4 },
  })
    .png({ compressionLevel: 9, adaptiveFiltering: true })
    .toBuffer();
}

async function createApplicationIcon(applicationMark) {
  const background = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${BRAND_SIZE}" height="${BRAND_SIZE}" viewBox="0 0 ${BRAND_SIZE} ${BRAND_SIZE}">` +
      `<rect width="${BRAND_SIZE}" height="${BRAND_SIZE}" rx="${APP_ICON_CORNER_RADIUS}" fill="#000000"/>` +
    '</svg>',
  );

  return sharp({
    create: {
      width: BRAND_SIZE,
      height: BRAND_SIZE,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite([
      { input: background },
      { input: applicationMark },
    ])
    .png({ compressionLevel: 9, adaptiveFiltering: true })
    .toBuffer();
}

async function createAdaptiveForeground(applicationMark, size) {
  const artworkSize = Math.round(size * ANDROID_FOREGROUND_SCALE);
  const artwork = await resizePng(applicationMark, artworkSize);

  return sharp({
    create: {
      width: size,
      height: size,
      channels: 4,
      background: { r: 255, g: 255, b: 255, alpha: 0 },
    },
  })
    .composite([{
      input: artwork,
      left: Math.floor((size - artworkSize) / 2),
      top: Math.floor((size - artworkSize) / 2),
    }])
    .png({ compressionLevel: 9, adaptiveFiltering: true })
    .toBuffer();
}

async function generateTauriContainers(applicationIcon) {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'openbitfun-tauri-icons-'));
  const inputPath = path.join(tempDir, 'openbitfun-app-icon.png');
  const tauriCliPath = path.join(ROOT_DIR, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');

  try {
    await writeFile(inputPath, applicationIcon);
    execFileSync(
      process.execPath,
      [tauriCliPath, 'icon', inputPath, '--output', tempDir],
      { cwd: ROOT_DIR, stdio: 'ignore' },
    );

    return {
      ico: await readFile(path.join(tempDir, 'icon.ico')),
      icns: canonicalizeIcns(await readFile(path.join(tempDir, 'icon.icns'))),
    };
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function removeLegacyApplicationAssets() {
  await Promise.all(
    LEGACY_APPLICATION_ASSETS.map(relativePath =>
      rm(outputPath(...relativePath.split('/')), { recursive: true, force: true })),
  );
}

async function generateBrandAssets() {
  const [darkMark, lightMark] = await Promise.all([
    normalizePng(SOURCE_MARKS.dark),
    normalizePng(SOURCE_MARKS.light),
  ]);
  const applicationMark = await createApplicationMark(lightMark);
  const applicationIcon = await createApplicationIcon(applicationMark);
  const applicationIconLarge = await resizePng(applicationIcon, APP_ICON_SIZE);
  const darkMarkSmall = await resizePng(darkMark, 128);
  const lightMarkSmall = await resizePng(lightMark, 128);
  const tauriContainers = await generateTauriContainers(applicationIconLarge);

  const webBrandDir = outputPath('src', 'web-ui', 'public', 'brand');
  await writePng(path.join(webBrandDir, 'openbitfun-mark-dark.png'), darkMark);
  await writePng(path.join(webBrandDir, 'openbitfun-mark-light.png'), lightMark);
  await writePng(path.join(webBrandDir, 'openbitfun-mark-dark-128.png'), darkMarkSmall);
  await writePng(path.join(webBrandDir, 'openbitfun-mark-light-128.png'), lightMarkSmall);
  await writePng(path.join(webBrandDir, 'openbitfun-app-icon.png'), applicationIcon);

  const desktopIconDir = outputPath('src', 'apps', 'desktop', 'icons');
  await writePng(path.join(desktopIconDir, 'openbitfun-app-icon.png'), applicationIconLarge);
  await writePng(path.join(desktopIconDir, 'openbitfun-app-icon.ico'), tauriContainers.ico);
  await writePng(path.join(desktopIconDir, 'openbitfun-app-icon.icns'), tauriContainers.icns);
  for (const size of DESKTOP_HICOLOR_SIZES) {
    const icon = await resizePng(applicationIcon, size);
    await writePng(
      outputPath('src', 'apps', 'desktop', 'icons', 'hicolor', `${size}x${size}`, 'apps', 'openbitfun-desktop.png'),
      icon,
    );
  }

  const mobileWebAssetDir = outputPath('src', 'mobile-web', 'src', 'assets');
  await writePng(path.join(mobileWebAssetDir, 'openbitfun-mark-dark.png'), darkMark);
  await writePng(path.join(mobileWebAssetDir, 'openbitfun-mark-light.png'), lightMark);
  await writePng(
    outputPath('src', 'mobile-web', 'public', 'brand', 'openbitfun-app-icon.png'),
    applicationIcon,
  );
  await writePng(
    outputPath('src', 'apps', 'relay-server', 'static', 'brand', 'openbitfun-app-icon.png'),
    applicationIcon,
  );

  const installerBrandDir = outputPath('OpenBitFun-Installer', 'src', 'assets');
  await writePng(path.join(installerBrandDir, 'openbitfun-mark-dark.png'), darkMark);
  await writePng(path.join(installerBrandDir, 'openbitfun-mark-light.png'), lightMark);
  await writePng(path.join(installerBrandDir, 'openbitfun-app-icon.png'), applicationIcon);
  const installerIconDir = outputPath('OpenBitFun-Installer', 'src-tauri', 'icons');
  await writePng(path.join(installerIconDir, 'openbitfun-app-icon.png'), applicationIconLarge);
  await writePng(path.join(installerIconDir, 'openbitfun-app-icon.ico'), tauriContainers.ico);
  await writePng(path.join(installerIconDir, 'openbitfun-app-icon.icns'), tauriContainers.icns);

  for (const [density, sizes] of Object.entries(ANDROID_DENSITIES)) {
    const legacyIcon = await resizePng(applicationIcon, sizes.icon);
    const adaptiveForeground = await createAdaptiveForeground(applicationMark, sizes.adaptive);
    const androidDir = outputPath('src', 'apps', 'mobile', 'android', 'app', 'src', 'main', 'res', `mipmap-${density}`);
    await writePng(path.join(androidDir, 'ic_launcher.png'), legacyIcon);
    await writePng(path.join(androidDir, 'ic_launcher_round.png'), legacyIcon);
    await writePng(path.join(androidDir, 'ic_launcher_foreground.png'), adaptiveForeground);
    await writePng(path.join(androidDir, 'ic_launcher_monochrome.png'), adaptiveForeground);
  }

  await writePng(
    outputPath('src', 'apps', 'mobile', 'ios', 'OpenBitFun', 'Resources.xcassets', 'AppIcon.appiconset', 'openbitfun-app-icon.png'),
    applicationIconLarge,
  );
  await writePng(
    outputPath('src', 'apps', 'mobile', 'ios', 'OpenBitFun', 'Resources.xcassets', 'OpenBitFunMark.imageset', 'openbitfun-mark-light.png'),
    lightMark,
  );

  await writePng(
    outputPath('src', 'apps', 'mobile', 'harmonyos', 'AppScope', 'resources', 'base', 'media', 'openbitfun_app_icon.png'),
    applicationIconLarge,
  );
  await writePng(
    outputPath('src', 'apps', 'mobile', 'harmonyos', 'entry', 'src', 'main', 'resources', 'base', 'media', 'openbitfun_app_icon.png'),
    applicationIconLarge,
  );
  await writePng(
    outputPath('src', 'apps', 'mobile', 'harmonyos', 'entry', 'src', 'main', 'resources', 'base', 'media', 'openbitfun_start_window.png'),
    await resizePng(lightMark, 144),
  );

  await removeLegacyApplicationAssets();

  console.log('Generated OpenBitFun application brand assets.');
}

await generateBrandAssets();
