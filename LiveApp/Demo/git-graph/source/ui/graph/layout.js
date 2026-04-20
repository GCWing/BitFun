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
