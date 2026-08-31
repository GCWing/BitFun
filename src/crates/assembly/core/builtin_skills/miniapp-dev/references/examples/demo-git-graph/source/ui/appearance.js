/**
 * Git Graph MiniApp — appearance adapter: read --branch-* and node stroke from CSS for graph colors.
 */
(function () {
  window.__GG = window.__GG || {};
  const root = document.documentElement;

  function getComputed(name) {
    return getComputedStyle(root).getPropertyValue(name).trim();
  }

  function requireComputed(name) {
    const value = getComputed(name);
    if (!value) {
      throw new Error('Git Graph requires the MiniApp appearance contract variable ' + name + '.');
    }
    return value;
  }

  /** Returns array of 7 branch/lane colors from CSS variables (appearance-aware). */
  window.__GG.getGraphColors = function () {
    const colors = [];
    for (let i = 1; i <= 7; i++) {
      colors.push(requireComputed('--branch-' + i));
    }
    return colors;
  };

  /** Node stroke color (contrast with background). */
  window.__GG.getNodeStroke = function () {
    return requireComputed('--graph-node-stroke');
  };

  /** Uncommitted / WIP line and node color. */
  window.__GG.getUncommittedColor = function () {
    return requireComputed('--graph-uncommitted');
  };
})();
