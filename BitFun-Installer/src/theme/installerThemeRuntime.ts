import { useLayoutEffect } from 'react';
import { SYSTEM_THEME_ID, type ThemeId, type ThemePreferenceId } from '../types/installer';
import type { InstallerTheme } from './installerThemesData';
import { findInstallerThemeById } from './installerThemesData';

/** Same rule as main app `getSystemPreferredDefaultThemeId`: dark -> bitfun-dark, else bitfun-light. */
export function getSystemPreferredBuiltinThemeId(): ThemeId {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return 'bitfun-light';
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'bitfun-dark' : 'bitfun-light';
}

export function applyInstallerThemeToDocument(theme: InstallerTheme): void {
  const root = document.documentElement;
  const { colors } = theme;

  root.style.setProperty('--bf-color-surface-canvas', colors.background.primary);
  root.style.setProperty('--bf-color-surface-panel', colors.background.secondary);
  root.style.setProperty('--bf-color-content-primary', colors.text.primary);
  root.style.setProperty('--bf-color-content-secondary', colors.text.secondary);
  root.style.setProperty('--bf-color-content-muted', colors.text.muted);
  root.style.setProperty('--bf-color-surface-subtle', colors.element.subtle);
  root.style.setProperty('--bf-color-action-neutral-surface', colors.element.soft);
  root.style.setProperty('--bf-color-action-neutral-surface-pressed', colors.element.medium);
  root.style.setProperty('--bf-color-border-subtle', colors.border.subtle);
  root.style.setProperty('--bf-color-border-default', colors.border.base);
  root.style.setProperty('--bf-color-status-success-content', colors.semantic.success);
  root.style.setProperty('--bf-color-status-warning-content', colors.semantic.warning);
  root.style.setProperty('--bf-color-status-danger-content', colors.semantic.error);

  root.style.setProperty('--bf-color-accent-default', colors.accent);

  root.setAttribute('data-bf-design-system-root', '');
  root.setAttribute('data-color-scheme', theme.type);
  root.setAttribute('data-contrast', 'standard');
  root.setAttribute('data-density', 'comfortable');
  root.setAttribute('data-installer-theme', theme.id);
  root.style.colorScheme = theme.type;
  document.querySelector('meta[name="theme-color"]')?.setAttribute(
    'content',
    colors.background.primary,
  );
}

/**
 * Keeps the installer shell CSS variables aligned with the user's theme preference.
 * When preference is `system`, follows `prefers-color-scheme` like the main BitFun ThemeService.
 */
export function useSyncInstallerRootTheme(preference: ThemePreferenceId): void {
  useLayoutEffect(() => {
    if (preference !== SYSTEM_THEME_ID) {
      applyInstallerThemeToDocument(findInstallerThemeById(preference));
      return;
    }

    const applyResolved = () => {
      applyInstallerThemeToDocument(findInstallerThemeById(getSystemPreferredBuiltinThemeId()));
    };

    applyResolved();

    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return;
    }

    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => {
      applyResolved();
    };

    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, [preference]);
}
