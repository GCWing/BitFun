import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  WIDGET_THEME_STATIC_SHELL_VAR_NAMES,
  WIDGET_THEME_FALLBACK_VARS,
  createWidgetThemeFallbackCss,
  createWidgetThemeStaticShellCss,
  readWidgetThemePayload,
} from './themePayload';
import { createWidgetThemeCompatibilityAliasCss } from './themePayloadCompatibility';

const WIDGET_THEME_VAR_NAMES_HASH = '3c69b6767d5c5aa2752af7eab4c68596f1ab6014397dac02fa2b70343c771981';
const WIDGET_THEME_STATIC_SHELL_VAR_NAMES_HASH = '7c53d90b3bc30c9d1c8494bb3865f42108e10e747841bae493ad26bcb30bf9b8';
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
  '--color-overlay-white-06',
  '--color-overlay-white-10',
  '--color-overlay-black-06',
  '--color-overlay-black-10',
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
const DERIVED_WIDGET_THEME_PAYLOAD_KEYS = [
  '--color-accent-50',
  '--color-accent-100',
  '--color-accent-200',
  '--color-accent-300',
  '--color-accent-400',
  '--color-accent-700',
  '--color-accent-800',
  '--color-success-border',
  '--color-warning-border',
  '--color-error-border',
  '--border-strong',
  '--border-prominent',
  '--element-bg-strong',
  '--size-radius-md',
] as const;
const STATIC_WIDGET_SHELL_THEME_VARS = new Set([
  ...WIDGET_THEME_STATIC_SHELL_VAR_NAMES,
  '--font-family-mono',
  '--font-family-sans',
]);
const LOCAL_ONLY_WIDGET_SHELL_KEYS = [
  '--color-bg-workbench',
  '--color-overlay-white-08',
  '--color-overlay-black-12',
  '--glass-base',
  '--tool-card-header-pad-y',
  '--tool-card-action-line-height',
] as const;
const IFRAME_ROOT_BASE_KEYS = [
  '--font-family-sans',
  '--font-family-mono',
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
      count: 80,
      hash: WIDGET_THEME_VAR_NAMES_HASH,
      first: '--color-bg-primary',
      last: '--btn-ghost-hover-border',
    });
  });

  it('uses reviewed iframe fallback values for requested host payload keys', () => {
    const { payload, requestedNames } = readPayloadWithHostValues();
    const requestedFallbackVars = Object.fromEntries(
      Object.entries(WIDGET_THEME_FALLBACK_VARS).filter(([name]) => requestedNames.includes(name)),
    );

    expect(payload?.vars).toEqual(requestedFallbackVars);
  });

  it('does not export retired low-risk compatibility keys', () => {
    const { requestedNames } = readPayloadWithHostValues();

    expect(requestedNames).not.toEqual(expect.arrayContaining(RETIRED_WIDGET_THEME_COMPAT_KEYS));
    expect(requestedNames).not.toEqual(expect.arrayContaining(RETIRED_WIDGET_THEME_INTERNAL_KEYS));
    expect(requestedNames).not.toEqual(expect.arrayContaining(DERIVED_WIDGET_THEME_PAYLOAD_KEYS));
    expect(requestedNames).toEqual(
      expect.arrayContaining([
        '--color-accent-500',
        '--color-accent-500-rgb',
        '--color-accent-600',
        '--color-error',
        '--color-error-bg',
        '--color-info',
        '--color-info-bg',
        '--color-info-border',
        '--size-radius-xl',
        '--size-gap-16',
        '--btn-primary-bg',
        '--btn-primary-hover-bg',
        '--btn-primary-hover-transform',
        '--btn-ghost-hover-bg',
      ])
    );
  });

  it('exports button component tokens from the host theme payload', () => {
    const hostValues = {
      '--btn-primary-bg': 'linear-gradient(test-primary)',
      '--btn-primary-color': '#101010',
      '--btn-primary-border': '1px solid #202020',
      '--btn-primary-shadow': '0 1px 2px #303030',
      '--btn-primary-hover-bg': 'linear-gradient(test-hover)',
      '--btn-primary-hover-color': '#404040',
      '--btn-primary-hover-border': '1px solid #505050',
      '--btn-primary-hover-shadow': '0 2px 4px #606060',
      '--btn-primary-hover-transform': 'translateY(-1px)',
      '--btn-primary-active-bg': 'linear-gradient(test-active)',
      '--btn-primary-active-color': '#707070',
      '--btn-primary-active-border': '1px solid #808080',
      '--btn-primary-active-shadow': 'none',
      '--btn-primary-active-transform': 'none',
      '--btn-ghost-color': '#909090',
      '--btn-ghost-hover-bg': 'rgba(144, 144, 144, 0.1)',
      '--btn-ghost-hover-color': '#a0a0a0',
      '--btn-ghost-hover-border': '1px solid #b0b0b0',
    };
    const { payload } = readPayloadWithHostValues(hostValues);

    expect(payload?.vars).toMatchObject(hostValues);
  });

  it('keeps retired payload keys available as iframe aliases', () => {
    const { requestedNames } = readPayloadWithHostValues();
    const compatibilityAliasCss = createWidgetThemeCompatibilityAliasCss();
    const aliasEntries = readCompatibilityAliasEntries(compatibilityAliasCss);
    const aliasNames = new Set(aliasEntries.map(([name]) => name));

    expect(aliasEntries.map(([name]) => name).sort()).toEqual([...RETIRED_WIDGET_THEME_COMPAT_KEYS].sort());
    for (const [key, canonical] of aliasEntries) {
      expect(RETIRED_WIDGET_THEME_COMPAT_KEYS).toContain(key);
      expect(aliasNames.has(canonical)).toBe(false);
      expect(
        requestedNames.includes(canonical)
        || canonical in WIDGET_THEME_FALLBACK_VARS
        || STATIC_WIDGET_SHELL_THEME_VARS.has(canonical),
      ).toBe(true);
    }
  });

  it('keeps derived widget keys resolvable without host payload reads', () => {
    const { requestedNames } = readPayloadWithHostValues();

    for (const name of DERIVED_WIDGET_THEME_PAYLOAD_KEYS) {
      expect(requestedNames).not.toContain(name);
      expect(name in WIDGET_THEME_FALLBACK_VARS || STATIC_WIDGET_SHELL_THEME_VARS.has(name)).toBe(true);
    }
  });

  it('renders fallback CSS from the same reviewed fallback map', () => {
    const css = createWidgetThemeFallbackCss();

    for (const [name, value] of Object.entries(WIDGET_THEME_FALLBACK_VARS)) {
      expect(css).toContain(`      ${name}: ${value};`);
    }
  });

  it('keeps host-internal shell vars available without exporting them from the host payload', () => {
    const { requestedNames } = readPayloadWithHostValues();
    const css = createWidgetThemeStaticShellCss();
    const resolvableVars = new Set([
      ...Object.keys(WIDGET_THEME_FALLBACK_VARS),
      ...requestedNames,
      ...WIDGET_THEME_STATIC_SHELL_VAR_NAMES,
      ...IFRAME_ROOT_BASE_KEYS,
    ]);

    for (const name of WIDGET_THEME_STATIC_SHELL_VAR_NAMES) {
      expect(css).toContain(`      ${name}: `);
    }
    expect({
      count: WIDGET_THEME_STATIC_SHELL_VAR_NAMES.length,
      hash: hashNames(WIDGET_THEME_STATIC_SHELL_VAR_NAMES),
      first: WIDGET_THEME_STATIC_SHELL_VAR_NAMES[0],
      last: WIDGET_THEME_STATIC_SHELL_VAR_NAMES[WIDGET_THEME_STATIC_SHELL_VAR_NAMES.length - 1],
    }).toEqual({
      count: 83,
      hash: WIDGET_THEME_STATIC_SHELL_VAR_NAMES_HASH,
      first: '--color-bg-quaternary',
      last: '--tool-card-action-font-weight',
    });
    const referencedVars = [...css.matchAll(/var\((--[a-zA-Z0-9_-]+)/g)].map((match) => match[1]);
    expect(referencedVars.filter((name) => !resolvableVars.has(name))).toEqual([]);
    expect(requestedNames).not.toEqual(expect.arrayContaining(LOCAL_ONLY_WIDGET_SHELL_KEYS));
  });
});
