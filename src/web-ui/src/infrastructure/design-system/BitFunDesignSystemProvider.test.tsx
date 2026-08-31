// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BitFunDesignSystemProvider } from './BitFunDesignSystemProvider';

const appearanceState = vi.hoisted(() => ({ mode: 'dark' as 'dark' | 'light' }));

vi.mock('@/infrastructure/appearance', () => ({
  useAppearance: () => ({ current: { mode: appearanceState.mode } }),
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    currentLanguage: 'en',
    t: (key: string) => key,
  }),
}));

describe('BitFunDesignSystemProvider', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    appearanceState.mode = 'dark';
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('maps product appearance and desktop density onto the global package token root', () => {
    act(() => {
      root.render(
        <BitFunDesignSystemProvider>
          <span>content</span>
        </BitFunDesignSystemProvider>,
      );
    });

    expect(document.documentElement.getAttribute('data-bf-design-system-root')).toBe('');
    expect(document.documentElement.getAttribute('data-color-scheme')).toBe('dark');
    expect(document.documentElement.getAttribute('data-contrast')).toBe('standard');
    expect(document.documentElement.getAttribute('data-density')).toBe('compact');

    appearanceState.mode = 'light';
    act(() => {
      root.render(
        <BitFunDesignSystemProvider>
          <span>content</span>
        </BitFunDesignSystemProvider>,
      );
    });

    expect(document.documentElement.getAttribute('data-color-scheme')).toBe('light');
  });

  it('tracks the operating system high-contrast preference', () => {
    let matches = true;
    let changeListener: (() => void) | undefined;
    vi.stubGlobal('matchMedia', vi.fn(() => ({
      get matches() {
        return matches;
      },
      addEventListener: vi.fn((type: string, listener: () => void) => {
        if (type === 'change') changeListener = listener;
      }),
      removeEventListener: vi.fn(),
    })));

    act(() => {
      root.render(
        <BitFunDesignSystemProvider>
          <span>content</span>
        </BitFunDesignSystemProvider>,
      );
    });
    expect(document.documentElement.getAttribute('data-contrast')).toBe('high');

    matches = false;
    act(() => changeListener?.());
    expect(document.documentElement.getAttribute('data-contrast')).toBe('standard');
  });
});
