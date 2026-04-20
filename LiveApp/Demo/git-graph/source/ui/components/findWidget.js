/**
 * Git Graph Live App — find widget and branch filter dropdown.
 */
(function () {
  window.__GG = window.__GG || {};
  const state = window.__GG.state;
  const $ = window.__GG.$;
  const show = window.__GG.show;

  window.__GG.updateFindMatches = function () {
    const q = state.findQuery.trim().toLowerCase();
    if (!q) {
      state.findMatches = [];
      state.findIndex = 0;
      return;
    }
    const list = window.__GG.getDisplayCommits();
    state.findMatches = list
      .map(function (c, i) { return { c: c, i: i }; })
      .filter(function (x) {
        const c = x.c;
        return (c.message && c.message.toLowerCase().indexOf(q) !== -1) ||
          (c.hash && c.hash.toLowerCase().indexOf(q) !== -1) ||
          (c.shortHash && c.shortHash.toLowerCase().indexOf(q) !== -1) ||
          (c.author && c.author.toLowerCase().indexOf(q) !== -1);
      })
      .map(function (x) { return x.i; });
    state.findIndex = 0;
  };

  window.__GG.showFindWidget = function () {
    show($('find-widget'), true);
    $('find-input').value = state.findQuery;
    $('find-input').focus();
    window.__GG.updateFindMatches();
    window.__GG.renderCommitList();
    $('find-result').textContent = state.findMatches.length > 0 ? '1 / ' + state.findMatches.length : '0';
  };

  window.__GG.findPrev = function () {
    if (state.findMatches.length === 0) return;
    state.findIndex = (state.findIndex - 1 + state.findMatches.length) % state.findMatches.length;
    window.__GG.scrollToFindIndex();
  };

  window.__GG.findNext = function () {
    if (state.findMatches.length === 0) return;
    state.findIndex = (state.findIndex + 1) % state.findMatches.length;
    window.__GG.scrollToFindIndex();
  };

  window.__GG.scrollToFindIndex = function () {
    const idx = state.findMatches[state.findIndex];
    if (idx === undefined) return;
    const list = $('commit-list');
    const rows = list.querySelectorAll('.commit-row');
    const row = rows[idx];
    if (row) row.scrollIntoView({ block: 'nearest' });
    $('find-result').textContent = (state.findIndex + 1) + ' / ' + state.findMatches.length;
    window.__GG.renderCommitList();
  };

  window.__GG.renderBranchFilterDropdown = function () {
    const dropdown = $('branch-filter-dropdown');
    if (!state.branches || !dropdown) return;
    const all = state.branches.all || [];
    const selected = state.selectedBranchFilter.length === 0 ? 'all' : state.selectedBranchFilter;
    dropdown.innerHTML = '';
    const allItem = document.createElement('div');
    allItem.className = 'dropdown-panel__item';
    allItem.textContent = 'All branches';
    allItem.addEventListener('click', function () {
      state.selectedBranchFilter = [];
      $('branch-filter-label').textContent = 'All branches';
      dropdown.setAttribute('aria-hidden', 'true');
      window.__GG.loadRepo();
    });
    dropdown.appendChild(allItem);
    const sep = document.createElement('div');
    sep.className = 'dropdown-panel__sep';
    dropdown.appendChild(sep);
    all.forEach(function (name) {
      const isSelected = selected === 'all' || selected.indexOf(name) !== -1;
      const div = document.createElement('div');
      div.className = 'dropdown-panel__item';
      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.checked = isSelected;
      cb.addEventListener('change', function () {
        if (selected === 'all') {
          state.selectedBranchFilter = [name];
        } else {
          if (cb.checked) state.selectedBranchFilter = state.selectedBranchFilter.concat(name);
          else state.selectedBranchFilter = state.selectedBranchFilter.filter(function (n) { return n !== name; });
        }
        $('branch-filter-label').textContent =
          state.selectedBranchFilter.length === 0 ? 'All branches' : state.selectedBranchFilter.join(', ');
        window.__GG.loadRepo();
      });
      div.appendChild(cb);
      div.appendChild(document.createTextNode(' ' + name));
      dropdown.appendChild(div);
    });
  };
})();
