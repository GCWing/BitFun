(function () {
  var storageKey = 'bitfun-market-theme';
  var storedTheme = null;

  try {
    storedTheme = window.localStorage.getItem(storageKey);
  } catch {
    storedTheme = null;
  }

  var theme =
    storedTheme === 'light' || storedTheme === 'dark'
      ? storedTheme
      : window.matchMedia('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light';

  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute('content', theme === 'dark' ? '#0e0e10' : '#f7f8fa');
})();
