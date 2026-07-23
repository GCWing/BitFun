import React, { createContext, useCallback, useLayoutEffect, useState } from 'react';
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
const THEME_STYLE_ATTR = 'data-bitfun-theme';

const themeMap: Record<ThemeId, Record<string, string>> = {
  dark: darkTheme,
  light: lightTheme,
};

const BACKGROUND_KEY = '--color-bg-primary';
const TEXT_KEY = '--color-text-primary';

function getThemeValue(id: ThemeId, key: string): string {
  return themeMap[id][key];
}

function buildThemeCSS(id: ThemeId, vars: Record<string, string>): string {
  const bg = getThemeValue(id, BACKGROUND_KEY);
  const fg = getThemeValue(id, TEXT_KEY);
  const parts: string[] = [':root {'];
  for (const [key, value] of Object.entries(vars)) {
    parts.push(`  ${key}: ${value};`);
  }
  parts.push(`  color-scheme: ${id};`);
  parts.push('}');
  parts.push(`html, body { background-color: ${bg}; color: ${fg}; color-scheme: ${id}; }`);
  return parts.join('\n');
}

const cssCache: Record<ThemeId, string> = {
  dark: buildThemeCSS('dark', darkTheme),
  light: buildThemeCSS('light', lightTheme),
};

function commitThemeDOM(id: ThemeId) {
  const root = document.documentElement;
  const body = document.body;
  const vars = themeMap[id];

  // 1. Inject new <style> BEFORE removing old ones to avoid a CSS-variable gap
  //    where variables are temporarily undefined. Use a data attribute selector
  //    instead of id to allow brief overlap of both style elements.
  const newStyleEl = document.createElement('style');
  newStyleEl.setAttribute(THEME_STYLE_ATTR, id);
  newStyleEl.textContent = cssCache[id];
  document.head.appendChild(newStyleEl);

  // Remove all previous theme style elements
  document.head.querySelectorAll(`style[${THEME_STYLE_ATTR}]`).forEach(el => {
    if (el !== newStyleEl) el.remove();
  });

  // 2. Set every CSS variable directly on :root via setProperty().
  //    This is the most reliable cross-browser path — it guarantees
  //    variables resolve even if the <style> element is ignored.
  for (const key of Object.keys(vars)) {
    root.style.setProperty(key, vars[key]);
  }
  root.style.setProperty('color-scheme', id);

  // 3. data-theme attributes
  root.setAttribute('data-theme', id);
  root.setAttribute('data-theme-type', id);
  body.setAttribute('data-theme', id);

  // 4. Inline style fallbacks (highest specificity)
  body.style.backgroundColor = getThemeValue(id, BACKGROUND_KEY);
  body.style.color = getThemeValue(id, TEXT_KEY);

  // 5. Update <meta name="theme-color">
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) {
    meta.setAttribute('content', getThemeValue(id, BACKGROUND_KEY));
    meta.removeAttribute('media');
  }
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

const TRANSITION_MS = 280;
let switchTimer: ReturnType<typeof setTimeout> | undefined;

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [themeId, setThemeId] = useState<ThemeId>(getInitialTheme);

  useLayoutEffect(() => {
    const root = document.documentElement;
    const prevTheme = root.getAttribute('data-theme');
    const isSwitch = prevTheme && prevTheme !== themeId;

    if (isSwitch) {
      clearTimeout(switchTimer);
      root.classList.add('theme-switching');
      // Force reflow so the browser registers the transition rules
      // BEFORE we change CSS variable values — otherwise the start and
      // end states are set in the same frame and no animation occurs.
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
  const toggleTheme = useCallback(() => setThemeId(prev => prev === 'dark' ? 'light' : 'dark'), []);

  return (
    <ThemeContext.Provider value={{ themeId, isDark: themeId === 'dark', setTheme, toggleTheme }}>
      {children}
    </ThemeContext.Provider>
  );
};
