/**
 * Git Graph Live App — build: concatenate source/ui/*.js → ui.js, source/styles/*.css → style.css.
 * Run from MiniApp/Demo/git-graph (or your liveapps/{app_id} folder): node source/build.js
 */
const fs = require('fs');
const path = require('path');

const SOURCE_DIR = path.join(__dirname);
const ROOT = path.dirname(SOURCE_DIR);

const UI_ORDER = [
  'ui/state.js',
  'ui/theme.js',
  'ui/graph/layout.js',
  'ui/graph/renderRowSvg.js',
  'ui/services/gitClient.js',
  'ui/components/contextMenu.js',
  'ui/components/modal.js',
  'ui/components/findWidget.js',
  'ui/panels/remotePanel.js',
  'ui/panels/detailPanel.js',
  'ui/main.js',
  'ui/bootstrap.js',
];

const STYLES_ORDER = [
  'styles/tokens.css',
  'styles/layout.css',
  'styles/graph.css',
  'styles/detail-panel.css',
  'styles/overlay.css',
];

function concat(files, dir) {
  let out = '';
  for (const f of files) {
    const full = path.join(dir, f);
    if (!fs.existsSync(full)) {
      console.warn('Missing:', full);
      continue;
    }
    out += '/* ' + f + ' */\n' + fs.readFileSync(full, 'utf8') + '\n';
  }
  return out;
}

const uiOut = path.join(SOURCE_DIR, 'ui.js');
const styleOut = path.join(SOURCE_DIR, 'style.css');

fs.writeFileSync(uiOut, concat(UI_ORDER, SOURCE_DIR), 'utf8');
fs.writeFileSync(styleOut, concat(STYLES_ORDER, SOURCE_DIR), 'utf8');

console.log('Built', uiOut, 'and', styleOut);
