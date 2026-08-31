import { existsSync, readFileSync } from 'node:fs';
import { basename, dirname, extname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { brotliDecompressSync } from 'node:zlib';
import { compile } from 'sass';
import { describe, expect, it } from 'vitest';

const webRoot = resolve(__dirname, '../../..');
const systemTokens = JSON.parse(readFileSync(
  resolve(webRoot, '../../design-system/packages/design-tokens/src/system.tokens.json'), 'utf8',
));

// Read the bundled WOFF2's untransformed head/hhea tables. No font download or
// platform font fallback is involved in this line-box contract.
function bundledFontLineHeight(): number {
  const font = readFileSync(resolve(webRoot,
    'public/fonts/Noto_Sans_SC/variable/noto-sans-sc-latin-wght-normal.woff2'));
  expect(font.toString('ascii', 0, 4)).toBe('wOF2');
  let cursor = 48;
  const readBase128 = () => {
    let value = 0;
    for (let index = 0; index < 5; index += 1) {
      const byte = font[cursor++];
      value = value * 128 + (byte & 127);
      if (!(byte & 128)) return value;
    }
    throw new Error('Invalid WOFF2 table length');
  };
  const tables: { tag: string; length: number }[] = [];
  for (let index = 0; index < font.readUInt16BE(12); index += 1) {
    const flags = font[cursor++];
    const tagIndex = flags & 63;
    let tag = String(tagIndex);
    if (tagIndex === 63) {
      tag = font.toString('ascii', cursor, cursor + 4);
      cursor += 4;
    } else if (tagIndex === 1) tag = 'head';
    else if (tagIndex === 2) tag = 'hhea';
    const originalLength = readBase128();
    const transformed = tagIndex === 10 || tagIndex === 11
      ? (flags >> 6) === 0 : (flags >> 6) !== 0;
    if (tag === 'head' || tag === 'hhea') expect(transformed).toBe(false);
    tables.push({ tag, length: transformed ? readBase128() : originalLength });
  }
  const data = brotliDecompressSync(font.subarray(cursor, cursor + font.readUInt32BE(20)));
  let offset = 0;
  let unitsPerEm = 0;
  let extent = 0;
  for (const { tag, length } of tables) {
    if (tag === 'head') unitsPerEm = data.readUInt16BE(offset + 18);
    if (tag === 'hhea') {
      extent = data.readInt16BE(offset + 4) - data.readInt16BE(offset + 6)
        + data.readInt16BE(offset + 8);
    }
    offset += length;
  }
  expect(unitsPerEm).toBeGreaterThan(0);
  expect(extent).toBeGreaterThan(0);
  return extent / unitsPerEm;
}

function compiledRules(filename: string) {
  const css = compile(resolve(webRoot, 'src/app', filename), {
    importers: [{
      findFileUrl(url: string) {
        if (!url.startsWith('@/')) return null;
        const unresolved = resolve(webRoot, 'src', url.slice(2));
        const candidates = extname(unresolved)
          ? [unresolved]
          : [
              `${unresolved}.scss`,
              resolve(dirname(unresolved), `_${basename(unresolved)}.scss`),
              resolve(unresolved, '_index.scss'),
              resolve(unresolved, 'index.scss'),
            ];
        const matched = candidates.find(candidate => existsSync(candidate));
        return matched ? pathToFileURL(matched) : null;
      },
    }],
  }).css;
  return (selector: string) => {
    const declarations: Record<string, string> = {};
    let matched = false;
    for (const rule of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      if (!rule[1].split(',').some((entry) => entry.trim() === selector)) continue;
      matched = true;
      for (const declaration of rule[2].split(';')) {
        const colon = declaration.indexOf(':');
        if (colon < 0) continue;
        declarations[declaration.slice(0, colon).trim()] = declaration.slice(colon + 1).trim();
      }
    }
    expect(matched, `Missing selector: ${selector}`).toBe(true);
    return declarations;
  };
}

describe('Truncated product text line boxes', () => {
  it('fits the bundled font metrics without changing the compact system scale globally', () => {
    const required = bundledFontLineHeight();
    expect(systemTokens.lineHeight.tight.$value).toBeLessThan(required);
    expect(systemTokens.lineHeight.base.$value).toBeGreaterThanOrEqual(required);
  });

  it.each([
    ['components/NavPanel/NavPanel.scss', '.bitfun-nav-panel__search-trigger'],
    ['components/NavPanel/NavPanel.scss', '.bitfun-nav-panel__section-label'],
    ['components/NavPanel/sections/sessions/SessionsSection.scss', '.bitfun-nav-panel__inline-item'],
    ['components/NavPanel/sections/sessions/SessionsSection.scss', '.bitfun-nav-panel__inline-item-assistant-name'],
    ['components/NavPanel/sections/sessions/SessionsSection.scss', '.bitfun-nav-panel__inline-item-workspace-name'],
    ['components/NavPanel/sections/sessions/SessionsSection.scss', '.bitfun-nav-panel__inline-toggle'],
    ['components/NavPanel/sections/workspaces/WorkspaceListSection.scss', '.bitfun-nav-panel__workspace-item-label'],
    ['components/NavPanel/sections/workspaces/WorkspaceListSection.scss', '.bitfun-nav-panel__assistant-item-label'],
    ['scenes/skills/SkillsScene.scss', '.skills-card__name'],
    ['scenes/skills/SkillsScene.scss', '.skills-card__desc'],
  ])('%s gives %s a font-relative, descender-safe line height', (filename, selector) => {
    expect(compiledRules(filename)(selector)['line-height']).toBe('var(--bf-line-height-base)');
  });

  it('retains single-line ellipsis and the two-line skills description clamp', () => {
    const skills = compiledRules('scenes/skills/SkillsScene.scss');
    expect(skills('.skills-card__name')['text-overflow']).toBe('ellipsis');
    expect(skills('.skills-card__desc')['-webkit-line-clamp']).toBe('2');
    const sessions = compiledRules('components/NavPanel/sections/sessions/SessionsSection.scss');
    expect(sessions('.bitfun-nav-panel__inline-item-label')['text-overflow']).toBe('ellipsis');
    expect(sessions('.bitfun-nav-panel__inline-item-label')['line-height']).toBe('inherit');
    expect(sessions('.bitfun-nav-panel__inline-item').height).toBe('30px');
    expect(sessions('.bitfun-nav-panel__inline-item.is-assistant-session').height).toBe('auto');
    expect(sessions('.bitfun-nav-panel__inline-item.is-assistant-session')['min-height']).toBe('42px');
  });
});
