import { useCallback, useEffect, useState } from 'react';
import { themes } from '@bitfun/theme-bitfun';

export type Theme = 'light' | 'dark';

export const THEME_STORAGE_KEY = 'bitfun-market-theme';

export function isTheme(value: string | null): value is Theme {
  return value === 'light' || value === 'dark';
}

export function resolveTheme(storedTheme: string | null, systemPrefersDark: boolean): Theme {
  return isTheme(storedTheme) ? storedTheme : systemPrefersDark ? 'dark' : 'light';
}

function readStoredTheme(): string | null {
  try {
    return window.localStorage.getItem(THEME_STORAGE_KEY);
  } catch {
    return null;
  }
}

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  root.setAttribute('data-bf-design-system-root', '');
  root.dataset.colorScheme = theme;
  root.dataset.contrast = 'standard';
  root.dataset.density = 'comfortable';
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute('content', String(themes[theme]['color.surface.canvas']));
}

function initialTheme(): Theme {
  const documentTheme = document.documentElement.dataset.theme ?? null;
  if (isTheme(documentTheme)) return documentTheme;

  return resolveTheme(
    readStoredTheme(),
    window.matchMedia('(prefers-color-scheme: dark)').matches,
  );
}

export function useTheme() {
  const [theme, setTheme] = useState<Theme>(initialTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  useEffect(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleSystemThemeChange = (event: MediaQueryListEvent) => {
      if (!isTheme(readStoredTheme())) {
        setTheme(event.matches ? 'dark' : 'light');
      }
    };
    const handleStorageChange = (event: StorageEvent) => {
      if (event.key === THEME_STORAGE_KEY) {
        setTheme(resolveTheme(event.newValue, mediaQuery.matches));
      }
    };

    mediaQuery.addEventListener('change', handleSystemThemeChange);
    window.addEventListener('storage', handleStorageChange);
    return () => {
      mediaQuery.removeEventListener('change', handleSystemThemeChange);
      window.removeEventListener('storage', handleStorageChange);
    };
  }, []);

  const toggleTheme = useCallback(() => {
    setTheme((currentTheme) => {
      const nextTheme = currentTheme === 'dark' ? 'light' : 'dark';
      try {
        window.localStorage.setItem(THEME_STORAGE_KEY, nextTheme);
      } catch {
        // The in-memory preference still applies when storage is unavailable.
      }
      return nextTheme;
    });
  }, []);

  return { theme, toggleTheme };
}
