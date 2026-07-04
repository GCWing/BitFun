import { useLayoutEffect } from 'react';
import { SYSTEM_THEME_ID, type ThemeId, type ThemePreferenceId } from '../types/installer';
import type { InstallerTheme } from './installerThemesData';
import { findInstallerThemeById } from './installerThemesData';

const ACCENT_STOPS = ['50', '100', '200', '300', '400', '500', '600'] as const;

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

  root.style.setProperty('--color-bg-primary', colors.background.primary);
  root.style.setProperty('--color-bg-secondary', colors.background.secondary);
  root.style.setProperty('--color-text-primary', colors.text.primary);
  root.style.setProperty('--color-text-secondary', colors.text.secondary);
  root.style.setProperty('--color-text-muted', colors.text.muted);
  root.style.setProperty('--element-bg-subtle', colors.element.subtle);
  root.style.setProperty('--element-bg-soft', colors.element.soft);
  root.style.setProperty('--element-bg-medium', colors.element.medium);
  root.style.setProperty('--element-bg-strong', colors.element.strong);
  root.style.setProperty('--border-subtle', colors.border.subtle);
  root.style.setProperty('--border-base', colors.border.base);
  root.style.setProperty('--border-medium', colors.border.medium);
  root.style.setProperty('--color-success', colors.semantic.success);
  root.style.setProperty('--color-warning', colors.semantic.warning);
  root.style.setProperty('--color-error', colors.semantic.error);

  ACCENT_STOPS.forEach((key) => {
    root.style.setProperty(`--color-accent-${key}`, colors.accent[key]);
  });

  root.setAttribute('data-theme', theme.id);
  root.setAttribute('data-theme-type', theme.type);
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
