#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const ROOT_DIR = path.resolve(__dirname, '..');
const DIST_DIR = path.join(ROOT_DIR, 'dist', 'monaco-editor', 'vs');

const requiredFiles = [
  'loader.js',
  path.join('base', 'worker', 'workerMain.js'),
  path.join('language', 'json', 'jsonWorker.js'),
  path.join('language', 'html', 'htmlWorker.js'),
  path.join('language', 'css', 'cssWorker.js'),
  path.join('language', 'typescript', 'tsWorker.js'),
];

const missingFiles = requiredFiles.filter((relativePath) => {
  return !fs.existsSync(path.join(DIST_DIR, relativePath));
});

if (missingFiles.length > 0) {
  console.error('[verify-monaco-assets] Missing Monaco production assets:');
  for (const relativePath of missingFiles) {
    console.error(`  - dist/monaco-editor/vs/${relativePath.replace(/\\/g, '/')}`);
  }
  console.error(
    '[verify-monaco-assets] Build output is incomplete. Check the copy-monaco script and Monaco public asset layout.'
  );
  process.exit(1);
}

console.log('[verify-monaco-assets] Monaco production assets verified.');
