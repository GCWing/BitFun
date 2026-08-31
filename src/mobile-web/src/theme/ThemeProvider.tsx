import { themes } from '@bitfun/theme-bitfun';
import React, { createContext, useCallback, useLayoutEffect, useState } from 'react';

export type ThemeId = 'dark' | 'light';

interface ThemeContextValue {
  themeId: ThemeId;
  isDark: boolean;
  setTheme: (id: ThemeId) => void;
  toggleTheme: () => void;
}

const STORAGE_KEY = 'bitfun-mobile-theme';

function commitThemeDOM(id: ThemeId) {
  const root = document.documentElement;
  root.setAttribute('data-bf-design-system-root', '');
  root.setAttribute('data-color-scheme', id);
  root.setAttribute('data-contrast', 'standard');
  root.setAttribute('data-density', 'comfortable');
  root.style.colorScheme = id;

  document.querySelector('meta[name="theme-color"]')?.setAttribute(
    'content',
    String(themes[id]['color.surface.canvas']),
  );
}

function getInitialTheme(): ThemeId {
  const bootstrapped = document.documentElement.getAttribute('data-color-scheme');
  if (bootstrapped === 'dark' || bootstrapped === 'light') return bootstrapped;

  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'dark' || stored === 'light') return stored;
  } catch { /* ignore */ }
  return window.matchMedia?.('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

export const ThemeContext = createContext<ThemeContextValue>({
  themeId: 'dark',
  isDark: true,
  setTheme: () => {},
  toggleTheme: () => {},
});

const TRANSITION_MS = 280;
let switchTimer: ReturnType<typeof setTimeout> | undefined;

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [themeId, setThemeId] = useState<ThemeId>(getInitialTheme);

  useLayoutEffect(() => {
    const root = document.documentElement;
    const previousTheme = root.getAttribute('data-color-scheme');
    const isSwitch = previousTheme != null && previousTheme !== themeId;

    if (isSwitch) {
      clearTimeout(switchTimer);
      root.classList.add('theme-switching');
      void root.offsetHeight;
    }

    commitThemeDOM(themeId);

    if (isSwitch) {
      switchTimer = setTimeout(() => {
        root.classList.remove('theme-switching');
      }, TRANSITION_MS + 40);
    }

    try { localStorage.setItem(STORAGE_KEY, themeId); } catch { /* ignore */ }
  }, [themeId]);

  const setTheme = useCallback((id: ThemeId) => setThemeId(id), []);
  const toggleTheme = useCallback(() => setThemeId(previous => previous === 'dark' ? 'light' : 'dark'), []);

  return (
    <ThemeContext.Provider value={{ themeId, isDark: themeId === 'dark', setTheme, toggleTheme }}>
      {children}
    </ThemeContext.Provider>
  );
};
