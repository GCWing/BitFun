import { createHash } from 'node:crypto';
import {
  cssVariables as systemCssVariables,
  tokens as systemTokens,
} from '@bitfun/design-tokens';
import { themeCssVariables } from '@bitfun/theme-bitfun';
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
const CANONICAL_THEME_VARIABLE_NAMES_HASH = '20c775e19798c1fc3add12c0b7c2b61f457bc35fb2bc3d885068e2ecfb011e0a';
const RETIRED_WIDGET_VARIABLE_NAMES = [
  '--background-primary',
  '--bg-primary',
  '--text-primary',
  '--accent-primary',
  '--bf-appearance-token-color-bg-primary',
  '--bf-appearance-token-color-text-primary',
  '--bf-appearance-token-color-accent-500',
  '--bf-appearance-token-btn-primary-bg',
  '--bf-color-accent-default-rgb',
  '--bf-color-status-success-content-bg',
  '--bf-color-status-success-content-border',
  '--bf-color-status-warning-content-bg',
  '--bf-color-status-warning-content-border',
  '--bf-color-status-danger-content-bg',
  '--bf-color-status-danger-content-border',
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
      count: 111,
      hash: CANONICAL_THEME_VARIABLE_NAMES_HASH,
      first: '--bf-color-accent-border',
      last: '--bf-shadow-xs',
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
    expect(WIDGET_APPEARANCE_FALLBACK_VARS['--bf-color-surface-canvas']).toBe('transparent');
  });

  it('exports canonical state and status semantics without legacy aliases', () => {
    expect(WIDGET_APPEARANCE_VAR_NAMES).toEqual(expect.arrayContaining([
      '--bf-color-action-primary-background',
      '--bf-color-action-primary-hover',
      '--bf-color-action-primary-pressed',
      '--bf-color-status-success-content',
      '--bf-color-status-success-surface',
      '--bf-color-status-success-border',
      '--bf-color-status-warning-surface',
      '--bf-color-status-danger-surface',
      '--bf-color-status-info-surface',
    ]));
    expect(WIDGET_APPEARANCE_VAR_NAMES).not.toEqual(
      expect.arrayContaining(RETIRED_WIDGET_VARIABLE_NAMES),
    );
    expect(WIDGET_APPEARANCE_VAR_NAMES.some(name => name.startsWith('--bf-appearance-token-')))
      .toBe(false);
  });

  it('passes canonical host overrides through unchanged', () => {
    const hostValues = {
      '--bf-color-action-primary-background': 'linear-gradient(test-primary)',
      '--bf-color-action-primary-content': '#101010',
      '--bf-color-action-primary-hover': 'linear-gradient(test-hover)',
      '--bf-color-action-primary-pressed': '#202020',
      '--bf-color-status-danger-surface': 'rgba(200, 0, 0, 0.12)',
      '--bf-color-status-danger-border': '#303030',
      '--bf-shadow-raised': '0 1px 2px #404040',
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
