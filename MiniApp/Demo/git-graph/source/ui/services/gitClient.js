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
