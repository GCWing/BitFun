#!/usr/bin/env node

/**
 * Repository hygiene guardrails for tracked and untracked workspace files.
 *
 * Current scanning rules:
 * - Always check filenames for transient review prompts and sensitive key/cert names.
 * - Scan changed text files for private key markers and token-like secrets.
 * - Scan changed text files for local absolute paths that look workspace- or user-specific:
 *   Windows drive paths under folders such as Users, workspace, Projects, code, dev, tmp;
 *   file:// Windows paths under those folders; and Unix paths under /Users or /home.
 * - Skip generated and dependency outputs such as node_modules, dist, target, Monaco assets,
 *   mobile-web dist, relay static assets, and lockfiles.
 * - Skip local-path and token checks in recognized test files; also skip local-path checks for
 *   comment-only lines and Rust inline test blocks inside non-test source files.
 */
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
  /(^|\/)src\/apps\/relay-server\/static\/assets\//,
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
const windowsLocalPathSegment = String.raw`[A-Za-z]:[\\/](?:Users|Documents and Settings|workspace|workspaces|work|Projects|code|dev|repos?|src|tmp|temp)(?:[\\/][^\s'"\`)<\]]+)*`;
const localAbsolutePathPattern =
  new RegExp(
    String.raw`(^|[^A-Za-z])((?:${windowsLocalPathSegment})|(?:file:\/\/\/(?:Users|Documents and Settings|workspace|workspaces|work|Projects|code|dev|repos?|src|tmp|temp)(?:\/[^\s'"\`)<\]]+)*)|(?:file:\/\/\/[A-Za-z]:\/(?:Users|Documents and Settings|workspace|workspaces|work|Projects|code|dev|repos?|src|tmp|temp)(?:\/[^\s'"\`)<\]]+)*)|(?:\/(?:Users|home)\/[^\s'"\`)<\]]+))`,
    'g',
  );
const tokenPattern =
  /\b(?:gh[pousr]_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|xox[baprs]-[A-Za-z0-9-]{20,})\b/g;
const privateKeyPattern = /-----BEGIN (?:RSA |DSA |EC |OPENSSH |)?PRIVATE KEY-----/;
const slashCommentExtensions = new Set([
  '.cjs',
  '.css',
  '.js',
  '.jsx',
  '.mjs',
  '.rs',
  '.scss',
  '.ts',
  '.tsx',
]);
const hashCommentExtensions = new Set(['.toml', '.yaml', '.yml']);

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

function countMatches(line, pattern) {
  return (line.match(pattern) || []).length;
}

function isCommentOnlyLine(line, ext) {
  const trimmed = line.trim();

  if (trimmed.length === 0) {
    return false;
  }

  if (slashCommentExtensions.has(ext)) {
    return (
      trimmed.startsWith('//') ||
      trimmed.startsWith('/*') ||
      trimmed.startsWith('*') ||
      trimmed.startsWith('*/')
    );
  }

  if (hashCommentExtensions.has(ext)) {
    return trimmed.startsWith('#');
  }

  return false;
}

function getRustInlineTestSkipLines(lines) {
  const skipLines = new Array(lines.length).fill(false);
  let braceDepth = 0;
  let pendingCfgTestModule = false;
  let pendingTestFunction = false;
  const activeBlocks = [];

  for (const [index, line] of lines.entries()) {
    const trimmed = line.trim();

    if (activeBlocks.length > 0 || pendingCfgTestModule || pendingTestFunction) {
      skipLines[index] = true;
    }

    if (/^#\[\s*cfg\s*\(\s*test\s*\)\s*\]$/.test(trimmed)) {
      pendingCfgTestModule = true;
      skipLines[index] = true;
    }

    if (/^#\[\s*(?:[A-Za-z0-9_]+::)*test(?:\s*\(|\s*\])/.test(trimmed)) {
      pendingTestFunction = true;
      skipLines[index] = true;
    }

    if (pendingCfgTestModule && /\bmod\b/.test(trimmed) && line.includes('{')) {
      activeBlocks.push({ startDepth: braceDepth + 1 });
      pendingCfgTestModule = false;
      skipLines[index] = true;
    }

    if (pendingTestFunction && /\bfn\b/.test(trimmed) && line.includes('{')) {
      activeBlocks.push({ startDepth: braceDepth + 1 });
      pendingTestFunction = false;
      skipLines[index] = true;
    }

    braceDepth += countMatches(line, /\{/g) - countMatches(line, /\}/g);

    while (activeBlocks.length > 0 && braceDepth < activeBlocks[activeBlocks.length - 1].startDepth) {
      activeBlocks.pop();
    }
  }

  return skipLines;
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
  const ext = path.extname(normalized).toLowerCase();
  const scanLocalPaths = !isTestFile;
  const scanTokenLikeSecrets = !isTestFile;
  const lines = content.split(/\r?\n/);
  const rustInlineTestSkipLines = ext === '.rs' ? getRustInlineTestSkipLines(lines) : null;

  for (const [index, line] of lines.entries()) {
    const lineNumber = index + 1;
    const isInlineRustTestLine = rustInlineTestSkipLines?.[index] === true;
    const skipLocalPathScan =
      isInlineRustTestLine || isCommentOnlyLine(line, ext);
    const skipTokenScan = isInlineRustTestLine;

    if (privateKeyPattern.test(line)) {
      addViolation(file, lineNumber, 'contains a private key marker.');
    }

    if (scanTokenLikeSecrets && !skipTokenScan && tokenPattern.test(line)) {
      addViolation(file, lineNumber, 'contains a token-like secret.');
    }

    if (scanLocalPaths && !skipLocalPathScan && localAbsolutePathPattern.test(line)) {
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
