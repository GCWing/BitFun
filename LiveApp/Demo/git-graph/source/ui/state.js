/**
 * Git Graph Live App — shared state, constants, DOM helpers.
 */
(function () {
  window.__GG = window.__GG || {};

  window.__GG.STORAGE_KEY = 'lastRepo';
  window.__GG.MAX_COMMITS = 300;
  window.__GG.ROW_H = 28;
  window.__GG.LANE_W = 18;
  window.__GG.NODE_R = 4;

  window.__GG.$ = function (id) {
    return document.getElementById(id);
  };

  window.__GG.state = {
    cwd: null,
    commits: [],
    stash: [],
    branches: null,
    refs: null,
    head: null,
    uncommitted: null,
    status: null,
    remotes: [],
    selectedHash: null,
    selectedBranchFilter: [],
    firstParent: false,
    order: 'date',
    compareHashes: [],
    findQuery: '',
    findIndex: 0,
    findMatches: [],
    offset: 0,
    hasMore: true,
  };

  window.__GG.show = function (el, v) {
    if (el) el.style.display = v ? '' : 'none';
  };

  window.__GG.formatDate = function (dateStr) {
    if (!dateStr) return '';
    try {
      const d = new Date(dateStr);
      const mm = String(d.getMonth() + 1).padStart(2, '0');
      const dd = String(d.getDate()).padStart(2, '0');
      const hh = String(d.getHours()).padStart(2, '0');
      const mi = String(d.getMinutes()).padStart(2, '0');
      return `${mm}-${dd} ${hh}:${mi}`;
    } catch {
      return String(dateStr).slice(0, 10);
    }
  };

  window.__GG.parseRefs = function (refStr) {
    if (!refStr) return [];
    return refStr
      .split(',')
      .map(function (r) { return r.trim(); })
      .filter(Boolean)
      .map(function (r) {
        if (r.startsWith('HEAD -> ')) return { type: 'head', label: r.replace('HEAD -> ', '') };
        if (r.startsWith('tag: ')) return { type: 'tag', label: r.replace('tag: ', '') };
        if (r.includes('/')) return { type: 'remote', label: r };
        return { type: 'branch', label: r };
      });
  };

  window.__GG.setLoading = function (v) {
    window.__GG.show(window.__GG.$('loading-overlay'), v);
  };

  window.__GG.escapeHtml = function (s) {
    if (s == null) return '';
    const div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
  };

  /**
   * Returns display list from state.commits (already built by git.graphData:
   * uncommitted + commits with stash rows in correct order). No client-side
   * stash-by-date or status-based uncommitted fabrication.
   */
  window.__GG.getDisplayCommits = function () {
    return (window.__GG.state.commits || []).slice();
  };

  /**
   * Build ref tag list for a commit from structured refs (heads/tags/remotes).
   * currentBranch: name of current branch for HEAD -> label.
   */
  window.__GG.getRefsFromStructured = function (commit, currentBranch) {
    if (!commit) return [];
    const out = [];
    const heads = commit.heads || [];
    const tags = commit.tags || [];
    const remotes = commit.remotes || [];
    heads.forEach(function (name) {
      out.push({ type: name === currentBranch ? 'head' : 'branch', label: name === currentBranch ? 'HEAD -> ' + name : name });
    });
    tags.forEach(function (t) {
      out.push({ type: 'tag', label: typeof t === 'string' ? t : (t.name || '') });
    });
    remotes.forEach(function (r) {
      out.push({ type: 'remote', label: typeof r === 'string' ? r : (r.name || '') });
    });
    return out;
  };
})();

