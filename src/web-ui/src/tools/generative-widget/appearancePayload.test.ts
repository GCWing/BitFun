import { createHash } from 'node:crypto';
import {
  cssVariables as systemCssVariables,
  tokens as systemTokens,
} from '@openbitfun/design-tokens';
import { themeCssVariables } from '@openbitfun/theme-openbitfun';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { widgetAppearanceAdapter } from '@/infrastructure/appearance/adapters/WidgetAppearanceAdapter';
import {
  WIDGET_APPEARANCE_FALLBACK_VARS,
  WIDGET_APPEARANCE_VAR_NAMES,
  WIDGET_TYPOGRAPHY_VARIABLE_NAMES,
  createWidgetAppearanceFallbackCss,
  readWidgetAppearancePayload,
} from './appearancePayload';

const CANONICAL_THEME_VARIABLE_NAMES = Object.values(themeCssVariables);
const CANONICAL_THEME_VARIABLE_NAMES_HASH = '0c1109009e2a88adbade099e551aacecc2d18a4f6dbcf8436f0de80999c90d5d';
const RETIRED_WIDGET_VARIABLE_NAMES = [
  '--background-primary',
  '--bg-primary',
  '--text-primary',
  '--accent-primary',
  '--openbitfun-appearance-token-color-bg-primary',
  '--openbitfun-appearance-token-color-text-primary',
  '--openbitfun-appearance-token-color-accent-500',
  '--openbitfun-appearance-token-btn-primary-bg',
  '--openbitfun-color-accent-default-rgb',
  '--openbitfun-color-status-success-content-bg',
  '--openbitfun-color-status-success-content-border',
  '--openbitfun-color-status-warning-content-bg',
  '--openbitfun-color-status-warning-content-border',
  '--openbitfun-color-status-danger-content-bg',
  '--openbitfun-color-status-danger-content-border',
] as const;

function readPayloadWithHostValues(hostValues: Record<string, string> = {}) {
  widgetAppearanceAdapter.apply({
    id: 'test-appearance',
    mode: 'dark',
    vars: hostValues,
  }, undefined, { revision: 1, mode: 'dark', assets: {} });

  return readWidgetAppearancePayload();
}

function hashNames(names: readonly string[]): string {
  return createHash('sha256')
    .update(names.join('\n'))
    .digest('hex');
}

describe('generated widget appearance payload contract', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('derives its complete host payload allowlist from the canonical theme package', () => {
    expect(WIDGET_APPEARANCE_VAR_NAMES).toEqual(CANONICAL_THEME_VARIABLE_NAMES);
    expect(new Set(WIDGET_APPEARANCE_VAR_NAMES).size).toBe(WIDGET_APPEARANCE_VAR_NAMES.length);
    expect({
      count: WIDGET_APPEARANCE_VAR_NAMES.length,
      hash: hashNames(WIDGET_APPEARANCE_VAR_NAMES),
      first: WIDGET_APPEARANCE_VAR_NAMES[0],
      last: WIDGET_APPEARANCE_VAR_NAMES[WIDGET_APPEARANCE_VAR_NAMES.length - 1],
    }).toEqual({
      count: 124,
      hash: CANONICAL_THEME_VARIABLE_NAMES_HASH,
      first: '--openbitfun-color-accent-border',
      last: '--openbitfun-shadow-xs',
    });
  });

  it('uses the builtin canonical theme as the payload fallback', () => {
    const payload = readPayloadWithHostValues();

    expect(payload?.vars).toMatchObject(WIDGET_APPEARANCE_FALLBACK_VARS);
    expect(Object.keys(WIDGET_APPEARANCE_FALLBACK_VARS)).toEqual(WIDGET_APPEARANCE_VAR_NAMES);
    expect(Object.keys(payload?.vars ?? {}).sort()).toEqual([
      ...WIDGET_APPEARANCE_VAR_NAMES,
      ...WIDGET_TYPOGRAPHY_VARIABLE_NAMES,
    ].sort());
    expect(WIDGET_APPEARANCE_FALLBACK_VARS['--openbitfun-color-surface-canvas']).toBe('transparent');
  });

  it('exports canonical state and status semantics without legacy aliases', () => {
    expect(WIDGET_APPEARANCE_VAR_NAMES).toEqual(expect.arrayContaining([
      '--openbitfun-color-action-primary-background',
      '--openbitfun-color-action-primary-hover',
      '--openbitfun-color-action-primary-pressed',
      '--openbitfun-color-code-change-added',
      '--openbitfun-color-code-change-removed',
      '--openbitfun-color-status-success-content',
      '--openbitfun-color-status-success-surface',
      '--openbitfun-color-status-success-border',
      '--openbitfun-color-status-warning-surface',
      '--openbitfun-color-status-danger-surface',
      '--openbitfun-color-status-info-surface',
    ]));
    expect(WIDGET_APPEARANCE_VAR_NAMES).not.toEqual(
      expect.arrayContaining(RETIRED_WIDGET_VARIABLE_NAMES),
    );
    expect(WIDGET_APPEARANCE_VAR_NAMES.some(name => name.startsWith('--openbitfun-appearance-token-')))
      .toBe(false);
  });

  it('passes canonical host overrides through unchanged', () => {
    const hostValues = {
      '--openbitfun-color-action-primary-background': 'linear-gradient(test-primary)',
      '--openbitfun-color-action-primary-content': '#101010',
      '--openbitfun-color-action-primary-hover': 'linear-gradient(test-hover)',
      '--openbitfun-color-action-primary-pressed': '#202020',
      '--openbitfun-color-status-danger-surface': 'rgba(200, 0, 0, 0.12)',
      '--openbitfun-color-status-danger-border': '#303030',
      '--openbitfun-shadow-raised': '0 1px 2px #404040',
    };

    expect(readPayloadWithHostValues(hostValues)?.vars).toMatchObject(hostValues);
  });

  it('renders one self-contained fallback scope from canonical theme and system tokens', () => {
    const css = createWidgetAppearanceFallbackCss();

    for (const [name, value] of Object.entries(WIDGET_APPEARANCE_FALLBACK_VARS)) {
      expect(css).toContain(`      ${name}: ${value};`);
    }
    for (const [tokenName, value] of Object.entries(systemTokens)) {
      const variableName = systemCssVariables[tokenName as keyof typeof systemCssVariables];
      expect(css).toContain(`      ${variableName}: ${String(value)};`);
    }
    for (const retiredName of RETIRED_WIDGET_VARIABLE_NAMES) {
      expect(css).not.toContain(`${retiredName}:`);
    }
  });
});
