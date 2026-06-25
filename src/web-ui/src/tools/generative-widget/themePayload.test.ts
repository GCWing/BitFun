import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  WIDGET_THEME_FALLBACK_VARS,
  createWidgetThemeFallbackCss,
  readWidgetThemePayload,
} from './themePayload';

const WIDGET_THEME_VAR_NAMES_HASH = 'c33a807d44e85a0771a50bfc9277eb98ffbd717d9b750ff742aab06420cc46ed';

function readPayloadWithHostValues(hostValues: Record<string, string> = {}) {
  const requestedNames: string[] = [];
  const root = {
    getAttribute(name: string): string | null {
      if (name === 'data-theme') {
        return 'test-theme';
      }
      if (name === 'data-theme-type') {
        return 'dark';
      }
      return null;
    },
  };

  vi.stubGlobal('document', { documentElement: root });
  vi.stubGlobal('window', {
    getComputedStyle: () => ({
      getPropertyValue: (name: string) => {
        requestedNames.push(name);
        return hostValues[name] || '';
      },
    }),
  });

  return {
    payload: readWidgetThemePayload(),
    requestedNames,
  };
}

function hashNames(names: string[]): string {
  return createHash('sha256')
    .update(names.join('\n'))
    .digest('hex');
}

describe('generated widget theme payload contract', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('keeps the host payload allowlist stable without exposing it as API', () => {
    const { requestedNames } = readPayloadWithHostValues();

    expect(new Set(requestedNames).size).toBe(requestedNames.length);
    expect({
      count: requestedNames.length,
      hash: hashNames(requestedNames),
      first: requestedNames[0],
      last: requestedNames[requestedNames.length - 1],
    }).toEqual({
      count: 323,
      hash: WIDGET_THEME_VAR_NAMES_HASH,
      first: '--color-bg-primary',
      last: '--tool-card-action-font-weight',
    });
  });

  it('includes every static iframe fallback key in the host payload allowlist', () => {
    const { payload } = readPayloadWithHostValues();

    expect(payload?.vars).toEqual(WIDGET_THEME_FALLBACK_VARS);
  });

  it('renders fallback CSS from the same reviewed fallback map', () => {
    const css = createWidgetThemeFallbackCss();

    for (const [name, value] of Object.entries(WIDGET_THEME_FALLBACK_VARS)) {
      expect(css).toContain(`      ${name}: ${value};`);
    }
  });
});
