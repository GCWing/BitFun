import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  WIDGET_THEME_FALLBACK_VARS,
  createWidgetThemeFallbackCss,
  readWidgetThemePayload,
} from './themePayload';

const WIDGET_THEME_VAR_NAMES_HASH = 'c674aa29ababf56a37ef1d9c7ccf04de06ccddc4ba7599337e079b3f8a42b3e8';
const RETIRED_WIDGET_THEME_COMPAT_KEYS = [
  '--color-accent',
  '--color-accent-primary',
  '--color-accent-alpha',
  '--color-primary',
  '--color-primary-rgb',
  '--color-primary-400',
  '--color-primary-hover',
  '--color-primary-500',
  '--color-primary-alpha',
  '--color-primary-bg',
  '--color-primary-bg-subtle',
  '--accent-primary',
  '--accent-primary-hover',
  '--color-danger',
  '--color-danger-500',
  '--color-danger-text',
  '--color-danger-bg',
  '--color-danger-border',
  '--color-danger-hover',
] as const;

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
      count: 307,
      hash: WIDGET_THEME_VAR_NAMES_HASH,
      first: '--color-bg-primary',
      last: '--tool-card-action-font-weight',
    });
  });

  it('includes every static iframe fallback key in the host payload allowlist', () => {
    const { payload } = readPayloadWithHostValues();

    expect(payload?.vars).toEqual(WIDGET_THEME_FALLBACK_VARS);
  });

  it('does not export retired accent and danger compatibility keys', () => {
    const { requestedNames } = readPayloadWithHostValues();

    expect(requestedNames).not.toEqual(expect.arrayContaining(RETIRED_WIDGET_THEME_COMPAT_KEYS));
    expect(requestedNames).toEqual(
      expect.arrayContaining([
        '--color-accent-50',
        '--color-accent-100',
        '--color-accent-400',
        '--color-accent-500',
        '--color-accent-500-rgb',
        '--color-accent-600',
        '--color-error',
        '--color-error-bg',
        '--color-error-border',
      ])
    );
  });

  it('renders fallback CSS from the same reviewed fallback map', () => {
    const css = createWidgetThemeFallbackCss();

    for (const [name, value] of Object.entries(WIDGET_THEME_FALLBACK_VARS)) {
      expect(css).toContain(`      ${name}: ${value};`);
    }
  });
});
