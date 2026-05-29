#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

function runGit(args) {
  try {
    return execFileSync('git', args, { encoding: 'utf8' }).split(/\r?\n/).filter(Boolean);
  } catch {
    return [];
  }
}

function uniqueFiles(files) {
  return [...new Set(files.filter(Boolean))];
}

function hasCommit(ref) {
  try {
    execFileSync('git', ['rev-parse', '--verify', `${ref}^{commit}`], { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

const trackedFiles = runGit(['ls-files']);
const untrackedFiles = runGit(['ls-files', '--others', '--exclude-standard']);
const repositoryFiles = uniqueFiles([...trackedFiles, ...untrackedFiles]);
const localChangedFiles = uniqueFiles([
  ...runGit(['diff', '--name-only', '--diff-filter=ACMRT', 'HEAD']),
  ...untrackedFiles,
]);
const committedChangedFiles = hasCommit('HEAD^1')
  ? runGit(['diff', '--name-only', '--diff-filter=ACMRT', 'HEAD^1', 'HEAD'])
  : [];
const contentScanFiles = uniqueFiles(
  localChangedFiles.length > 0
    ? localChangedFiles
    : committedChangedFiles.length > 0
      ? committedChangedFiles
      : trackedFiles,
);
const contentScanFileSet = new Set(contentScanFiles.map(normalizePath));

const textExtensions = new Set([
  '.cjs',
  '.css',
  '.html',
  '.js',
  '.json',
  '.jsx',
  '.md',
  '.mjs',
  '.rs',
  '.scss',
  '.toml',
  '.ts',
  '.tsx',
  '.txt',
  '.yaml',
  '.yml',
]);

const ignoredContentPaths = [
  /(^|\/)node_modules\//,
  /(^|\/)dist\//,
  /(^|\/)target\//,
  /(^|\/)src\/web-ui\/public\/monaco-editor\//,
  /(^|\/)src\/mobile-web\/dist\//,
  /(^|\/).*package-lock\.json$/,
  /(^|\/)pnpm-lock\.yaml$/,
  /(^|\/)Cargo\.lock$/,
];

const testFilePattern = /(^|\/)(tests?|__tests__)\/|[._-](test|spec)\.[cm]?[jt]sx?$|_tests?\.rs$|\/tests\.rs$/;
const temporaryPromptNames = new Set([
  '_codex_review_prompt.txt',
  'codex_review_prompt.txt',
  'review_prompt.txt',
]);
const sensitiveFilenamePattern =
  /(^|[._-])(id_rsa|id_dsa|id_ecdsa|id_ed25519)([._-]|$)|\.(pem|p12|pfx|mobileprovision)$/i;
const localAbsolutePathPattern =
  /(^|[^A-Za-z])((?:[A-Za-z]:[\\/][^\s'"`)<\]]+)|(?:file:\/\/\/[A-Za-z]:\/[^\s'"`)<\]]+))/g;
const tokenPattern =
  /\b(?:gh[pousr]_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|xox[baprs]-[A-Za-z0-9-]{20,})\b/g;
const privateKeyPattern = /-----BEGIN (?:RSA |DSA |EC |OPENSSH |)?PRIVATE KEY-----/;

const violations = [];

function normalizePath(file) {
  return file.replace(/\\/g, '/');
}

function shouldScanText(file) {
  const normalized = normalizePath(file);
  const ext = path.extname(normalized).toLowerCase();
  return textExtensions.has(ext) && !ignoredContentPaths.some((pattern) => pattern.test(normalized));
}

function addViolation(file, line, message) {
  violations.push(line ? `${file}:${line} ${message}` : `${file} ${message}`);
}

for (const file of repositoryFiles) {
  const normalized = normalizePath(file);
  const basename = path.posix.basename(normalized).toLowerCase();

  if (
    temporaryPromptNames.has(basename) ||
    /(^|[-_])review[-_]?prompt\.(txt|md)$/i.test(basename)
  ) {
    addViolation(file, null, 'looks like a transient review prompt file.');
  }

  if (sensitiveFilenamePattern.test(basename)) {
    addViolation(file, null, 'looks like a private key, certificate, or provisioning file.');
  }

  if (!contentScanFileSet.has(normalized) || !shouldScanText(file)) {
    continue;
  }

  let content;
  try {
    content = readFileSync(file, 'utf8');
  } catch {
    continue;
  }

  const isTestFile = testFilePattern.test(normalized);
  const scanLocalPaths = !isTestFile;
  const scanTokenLikeSecrets = !isTestFile;
  const lines = content.split(/\r?\n/);

  for (const [index, line] of lines.entries()) {
    const lineNumber = index + 1;

    if (privateKeyPattern.test(line)) {
      addViolation(file, lineNumber, 'contains a private key marker.');
    }

    if (scanTokenLikeSecrets && tokenPattern.test(line)) {
      addViolation(file, lineNumber, 'contains a token-like secret.');
    }

    if (scanLocalPaths && localAbsolutePathPattern.test(line)) {
      addViolation(file, lineNumber, 'contains a local absolute path.');
    }

    localAbsolutePathPattern.lastIndex = 0;
    tokenPattern.lastIndex = 0;
  }
}

if (violations.length > 0) {
  console.error('Repository hygiene check failed:');
  for (const violation of violations) {
    console.error(`- ${violation}`);
  }
  process.exit(1);
}

console.log(
  `Repository hygiene check passed (${contentScanFiles.length} content files scanned, ${repositoryFiles.length} filenames checked).`,
);
