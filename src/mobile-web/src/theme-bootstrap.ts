// Apply the saved theme before React and the main stylesheet load, avoiding a
// light/dark flash without requiring an inline script under the mobile CSP.
let initialTheme = 'dark';
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
document.documentElement.setAttribute('data-theme', initialTheme);
document.documentElement.style.colorScheme = initialTheme;
