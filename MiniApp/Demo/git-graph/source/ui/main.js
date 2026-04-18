/**
 * Git Graph Live App — commit list, context menus, git actions, loadRepo.
 */
(function () {
  window.__GG = window.__GG || {};
  const state = window.__GG.state;
  const $ = window.__GG.$;
  const show = window.__GG.show;
  const call = window.__GG.call;
  const setLoading = window.__GG.setLoading;
  const formatDate = window.__GG.formatDate;
  const parseRefs = window.__GG.parseRefs;
  const getRefsFromStructured = window.__GG.getRefsFromStructured;
  const escapeHtml = window.__GG.escapeHtml;
  const showContextMenu = window.__GG.showContextMenu;
  const showModal = window.__GG.showModal;
  const hideModal = window.__GG.hideModal;
  const MAX_COMMITS = window.__GG.MAX_COMMITS;
  const LANE_W = window.__GG.LANE_W;

  window.__GG.renderCommitList = function () {
    const list = $('commit-list');
    list.innerHTML = '';
    const display = window.__GG.getDisplayCommits();
    if (!display.length) return;

    var layout = window.__GG.computeGraphLayout(display, state.firstParent);
    const laneCount = layout.laneCount;
    const rowDrawInfo = layout.rowDrawInfo || [];
    const graphW = Math.max(32, laneCount * LANE_W + 16);

    display.forEach(function (c, i) {
      const isUncommitted = c.hash === '__uncommitted__' || c.isUncommitted;
      const isStash = c.isStash === true || (c.stash && c.stash.selector);
      const drawInfo = rowDrawInfo[i] || { lane: 0, lanesBefore: [], parentLanes: [] };

      const row = document.createElement('div');
      row.className =
        'commit-row' +
        (state.selectedHash === c.hash ? ' selected' : '') +
        (state.compareHashes.indexOf(c.hash) !== -1 ? ' compare-selected' : '') +
        (state.findMatches.length && state.findMatches[state.findIndex] === i ? ' find-highlight' : '') +
        (isUncommitted ? ' commit-row--uncommitted' : '') +
        (isStash ? ' commit-row--stash' : '');
      row.dataset.hash = c.hash;
      row.dataset.index = String(i);

      row.addEventListener('click', function (e) {
        if (e.ctrlKey || e.metaKey) {
          if (state.compareHashes.indexOf(c.hash) !== -1) {
            state.compareHashes = state.compareHashes.filter(function (h) { return h !== c.hash; });
          } else {
            state.compareHashes = state.compareHashes.concat(c.hash).slice(-2);
          }
          window.__GG.renderCommitList();
          if (state.compareHashes.length === 2) window.__GG.openComparePanel(state.compareHashes[0], state.compareHashes[1]);
          return;
        }
        if (isUncommitted) {
          window.__GG.selectCommit('__uncommitted__');
          return;
        }
        window.__GG.selectCommit(c.hash);
      });

      row.addEventListener('contextmenu', function (e) {
        e.preventDefault();
        if (isUncommitted) window.__GG.showUncommittedContextMenu(e.clientX, e.clientY);
        else if (isStash) window.__GG.showStashContextMenu(e.clientX, e.clientY, c);
        else window.__GG.showCommitContextMenu(e.clientX, e.clientY, c);
      });

      const graphCell = document.createElement('div');
      graphCell.className = 'commit-row__graph';
      graphCell.style.width = graphW + 'px';
      const svg = window.__GG.buildRowSvg(
        isUncommitted ? { parentHashes: [], hash: '' } : c,
        drawInfo,
        graphW,
        isStash,
        isUncommitted
      );
      graphCell.appendChild(svg);
      row.appendChild(graphCell);

      const info = document.createElement('div');
      info.className = 'commit-row__info';
      const hash = document.createElement('span');
      hash.className = 'commit-row__hash';
      hash.textContent = isUncommitted ? 'WIP' : (c.shortHash || (c.hash && c.hash.slice(0, 7)) || '');
      const msg = document.createElement('span');
      msg.className = 'commit-row__message';
      msg.textContent = c.message || (isUncommitted ? 'Uncommitted changes' : '');
      const refsSpan = document.createElement('span');
      refsSpan.className = 'commit-row__refs';
      var refTags = (c.heads || c.tags || c.remotes) ? getRefsFromStructured(c, state.branches && state.branches.current) : (c.refs ? parseRefs(c.refs) : []);
      refTags.forEach(function (r) {
          const tag = document.createElement('span');
          tag.className = 'ref-tag ref-tag--' + r.type;
          tag.textContent = r.label;
          if (r.type === 'branch') {
            tag.addEventListener('contextmenu', function (e) {
              e.preventDefault();
              e.stopPropagation();
              window.__GG.showBranchContextMenu(e.clientX, e.clientY, r.label, false);
            });
          } else if (r.type === 'remote') {
            const parts = r.label.split('/');
            if (parts.length >= 2) {
              tag.addEventListener('contextmenu', function (e) {
                e.preventDefault();
                e.stopPropagation();
                window.__GG.showBranchContextMenu(e.clientX, e.clientY, parts.slice(1).join('/'), true, parts[0]);
              });
            }
          }
          refsSpan.appendChild(tag);
      });
      if (isStash && (c.stashSelector || (c.stash && c.stash.selector))) {
        const t = document.createElement('span');
        t.className = 'ref-tag ref-tag--tag';
        t.textContent = c.stashSelector || (c.stash && c.stash.selector) || '';
        refsSpan.appendChild(t);
      }
      const author = document.createElement('span');
      author.className = 'commit-row__author';
      author.textContent = c.author || '';
      const date = document.createElement('span');
      date.className = 'commit-row__date';
      date.textContent = isUncommitted ? '' : formatDate(c.date);
      info.appendChild(hash);
      info.appendChild(refsSpan);
      info.appendChild(msg);
      info.appendChild(author);
      info.appendChild(date);
      row.appendChild(info);
      list.appendChild(row);
    });

    show($('load-more'), state.hasMore && state.commits.length >= MAX_COMMITS);
  };

  window.__GG.showCommitContextMenu = function (x, y, c) {
    showContextMenu(x, y, [
      { label: 'Add Tag\u2026', action: function () { window.__GG.openAddTagDialog(c.hash); } },
      { label: 'Create Branch\u2026', action: function () { window.__GG.openCreateBranchDialog(c.hash); } },
      null,
      { label: 'Checkout\u2026', action: function () { window.__GG.checkoutCommit(c.hash); } },
      { label: 'Cherry Pick\u2026', action: function () { window.__GG.cherryPick(c.hash); } },
      { label: 'Revert\u2026', action: function () { window.__GG.revertCommit(c.hash); } },
      { label: 'Drop Commit\u2026', action: function () { window.__GG.dropCommit(c.hash); } },
      null,
      { label: 'Merge into current branch\u2026', action: function () { window.__GG.openMergeDialog(c.hash); } },
      { label: 'Rebase onto this commit\u2026', action: function () { window.__GG.openRebaseDialog(c.hash); } },
      { label: 'Reset current branch\u2026', action: function () { window.__GG.openResetDialog(c.hash); } },
      null,
      { label: 'Copy Hash', action: function () { navigator.clipboard.writeText(c.hash); } },
      { label: 'Copy Subject', action: function () { navigator.clipboard.writeText(c.message || ''); } },
    ]);
  };

  window.__GG.showStashContextMenu = function (x, y, c) {
    var selector = c.stashSelector || (c.stash && c.stash.selector);
    showContextMenu(x, y, [
      { label: 'Apply Stash\u2026', action: function () { window.__GG.stashApply(selector); } },
      { label: 'Pop Stash\u2026', action: function () { window.__GG.stashPop(selector); } },
      { label: 'Drop Stash\u2026', action: function () { window.__GG.stashDrop(selector); } },
      { label: 'Create Branch from Stash\u2026', action: function () { window.__GG.openStashBranchDialog(selector); } },
      null,
      { label: 'Copy Stash Name', action: function () { navigator.clipboard.writeText(selector || ''); } },
      { label: 'Copy Hash', action: function () { navigator.clipboard.writeText(c.hash); } },
    ]);
  };

  window.__GG.showUncommittedContextMenu = function (x, y) {
    showContextMenu(x, y, [
      { label: 'Stash uncommitted changes\u2026', action: function () { window.__GG.openStashPushDialog(); } },
      null,
      { label: 'Reset uncommitted changes\u2026', action: function () { window.__GG.openResetUncommittedDialog(); } },
      { label: 'Clean untracked files\u2026', action: function () { window.__GG.cleanUntracked(); } },
    ]);
  };

  window.__GG.showBranchContextMenu = function (x, y, branchName, isRemote, remoteName) {
    isRemote = !!isRemote;
    remoteName = remoteName || null;
    const items = [];
    if (isRemote) {
      items.push(
        { label: 'Checkout\u2026', action: function () { window.__GG.openCheckoutRemoteBranchDialog(remoteName, branchName); } },
        { label: 'Fetch into local branch\u2026', action: function () { window.__GG.openFetchIntoLocalDialog(remoteName, branchName); } },
        { label: 'Delete Remote Branch\u2026', action: function () { window.__GG.deleteRemoteBranch(remoteName, branchName); } }
      );
    } else {
      items.push(
        { label: 'Checkout', action: function () { window.__GG.checkoutBranch(branchName); } },
        { label: 'Rename\u2026', action: function () { window.__GG.openRenameBranchDialog(branchName); } },
        { label: 'Delete\u2026', action: function () { window.__GG.openDeleteBranchDialog(branchName); } },
        null,
        { label: 'Merge into current branch\u2026', action: function () { window.__GG.openMergeDialog(branchName); } },
        { label: 'Rebase onto\u2026', action: function () { window.__GG.openRebaseDialog(branchName); } },
        { label: 'Push\u2026', action: function () { window.__GG.openPushDialog(branchName); } }
      );
    }
    items.push(null, { label: 'Copy Branch Name', action: function () { navigator.clipboard.writeText(branchName); } });
    showContextMenu(x, y, items);
  };

  window.__GG.checkoutBranch = async function (name) {
    setLoading(true);
    try {
      await call('git.checkout', { ref: name });
      await window.__GG.loadRepo();
    } finally {
      setLoading(false);
    }
  };

  window.__GG.checkoutCommit = async function (hash) {
    setLoading(true);
    try {
      await call('git.checkout', { ref: hash });
      await window.__GG.loadRepo();
    } finally {
      setLoading(false);
    }
  };

  window.__GG.openCreateBranchDialog = function (startHash) {
    showModal('Create Branch',
      '<div class="modal-form-group"><label>Branch name</label><input type="text" id="modal-branch-name" placeholder="feature/xxx" /></div>' +
      '<div class="modal-form-group checkbox-wrap"><input type="checkbox" id="modal-branch-checkout" checked /><label for="modal-branch-checkout">Checkout after create</label></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Create',
          primary: true,
          action: async function () {
            const name = ($('modal-branch-name').value || '').trim();
            if (!name) return;
            const checkout = $('modal-branch-checkout').checked;
            await call('git.createBranch', { name: name, startPoint: startHash, checkout: checkout });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.openAddTagDialog = function (ref) {
    showModal('Add Tag',
      '<div class="modal-form-group"><label>Tag name</label><input type="text" id="modal-tag-name" placeholder="v1.0" /></div>' +
      '<div class="modal-form-group checkbox-wrap"><input type="checkbox" id="modal-tag-annotated" /><label for="modal-tag-annotated">Annotated (with message)</label></div>' +
      '<div class="modal-form-group" id="modal-tag-message-wrap" style="display:none"><label>Message</label><textarea id="modal-tag-message" rows="2"></textarea></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Add',
          primary: true,
          action: async function () {
            const name = ($('modal-tag-name').value || '').trim();
            if (!name) return;
            const annotated = $('modal-tag-annotated').checked;
            const message = ($('modal-tag-message').value || '').trim();
            await call('git.addTag', { name: name, ref: ref, annotated: annotated, message: annotated ? message : null });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
    $('modal-tag-annotated').addEventListener('change', function () {
      show($('modal-tag-message-wrap'), $('modal-tag-annotated').checked);
    });
  };

  window.__GG.openMergeDialog = function (ref) {
    showModal('Merge',
      '<p>Merge <strong>' + escapeHtml(ref) + '</strong> into current branch?</p>' +
      '<div class="modal-form-group checkbox-wrap"><input type="checkbox" id="modal-merge-no-ff" /><label for="modal-merge-no-ff">No fast-forward</label></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Merge',
          primary: true,
          action: async function () {
            const noFF = $('modal-merge-no-ff').checked;
            await call('git.merge', { ref: ref, noFF: noFF });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.openRebaseDialog = function (ref) {
    showModal('Rebase', 'Rebase current branch onto <strong>' + escapeHtml(ref) + '</strong>?', [
      { label: 'Cancel' },
      {
        label: 'Rebase',
        primary: true,
        action: async function () {
          await call('git.rebase', { onto: ref });
          hideModal();
          await window.__GG.loadRepo();
        },
      },
    ]);
  };

  window.__GG.openResetDialog = function (hash) {
    showModal('Reset',
      '<p>Reset current branch to <code>' + escapeHtml(hash.slice(0, 7)) + '</code>?</p>' +
      '<div class="modal-form-group"><label>Mode</label><select id="modal-reset-mode"><option value="soft">Soft</option><option value="mixed" selected>Mixed</option><option value="hard">Hard</option></select></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Reset',
          primary: true,
          action: async function () {
            const mode = $('modal-reset-mode').value;
            await call('git.reset', { hash: hash, mode: mode });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.openResetUncommittedDialog = function () {
    showModal('Reset Uncommitted',
      '<p>Reset all uncommitted changes?</p><div class="modal-form-group"><label>Mode</label><select id="modal-reset-uc-mode"><option value="mixed" selected>Mixed</option><option value="hard">Hard</option></select></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Reset',
          primary: true,
          action: async function () {
            const mode = $('modal-reset-uc-mode').value;
            await call('git.resetUncommitted', { mode: mode });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.cherryPick = async function (hash) {
    setLoading(true);
    try {
      await call('git.cherryPick', { hash: hash });
      await window.__GG.loadRepo();
    } finally {
      setLoading(false);
    }
  };

  window.__GG.revertCommit = async function (hash) {
    setLoading(true);
    try {
      await call('git.revert', { hash: hash });
      await window.__GG.loadRepo();
    } finally {
      setLoading(false);
    }
  };

  window.__GG.dropCommit = function (hash) {
    showModal('Drop Commit', 'Remove commit ' + hash.slice(0, 7) + ' from history?', [
      { label: 'Cancel' },
      {
        label: 'Drop',
        primary: true,
        action: async function () {
          await call('git.dropCommit', { hash: hash });
          hideModal();
          await window.__GG.loadRepo();
        },
      },
    ]);
  };

  window.__GG.openRenameBranchDialog = function (oldName) {
    showModal('Rename Branch',
      '<div class="modal-form-group"><label>New name</label><input type="text" id="modal-rename-branch" value="' + escapeHtml(oldName) + '" /></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Rename',
          primary: true,
          action: async function () {
            const newName = ($('modal-rename-branch').value || '').trim();
            if (!newName) return;
            await call('git.renameBranch', { oldName: oldName, newName: newName });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.openDeleteBranchDialog = function (name) {
    showModal('Delete Branch',
      '<p>Delete branch <strong>' + escapeHtml(name) + '</strong>?</p>' +
      '<div class="modal-form-group checkbox-wrap"><input type="checkbox" id="modal-delete-force" /><label for="modal-delete-force">Force delete</label></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Delete',
          primary: true,
          action: async function () {
            const force = $('modal-delete-force').checked;
            await call('git.deleteBranch', { name: name, force: force });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.openCheckoutRemoteBranchDialog = function (remoteName, branchName) {
    showModal('Checkout Remote Branch',
      '<p>Create local branch from <strong>' + escapeHtml(remoteName + '/' + branchName) + '</strong></p>' +
      '<div class="modal-form-group"><label>Local branch name</label><input type="text" id="modal-local-branch-name" value="' + escapeHtml(branchName) + '" /></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Checkout',
          primary: true,
          action: async function () {
            const localName = ($('modal-local-branch-name').value || '').trim() || branchName;
            await call('git.checkout', { ref: remoteName + '/' + branchName, createBranch: localName });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.openFetchIntoLocalDialog = function (remoteName, remoteBranch) {
    showModal('Fetch into Local Branch',
      '<div class="modal-form-group"><label>Local branch name</label><input type="text" id="modal-fetch-local-name" value="' + escapeHtml(remoteBranch) + '" /></div>' +
      '<div class="modal-form-group checkbox-wrap"><input type="checkbox" id="modal-fetch-force" /><label for="modal-fetch-force">Force</label></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Fetch',
          primary: true,
          action: async function () {
            const localBranch = ($('modal-fetch-local-name').value || '').trim() || remoteBranch;
            const force = $('modal-fetch-force').checked;
            await call('git.fetchIntoLocalBranch', { remote: remoteName, remoteBranch: remoteBranch, localBranch: localBranch, force: force });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.deleteRemoteBranch = async function (remoteName, branchName) {
    setLoading(true);
    try {
      await call('git.push', { remote: remoteName, branch: ':' + branchName });
      await window.__GG.loadRepo();
    } finally {
      setLoading(false);
    }
  };

  window.__GG.openPushDialog = function (branchName) {
    const remotes = state.remotes.map(function (r) { return r.name; });
    if (!remotes.length) {
      showModal('Push', '<p>No remotes configured. Add one in Remote panel.</p>', [{ label: 'OK' }]);
      return;
    }
    showModal('Push Branch',
      '<div class="modal-form-group"><label>Remote</label><select id="modal-push-remote">' +
      remotes.map(function (r) { return '<option value="' + escapeHtml(r) + '">' + escapeHtml(r) + '</option>'; }).join('') +
      '</select></div>' +
      '<div class="modal-form-group checkbox-wrap"><input type="checkbox" id="modal-push-set-upstream" /><label for="modal-push-set-upstream">Set upstream</label></div>' +
      '<div class="modal-form-group checkbox-wrap"><input type="checkbox" id="modal-push-force" /><label for="modal-push-force">Force</label></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Push',
          primary: true,
          action: async function () {
            const remote = $('modal-push-remote').value;
            const setUpstream = $('modal-push-set-upstream').checked;
            const force = $('modal-push-force').checked;
            await call('git.push', { remote: remote, branch: branchName, setUpstream: setUpstream, force: force });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.openStashPushDialog = function () {
    showModal('Stash',
      '<div class="modal-form-group"><label>Message (optional)</label><input type="text" id="modal-stash-msg" placeholder="WIP: ..." /></div>' +
      '<div class="modal-form-group checkbox-wrap"><input type="checkbox" id="modal-stash-untracked" /><label for="modal-stash-untracked">Include untracked</label></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Stash',
          primary: true,
          action: async function () {
            const message = ($('modal-stash-msg').value || '').trim() || null;
            const includeUntracked = $('modal-stash-untracked').checked;
            await call('git.stashPush', { message: message, includeUntracked: includeUntracked });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.stashApply = async function (selector) {
    setLoading(true);
    try {
      await call('git.stashApply', { selector: selector });
      await window.__GG.loadRepo();
    } finally {
      setLoading(false);
    }
  };

  window.__GG.stashPop = async function (selector) {
    setLoading(true);
    try {
      await call('git.stashPop', { selector: selector });
      await window.__GG.loadRepo();
    } finally {
      setLoading(false);
    }
  };

  window.__GG.stashDrop = function (selector) {
    showModal('Drop Stash', 'Drop ' + selector + '?', [
      { label: 'Cancel' },
      {
        label: 'Drop',
        primary: true,
        action: async function () {
          await call('git.stashDrop', { selector: selector });
          hideModal();
          await window.__GG.loadRepo();
        },
      },
    ]);
  };

  window.__GG.openStashBranchDialog = function (selector) {
    showModal('Create Branch from Stash',
      '<div class="modal-form-group"><label>Branch name</label><input type="text" id="modal-stash-branch-name" placeholder="branch-name" /></div>',
      [
        { label: 'Cancel' },
        {
          label: 'Create',
          primary: true,
          action: async function () {
            const branchName = ($('modal-stash-branch-name').value || '').trim();
            if (!branchName) return;
            await call('git.stashBranch', { branchName: branchName, selector: selector });
            hideModal();
            await window.__GG.loadRepo();
          },
        },
      ]
    );
  };

  window.__GG.cleanUntracked = function () {
    showModal('Clean Untracked', 'Remove all untracked files?', [
      { label: 'Cancel' },
      {
        label: 'Clean',
        primary: true,
        action: async function () {
          await call('git.cleanUntracked', { force: true, directories: true });
          hideModal();
          await window.__GG.loadRepo();
        },
      },
    ]);
  };

  window.__GG.loadRepo = async function () {
    if (!state.cwd) return;
    setLoading(true);
    $('repo-path').textContent = state.cwd;
    $('repo-path').title = state.cwd;

    try {
      const branchesParam = state.selectedBranchFilter.length > 0 ? state.selectedBranchFilter : [];
      const graphData = await call('git.graphData', {
        maxCount: MAX_COMMITS,
        order: state.order,
        firstParent: state.firstParent,
        branches: branchesParam,
        showRemoteBranches: true,
        showStashes: true,
        showUncommittedChanges: true,
        hideRemotes: [],
      });

      state.commits = graphData.commits || [];
      state.refs = graphData.refs || { head: null, heads: [], tags: [], remotes: [] };
      state.stash = graphData.stashes || [];
      state.uncommitted = graphData.uncommitted || null;
      state.status = graphData.status || null;
      state.remotes = graphData.remotes || [];
      state.head = graphData.head || null;
      state.hasMore = !!graphData.moreCommitsAvailable;

      var currentBranch = null;
      if (state.refs.head && state.refs.heads && state.refs.heads.length) {
        var headEntry = state.refs.heads.find(function (h) { return h.hash === state.refs.head; });
        if (headEntry) currentBranch = headEntry.name;
      }
      state.branches = {
        current: currentBranch,
        all: (state.refs.heads || []).map(function (h) { return h.name; }),
      };

      if (state.branches.current) {
        $('branch-name').textContent = state.branches.current;
        show($('branch-badge'), true);
      }

      const badge = $('status-badge');
      if (state.status) {
        const m = (state.status.modified && state.status.modified.length) || 0;
        const s = (state.status.staged && state.status.staged.length) || 0;
        const u = (state.status.not_added && state.status.not_added.length) || 0;
        const total = m + s + u;
        if (total > 0) {
          const p = [];
          if (m) p.push(m + ' modified');
          if (s) p.push(s + ' staged');
          if (u) p.push(u + ' untracked');
          badge.textContent = p.join(' \u00b7 ');
          badge.classList.add('has-changes');
        } else {
          badge.textContent = '\u2713 clean';
          badge.classList.remove('has-changes');
        }
        show(badge, true);
      }

      show($('empty-state'), false);
      show($('graph-area'), true);
      window.__GG.renderCommitList();
      window.__GG.updateFindMatches();
      if ($('find-widget').style.display !== 'none') {
        $('find-result').textContent = state.findMatches.length > 0 ? (state.findIndex + 1) + ' / ' + state.findMatches.length : '0';
      }
    } catch (e) {
      console.error('load failed', e);
      show($('empty-state'), true);
      show($('graph-area'), false);
      var desc = $('empty-state') && $('empty-state').querySelector('.empty-state__desc');
      if (desc) desc.textContent = 'Load failed: ' + (e && e.message ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };
})();
