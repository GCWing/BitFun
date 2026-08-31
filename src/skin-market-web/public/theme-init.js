(function () {
  var storageKey = 'bitfun-skin-market-theme';
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

  var root = document.documentElement;
  root.setAttribute('data-bf-design-system-root', '');
  root.dataset.colorScheme = theme;
  root.dataset.contrast = 'standard';
  root.dataset.density = 'comfortable';
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
})();
