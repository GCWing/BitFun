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

vi.mock('@bitfun/ui', async (importOriginal) => ({
  ...await importOriginal<typeof import('@bitfun/ui')>(),
  Combobox: ({ triggerTestId }: { triggerTestId?: string }) => (
    <button type="button" data-testid={triggerTestId} />
  ),
  ScrollArea: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  FormSection: ({ children, ...props }: React.HTMLAttributes<HTMLElement>) => <section {...props}>{children}</section>,
  FieldGroup: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
  Select: ({ options, ...props }: any) => (
    <select {...props}>
      {options.map((option: any) => (
        <option key={option.value} value={option.value}>{option.label}</option>
      ))}
    </select>
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
  useI18n: () => ({
    t: (key: string) => key,
  }),
  useLanguageSelector: () => ({
    currentLanguage: 'zh-CN',
    supportedLocales: [{ id: 'zh-CN', nativeName: '简体中文' }],
    selectLanguage: vi.fn(),
    isChanging: false,
  }),
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
    const packageSelect = document.querySelector('[data-testid="appearance-package-select"]');
    const packageManagement = document.querySelector('[data-testid="appearance-package-config"]');

    expect(appearanceSection).not.toBeNull();
    expect(packageSelect?.closest('.bitfun-config-page-section')).toBe(appearanceSection);
    expect(packageSelect?.closest('.appearance-settings__package-row')).not.toBeNull();
    expect(packageManagement?.closest('.bitfun-config-page-section')).toBeNull();
  });
});
