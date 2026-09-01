import { themes } from '@bitfun/theme-bitfun';

// Apply the saved theme before React and the main stylesheet load, avoiding a
// light/dark flash without requiring an inline script under the mobile CSP.
let initialTheme: 'dark' | 'light' = 'dark';
try {
  const savedTheme = localStorage.getItem('bitfun-mobile-theme');
  if (savedTheme === 'dark' || savedTheme === 'light') {
    initialTheme = savedTheme;
  }
} catch {
  if (window.matchMedia?.('(prefers-color-scheme: light)').matches) {
    initialTheme = 'light';
  }
}
const root = document.documentElement;
root.setAttribute('data-bf-design-system-root', '');
root.setAttribute('data-color-scheme', initialTheme);
root.setAttribute('data-contrast', 'standard');
root.setAttribute('data-density', 'comfortable');
root.style.colorScheme = initialTheme;

const themeColor = document.querySelector('meta[name="theme-color"]');
themeColor?.setAttribute('content', String(themes[initialTheme]['color.surface.canvas']));
