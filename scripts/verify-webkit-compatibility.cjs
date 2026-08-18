#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const ROOT_DIR = path.resolve(__dirname, '..');
const DEFAULT_DIST_DIR = path.join(ROOT_DIR, 'dist');
const FORBIDDEN_PATTERNS = [
  {
    label: 'remark-gfm variable-length email lookbehind',
    source: '(?<=^|\\s|\\p{P}|\\p{S})',
  },
];

function collectJavaScriptFiles(directory) {
  if (!fs.existsSync(directory)) {
    return [];
  }

  const files = [];
  const entries = fs.readdirSync(directory, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectJavaScriptFiles(entryPath));
    } else if (entry.isFile() && entry.name.endsWith('.js')) {
      files.push(entryPath);
    }
  }
  return files;
}

function findWebKitCompatibilityViolations(directory = DEFAULT_DIST_DIR) {
  const violations = [];
  for (const filePath of collectJavaScriptFiles(directory)) {
    const content = fs.readFileSync(filePath, 'utf8');
    for (const pattern of FORBIDDEN_PATTERNS) {
      if (content.includes(pattern.source)) {
        violations.push({ filePath, label: pattern.label });
      }
    }
  }
  return violations;
}

function main() {
  if (!fs.existsSync(DEFAULT_DIST_DIR)) {
    console.error('[verify-webkit-compatibility] Production output directory is missing.');
    process.exitCode = 1;
    return;
  }

  const violations = findWebKitCompatibilityViolations();
  if (violations.length === 0) {
    console.log(
      '[verify-webkit-compatibility] Known Markdown WebKit incompatibilities were not found.'
    );
    return;
  }

  console.error('[verify-webkit-compatibility] Unsupported JavaScript found:');
  for (const violation of violations) {
    console.error(
      `  - ${path.relative(ROOT_DIR, violation.filePath)}: ${violation.label}`
    );
  }
  process.exitCode = 1;
}

if (require.main === module) {
  main();
}

module.exports = { findWebKitCompatibilityViolations };
