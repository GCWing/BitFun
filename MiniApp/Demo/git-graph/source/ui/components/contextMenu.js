/**
 * Git Graph Live App — context menu.
 */
(function () {
  window.__GG = window.__GG || {};
  const $ = window.__GG.$;

  window.__GG.showContextMenu = function (x, y, items) {
    const menu = $('context-menu');
    menu.innerHTML = '';
    menu.setAttribute('aria-hidden', 'false');
    menu.style.left = x + 'px';
    menu.style.top = y + 'px';
    items.forEach(function (item) {
      if (item === null) {
        const sep = document.createElement('div');
        sep.className = 'context-menu__sep';
        menu.appendChild(sep);
        return;
      }
      const el = document.createElement('div');
      el.className = 'context-menu__item' + (item.disabled ? ' context-menu__item--disabled' : '');
      el.textContent = item.label;
      if (!item.disabled && item.action) {
        el.addEventListener('click', function () {
          window.__GG.hideContextMenu();
          item.action();
        });
      }
      menu.appendChild(el);
    });
  };

  window.__GG.hideContextMenu = function () {
    const menu = $('context-menu');
    if (menu) {
      menu.setAttribute('aria-hidden', 'true');
      menu.innerHTML = '';
    }
  };

  document.addEventListener('click', function () { window.__GG.hideContextMenu(); });
  document.addEventListener('contextmenu', function (e) {
    if (e.target.closest('#context-menu')) return;
    if (!e.target.closest('.commit-row')) window.__GG.hideContextMenu();
  });
})();
