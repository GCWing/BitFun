#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { prunedNlsFileNames } = require('./prune-monaco-nls.cjs');

const ROOT_DIR = path.resolve(__dirname, '..');
const DIST_DIR = path.join(ROOT_DIR, 'dist', 'monaco-editor', 'vs');

const requiredFiles = [
  'loader.js',
  path.join('base', 'worker', 'workerMain.js'),
  path.join('language', 'json', 'jsonWorker.js'),
  path.join('language', 'html', 'htmlWorker.js'),
  path.join('language', 'css', 'cssWorker.js'),
  path.join('language', 'typescript', 'tsWorker.js'),
  // Supported non-English locales must survive the NLS pruning step.
  'nls.messages.zh-cn.js',
  'nls.messages.zh-tw.js',
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

// The app only ships en-US / zh-CN / zh-TW; the other NLS packs are pruned by
// scripts/prune-monaco-nls.cjs and must never reappear in dist (regression
// guard for ~1.4 MB of dead weight in every installer).
const forbiddenNlsFiles = prunedNlsFileNames().filter((fileName) => {
  return fs.existsSync(path.join(DIST_DIR, fileName));
});

if (forbiddenNlsFiles.length > 0) {
  console.error('[verify-monaco-assets] Unused Monaco NLS packs found in dist (should be pruned):');
  for (const fileName of forbiddenNlsFiles) {
    console.error(`  - dist/monaco-editor/vs/${fileName}`);
  }
  console.error(
    '[verify-monaco-assets] Run `pnpm run copy-monaco` (which invokes scripts/prune-monaco-nls.cjs) before building.'
  );
  process.exit(1);
}

console.log('[verify-monaco-assets] Monaco production assets verified.');
