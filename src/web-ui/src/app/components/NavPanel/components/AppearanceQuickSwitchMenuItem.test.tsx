// @vitest-environment jsdom

import React, { act, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const selectAppearance = vi.hoisted(() => vi.fn());
const notifyError = vi.hoisted(() => vi.fn());

vi.mock('@/infrastructure/appearance', () => ({
  SYSTEM_APPEARANCE_ID: 'system',
  useAppearance: () => ({
    appearances: [
      {
        id: 'bitfun-light',
        name: 'BitFun Light',
        version: '1.0.0',
        mode: 'light',
        source: 'builtin',
      },
      {
        id: 'custom-ocean',
        name: 'Ocean',
        version: '1.0.0',
        mode: 'dark',
        source: 'imported',
      },
    ],
    current: { id: 'bitfun-light', name: 'BitFun Light' },
    initialized: true,
    select: selectAppearance,
    selectedAppearanceId: 'bitfun-light',
    status: 'ready',
  }),
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: (namespace: string) => ({
    t: (key: string, options?: { defaultValue?: string }) => {
      const translations: Record<string, string> = namespace === 'settings/application'
        ? {
            'appearance.systemAppearance': 'Match system',
            'appearance.presets.bitfun-light.name': 'Light',
          }
        : {
            'nav.settingsMenu.theme': 'Theme',
            'nav.settingsMenu.themeConfiguration': 'Theme configuration',
            'nav.settingsMenu.appearanceSwitchFailed': 'Could not switch the theme. Try again.',
          };
      return translations[key] ?? options?.defaultValue ?? key;
    },
  }),
}));

vi.mock('@/infrastructure/appearance/runtime/AppearanceOverlayHost', () => ({
  getAppearanceOverlayHost: () => document.body,
}));

vi.mock('@/shared/notification-system', () => ({
  useNotification: () => ({ error: notifyError }),
}));

import AppearanceQuickSwitchMenuItem from './AppearanceQuickSwitchMenuItem';

describe('AppearanceQuickSwitchMenuItem', () => {
  let container: HTMLDivElement;
  let root: Root;
  let closeParentMenu: ReturnType<typeof vi.fn>;
  let openAppearanceSettings: ReturnType<typeof vi.fn>;

  const Harness = () => {
    const [open, setOpen] = useState(false);
    return (
      <AppearanceQuickSwitchMenuItem
        open={open}
        onOpenChange={setOpen}
        onCloseParentMenu={closeParentMenu}
        onOpenAppearanceSettings={openAppearanceSettings}
      />
    );
  };

  beforeEach(() => {
    selectAppearance.mockReset().mockResolvedValue(undefined);
    notifyError.mockReset();
    closeParentMenu = vi.fn();
    openAppearanceSettings = vi.fn();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  });

  it('shows the current theme and opens the checked quick-switch submenu only on click', () => {
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="nav-settings-appearance-item"]',
    );
    expect(trigger).not.toBeNull();
    expect(trigger?.textContent).toContain('Theme');
    expect(trigger?.textContent).toContain('Light');
    expect(trigger?.getAttribute('aria-haspopup')).toBe('menu');

    act(() => {
      trigger!.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    });
    expect(document.querySelector('[data-testid="nav-settings-appearance-menu"]')).toBeNull();

    act(() => trigger!.click());

    const submenu = document.querySelector('[data-testid="nav-settings-appearance-menu"]');
    const selected = document.querySelector(
      '[data-testid="nav-settings-appearance-option"][data-appearance-id="bitfun-light"]',
    );
    expect(submenu).not.toBeNull();
    expect(selected?.getAttribute('role')).toBe('menuitemradio');
    expect(selected?.getAttribute('aria-checked')).toBe('true');
    expect(submenu?.textContent).toContain('Match system');
    expect(submenu?.textContent).toContain('Ocean');

    act(() => trigger!.click());
    expect(document.querySelector('[data-testid="nav-settings-appearance-menu"]')).toBeNull();
  });

  it('applies a selected theme immediately and closes the parent menu', async () => {
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="nav-settings-appearance-item"]',
    );
    act(() => trigger!.click());
    const ocean = document.querySelector<HTMLButtonElement>(
      '[data-testid="nav-settings-appearance-option"][data-appearance-id="custom-ocean"]',
    );

    await act(async () => {
      ocean!.click();
      await Promise.resolve();
    });

    expect(selectAppearance).toHaveBeenCalledWith('custom-ocean');
    expect(closeParentMenu).toHaveBeenCalledOnce();
  });

  it('keeps full theme configuration available from the submenu', () => {
    const trigger = container.querySelector<HTMLButtonElement>(
      '[data-testid="nav-settings-appearance-item"]',
    );
    act(() => trigger!.click());
    const configure = document.querySelector<HTMLButtonElement>(
      '[data-testid="nav-settings-appearance-settings"]',
    );
    act(() => configure!.click());

    expect(openAppearanceSettings).toHaveBeenCalledOnce();
  });
});
