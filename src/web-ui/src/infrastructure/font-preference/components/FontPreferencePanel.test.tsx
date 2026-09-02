// @vitest-environment jsdom

import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FontSizeLevel } from '../types';
import { FontPreferencePanel } from './FontPreferencePanel';

const fontPreferenceState = vi.hoisted(() => ({
  level: 'default' as FontSizeLevel,
  customPx: undefined as number | undefined,
  setUiSize: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('../hooks/useFontPreference', () => ({
  useFontPreference: () => ({
    preference: {
      uiSize: {
        level: fontPreferenceState.level,
        customPx: fontPreferenceState.customPx,
      },
    },
    setUiSize: fontPreferenceState.setUiSize,
  }),
}));

describe('FontPreferencePanel', () => {
  beforeEach(() => {
    fontPreferenceState.level = 'default';
    fontPreferenceState.customPx = undefined;
    fontPreferenceState.setUiSize.mockReset();
  });

  it('presents equal-weight size presets and an editable preview', () => {
    document.body.innerHTML = renderToStaticMarkup(<FontPreferencePanel />);

    const levelGroup = document.querySelector('[data-testid="appearance-ui-font-level-group"]');
    const segmentedControl = levelGroup?.querySelector('[data-bf-component="segmented-control"]');
    const segments = Array.from(levelGroup?.querySelectorAll('[role="radio"]') ?? []);
    const previewInput = document.querySelector<HTMLInputElement>(
      '[data-testid="appearance-ui-font-preview-input"]',
    );

    expect(segmentedControl?.getAttribute('data-variant')).toBe('pills');
    expect(segmentedControl?.getAttribute('data-size')).toBe('md');
    expect(segmentedControl?.getAttribute('data-tone')).toBe('neutral');
    expect(segments.map(segment => segment.textContent)).toEqual([
      'appearance.fontSize.levels.compact',
      'appearance.fontSize.levels.small',
      'appearance.fontSize.levels.default',
      'appearance.fontSize.levels.medium',
      'appearance.fontSize.levels.large',
      'appearance.fontSize.levels.custom',
    ]);
    expect(
      segments.every(
        segment => segment.querySelector('[data-bf-part="label"]')?.getAttribute('style') === null,
      ),
    ).toBe(true);
    expect(previewInput?.placeholder).toBe('appearance.fontSize.previewPlaceholder');
    expect(previewInput?.style.fontSize).toBe('14px');
    expect(previewInput?.closest('[data-bf-component="input"]')?.getAttribute('data-size')).toBe('md');
    expect(previewInput?.closest('[data-bf-component="input"]')?.getAttribute('data-field-surface')).toBe('default');
    expect(document.querySelector('[data-testid="appearance-font-reset-btn"]')).toBeNull();
  });

  it('reveals the custom stepper and previews its persisted size', () => {
    fontPreferenceState.level = 'custom';
    fontPreferenceState.customPx = 18;

    document.body.innerHTML = renderToStaticMarkup(<FontPreferencePanel />);

    const customControls = document.querySelector('[data-testid="appearance-ui-font-custom-controls"]');
    const numberInput = customControls?.querySelector<HTMLInputElement>('input');
    const previewInput = document.querySelector<HTMLInputElement>(
      '[data-testid="appearance-ui-font-preview-input"]',
    );

    expect(
      customControls
        ?.querySelector('[data-bf-component="number-input"]')
        ?.getAttribute('data-size'),
    ).toBe('md');
    expect(numberInput?.value).toBe('18');
    expect(previewInput?.style.fontSize).toBe('18px');
  });
});
