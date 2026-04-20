/* ui/state.js */
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


/* ui/theme.js */
/**
 * Git Graph Live App — theme adapter: read --branch-* and node stroke from CSS for graph colors.
 */
(function () {
  window.__GG = window.__GG || {};
  const root = document.documentElement;

  function getComputed(name) {
    return getComputedStyle(root).getPropertyValue(name).trim() || null;
  }

  /** Returns array of 7 branch/lane colors from CSS variables (theme-aware). */
  window.__GG.getGraphColors = function () {
    const colors = [];
    for (let i = 1; i <= 7; i++) {
      const v = getComputed('--branch-' + i);
      colors.push(v || '#58a6ff');
    }
    return colors;
  };

  /** Node stroke color (contrast with background). */
  window.__GG.getNodeStroke = function () {
    return getComputed('--graph-node-stroke') || getComputed('--bitfun-bg') || getComputed('--bg') || '#0d1117';
  };

  /** Uncommitted / WIP line and node color. */
  window.__GG.getUncommittedColor = function () {
    return getComputed('--graph-uncommitted') || getComputed('--text-dim') || '#808080';
  };
})();

/* ui/graph/layout.js */
/**
 * Git Graph Live App — global topology graph layout (Vertex/Branch/determinePath).
 * Outputs per-row drawInfo compatible with renderRowSvg: { lane, lanesBefore, parentLanes }.
 */
(function () {
  window.__GG = window.__GG || {};
  const NULL_VERTEX_ID = -1;

  function Vertex(id, isStash) {
    this.id = id;
    this.isStash = !!isStash;
    this.x = 0;
    this.children = [];
    this.parents = [];
    this.nextParent = 0;
    this.onBranch = null;
    this.isCommitted = true;
    this.nextX = 0;
    this.connections = [];
  }
  Vertex.prototype.addChild = function (v) { this.children.push(v); };
  Vertex.prototype.addParent = function (v) { this.parents.push(v); };
  Vertex.prototype.getNextParent = function () {
    return this.nextParent < this.parents.length ? this.parents[this.nextParent] : null;
  };
  Vertex.prototype.registerParentProcessed = function () { this.nextParent++; };
  Vertex.prototype.isNotOnBranch = function () { return this.onBranch === null; };
  Vertex.prototype.getPoint = function () { return { x: this.x, y: this.id }; };
  Vertex.prototype.getNextPoint = function () { return { x: this.nextX, y: this.id }; };
  Vertex.prototype.getPointConnectingTo = function (vertex, onBranch) {
    for (let i = 0; i < this.connections.length; i++) {
      if (this.connections[i] && this.connections[i].connectsTo === vertex && this.connections[i].onBranch === onBranch) {
        return { x: i, y: this.id };
      }
    }
    return null;
  };
  Vertex.prototype.registerUnavailablePoint = function (x, connectsToVertex, onBranch) {
    if (x === this.nextX) {
      this.nextX = x + 1;
      while (this.connections.length <= x) this.connections.push(null);
      this.connections[x] = { connectsTo: connectsToVertex, onBranch: onBranch };
    }
  };
  Vertex.prototype.addToBranch = function (branch, x) {
    if (this.onBranch === null) {
      this.onBranch = branch;
      this.x = x;
    }
  };
  Vertex.prototype.getBranch = function () { return this.onBranch; };
  Vertex.prototype.getIsCommitted = function () { return this.isCommitted; };
  Vertex.prototype.setNotCommitted = function () { this.isCommitted = false; };
  Vertex.prototype.isMerge = function () { return this.parents.length > 1; };

  function Branch(colour) {
    this.colour = colour;
    this.lines = [];
  }
  Branch.prototype.getColour = function () { return this.colour; };
  Branch.prototype.addLine = function (p1, p2, isCommitted, lockedFirst) {
    this.lines.push({ p1: p1, p2: p2, lockedFirst: lockedFirst });
  };

  function getAvailableColour(availableColours, startAt) {
    for (let i = 0; i < availableColours.length; i++) {
      if (startAt > availableColours[i]) return i;
    }
    availableColours.push(0);
    return availableColours.length - 1;
  }

  function determinePath(vertices, branches, availableColours, commits, commitLookup, onlyFollowFirstParent) {
    function run(startAt) {
      let i = startAt;
      let vertex = vertices[i];
      let parentVertex = vertex.getNextParent();
      let lastPoint = vertex.isNotOnBranch() ? vertex.getNextPoint() : vertex.getPoint();

      if (parentVertex !== null && parentVertex.id !== NULL_VERTEX_ID && vertex.isMerge() && !vertex.isNotOnBranch() && !parentVertex.isNotOnBranch()) {
        var parentBranch = parentVertex.getBranch();
        var foundPointToParent = false;
        for (i = startAt + 1; i < vertices.length; i++) {
          var curVertex = vertices[i];
          var curPoint = curVertex.getPointConnectingTo(parentVertex, parentBranch);
          if (curPoint === null) curPoint = curVertex.getNextPoint();
          parentBranch.addLine(lastPoint, curPoint, vertex.getIsCommitted(), !foundPointToParent && curVertex !== parentVertex ? lastPoint.x < curPoint.x : true);
          curVertex.registerUnavailablePoint(curPoint.x, parentVertex, parentBranch);
          lastPoint = curPoint;
          if (curVertex.getPointConnectingTo(parentVertex, parentBranch) !== null) foundPointToParent = true;
          if (foundPointToParent) {
            vertex.registerParentProcessed();
            return;
          }
        }
      } else {
        var branch = new Branch(getAvailableColour(availableColours, startAt));
        vertex.addToBranch(branch, lastPoint.x);
        vertex.registerUnavailablePoint(lastPoint.x, vertex, branch);
        for (i = startAt + 1; i < vertices.length; i++) {
          var curVertex = vertices[i];
          var curPoint = (parentVertex === curVertex && parentVertex && !parentVertex.isNotOnBranch()) ? curVertex.getPoint() : curVertex.getNextPoint();
          branch.addLine(lastPoint, curPoint, vertex.getIsCommitted(), lastPoint.x < curPoint.x);
          curVertex.registerUnavailablePoint(curPoint.x, parentVertex, branch);
          lastPoint = curPoint;
          if (parentVertex === curVertex) {
            vertex.registerParentProcessed();
            var parentVertexOnBranch = parentVertex && !parentVertex.isNotOnBranch();
            parentVertex.addToBranch(branch, curPoint.x);
            vertex = parentVertex;
            parentVertex = vertex.getNextParent();
            if (parentVertex === null || parentVertexOnBranch) return;
          }
        }
        if (i === vertices.length && parentVertex !== null && parentVertex.id === NULL_VERTEX_ID) {
          vertex.registerParentProcessed();
        }
        branches.push(branch);
        availableColours[branch.getColour()] = i;
      }
    }

    var idx = 0;
    while (idx < vertices.length) {
      var v = vertices[idx];
      if (v.getNextParent() !== null || v.isNotOnBranch()) {
        run(idx);
      } else {
        idx++;
      }
    }
  }

  function computeFallbackLayout(commits) {
    const idx = {};
    commits.forEach(function (c, i) { idx[c.hash] = i; });

    const commitLane = new Array(commits.length);
    const rowDrawInfo = [];
    const activeLanes = [];
    let maxLane = 0;

    for (let i = 0; i < commits.length; i++) {
      const c = commits[i];
      const lanesBefore = activeLanes.slice();

      let lane = lanesBefore.indexOf(c.hash);
      if (lane === -1) {
        lane = activeLanes.indexOf(null);
        if (lane === -1) {
          lane = activeLanes.length;
          activeLanes.push(null);
        }
      }

      commitLane[i] = lane;
      while (activeLanes.length <= lane) activeLanes.push(null);
      activeLanes[lane] = null;

      const raw = c.parentHashes || c.parents || (c.parent != null ? [c.parent] : []);
      const parents = Array.isArray(raw) ? raw : [raw];
      const parentLanes = [];
      for (let p = 0; p < parents.length; p++) {
        const ph = parents[p];
        if (idx[ph] === undefined) continue;

        const existing = activeLanes.indexOf(ph);
        if (existing >= 0) {
          parentLanes.push({ lane: existing });
        } else if (p === 0) {
          activeLanes[lane] = ph;
          parentLanes.push({ lane: lane });
        } else {
          let sl = activeLanes.indexOf(null);
          if (sl === -1) {
            sl = activeLanes.length;
            activeLanes.push(null);
          }
          activeLanes[sl] = ph;
          parentLanes.push({ lane: sl });
        }
      }

      maxLane = Math.max(
        maxLane,
        lane,
        parentLanes.length ? Math.max.apply(null, parentLanes.map(function (pl) { return pl.lane; })) : 0
      );
      rowDrawInfo.push({ lane: lane, lanesBefore: lanesBefore, parentLanes: parentLanes });
    }

    return { commitLane: commitLane, laneCount: maxLane + 1, idx: idx, rowDrawInfo: rowDrawInfo };
  }

  function isReasonableLayout(layout, commitCount) {
    if (!layout || !Array.isArray(layout.rowDrawInfo) || layout.rowDrawInfo.length !== commitCount) return false;
    if (!Number.isFinite(layout.laneCount) || layout.laneCount < 1) return false;

    for (let i = 0; i < layout.rowDrawInfo.length; i++) {
      const row = layout.rowDrawInfo[i];
      if (!row || !Number.isFinite(row.lane) || row.lane < 0) return false;
      if (!Array.isArray(row.parentLanes) || !Array.isArray(row.lanesBefore)) return false;
      for (let j = 0; j < row.parentLanes.length; j++) {
        if (!Number.isFinite(row.parentLanes[j].lane) || row.parentLanes[j].lane < 0) return false;
      }
    }

    // If almost every row gets its own lane, the topology solver likely drifted.
    if (commitCount >= 12 && layout.laneCount > Math.ceil(commitCount * 0.5)) return false;
    return true;
  }

  /**
   * Compute per-row graph layout using global topology (Vertex/Branch/determinePath).
   * commits: array of { hash, parentHashes, stash } (parentHashes = array of hash strings).
   * onlyFollowFirstParent: optional boolean (default false).
   * Returns { commitLane, laneCount, idx, rowDrawInfo } for use by renderRowSvg.
   */
  window.__GG.computeGraphLayout = function (commits, onlyFollowFirstParent) {
    onlyFollowFirstParent = !!onlyFollowFirstParent;
    const idx = {};
    commits.forEach(function (c, i) { idx[c.hash] = i; });
    const n = commits.length;
    if (n === 0) return { commitLane: [], laneCount: 1, idx: idx, rowDrawInfo: [] };

    const nullVertex = new Vertex(NULL_VERTEX_ID, false);
    const vertices = [];
    for (let i = 0; i < n; i++) {
      vertices.push(new Vertex(i, !!(commits[i].stash)));
    }
    for (let i = 0; i < n; i++) {
      const raw = commits[i].parentHashes || commits[i].parents || (commits[i].parent != null ? [commits[i].parent] : []);
      const parents = Array.isArray(raw) ? raw : [raw];
      for (let p = 0; p < parents.length; p++) {
        const ph = parents[p];
        if (typeof idx[ph] === 'number') {
          vertices[i].addParent(vertices[idx[ph]]);
          vertices[idx[ph]].addChild(vertices[i]);
        } else if (!onlyFollowFirstParent || p === 0) {
          vertices[i].addParent(nullVertex);
        }
      }
    }
    if ((commits[0] && (commits[0].hash === '__uncommitted__' || commits[0].isUncommitted))) {
      vertices[0].setNotCommitted();
    }
    const branches = [];
    const availableColours = [];
    determinePath(vertices, branches, availableColours, commits, idx, onlyFollowFirstParent);

    const commitLane = [];
    const rowDrawInfo = [];
    let maxLane = 0;
    const activeLanes = [];
    for (let i = 0; i < n; i++) {
      const v = vertices[i];
      const lane = v.x;
      maxLane = Math.max(maxLane, lane);
      commitLane[i] = lane;
      const lanesBefore = activeLanes.slice();
      while (activeLanes.length <= lane) activeLanes.push(null);
      activeLanes[lane] = null;
      const parentLanes = [];
      const parents = v.parents;
      for (let p = 0; p < parents.length; p++) {
        const pv = parents[p];
        if (pv.id === NULL_VERTEX_ID) continue;
        const pl = pv.x;
        parentLanes.push({ lane: pl });
        maxLane = Math.max(maxLane, pl);
        while (activeLanes.length <= pl) activeLanes.push(null);
        activeLanes[pl] = commits[pv.id].hash;
      }
      rowDrawInfo.push({ lane: lane, lanesBefore: lanesBefore, parentLanes: parentLanes });
    }
    const result = {
      commitLane: commitLane,
      laneCount: maxLane + 1,
      idx: idx,
      rowDrawInfo: rowDrawInfo,
    };
    return isReasonableLayout(result, n) ? result : computeFallbackLayout(commits);
  };
})();

/* ui/graph/renderRowSvg.js */
/**
 * Git Graph Live App — build SVG for one commit row (theme-aware colors).
 */
(function () {
  window.__GG = window.__GG || {};
  const ROW_H = window.__GG.ROW_H;
  const LANE_W = window.__GG.LANE_W;
  const NODE_R = window.__GG.NODE_R;

  window.__GG.buildRowSvg = function (commit, drawInfo, graphW, isStash, isUncommitted) {
    isStash = !!isStash;
    isUncommitted = !!isUncommitted;
    const lane = drawInfo.lane;
    const lanesBefore = drawInfo.lanesBefore;
    const parentLanes = drawInfo.parentLanes;
    const colors = window.__GG.getGraphColors();
    const nodeStroke = window.__GG.getNodeStroke();
    const uncommittedColor = window.__GG.getUncommittedColor();

    const svgNS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(svgNS, 'svg');
    svg.setAttribute('width', graphW);
    svg.setAttribute('height', ROW_H);
    svg.setAttribute('viewBox', '0 0 ' + graphW + ' ' + ROW_H);
    svg.style.display = 'block';
    svg.style.overflow = 'visible';

    const cx = lane * LANE_W + LANE_W / 2 + 4;
    const cy = ROW_H / 2;
    const nodeColor = isUncommitted ? uncommittedColor : (colors[lane % colors.length] || colors[0]);
    const bezierD = ROW_H * 0.8;

    function laneX(l) { return l * LANE_W + LANE_W / 2 + 4; }

    function mkPath(dAttr, stroke, dash) {
      const p = document.createElementNS(svgNS, 'path');
      p.setAttribute('d', dAttr);
      p.setAttribute('stroke', stroke);
      p.setAttribute('fill', 'none');
      p.setAttribute('stroke-width', '1.5');
      p.setAttribute('class', 'graph-line');
      if (dash) p.setAttribute('stroke-dasharray', dash);
      return p;
    }

    for (let l = 0; l < lanesBefore.length; l++) {
      if (lanesBefore[l] !== null && l !== lane) {
        const stroke = colors[l % colors.length] || colors[0];
        svg.appendChild(mkPath('M' + laneX(l) + ' 0 L' + laneX(l) + ' ' + ROW_H, stroke, null));
      }
    }

    const wasActive = lane < lanesBefore.length && lanesBefore[lane] !== null;
    if (wasActive) {
      const path = mkPath('M' + cx + ' 0 L' + cx + ' ' + cy, nodeColor, isUncommitted ? '3 3' : null);
      if (isUncommitted) path.setAttribute('class', 'graph-line graph-line--uncommitted');
      svg.appendChild(path);
    }

    for (let i = 0; i < parentLanes.length; i++) {
      const pl = parentLanes[i];
      const px = laneX(pl.lane);
      const lineColor = isUncommitted ? uncommittedColor : (colors[pl.lane % colors.length] || colors[0]);
      const dash = isUncommitted ? '3 3' : null;
      var path;
      if (px === cx) {
        path = mkPath('M' + cx + ' ' + cy + ' L' + cx + ' ' + ROW_H, lineColor, dash);
      } else {
        path = mkPath(
          'M' + cx + ' ' + cy + ' C' + cx + ' ' + (cy + bezierD) + ' ' + px + ' ' + (ROW_H - bezierD) + ' ' + px + ' ' + ROW_H,
          lineColor, dash
        );
      }
      if (isUncommitted) path.setAttribute('class', 'graph-line graph-line--uncommitted');
      svg.appendChild(path);
    }

    if (isStash) {
      const outer = document.createElementNS(svgNS, 'circle');
      outer.setAttribute('cx', cx); outer.setAttribute('cy', cy); outer.setAttribute('r', 4.5);
      outer.setAttribute('fill', 'none'); outer.setAttribute('stroke', nodeColor); outer.setAttribute('stroke-width', '1.5');
      outer.setAttribute('class', 'graph-node graph-node--stash-outer');
      svg.appendChild(outer);
      const inner = document.createElementNS(svgNS, 'circle');
      inner.setAttribute('cx', cx); inner.setAttribute('cy', cy); inner.setAttribute('r', 2);
      inner.setAttribute('fill', nodeColor);
      inner.setAttribute('class', 'graph-node graph-node--stash-inner');
      svg.appendChild(inner);
    } else if (isUncommitted) {
      const circle = document.createElementNS(svgNS, 'circle');
      circle.setAttribute('cx', cx); circle.setAttribute('cy', cy); circle.setAttribute('r', NODE_R);
      circle.setAttribute('fill', 'none'); circle.setAttribute('stroke', uncommittedColor); circle.setAttribute('stroke-width', '1.5');
      circle.setAttribute('class', 'graph-node graph-node--uncommitted');
      svg.appendChild(circle);
    } else {
      const circle = document.createElementNS(svgNS, 'circle');
      circle.setAttribute('cx', cx); circle.setAttribute('cy', cy); circle.setAttribute('r', NODE_R);
      circle.setAttribute('fill', nodeColor); circle.setAttribute('stroke', nodeStroke); circle.setAttribute('stroke-width', '1.5');
      circle.setAttribute('class', 'graph-node graph-node--commit');
      svg.appendChild(circle);
    }

    return svg;
  };
})();

/* ui/services/gitClient.js */
/**
 * Git Graph Live App — worker call wrapper.
 */
(function () {
  window.__GG = window.__GG || {};

  window.__GG.call = function (method, params) {
    const state = window.__GG.state;
    const p = Object.assign({ cwd: state.cwd }, params || {});
    return window.app.call(method, p);
  };
})();

/* ui/components/contextMenu.js */
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

/* ui/components/modal.js */
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

/* ui/components/findWidget.js */
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

/* ui/panels/remotePanel.js */
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

/* ui/panels/detailPanel.js */
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

/* ui/main.js */
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

/* ui/bootstrap.js */
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

