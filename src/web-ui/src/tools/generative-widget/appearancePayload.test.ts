import { createHash } from 'node:crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES,
  WIDGET_APPEARANCE_VAR_NAMES,
  WIDGET_APPEARANCE_FALLBACK_VARS,
  createWidgetAppearanceFallbackCss,
  createWidgetAppearanceStaticShellCss,
  readWidgetAppearancePayload,
} from './appearancePayload';
import { widgetAppearanceAdapter } from '@/infrastructure/appearance/adapters/WidgetAppearanceAdapter';

const WIDGET_APPEARANCE_VAR_NAMES_HASH = '19b0247c7dd289fa9cd550c072447967af335aa1947fbd6410616c4e0809fbd0';
const WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES_HASH = '7fc7b32bf52a4c03333000ff3dadedf2af6f4ee75db788d4111aec50cab752f2';
const RETIRED_WIDGET_APPEARANCE_COMPAT_KEYS = [
  '--background-primary',
  '--background-secondary',
  '--background-tertiary',
  '--bf-appearance-token-border-muted',
  '--bf-appearance-token-border-primary',
  '--bf-appearance-token-border-color',
  '--bf-appearance-token-border-hover',
  '--bg-elevated',
  '--bg-hover',
  '--bg-primary',
  '--bg-secondary',
  '--bg-tertiary',
  '--bf-appearance-token-color-background-secondary',
  '--bf-appearance-token-color-background-tertiary',
  '--bf-appearance-token-color-bg-base',
  '--bf-appearance-token-color-bg-elevated-hover',
  '--bf-appearance-token-color-bg-flowchat',
  '--bf-appearance-token-color-bg-hover',
  '--bf-appearance-token-color-bg-subtle',
  '--bf-appearance-token-color-bg-surface',
  '--bf-appearance-token-color-border',
  '--bf-appearance-token-color-border-primary',
  '--bf-appearance-token-color-border-subtle',
  '--bf-appearance-token-color-hover',
  '--bf-appearance-token-color-semantic-error',
  '--bf-appearance-token-color-success-100',
  '--bf-appearance-token-color-success-500',
  '--bf-appearance-token-color-surface-elevated',
  '--bf-appearance-token-color-surface-hover',
  '--bf-appearance-token-color-text-tertiary',
  '--bf-appearance-token-color-warning-100',
  '--bf-appearance-token-color-warning-500',
  '--bf-appearance-token-color-warning-700',
  '--bf-appearance-token-color-overlay-white-02',
  '--bf-appearance-token-color-overlay-white-03',
  '--bf-appearance-token-color-overlay-white-05',
  '--bf-appearance-token-color-overlay-white-06',
  '--bf-appearance-token-color-overlay-white-10',
  '--bf-appearance-token-color-overlay-black-06',
  '--bf-appearance-token-color-overlay-black-10',
  '--bf-appearance-token-color-overlay-black-25',
  '--bf-appearance-token-color-accent',
  '--bf-appearance-token-color-accent-primary',
  '--bf-appearance-token-color-accent-alpha',
  '--bf-appearance-token-color-primary',
  '--bf-appearance-token-color-primary-rgb',
  '--bf-appearance-token-color-primary-400',
  '--bf-appearance-token-color-primary-hover',
  '--bf-appearance-token-color-primary-500',
  '--bf-appearance-token-color-primary-alpha',
  '--bf-appearance-token-color-primary-bg',
  '--bf-appearance-token-color-primary-bg-subtle',
  '--accent-primary',
  '--accent-primary-hover',
  '--bf-appearance-token-color-danger',
  '--bf-appearance-token-color-danger-500',
  '--bf-appearance-token-color-danger-text',
  '--bf-appearance-token-color-danger-bg',
  '--bf-appearance-token-color-danger-border',
  '--bf-appearance-token-color-danger-hover',
  '--bf-appearance-token-element-bg',
  '--bf-appearance-token-element-bg-elevated',
  '--bf-appearance-token-font-mono',
  '--bf-appearance-token-font-sans',
  '--markdown-font-mono',
  '--bf-appearance-token-motion-normal',
  '--smooth-height-collapse-duration',
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
  '--bf-appearance-token-tool-card-bg-primary',
  '--bf-appearance-token-tool-card-bg-secondary',
  '--bf-appearance-token-tool-card-bg-hover',
  '--bf-appearance-token-tool-card-bg-elevated',
  '--bf-appearance-token-tool-card-border',
  '--bf-appearance-token-tool-card-border-subtle',
  '--bf-appearance-token-tool-card-text-primary',
  '--bf-appearance-token-tool-card-text-secondary',
  '--bf-appearance-token-tool-card-text-muted',
  '--tool-compact-summary-font',
] as const;
const RETIRED_WIDGET_APPEARANCE_INTERNAL_KEYS = [
  '--bf-appearance-token-btn-ghost-bg',
  '--bf-appearance-token-btn-ghost-border',
  '--bf-appearance-token-btn-ghost-shadow',
  '--bf-appearance-token-btn-ghost-hover-shadow',
  '--bf-appearance-token-btn-ghost-hover-transform',
  '--bf-appearance-token-btn-ghost-active-bg',
  '--bf-appearance-token-btn-ghost-active-color',
  '--bf-appearance-token-btn-ghost-active-border',
  '--bf-appearance-token-btn-ghost-active-shadow',
  '--bf-appearance-token-btn-ghost-active-transform',
] as const;
const DERIVED_WIDGET_APPEARANCE_PAYLOAD_KEYS = [
  '--bf-appearance-token-color-accent-50',
  '--bf-appearance-token-color-accent-100',
  '--bf-appearance-token-color-accent-200',
  '--bf-appearance-token-color-accent-300',
  '--bf-appearance-token-color-accent-400',
  '--bf-appearance-token-color-accent-700',
  '--bf-appearance-token-color-accent-800',
  '--bf-appearance-token-color-success-border',
  '--bf-appearance-token-color-warning-border',
  '--bf-appearance-token-color-error-border',
  '--bf-appearance-token-border-strong',
  '--bf-appearance-token-border-prominent',
  '--bf-appearance-token-element-bg-strong',
  '--bf-appearance-token-size-radius-sm',
  '--bf-appearance-token-size-radius-base',
  '--bf-appearance-token-size-radius-md',
  '--bf-appearance-token-size-radius-lg',
  '--bf-appearance-token-size-radius-xl',
  '--bf-appearance-token-size-radius-2xl',
  '--bf-appearance-token-size-radius-full',
  '--bf-appearance-token-size-gap-1',
  '--bf-appearance-token-size-gap-2',
  '--bf-appearance-token-size-gap-3',
  '--bf-appearance-token-size-gap-4',
  '--bf-appearance-token-size-gap-5',
  '--bf-appearance-token-size-gap-6',
  '--bf-appearance-token-size-gap-8',
  '--bf-appearance-token-size-gap-10',
  '--bf-appearance-token-size-gap-12',
  '--bf-appearance-token-size-gap-16',
  '--bf-appearance-token-font-size-xs',
  '--bf-appearance-token-font-size-sm',
  '--bf-appearance-token-font-size-base',
  '--bf-appearance-token-font-size-lg',
  '--bf-appearance-token-font-size-2xl',
  '--bf-appearance-token-font-weight-medium',
  '--bf-appearance-token-font-weight-semibold',
] as const;
const STATIC_WIDGET_SHELL_APPEARANCE_VARS = new Set([
  ...WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES,
  '--bf-appearance-token-font-family-mono',
  '--bf-appearance-token-font-family-sans',
]);
const LOCAL_ONLY_WIDGET_SHELL_KEYS = [
  '--bf-appearance-token-color-bg-workbench',
  '--bf-appearance-token-color-overlay-white-08',
  '--bf-appearance-token-color-overlay-black-12',
  '--bf-appearance-token-tool-card-header-pad-y',
  '--bf-appearance-token-tool-card-action-line-height',
] as const;
const IFRAME_ROOT_BASE_KEYS = [
  '--bf-appearance-token-font-family-sans',
  '--bf-appearance-token-font-family-mono',
  '--bf-appearance-token-font-size-xs',
  '--bf-appearance-token-font-size-sm',
  '--bf-appearance-token-font-size-base',
  '--bf-appearance-token-font-size-lg',
  '--bf-appearance-token-font-size-2xl',
  '--bf-appearance-token-font-weight-medium',
  '--bf-appearance-token-font-weight-semibold',
] as const;
const IFRAME_ROOT_BASE_APPEARANCE_VARS = new Set<string>(IFRAME_ROOT_BASE_KEYS);

function readPayloadWithHostValues(hostValues: Record<string, string> = {}) {
  widgetAppearanceAdapter.apply({
    id: 'test-appearance',
    mode: 'dark',
    vars: hostValues,
  }, undefined, { revision: 1, mode: 'dark', assets: {} });

  return {
    payload: readWidgetAppearancePayload(),
    requestedNames: [...WIDGET_APPEARANCE_VAR_NAMES],
  };
}

function hashNames(names: string[]): string {
  return createHash('sha256')
    .update(names.join('\n'))
    .digest('hex');
}

describe('generated widget appearance payload contract', () => {
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
      count: 56,
      hash: WIDGET_APPEARANCE_VAR_NAMES_HASH,
      first: '--bf-appearance-token-color-bg-primary',
      last: '--bf-appearance-token-btn-ghost-hover-border',
    });
  });

  it('uses reviewed iframe fallback values for requested host payload keys', () => {
    const { payload, requestedNames } = readPayloadWithHostValues();
    const requestedFallbackVars = Object.fromEntries(
      Object.entries(WIDGET_APPEARANCE_FALLBACK_VARS).filter(([name]) => requestedNames.includes(name)),
    );

    expect(payload?.vars).toEqual(requestedFallbackVars);
  });

  it('does not export retired low-risk compatibility keys', () => {
    const { requestedNames } = readPayloadWithHostValues();

    expect(requestedNames).not.toEqual(expect.arrayContaining(RETIRED_WIDGET_APPEARANCE_COMPAT_KEYS));
    expect(requestedNames).not.toEqual(expect.arrayContaining(RETIRED_WIDGET_APPEARANCE_INTERNAL_KEYS));
    expect(requestedNames).not.toEqual(expect.arrayContaining(DERIVED_WIDGET_APPEARANCE_PAYLOAD_KEYS));
    expect(requestedNames).toEqual(
      expect.arrayContaining([
        '--bf-appearance-token-color-accent-500',
        '--bf-appearance-token-color-accent-500-rgb',
        '--bf-appearance-token-color-accent-600',
        '--bf-appearance-token-color-error',
        '--bf-appearance-token-color-error-bg',
        '--bf-appearance-token-color-info',
        '--bf-appearance-token-color-info-bg',
        '--bf-appearance-token-color-info-border',
        '--bf-appearance-token-btn-primary-bg',
        '--bf-appearance-token-btn-primary-hover-bg',
        '--bf-appearance-token-btn-primary-hover-transform',
        '--bf-appearance-token-btn-ghost-hover-bg',
      ])
    );
  });

  it('exports button component tokens from the host Appearance payload', () => {
    const hostValues = {
      '--bf-appearance-token-btn-primary-bg': 'linear-gradient(test-primary)',
      '--bf-appearance-token-btn-primary-color': '#101010',
      '--bf-appearance-token-btn-primary-border': '1px solid #202020',
      '--bf-appearance-token-btn-primary-shadow': '0 1px 2px #303030',
      '--bf-appearance-token-btn-primary-hover-bg': 'linear-gradient(test-hover)',
      '--bf-appearance-token-btn-primary-hover-color': '#404040',
      '--bf-appearance-token-btn-primary-hover-border': '1px solid #505050',
      '--bf-appearance-token-btn-primary-hover-shadow': '0 2px 4px #606060',
      '--bf-appearance-token-btn-primary-hover-transform': 'translateY(-1px)',
      '--bf-appearance-token-btn-primary-active-bg': 'linear-gradient(test-active)',
      '--bf-appearance-token-btn-primary-active-color': '#707070',
      '--bf-appearance-token-btn-primary-active-border': '1px solid #808080',
      '--bf-appearance-token-btn-primary-active-shadow': 'none',
      '--bf-appearance-token-btn-primary-active-transform': 'none',
      '--bf-appearance-token-btn-ghost-color': '#909090',
      '--bf-appearance-token-btn-ghost-hover-bg': 'rgba(144, 144, 144, 0.1)',
      '--bf-appearance-token-btn-ghost-hover-color': '#a0a0a0',
      '--bf-appearance-token-btn-ghost-hover-border': '1px solid #b0b0b0',
    };
    const { payload } = readPayloadWithHostValues(hostValues);

    expect(payload?.vars).toMatchObject(hostValues);
  });

  it('keeps derived widget keys resolvable without host payload reads', () => {
    const { requestedNames } = readPayloadWithHostValues();

    for (const name of DERIVED_WIDGET_APPEARANCE_PAYLOAD_KEYS) {
      expect(requestedNames).not.toContain(name);
      expect(
        name in WIDGET_APPEARANCE_FALLBACK_VARS
        || STATIC_WIDGET_SHELL_APPEARANCE_VARS.has(name)
        || IFRAME_ROOT_BASE_APPEARANCE_VARS.has(name),
      ).toBe(true);
    }
  });

  it('renders fallback CSS from the same reviewed fallback map', () => {
    const css = createWidgetAppearanceFallbackCss();

    for (const [name, value] of Object.entries(WIDGET_APPEARANCE_FALLBACK_VARS)) {
      expect(css).toContain(`      ${name}: ${value};`);
    }
  });

  it('keeps host-internal shell vars available without exporting them from the host payload', () => {
    const { requestedNames } = readPayloadWithHostValues();
    const css = createWidgetAppearanceStaticShellCss();
    const resolvableVars = new Set([
      ...Object.keys(WIDGET_APPEARANCE_FALLBACK_VARS),
      ...requestedNames,
      ...WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES,
      ...IFRAME_ROOT_BASE_KEYS,
    ]);

    for (const name of WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES) {
      expect(css).toContain(`      ${name}: `);
    }
    expect({
      count: WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES.length,
      hash: hashNames(WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES),
      first: WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES[0],
      last: WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES[WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES.length - 1],
    }).toEqual({
      count: 68,
      hash: WIDGET_APPEARANCE_STATIC_SHELL_VAR_NAMES_HASH,
      first: '--bf-appearance-token-color-bg-quaternary',
      last: '--bf-appearance-token-tool-card-action-line-height',
    });
    const referencedVars = [...css.matchAll(/var\((--[a-zA-Z0-9_-]+)/g)].map((match) => match[1]);
    expect(referencedVars.filter((name) => !resolvableVars.has(name))).toEqual([]);
    expect(requestedNames).not.toEqual(expect.arrayContaining(LOCAL_ONLY_WIDGET_SHELL_KEYS));
  });
});
