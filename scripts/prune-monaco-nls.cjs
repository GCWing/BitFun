#!/usr/bin/env node

/**
 * Removes Monaco NLS language packs the app can never use.
 *
 * OpenBitFun only ships en-US / zh-CN / zh-TW locales (see index.html locale
 * resolution), but monaco-editor/min/vs carries 9 NLS packs. The 7 unreachable
 * ones are ~1.4 MB of dead weight that would otherwise end up inside dist/,
 * the desktop binary and the installers.
 * Runs right after copy-monaco. verify-monaco-assets.cjs asserts the pruned
 * files never come back.
 */

const fs = require('fs');
const path = require('path');

const ROOT_DIR = path.resolve(__dirname, '..');
const MONACO_VS_DIR = path.join(ROOT_DIR, 'src', 'web-ui', 'public', 'monaco-editor', 'vs');

// Keep: built-in English + zh-cn / zh-tw. Everything else is unreachable.
const PRUNED_NLS_LOCALES = ['ru', 'ja', 'ko', 'fr', 'it', 'es', 'de'];

function prunedNlsFileNames() {
  return PRUNED_NLS_LOCALES.map((locale) => `nls.messages.${locale}.js`);
}

function pruneMonacoNls({ log = console.log } = {}) {
  if (!fs.existsSync(MONACO_VS_DIR)) {
    log(`[prune-monaco-nls] skipped: ${path.relative(ROOT_DIR, MONACO_VS_DIR)} not found`);
    return 0;
  }

  let removed = 0;
  for (const fileName of prunedNlsFileNames()) {
    const filePath = path.join(MONACO_VS_DIR, fileName);
    if (fs.existsSync(filePath)) {
      fs.rmSync(filePath, { force: true });
      removed++;
    }
  }

  log(`[prune-monaco-nls] removed ${removed} unused Monaco NLS pack(s)`);
  return removed;
}

if (require.main === module) {
  pruneMonacoNls();
}

module.exports = {
  pruneMonacoNls,
  prunedNlsFileNames,
  PRUNED_NLS_LOCALES,
};
