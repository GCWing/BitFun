import assert from 'node:assert/strict';
import { join } from 'node:path';
import test from 'node:test';
import {
  APPLE_SYSTEM_FONT_PROFILE,
  HARMONY_BUNDLED_FONT_PROFILE,
  HARMONY_FONT_ASSETS,
  assertWebFontProfileBundle,
  fontProfileForDesktopTarget,
  normalizeWebFontProfile,
  resolveWebFontProfile,
  verifyHarmonyFontSources,
} from './web-font-profile.mjs';

const ROOT = join(import.meta.dirname, '..');
const harmonyBundle = [
  ...HARMONY_FONT_ASSETS
    .filter(({ relativePath }) => relativePath.endsWith('.ttf'))
    .map(({ relativePath }) => {
      const stem = relativePath.split('/').at(-1).replace(/\.ttf$/, '');
      return `assets/${stem}-contenthash.ttf`;
    }),
  'assets/FiraCode-Regular-contenthash.woff2',
  'assets/FiraCode-Medium-contenthash.woff2',
  'assets/FiraCode-SemiBold-contenthash.woff2',
  'assets/FiraCode-VF-contenthash.woff2',
  'third-party/fonts/harmonyos-sans/LICENSE.txt',
  'third-party/fonts/harmonyos-sans/NOTICE.txt',
  'third-party/fonts/fira-code/LICENSE.txt',
  'assets/KaTeX_Main-Regular-contenthash.woff2',
  'assets/codicon-contenthash.ttf',
];

test('Web font profiles resolve explicitly and use safe platform defaults', () => {
  assert.equal(normalizeWebFontProfile(APPLE_SYSTEM_FONT_PROFILE), APPLE_SYSTEM_FONT_PROFILE);
  assert.equal(
    normalizeWebFontProfile(HARMONY_BUNDLED_FONT_PROFILE),
    HARMONY_BUNDLED_FONT_PROFILE,
  );
  assert.throws(() => normalizeWebFontProfile('legacy-fonts'), /Unsupported/);

  assert.equal(resolveWebFontProfile({ command: 'build', platform: 'darwin' }), HARMONY_BUNDLED_FONT_PROFILE);
  assert.equal(resolveWebFontProfile({ command: 'serve', platform: 'darwin' }), APPLE_SYSTEM_FONT_PROFILE);
  assert.equal(resolveWebFontProfile({ command: 'serve', platform: 'win32' }), HARMONY_BUNDLED_FONT_PROFILE);
  assert.equal(
    resolveWebFontProfile({
      requested: APPLE_SYSTEM_FONT_PROFILE,
      command: 'build',
      platform: 'win32',
    }),
    APPLE_SYSTEM_FONT_PROFILE,
  );
});

test('Desktop targets select Apple system fonts only for Apple triples', () => {
  assert.equal(
    fontProfileForDesktopTarget({ target: 'aarch64-apple-darwin', platform: 'win32' }),
    APPLE_SYSTEM_FONT_PROFILE,
  );
  assert.equal(
    fontProfileForDesktopTarget({ target: 'x86_64-pc-windows-msvc', platform: 'darwin' }),
    HARMONY_BUNDLED_FONT_PROFILE,
  );
  assert.equal(
    fontProfileForDesktopTarget({ target: 'x86_64-unknown-linux-gnu', platform: 'darwin' }),
    HARMONY_BUNDLED_FONT_PROFILE,
  );
  assert.equal(fontProfileForDesktopTarget({ platform: 'darwin' }), APPLE_SYSTEM_FONT_PROFILE);
  assert.equal(fontProfileForDesktopTarget({ platform: 'linux' }), HARMONY_BUNDLED_FONT_PROFILE);
});

test('Harmony source profile contains only the six approved unmodified fonts', () => {
  const fonts = HARMONY_FONT_ASSETS.filter(({ relativePath }) => relativePath.endsWith('.ttf'));
  assert.equal(fonts.length, 6);
  assert.equal(fonts.some(({ relativePath }) => /(?:^|\/)tc(?:\/|$)/i.test(relativePath)), false);
  verifyHarmonyFontSources(join(
    ROOT,
    'src',
    'web-ui',
    'src',
    'assets',
    'fonts',
    'harmonyos-sans',
  ));
});

test('Apple bundles reject product text fonts but allow functional fonts', () => {
  assert.doesNotThrow(() => assertWebFontProfileBundle(APPLE_SYSTEM_FONT_PROFILE, [
    'index.html',
    'assets/KaTeX_Main-Regular-contenthash.woff2',
    'assets/codicon-contenthash.ttf',
  ]));

  for (const forbidden of [
    'assets/HarmonyOS_Sans_SC_Regular-contenthash.ttf',
    'assets/FiraCode-Regular-contenthash.woff2',
    'fonts/noto-sans-sc-latin-wght-normal.woff2',
  ]) {
    assert.throws(
      () => assertWebFontProfileBundle(APPLE_SYSTEM_FONT_PROFILE, ['index.html', forbidden]),
      /contains product text fonts/,
    );
  }
});

test('Harmony bundles require the exact approved font and legal asset set', () => {
  assert.doesNotThrow(() => assertWebFontProfileBundle(HARMONY_BUNDLED_FONT_PROFILE, harmonyBundle));

  assert.throws(
    () => assertWebFontProfileBundle(
      HARMONY_BUNDLED_FONT_PROFILE,
      harmonyBundle.filter((name) => !name.includes('HarmonyOS_Sans_SC_Bold')),
    ),
    /missing font assets: HarmonyOS_Sans_SC_Bold/,
  );
  assert.throws(
    () => assertWebFontProfileBundle(HARMONY_BUNDLED_FONT_PROFILE, [
      ...harmonyBundle,
      'assets/HarmonyOS_Sans_SC_Bold-copy.ttf',
    ]),
    /duplicate font assets: HarmonyOS_Sans_SC_Bold/,
  );
  assert.throws(
    () => assertWebFontProfileBundle(HARMONY_BUNDLED_FONT_PROFILE, [
      ...harmonyBundle,
      'assets/HarmonyOS_Sans_TC_Regular-contenthash.ttf',
    ]),
    /unapproved font assets/,
  );
  assert.throws(
    () => assertWebFontProfileBundle(HARMONY_BUNDLED_FONT_PROFILE, [
      ...harmonyBundle,
      'assets/noto-sans-sc-latin-wght-normal.woff2',
    ]),
    /unapproved font assets/,
  );
  assert.throws(
    () => assertWebFontProfileBundle(
      HARMONY_BUNDLED_FONT_PROFILE,
      harmonyBundle.filter((name) => !name.endsWith('/NOTICE.txt')),
    ),
    /missing legal asset/,
  );
});
