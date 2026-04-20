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
