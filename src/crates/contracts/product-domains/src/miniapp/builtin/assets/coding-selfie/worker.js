// Daily Coding Snapshot — no Node Worker.
//
// As of v17 this MiniApp runs with `permissions.node.enabled = false`. All git
// scanning is performed in `ui.js` via `app.shell.exec("git ...")`, which the
// host serves directly without spawning Bun/Node. This file is intentionally
// empty (the framework still seeds it for shape compatibility with other apps).
module.exports = {};
