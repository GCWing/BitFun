// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
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

  it('presents standard size selection and an editable preview', () => {
    document.body.innerHTML = renderToStaticMarkup(<FontPreferencePanel />);

    const levelGroup = document.querySelector('[data-testid="appearance-ui-font-level-group"]');
    const select = levelGroup as HTMLSelectElement | null;
    const options = Array.from(select?.querySelectorAll('option') ?? []);
    const previewInput = document.querySelector<HTMLInputElement>(
      '[data-testid="appearance-ui-font-preview-input"]',
    );

    expect(select?.value).toBe('default');
    expect(options.map(option => option.textContent)).toEqual([
      'appearance.fontSize.levels.compact',
      'appearance.fontSize.levels.small',
      'appearance.fontSize.levels.default',
      'appearance.fontSize.levels.medium',
      'appearance.fontSize.levels.large',
      'appearance.fontSize.levels.custom',
    ]);
    expect(previewInput?.placeholder).toBe('appearance.fontSize.previewPlaceholder');
    expect(previewInput?.style.fontSize).toBe('14px');
    expect(previewInput?.closest('[data-openbitfun-component="input"]')?.getAttribute('data-size')).toBe('sm');
    expect(previewInput?.closest('[data-openbitfun-component="input"]')?.getAttribute('data-field-surface')).toBe('ambient');
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
        ?.querySelector('[data-openbitfun-component="number-input"]')
        ?.getAttribute('data-size'),
    ).toBe('sm');
    expect(numberInput?.value).toBe('18');
    expect(previewInput?.style.fontSize).toBe('18px');
  });
  it('applies presets and initializes custom sizing from the current preset', async () => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    const container = document.createElement('div');
    const root = createRoot(container);
    try {
      await act(async () => root.render(<FontPreferencePanel />));
      const select = container.querySelector<HTMLSelectElement>('select')!;
      await act(async () => {
        select.value = 'large';
        select.dispatchEvent(new Event('change', { bubbles: true }));
      });
      expect(fontPreferenceState.setUiSize).toHaveBeenCalledWith('large');
      fontPreferenceState.level = 'large';
      await act(async () => root.render(<FontPreferencePanel />));
      await act(async () => {
        select.value = 'custom';
        select.dispatchEvent(new Event('change', { bubbles: true }));
      });
      expect(fontPreferenceState.setUiSize).toHaveBeenLastCalledWith('custom', 16);
    } finally {
      await act(async () => root.unmount());
    }
  });

});
