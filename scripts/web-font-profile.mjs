import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const WEB_FONT_PROFILE_ENV = 'OPENBITFUN_WEB_FONT_PROFILE';
export const APPLE_SYSTEM_FONT_PROFILE = 'apple-system';
export const HARMONY_BUNDLED_FONT_PROFILE = 'harmony-bundled';

export const HARMONY_FONT_ASSETS = Object.freeze([
  {
    relativePath: 'base/HarmonyOS_Sans_Regular.ttf',
    bytes: 146_616,
    sha256: '4F00C7E80329238D0B6FC58E5C829C4086432BA9FA1A8C5CA3DA9A0442CE0452',
  },
  {
    relativePath: 'base/HarmonyOS_Sans_Medium.ttf',
    bytes: 146_164,
    sha256: 'F6B009D07D8D894D55EADEB7080B4916C3A2C83FF3EE60BBE851E6698D73BAFD',
  },
  {
    relativePath: 'base/HarmonyOS_Sans_Bold.ttf',
    bytes: 145_860,
    sha256: '7F973862C42353C9CC372DC2AE891D12C9EA5FE2A01B449ADAF1EADE9B469B47',
  },
  {
    relativePath: 'sc/HarmonyOS_Sans_SC_Regular.ttf',
    bytes: 8_261_128,
    sha256: '297B088424BE212207DF2CE8B98E335468B782AA6B96832AF0B8B773D711E2B1',
  },
  {
    relativePath: 'sc/HarmonyOS_Sans_SC_Medium.ttf',
    bytes: 8_227_312,
    sha256: '6ED1553EDCCDDC48EB27FF25D134A4A715CF54211238D4840B3038576CBA1944',
  },
  {
    relativePath: 'sc/HarmonyOS_Sans_SC_Bold.ttf',
    bytes: 8_158_996,
    sha256: '43A424B85E47FB53A17B3B32026A71801F86F8E022CA6798D186B47D39FA5F01',
  },
  {
    relativePath: 'LICENSE.txt',
    bytes: 32_768,
    sha256: 'B2FFEC0E6269EE41C3B5FC0345AB37600B46D66EBEA6C9C58FF37F517BDFA164',
  },
]);

const HARMONY_FONT_STEMS = Object.freeze(
  HARMONY_FONT_ASSETS
    .filter(({ relativePath }) => relativePath.endsWith('.ttf'))
    .map(({ relativePath }) => relativePath.split('/').at(-1).replace(/\.ttf$/, '')),
);

const FIRA_FONT_STEMS = Object.freeze([
  'FiraCode-Regular',
  'FiraCode-Medium',
  'FiraCode-SemiBold',
  'FiraCode-VF',
]);

const PRODUCT_FONT_ASSET_PATTERN = /(?:HarmonyOS[_-]Sans|FiraCode-|Noto[_-]Sans[_-]SC)[^/]*\.(?:ttf|otf|woff2?)$/i;

export function normalizeWebFontProfile(value) {
  if (value === APPLE_SYSTEM_FONT_PROFILE || value === HARMONY_BUNDLED_FONT_PROFILE) {
    return value;
  }
  throw new Error(
    `Unsupported ${WEB_FONT_PROFILE_ENV} value: ${value}. `
      + `Expected ${APPLE_SYSTEM_FONT_PROFILE} or ${HARMONY_BUNDLED_FONT_PROFILE}.`,
  );
}

export function resolveWebFontProfile({ requested, command = 'build', platform = process.platform } = {}) {
  if (requested) return normalizeWebFontProfile(requested);

  // A portable Web build must carry the non-Apple face. Development follows
  // the host so macOS never downloads product font packages merely to run HMR.
  if (command === 'serve' && platform === 'darwin') {
    return APPLE_SYSTEM_FONT_PROFILE;
  }
  return HARMONY_BUNDLED_FONT_PROFILE;
}

export function fontProfileForDesktopTarget({ target, platform = process.platform } = {}) {
  if (target) {
    return /apple/i.test(target)
      ? APPLE_SYSTEM_FONT_PROFILE
      : HARMONY_BUNDLED_FONT_PROFILE;
  }
  return platform === 'darwin'
    ? APPLE_SYSTEM_FONT_PROFILE
    : HARMONY_BUNDLED_FONT_PROFILE;
}

export function verifyHarmonyFontSources(assetRoot) {
  for (const expected of HARMONY_FONT_ASSETS) {
    const source = readFileSync(join(assetRoot, ...expected.relativePath.split('/')));
    const actualHash = createHash('sha256').update(source).digest('hex').toUpperCase();
    if (source.byteLength !== expected.bytes || actualHash !== expected.sha256) {
      throw new Error(
        `HarmonyOS Sans source changed: ${expected.relativePath}. `
          + `Expected ${expected.bytes} bytes / ${expected.sha256}, `
          + `received ${source.byteLength} bytes / ${actualHash}.`,
      );
    }
  }

  const expectedFontPaths = new Set(
    HARMONY_FONT_ASSETS
      .filter(({ relativePath }) => relativePath.endsWith('.ttf'))
      .map(({ relativePath }) => relativePath.toLowerCase()),
  );
  const unexpectedFonts = listFontFiles(assetRoot).filter(
    (relativePath) => !expectedFontPaths.has(relativePath.toLowerCase()),
  );
  if (unexpectedFonts.length > 0) {
    throw new Error(
      `HarmonyOS Sans source contains unapproved font files: ${unexpectedFonts.join(', ')}`,
    );
  }
}

export function assertWebFontProfileBundle(profile, bundleFileNames) {
  const normalized = normalizeWebFontProfile(profile);
  const names = [...bundleFileNames];
  const productFonts = names.filter((name) => PRODUCT_FONT_ASSET_PATTERN.test(name));

  if (normalized === APPLE_SYSTEM_FONT_PROFILE) {
    if (productFonts.length > 0) {
      throw new Error(
        `Apple Web bundle contains product text fonts: ${productFonts.join(', ')}`,
      );
    }
    return;
  }

  const expectedStems = [...HARMONY_FONT_STEMS, ...FIRA_FONT_STEMS];
  const matchesByStem = new Map(
    expectedStems.map((stem) => [
      stem,
      productFonts.filter((name) => name.includes(stem)),
    ]),
  );
  const missing = expectedStems.filter((stem) => matchesByStem.get(stem).length === 0);
  if (missing.length > 0) {
    throw new Error(`Harmony Web bundle is missing font assets: ${missing.join(', ')}`);
  }

  const duplicated = expectedStems.filter((stem) => matchesByStem.get(stem).length > 1);
  if (duplicated.length > 0) {
    throw new Error(`Harmony Web bundle contains duplicate font assets: ${duplicated.join(', ')}`);
  }

  const unexpectedFonts = productFonts.filter(
    (name) => !expectedStems.some((stem) => name.includes(stem)),
  );
  if (unexpectedFonts.length > 0) {
    throw new Error(
      `Harmony Web bundle contains unapproved font assets: ${unexpectedFonts.join(', ')}`,
    );
  }

  const wrongFormats = expectedStems.flatMap((stem) => {
    const expectedExtension = HARMONY_FONT_STEMS.includes(stem) ? '.ttf' : '.woff2';
    return matchesByStem.get(stem).filter(
      (name) => !name.toLowerCase().endsWith(expectedExtension),
    );
  });
  if (wrongFormats.length > 0) {
    throw new Error(
      `Harmony Web bundle contains font assets in unapproved formats: ${wrongFormats.join(', ')}`,
    );
  }

  for (const legalFile of [
    'third-party/fonts/harmonyos-sans/LICENSE.txt',
    'third-party/fonts/harmonyos-sans/NOTICE.txt',
    'third-party/fonts/fira-code/LICENSE.txt',
  ]) {
    if (!names.includes(legalFile)) {
      throw new Error(`Harmony Web bundle is missing legal asset: ${legalFile}`);
    }
  }
}

function listFontFiles(root, current = root, prefix = '') {
  const paths = [];
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) {
      paths.push(...listFontFiles(root, join(current, entry.name), relativePath));
    } else if (/\.(?:ttf|otf|woff2?)$/i.test(entry.name)) {
      paths.push(relativePath);
    }
  }
  return paths.sort();
}
