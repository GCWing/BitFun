// @vitest-environment jsdom

import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import AppearanceSettingsPage from './AppearanceSettingsPage';

vi.mock('react-i18next', () => ({
  useTranslation: (namespace: string) => ({
    t: (key: string) => `${namespace}:${key}`,
  }),
}));

vi.mock('@/component-library', () => ({
  ConfigPageLoading: () => null,
  ConfigPageMessage: () => null,
  Select: ({ triggerTestId }: { triggerTestId?: string }) => (
    <button type="button" data-testid={triggerTestId} />
  ),
  Switch: ({
    checked,
    onChange,
    'data-testid': testId,
  }: {
    checked?: boolean;
    onChange?: React.ChangeEventHandler<HTMLInputElement>;
    'data-testid'?: string;
  }) => (
    <input type="checkbox" checked={checked} onChange={onChange} data-testid={testId} />
  ),
}));

vi.mock('@/infrastructure/appearance', () => ({
  SYSTEM_APPEARANCE_ID: 'system',
  getAppearancePackageValidationError: () => null,
  useAppearance: () => ({
    selectedAppearanceId: 'system',
    appearances: [],
    select: vi.fn(),
    activate: vi.fn(),
    initialized: true,
    status: 'ready',
  }),
}));

vi.mock('@/infrastructure/i18n', () => ({
  useLanguageSelector: () => ({
    currentLanguage: 'zh-CN',
    supportedLocales: [{ id: 'zh-CN', nativeName: '简体中文' }],
    selectLanguage: vi.fn(),
    isChanging: false,
  }),
}));

vi.mock('@/infrastructure/mouse-glow', () => ({
  useMouseGlowPreference: () => ({ enabled: true, setEnabled: vi.fn() }),
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: { error: vi.fn() },
}));

vi.mock('@/infrastructure/font-preference', () => ({
  FontPreferencePanel: () => <div data-testid="appearance-font-section" />,
}));

vi.mock('./AppearancePackageConfigSection', () => ({
  AppearancePackageConfigSection: () => <div data-testid="appearance-package-config" />,
  AppearancePackageFailurePanel: () => <div data-testid="appearance-package-failure" />,
}));

describe('AppearanceSettingsPage', () => {
  it('keeps appearance controls in the option panel and package management in its own section', () => {
    document.body.innerHTML = renderToStaticMarkup(<AppearanceSettingsPage />);

    const appearanceSection = document.querySelector(
      '[data-testid="appearance-settings-section"] .bitfun-config-page-section',
    );
    const motionControl = document.querySelector('[data-testid="appearance-mouse-glow-switch"]');
    const packageSelect = document.querySelector('[data-testid="appearance-package-select"]');
    const packageManagement = document.querySelector('[data-testid="appearance-package-config"]');

    expect(appearanceSection).not.toBeNull();
    expect(motionControl?.closest('.bitfun-config-page-section')).toBe(appearanceSection);
    expect(packageSelect?.closest('.bitfun-config-page-section')).toBe(appearanceSection);
    expect(packageSelect?.closest('.appearance-settings__package-row')).not.toBeNull();
    expect(packageManagement?.closest('.bitfun-config-page-section')).toBeNull();
  });
});
