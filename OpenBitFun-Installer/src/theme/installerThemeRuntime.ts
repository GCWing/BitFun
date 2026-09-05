import { useLayoutEffect } from 'react';
import { SYSTEM_THEME_ID, type ThemeId, type ThemePreferenceId } from '../types/installer';
import type { InstallerTheme } from './installerThemesData';
import { findInstallerThemeById } from './installerThemesData';

/** Same rule as main app `getSystemPreferredDefaultThemeId`: dark -> openbitfun-dark, else openbitfun-light. */
export function getSystemPreferredBuiltinThemeId(): ThemeId {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return 'openbitfun-light';
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'openbitfun-dark' : 'openbitfun-light';
}

export function applyInstallerThemeToDocument(theme: InstallerTheme): void {
  const root = document.documentElement;
  const { colors } = theme;

  root.style.setProperty('--openbitfun-color-surface-canvas', colors.background.primary);
  root.style.setProperty('--openbitfun-color-surface-panel', colors.background.secondary);
  root.style.setProperty('--openbitfun-color-content-primary', colors.text.primary);
  root.style.setProperty('--openbitfun-color-content-secondary', colors.text.secondary);
  root.style.setProperty('--openbitfun-color-content-muted', colors.text.muted);
  root.style.setProperty('--openbitfun-color-surface-subtle', colors.element.subtle);
  root.style.setProperty('--openbitfun-color-action-neutral-surface', colors.element.soft);
  root.style.setProperty('--openbitfun-color-action-neutral-surface-pressed', colors.element.medium);
  root.style.setProperty('--openbitfun-color-border-subtle', colors.border.subtle);
  root.style.setProperty('--openbitfun-color-border-default', colors.border.base);
  root.style.setProperty('--openbitfun-color-status-success-content', colors.semantic.success);
  root.style.setProperty('--openbitfun-color-status-warning-content', colors.semantic.warning);
  root.style.setProperty('--openbitfun-color-status-danger-content', colors.semantic.error);

  root.style.setProperty('--openbitfun-color-accent-default', colors.accent);

  root.setAttribute('data-openbitfun-design-system-root', '');
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
 * When preference is `system`, follows `prefers-color-scheme` like the main OpenBitFun ThemeService.
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
