// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LocalizedDateTimeField from './LocalizedDateTimeField';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
  useI18n: () => ({
    currentLanguage: 'en-US',
    formatDate: (date: Date) => date.toISOString(),
    t: (key: string) => key,
  }),
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

describe('LocalizedDateTimeField picker action', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.querySelector('[data-testid="datetime-picker"]')?.remove();
  });

  it('opens the picker from an accessible design-system icon button', () => {
    act(() => {
      root.render(
        <LocalizedDateTimeField
          value="2026-08-27T09:30"
          onChange={vi.fn()}
        />,
      );
    });

    const pickerButton = container.querySelector<HTMLButtonElement>(
      'button[aria-label="dateTimeField.openPicker"]',
    );

    expect(pickerButton?.dataset.bfComponent).toBe('icon-button');
    expect(pickerButton?.getAttribute('aria-expanded')).toBe('false');

    act(() => pickerButton?.click());

    expect(pickerButton?.getAttribute('aria-expanded')).toBe('true');
    expect(document.querySelector('[data-testid="datetime-picker"]')).not.toBeNull();
  });
});
