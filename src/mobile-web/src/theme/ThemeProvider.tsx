import React, { createContext, useCallback, useEffect, useState } from 'react';
import { darkTheme } from './presets/dark';
import { lightTheme } from './presets/light';

export type ThemeId = 'dark' | 'light';

interface ThemeContextValue {
  themeId: ThemeId;
  isDark: boolean;
  setTheme: (id: ThemeId) => void;
  toggleTheme: () => void;
}

const STORAGE_KEY = 'bitfun-mobile-theme';

const themeMap: Record<ThemeId, Record<string, string>> = {
  dark: darkTheme,
  light: lightTheme,
};

function applyTheme(id: ThemeId) {
  const vars = themeMap[id];
  const root = document.documentElement;
  for (const [key, value] of Object.entries(vars)) {
    root.style.setProperty(key, value);
  }
  root.setAttribute('data-theme', id);
}

function getInitialTheme(): ThemeId {
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

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [themeId, setThemeId] = useState<ThemeId>(getInitialTheme);

  useEffect(() => {
    applyTheme(themeId);
    try { localStorage.setItem(STORAGE_KEY, themeId); } catch { /* ignore */ }
  }, [themeId]);

  const setTheme = useCallback((id: ThemeId) => setThemeId(id), []);
  const toggleTheme = useCallback(() => setThemeId(prev => prev === 'dark' ? 'light' : 'dark'), []);

  return (
    <ThemeContext.Provider value={{ themeId, isDark: themeId === 'dark', setTheme, toggleTheme }}>
      {children}
    </ThemeContext.Provider>
  );
};
