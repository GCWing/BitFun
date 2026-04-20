/**
 * Git Graph Live App — modal dialog.
 */
(function () {
  window.__GG = window.__GG || {};
  const $ = window.__GG.$;

  window.__GG.showModal = function (title, bodyHTML, buttons) {
    const overlay = $('modal-overlay');
    const titleEl = $('modal-title');
    const bodyEl = $('modal-body');
    const actionsEl = overlay.querySelector('.modal-dialog__actions');
    titleEl.textContent = title;
    bodyEl.innerHTML = bodyHTML;
    actionsEl.innerHTML = '';
    buttons.forEach(function (btn) {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = btn.primary ? 'btn btn--primary' : 'btn btn--secondary';
      b.textContent = btn.label;
      b.addEventListener('click', function () {
        if (btn.action) btn.action(b);
        else window.__GG.hideModal();
      });
      actionsEl.appendChild(b);
    });
    overlay.setAttribute('aria-hidden', 'false');
    $('modal-close').onclick = function () { window.__GG.hideModal(); };
    overlay.onclick = function (e) {
      if (e.target === overlay) window.__GG.hideModal();
    };
  };

  window.__GG.hideModal = function () {
    window.__GG.$('modal-overlay').setAttribute('aria-hidden', 'true');
  };
})();
