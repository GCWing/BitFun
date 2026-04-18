/**
 * Git Graph Live App — remote panel.
 */
(function () {
  window.__GG = window.__GG || {};
  const state = window.__GG.state;
  const $ = window.__GG.$;
  const show = window.__GG.show;
  const call = window.__GG.call;
  const showModal = window.__GG.showModal;
  const hideModal = window.__GG.hideModal;
  const escapeHtml = window.__GG.escapeHtml;
  const setLoading = window.__GG.setLoading;

  window.__GG.showRemotePanel = function () {
    show($('remote-panel'), true);
    window.__GG.renderRemoteList();
  };

  window.__GG.renderRemoteList = function () {
    const list = $('remote-list');
    list.innerHTML = '';
    (state.remotes || []).forEach(function (r) {
      const div = document.createElement('div');
      div.className = 'remote-item';
      div.innerHTML =
        '<div><div class="remote-item__name">' + escapeHtml(r.name) + '</div>' +
        '<div class="remote-item__url" title="' + escapeHtml(r.fetch || '') + '">' +
        escapeHtml((r.fetch || '').slice(0, 50)) + ((r.fetch || '').length > 50 ? '\u2026' : '') + '</div></div>' +
        '<div class="remote-item__actions">' +
        '<button type="button" class="btn btn--icon" data-action="fetch" title="Fetch">F</button>' +
        '<button type="button" class="btn btn--icon" data-action="remove" title="Delete">\u00d7</button></div>';
      div.querySelector('[data-action="fetch"]').addEventListener('click', async function () {
        setLoading(true);
        try {
          await call('git.fetch', { remote: r.name, prune: true });
          await window.__GG.loadRepo();
          state.remotes = (await call('git.remotes')).remotes || [];
          window.__GG.renderRemoteList();
        } finally {
          setLoading(false);
        }
      });
      div.querySelector('[data-action="remove"]').addEventListener('click', function () {
        showModal('Delete Remote', 'Delete remote <strong>' + escapeHtml(r.name) + '</strong>?', [
          { label: 'Cancel' },
          {
            label: 'Delete',
            primary: true,
            action: async function () {
              await call('git.removeRemote', { name: r.name });
              hideModal();
              await window.__GG.loadRepo();
              state.remotes = (await call('git.remotes')).remotes || [];
              window.__GG.renderRemoteList();
            },
          },
        ]);
      });
      list.appendChild(div);
    });
    const addBtn = document.createElement('button');
    addBtn.type = 'button';
    addBtn.className = 'btn btn--secondary';
    addBtn.textContent = 'Add Remote';
    addBtn.style.marginTop = '8px';
    addBtn.addEventListener('click', function () {
      showModal(
        'Add Remote',
        '<div class="modal-form-group"><label>Name</label><input type="text" id="modal-remote-name" placeholder="origin" /></div>' +
        '<div class="modal-form-group"><label>URL</label><input type="text" id="modal-remote-url" placeholder="https://..." /></div>',
        [
          { label: 'Cancel' },
          {
            label: 'Add',
            primary: true,
            action: async function () {
              const name = ($('modal-remote-name').value || '').trim() || 'origin';
              const url = ($('modal-remote-url').value || '').trim();
              if (!url) return;
              await call('git.addRemote', { name: name, url: url });
              hideModal();
              await window.__GG.loadRepo();
              state.remotes = (await call('git.remotes')).remotes || [];
              window.__GG.renderRemoteList();
            },
          },
        ]
      );
    });
    list.appendChild(addBtn);
  };
})();
