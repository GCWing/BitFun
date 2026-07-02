import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  WIDGET_THEME_FALLBACK_VARS,
  createWidgetThemeFallbackCss,
  readWidgetThemePayload,
} from './themePayload';
import { createWidgetThemeCompatibilityAliasCss } from './themePayloadCompatibility';

const WIDGET_THEME_VAR_NAMES_HASH = 'db206ee7339eafb5d58e16df81119e07573649897c40ac9907fa1ed38df6a451';
const RETIRED_WIDGET_THEME_COMPAT_KEYS = [
  '--background-primary',
  '--background-secondary',
  '--background-tertiary',
  '--border-muted',
  '--border-primary',
  '--border-color',
  '--border-hover',
  '--bg-elevated',
  '--bg-hover',
  '--bg-primary',
  '--bg-secondary',
  '--bg-tertiary',
  '--color-background-secondary',
  '--color-background-tertiary',
  '--color-bg-base',
  '--color-bg-elevated-hover',
  '--color-bg-flowchat',
  '--color-bg-hover',
  '--color-bg-subtle',
  '--color-bg-surface',
  '--color-border',
  '--color-border-primary',
  '--color-border-subtle',
  '--color-hover',
  '--color-semantic-error',
  '--color-success-100',
  '--color-success-500',
  '--color-surface-elevated',
  '--color-surface-hover',
  '--color-text-tertiary',
  '--color-warning-100',
  '--color-warning-500',
  '--color-warning-700',
  '--color-overlay-white-02',
  '--color-overlay-white-03',
  '--color-overlay-white-05',
  '--color-overlay-black-06',
  '--color-overlay-black-25',
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
  '--element-bg',
  '--font-mono',
  '--font-sans',
  '--markdown-font-mono',
  '--motion-normal',
  '--radius-2xl',
  '--radius-base',
  '--radius-full',
  '--radius-lg',
  '--radius-md',
  '--radius-sm',
  '--radius-xl',
  '--secondary-bg',
  '--spacing-1',
  '--spacing-10',
  '--spacing-12',
  '--spacing-16',
  '--spacing-2',
  '--spacing-3',
  '--spacing-4',
  '--spacing-5',
  '--spacing-6',
  '--spacing-8',
  '--text-disabled',
  '--text-muted',
  '--text-primary',
  '--text-secondary',
  '--text-tertiary',
  '--tool-card-bg-primary',
  '--tool-card-bg-secondary',
  '--tool-card-bg-hover',
  '--tool-card-bg-elevated',
  '--tool-card-border',
  '--tool-card-border-subtle',
  '--tool-card-text-primary',
  '--tool-card-text-secondary',
  '--tool-card-text-muted',
  '--tool-compact-summary-font',
] as const;
const RETIRED_WIDGET_THEME_INTERNAL_KEYS = [
  '--btn-ghost-bg',
  '--btn-ghost-border',
  '--btn-ghost-shadow',
  '--btn-ghost-hover-shadow',
  '--btn-ghost-hover-transform',
  '--btn-ghost-active-bg',
  '--btn-ghost-active-color',
  '--btn-ghost-active-border',
  '--btn-ghost-active-shadow',
  '--btn-ghost-active-transform',
] as const;
const STATIC_WIDGET_SHELL_THEME_VARS = new Set([
  '--font-family-mono',
  '--font-family-sans',
]);

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

function readCompatibilityAliasEntries(css: string): Array<[string, string]> {
  return Array.from(css.matchAll(/^\s+(--[-\w]+): var\((--[-\w]+)\);$/gm))
    .map(([, name, canonical]) => [name, canonical]);
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
      count: 153,
      hash: WIDGET_THEME_VAR_NAMES_HASH,
      first: '--color-bg-primary',
      last: '--tool-card-action-font-weight',
    });
  });

  it('includes every static iframe fallback key in the host payload allowlist', () => {
    const { payload } = readPayloadWithHostValues();

    expect(payload?.vars).toEqual(WIDGET_THEME_FALLBACK_VARS);
  });

  it('does not export retired low-risk compatibility keys', () => {
    const { requestedNames } = readPayloadWithHostValues();

    expect(requestedNames).not.toEqual(expect.arrayContaining(RETIRED_WIDGET_THEME_COMPAT_KEYS));
    expect(requestedNames).not.toEqual(expect.arrayContaining(RETIRED_WIDGET_THEME_INTERNAL_KEYS));
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

  it('keeps retired payload keys available as iframe aliases', () => {
    const { requestedNames } = readPayloadWithHostValues();
    const compatibilityAliasCss = createWidgetThemeCompatibilityAliasCss();
    const aliasEntries = readCompatibilityAliasEntries(compatibilityAliasCss);

    expect(aliasEntries.map(([name]) => name).sort()).toEqual([...RETIRED_WIDGET_THEME_COMPAT_KEYS].sort());
    for (const [key, canonical] of aliasEntries) {
      expect(RETIRED_WIDGET_THEME_COMPAT_KEYS).toContain(key);
      expect(requestedNames).toContain(canonical);
      expect(
        canonical in WIDGET_THEME_FALLBACK_VARS || STATIC_WIDGET_SHELL_THEME_VARS.has(canonical),
      ).toBe(true);
    }
  });

  it('renders fallback CSS from the same reviewed fallback map', () => {
    const css = createWidgetThemeFallbackCss();

    for (const [name, value] of Object.entries(WIDGET_THEME_FALLBACK_VARS)) {
      expect(css).toContain(`      ${name}: ${value};`);
    }
  });
});
