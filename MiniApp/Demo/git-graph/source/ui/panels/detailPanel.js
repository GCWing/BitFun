/**
 * Git Graph Live App — detail panel (commit / compare).
 */
(function () {
  window.__GG = window.__GG || {};
  const state = window.__GG.state;
  const $ = window.__GG.$;
  const show = window.__GG.show;
  const call = window.__GG.call;
  const parseRefs = window.__GG.parseRefs;
  const escapeHtml = window.__GG.escapeHtml;

  window.__GG.showDetailPanel = function () {
    show($('detail-resizer'), true);
    show($('detail-panel'), true);
  };

  window.__GG.openComparePanel = function (hash1, hash2) {
    state.selectedHash = null;
    state.compareHashes = [hash1, hash2];
    window.__GG.showDetailPanel();
    $('detail-panel-title').textContent = 'Compare';
    var summary = $('detail-summary');
    var filesSection = $('detail-files-section');
    var codePreview = $('detail-code-preview');
    if (summary) summary.innerHTML = '<div class="detail-loading">Loading\u2026</div>';
    if (filesSection) show(filesSection, false);
    if (codePreview) show(codePreview, false);
    (async function () {
      try {
        const res = await call('git.compareCommits', { hash1: hash1, hash2: hash2 });
        if (summary) {
          summary.innerHTML = '<div class="detail-section"><div class="detail-section__label">' + escapeHtml(hash1.slice(0, 7)) + ' \u2026 ' + escapeHtml(hash2.slice(0, 7)) + '</div></div>';
        }
        var list = $('detail-files-list');
        if (list) {
          list.innerHTML = '';
          (res.files || []).forEach(function (f) {
            var li = document.createElement('li');
            li.className = 'detail-file';
            li.innerHTML = '<span class="detail-file__name">' + escapeHtml(f.file) + '</span><span class="detail-file__stat">' + escapeHtml(f.status) + '</span>';
            list.appendChild(li);
          });
        }
        if (filesSection) show(filesSection, (res.files && res.files.length) ? true : false);
        if ($('detail-files-label')) $('detail-files-label').textContent = 'Changed Files (' + (res.files ? res.files.length : 0) + ')';
      } catch (e) {
        if (summary) summary.innerHTML = '<div class="detail-error">' + escapeHtml(e && e.message ? e.message : String(e)) + '</div>';
      }
    })();
  };

  window.__GG.selectCommit = async function (hash) {
    state.selectedHash = hash;
    state.compareHashes = [];
    window.__GG.renderCommitList();

    var summary = $('detail-summary');
    var filesSection = $('detail-files-section');
    var codePreview = $('detail-code-preview');
    window.__GG.showDetailPanel();
    $('detail-panel-title').textContent = hash === '__uncommitted__' ? 'Uncommitted changes' : 'Commit';
    if (summary) summary.innerHTML = '<div class="detail-loading">Loading\u2026</div>';
    if (filesSection) show(filesSection, false);
    if (codePreview) show(codePreview, false);

    if (hash === '__uncommitted__') {
      var uncommitted = state.uncommitted;
      if (!uncommitted) {
        if (summary) summary.innerHTML = '<div class="detail-message">No uncommitted changes</div>';
        return;
      }
      var summaryHtml = '<div class="detail-hash">WIP</div><div class="detail-message">Uncommitted changes</div>';
      if (summary) summary.innerHTML = summaryHtml;
      var list = $('detail-files-list');
      if (list) list.innerHTML = '';
      var files = (uncommitted.files || []);
      if (files.length && list) {
        if ($('detail-files-label')) $('detail-files-label').textContent = 'Changed Files (' + files.length + ')';
        show(filesSection, true);
        files.forEach(function (f) {
          var li = document.createElement('li');
          li.className = 'detail-file';
          li.dataset.file = f.path || f.file || '';
          var name = document.createElement('span');
          name.className = 'detail-file__name';
          name.textContent = f.path || f.file || '';
          var stat = document.createElement('span');
          stat.className = 'detail-file__stat';
          stat.textContent = f.status || '';
          li.appendChild(name);
          li.appendChild(stat);
          list.appendChild(li);
        });
      }
      return;
    }

    var displayCommit = (state.commits || []).find(function (c) { return c.hash === hash; });
    var isStashRow = displayCommit && (displayCommit.stash && displayCommit.stash.selector);

    try {
      const res = await call('git.show', { hash: hash });
      if (!res || !res.commit) {
        if (summary) summary.innerHTML = '<div class="detail-error">Commit not found</div>';
        return;
      }
      const c = res.commit;

      var summaryHtml = '';
      summaryHtml += '<div class="detail-hash">' + escapeHtml(c.hash) + '</div>';
      if (isStashRow && displayCommit.stash) {
        summaryHtml += '<div class="detail-meta">Stash: ' + escapeHtml(displayCommit.stash.selector || '') + '<br>Base: ' + escapeHtml((displayCommit.stash.baseHash || '').slice(0, 7)) + (displayCommit.stash.untrackedFilesHash ? ' &middot; Untracked: ' + escapeHtml(displayCommit.stash.untrackedFilesHash.slice(0, 7)) : '') + '</div>';
      }
      var msgFirst = (c.message || '').split('\n')[0];
      if (c.body && c.body.trim()) msgFirst += '\n\n' + c.body.trim();
      summaryHtml += '<div class="detail-message">' + escapeHtml(msgFirst) + '</div>';
      summaryHtml += '<div class="detail-meta"><strong>' + escapeHtml(c.author || '') + '</strong> &lt;' + escapeHtml(c.email || '') + '&gt;<br>' + escapeHtml(String(c.date || '')) + '</div>';
      if (c.refs) {
        summaryHtml += '<div class="detail-refs">';
        parseRefs(c.refs).forEach(function (r) {
          summaryHtml += '<span class="ref-tag ref-tag--' + r.type + '">' + escapeHtml(r.label) + '</span>';
        });
        summaryHtml += '</div>';
      }
      if ((c.heads || c.tags || c.remotes) && window.__GG.getRefsFromStructured) {
        var refTags = window.__GG.getRefsFromStructured(c, state.branches && state.branches.current);
        if (refTags.length) {
          summaryHtml += '<div class="detail-refs">';
          refTags.forEach(function (r) {
            summaryHtml += '<span class="ref-tag ref-tag--' + r.type + '">' + escapeHtml(r.label) + '</span>';
          });
          summaryHtml += '</div>';
        }
      } else if (c.refs) {
        summaryHtml += '<div class="detail-refs">';
        parseRefs(c.refs).forEach(function (r) {
          summaryHtml += '<span class="ref-tag ref-tag--' + r.type + '">' + escapeHtml(r.label) + '</span>';
        });
        summaryHtml += '</div>';
      }
      if (summary) summary.innerHTML = summaryHtml;

      var list = $('detail-files-list');
      if (list) list.innerHTML = '';
      if (res.files && res.files.length && list) {
        $('detail-files-label').textContent = 'Changed Files (' + res.files.length + ')';
        show(filesSection, true);
        res.files.forEach(function (f) {
          var li = document.createElement('li');
          li.className = 'detail-file';
          li.dataset.file = f.file || '';
          var name = document.createElement('span');
          name.className = 'detail-file__name';
          name.textContent = f.file || '';
          name.title = f.file || '';
          var stat = document.createElement('span');
          stat.className = 'detail-file__stat';
          if (f.insertions) {
            var s = document.createElement('span');
            s.className = 'stat-add';
            s.textContent = '+' + f.insertions;
            stat.appendChild(s);
          }
          if (f.deletions) {
            var s2 = document.createElement('span');
            s2.className = 'stat-del';
            s2.textContent = '-' + f.deletions;
            stat.appendChild(s2);
          }
          li.appendChild(name);
          li.appendChild(stat);
          li.addEventListener('click', function () {
            var prev = list.querySelector('.detail-file--selected');
            if (prev) prev.classList.remove('detail-file--selected');
            if (prev === li) {
              show(codePreview, false);
              return;
            }
            li.classList.add('detail-file--selected');
            var headerName = $('detail-code-preview-filename');
            var headerStats = $('detail-code-preview-stats');
            var content = $('detail-code-preview-content');
            if (headerName) headerName.textContent = f.file || '';
            if (headerName) headerName.title = f.file || '';
            if (headerStats) headerStats.textContent = (f.insertions ? '+' + f.insertions : '') + ' ' + (f.deletions ? '-' + f.deletions : '');
            if (content) {
              content.innerHTML = '<div class="detail-code-preview__loading">Loading\u2026</div>';
            }
            show(codePreview, true);
            (async function () {
              try {
                var diffRes = await call('git.fileDiff', { from: hash + '^', to: hash, file: f.file });
                var lines = (diffRes.diff || '').split('\n');
                var html = lines.map(function (line) {
                  var cls = (line.indexOf('+') === 0 && line.indexOf('+++') !== 0) ? 'diff-add'
                    : (line.indexOf('-') === 0 && line.indexOf('---') !== 0) ? 'diff-del'
                      : line.indexOf('@@') === 0 ? 'diff-hunk' : '';
                  return '<span class="diff-line ' + cls + '">' + escapeHtml(line) + '</span>';
                }).join('\n');
                if (content) content.innerHTML = '<pre class="detail-code-preview__diff">' + html + '</pre>';
              } catch (err) {
                if (content) content.innerHTML = '<div class="detail-code-preview__error">' + escapeHtml(err && err.message ? err.message : 'Failed to load diff') + '</div>';
              }
            })();
          });
          list.appendChild(li);
        });
      } else {
        if ($('detail-files-label')) $('detail-files-label').textContent = 'Changed Files (0)';
      }
    } catch (e) {
      if (summary) summary.innerHTML = '<div class="detail-error">' + escapeHtml(e && e.message ? e.message : e) + '</div>';
    }
  };

  window.__GG.closeDetail = function () {
    state.selectedHash = null;
    state.compareHashes = [];
    show($('detail-resizer'), false);
    show($('detail-panel'), false);
    window.__GG.renderCommitList();
  };

  window.__GG.initDetailResizer = function () {
    const resizer = $('detail-resizer');
    const panel = $('detail-panel');
    if (!resizer || !panel) return;
    var startX = 0;
    var startW = 0;
    var MIN_PANEL = 420;
    var MAX_PANEL = 720;
    resizer.addEventListener('mousedown', function (e) {
      e.preventDefault();
      startX = e.clientX;
      startW = panel.offsetWidth || Math.min(MAX_PANEL, Math.max(MIN_PANEL, Math.round(window.innerWidth * 0.36)));
      resizer.classList.add('dragging');
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
      function onMove(ev) {
        var delta = startX - ev.clientX;
        var mainW = (panel.parentElement && panel.parentElement.offsetWidth) || window.innerWidth;
        mainW -= 6;
        var maxPanelW = mainW - 80;
        var newW = Math.min(Math.max(MIN_PANEL, startW + delta), Math.min(MAX_PANEL, maxPanelW));
        panel.style.flexBasis = newW + 'px';
      }
      function onUp() {
        resizer.classList.remove('dragging');
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
      }
      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });
  };
})();
