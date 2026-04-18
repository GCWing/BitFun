/**
 * Git Graph Live App — bootstrap: bind events, init resizer, restore last repo, theme subscription.
 */
(function () {
  window.__GG = window.__GG || {};
  const state = window.__GG.state;
  const $ = window.__GG.$;
  const show = window.__GG.show;
  const STORAGE_KEY = window.__GG.STORAGE_KEY;

  function init() {
    $('btn-open-repo').addEventListener('click', window.__GG.openRepo);
    $('btn-empty-open').addEventListener('click', window.__GG.openRepo);
    $('btn-close-detail').addEventListener('click', window.__GG.closeDetail);
    $('btn-refresh').addEventListener('click', function () { window.__GG.loadRepo(); });
    $('btn-remotes').addEventListener('click', window.__GG.showRemotePanel);
    $('btn-remote-close').addEventListener('click', function () { show($('remote-panel'), false); });
    $('btn-find').addEventListener('click', window.__GG.showFindWidget);
    $('find-input').addEventListener('input', function () {
      state.findQuery = $('find-input').value;
      window.__GG.updateFindMatches();
      window.__GG.renderCommitList();
      $('find-result').textContent = state.findMatches.length > 0 ? (state.findIndex + 1) + ' / ' + state.findMatches.length : '0';
    });
    $('find-prev').addEventListener('click', window.__GG.findPrev);
    $('find-next').addEventListener('click', window.__GG.findNext);
    $('find-close').addEventListener('click', function () {
      show($('find-widget'), false);
      state.findQuery = '';
      state.findMatches = [];
      window.__GG.renderCommitList();
    });
    document.addEventListener('keydown', function (e) {
      if (e.ctrlKey && e.key === 'f') {
        e.preventDefault();
        window.__GG.showFindWidget();
      }
      if ($('find-widget').style.display !== 'none') {
        if (e.key === 'Escape') show($('find-widget'), false);
        if (e.key === 'Enter') (e.shiftKey ? window.__GG.findPrev : window.__GG.findNext)();
      }
    });
    var loadMore = $('btn-load-more');
    if (loadMore) loadMore.addEventListener('click', function () { window.__GG.loadRepo(); });

    window.__GG.initDetailResizer();

    var branchFilterBtn = $('btn-branch-filter');
    if (branchFilterBtn) {
      branchFilterBtn.addEventListener('click', function () {
        var dropdown = $('branch-filter-dropdown');
        var hidden = dropdown.getAttribute('aria-hidden') !== 'false';
        dropdown.setAttribute('aria-hidden', String(!hidden));
        if (hidden) window.__GG.renderBranchFilterDropdown();
      });
    }

    if (window.app && typeof window.app.onThemeChange === 'function') {
      window.app.onThemeChange(function () {
        if (state.cwd && $('commit-list').children.length) {
          window.__GG.renderCommitList();
        }
      });
    }

    (async function () {
      try {
        var last = await window.app.storage.get(STORAGE_KEY);
        if (last && typeof last === 'string') {
          state.cwd = last;
          await window.__GG.loadRepo();
        }
      } catch (_) {}
    })();
  }

  window.__GG.openRepo = async function () {
    try {
      var sel = await window.app.dialog.open({ directory: true, multiple: false });
      if (Array.isArray(sel)) sel = sel[0];
      if (!sel) return;
      state.cwd = sel;
      await window.app.storage.set(STORAGE_KEY, sel);
      await window.__GG.loadRepo();
    } catch (e) {
      console.error('open failed', e);
    }
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
