// Bee Colony DAG Monitor — main logic
// Renders an 8-role vertical DAG with real-time status polling via app.storage.
// Vanilla JS, no frameworks. SVG rendered via createElementNS.

const SVG_NS = "http://www.w3.org/2000/svg";

// ── Role definitions ──────────────────────────────────────────
const ROLE_DEFS = [
  { id: "cmd",     role: "\u6307\u6325\u5b98",    tree: "\u6811 1-3 P\u2192C\u2192D",       gate: false },
  { id: "sec",     role: "\u79d8\u4e66 B01",     tree: "L3 \u5e93\u68c0\u7d22",             gate: false },
  { id: "pm",      role: "\u4ea7\u54c1\u7ecf\u7406",  tree: "R-ID \u9700\u6c42\u5b9a\u4e49",  gate: false },
  { id: "plan",    role: "\u89c4\u5212\u5e08",    tree: "\u6811 4-6 P\u2192D\u2192C",       gate: false },
  { id: "exec",    role: "\u6267\u884c\u8005",    tree: "\u6811 7-10 O\u2192O\u2192D\u2192A",  gate: false },
  { id: "review",  role: "\u5ba1\u67e5\u8005",    tree: "\u529f\u80fd\u00b7\u5b89\u5168\u00b7Debug",    gate: true  },
  { id: "accept",  role: "\u9a8c\u6536\u8005",    tree: "R-ID \u9010\u9879\u95ed\u5408",      gate: true  },
  { id: "opt",     role: "\u4f18\u5316\u8005",    tree: "\u590d\u76d8\u00b7\u5f52\u6863\u00b7\u77e5\u8bc6\u5e93",  gate: false },
];

// ── Layout constants ──────────────────────────────────────────
const NODE_W = 180;
const NODE_H = 52;
const NODE_RX = 8;
const V_GAP = 88;       // center-to-center vertical spacing
const PAD_X = 60;
const PAD_TOP = 30;
const PAD_BOT = 30;

// ── State ──────────────────────────────────────────────────────
const state = {
  layout: null,        // { viewW, viewH, positions: [{x,y}] }
  nodes: {},           // { cmd: {status, detail}, ... }
  lastHash: "",
};

// ── Simple hash ────────────────────────────────────────────────
function simpleHash(str) {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const c = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + c;
    hash |= 0;
  }
  return String(hash);
}

// ── Compute layout ─────────────────────────────────────────────
function computeLayout(totalW, totalH) {
  const cx = totalW / 2;
  const positions = ROLE_DEFS.map((_, i) => ({
    x: cx - NODE_W / 2,
    y: PAD_TOP + i * V_GAP,
  }));

  const viewH = PAD_TOP + ROLE_DEFS.length * V_GAP + PAD_BOT;

  return {
    viewW: totalW,
    viewH: viewH,
    positions: positions,
  };
}

// ── Status helpers ─────────────────────────────────────────────
function getStatus(nodeId) {
  const n = state.nodes[nodeId];
  return n && n.status ? n.status : "idle";
}

function getFillColor(status) {
  switch (status) {
    case "running": return "var(--dag-running)";
    case "done":    return "var(--dag-done)";
    case "failed":  return "var(--dag-failed)";
    default:        return "var(--dag-idle)";
  }
}

// ── Render ─────────────────────────────────────────────────────
function render() {
  const svg = document.getElementById("dag-svg");
  if (!svg) return;

  // Measure container
  const wrap = svg.parentElement;
  const w = wrap.clientWidth || 800;
  const h = wrap.clientHeight || 600;

  const layout = computeLayout(w, h);
  state.layout = layout;

  svg.setAttribute("viewBox", "0 0 " + layout.viewW + " " + layout.viewH);
  svg.setAttribute("preserveAspectRatio", "xMidYMid meet");

  // Clear
  while (svg.firstChild) svg.removeChild(svg.firstChild);

  // Defs: arrow marker
  const defs = createSvgEl("defs");
  const marker = createSvgEl("marker", {
    id: "arrow",
    viewBox: "0 0 10 10",
    refX: "5",
    refY: "10",
    markerWidth: "6",
    markerHeight: "6",
    orient: "auto",
  });
  const arrowPath = createSvgEl("path", {
    d: "M 0 0 L 5 10 L 10 0",
    fill: "rgba(255,255,255,0.16)",
  });
  marker.appendChild(arrowPath);
  defs.appendChild(marker);

  // Gate edge marker
  const markerGate = createSvgEl("marker", {
    id: "arrow-gate",
    viewBox: "0 0 10 10",
    refX: "5",
    refY: "10",
    markerWidth: "6",
    markerHeight: "6",
    orient: "auto",
  });
  const arrowPathGate = createSvgEl("path", {
    d: "M 0 0 L 5 10 L 10 0",
    fill: "rgba(239,68,68,0.40)",
  });
  markerGate.appendChild(arrowPathGate);
  defs.appendChild(markerGate);
  svg.appendChild(defs);

  // ── Edges (behind nodes) ──
  for (let i = 0; i < ROLE_DEFS.length - 1; i++) {
    const src = layout.positions[i];
    const dst = layout.positions[i + 1];
    const isGate = ROLE_DEFS[i + 1].gate || ROLE_DEFS[i].gate;

    const x1 = src.x + NODE_W / 2;
    const y1 = src.y + NODE_H;
    const x2 = dst.x + NODE_W / 2;
    const y2 = dst.y;

    const line = createSvgEl("line", {
      x1: x1, y1: y1,
      x2: x2, y2: y2,
      class: "dag-edge" + (isGate ? " dag-edge--gate" : ""),
      "marker-end": isGate ? "url(#arrow-gate)" : "url(#arrow)",
    });
    svg.appendChild(line);
  }

  // ── Nodes ──
  for (let i = 0; i < ROLE_DEFS.length; i++) {
    const def = ROLE_DEFS[i];
    const pos = layout.positions[i];
    const status = getStatus(def.id);

    const g = createSvgEl("g", {
      class: "dag-node" + (status === "running" ? " node--running" : ""),
    });

    // Node rect
    const rect = createSvgEl("rect", {
      x: pos.x,
      y: pos.y,
      width: NODE_W,
      height: NODE_H,
      rx: NODE_RX,
      ry: NODE_RX,
      class: "dag-node-rect",
      fill: getFillColor(status),
      stroke: "rgba(255,255,255,0.08)",
      "stroke-width": "1",
    });
    g.appendChild(rect);

    // Gate dashed overlay
    if (def.gate) {
      const dash = createSvgEl("rect", {
        x: pos.x + 1.5,
        y: pos.y + 1.5,
        width: NODE_W - 3,
        height: NODE_H - 3,
        rx: NODE_RX - 1,
        ry: NODE_RX - 1,
        class: "dag-gate-dash",
      });
      g.appendChild(dash);
    }

    // Role name
    const textRole = createSvgEl("text", {
      x: pos.x + NODE_W / 2,
      y: pos.y + 21,
      "text-anchor": "middle",
      class: "dag-node-text",
    });
    textRole.textContent = def.role;
    g.appendChild(textRole);

    // Tree number
    const textTree = createSvgEl("text", {
      x: pos.x + NODE_W / 2,
      y: pos.y + 38,
      "text-anchor": "middle",
      class: "dag-node-sub",
    });
    textTree.textContent = def.tree;
    g.appendChild(textTree);

    svg.appendChild(g);
  }
}

// ── SVG element helper ─────────────────────────────────────────
function createSvgEl(tag, attrs) {
  const el = document.createElementNS(SVG_NS, tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      el.setAttribute(k, String(v));
    }
  }
  return el;
}

// ── Badge update ───────────────────────────────────────────────
function updateBadge(raw) {
  const badge = document.getElementById("status-badge");
  if (!badge) return;

  const nodes = raw && raw.nodes ? raw.nodes : {};
  const vals = Object.values(nodes);
  const statuses = vals.map(function (n) { return n.status || "idle"; });

  let label = "\u5f85\u547d";
  let cls = "";

  if (statuses.length > 0) {
    const hasFailed = statuses.indexOf("failed") !== -1;
    const hasRunning = statuses.indexOf("running") !== -1;
    const allDone = statuses.every(function (s) { return s === "done"; });
    const allIdle = statuses.every(function (s) { return s === "idle"; });

    if (hasFailed) {
      label = "\u5f02\u5e38";
      cls = "topbar__badge--failed";
    } else if (hasRunning) {
      label = "\u6267\u884c\u4e2d";
      cls = "topbar__badge--running";
    } else if (allDone) {
      label = "\u5b8c\u6210";
      cls = "topbar__badge--done";
    } else if (allIdle) {
      label = "\u5f85\u547d";
      cls = "";
    }
  }

  badge.textContent = label;
  badge.className = "topbar__badge " + cls;
}

// ── Polling ────────────────────────────────────────────────────
async function pollState() {
  try {
    const raw = await app.storage.get("bee-colony-state");
    if (!raw) return;

    const hash = simpleHash(JSON.stringify(raw));
    if (hash === state.lastHash) return;
    state.lastHash = hash;

    // Update node statuses
    if (raw.nodes) {
      for (const [id, nodeState] of Object.entries(raw.nodes)) {
        state.nodes[id] = nodeState;
      }
    }

    render();
    updateBadge(raw);
  } catch (_e) {
    // Ignore polling errors — the MiniApp may be running
    // in a context where app.storage is not yet available.
  }
}

// ── Init ───────────────────────────────────────────────────────
async function init() {
  // Load any previously persisted state
  try {
    const saved = await app.storage.get("bee-colony-state");
    if (saved && saved.nodes) {
      for (const [id, nodeState] of Object.entries(saved.nodes)) {
        state.nodes[id] = nodeState;
      }
    }
    updateBadge(saved);
  } catch (_e) {
    // No saved state yet
  }

  // Initial render to show the DAG structure
  render();

  // Start polling
  setInterval(pollState, 500);
  pollState();

  // Update poll hint
  const hint = document.getElementById("poll-hint");
  if (hint) {
    hint.textContent = "\u8f6e\u8be2\u4e2d (500ms)";
  }

  // Handle resize
  window.addEventListener("resize", function () {
    render();
  });
}

// ── Boot ───────────────────────────────────────────────────────
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", init);
} else {
  init();
}
