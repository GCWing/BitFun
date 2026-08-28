import { lstatSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

const RELEVANT_EXTENSIONS = new Set([
  '.css',
  '.html',
  '.js',
  '.json',
  '.jsx',
  '.mjs',
  '.scss',
  '.svg',
  '.ts',
  '.tsx',
  '.yaml',
  '.yml',
]);

const IGNORED_DIRECTORIES = new Set([
  '.git',
  'coverage',
  'dist',
  'node_modules',
  'target',
]);

function newestRelevantInput(entryPath) {
  let stat;
  try {
    stat = lstatSync(entryPath);
  } catch {
    return null;
  }

  if (stat.isSymbolicLink()) {
    return null;
  }
  if (stat.isFile()) {
    if (!RELEVANT_EXTENSIONS.has(path.extname(entryPath).toLowerCase())) {
      return null;
    }
    return { path: entryPath, mtimeMs: stat.mtimeMs };
  }
  if (!stat.isDirectory()) {
    return null;
  }

  let newest = null;
  for (const entry of readdirSync(entryPath, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) {
      continue;
    }
    if (entry.isDirectory() && IGNORED_DIRECTORIES.has(entry.name)) {
      continue;
    }
    const candidate = newestRelevantInput(path.join(entryPath, entry.name));
    if (candidate && (!newest || candidate.mtimeMs > newest.mtimeMs)) {
      newest = candidate;
    }
  }
  return newest;
}

export function getFrontendWorkbenchDevBaselinePlan(rootDir) {
  const baselineIndex = path.join(rootDir, 'dist', 'index.html');
  let baselineMtimeMs;
  try {
    baselineMtimeMs = statSync(baselineIndex).mtimeMs;
  } catch {
    return {
      shouldBuild: true,
      reason: 'FrontendWorkbench baseline is missing',
    };
  }

  const inputs = [
    path.join(rootDir, 'package.json'),
    path.join(rootDir, 'pnpm-lock.yaml'),
    path.join(rootDir, 'src', 'web-ui'),
    path.join(rootDir, 'design-system', 'packages'),
  ];
  let newest = null;
  for (const input of inputs) {
    const candidate = newestRelevantInput(input);
    if (candidate && (!newest || candidate.mtimeMs > newest.mtimeMs)) {
      newest = candidate;
    }
  }

  if (newest && newest.mtimeMs > baselineMtimeMs) {
    return {
      shouldBuild: true,
      reason: `FrontendWorkbench baseline is stale (${path.relative(rootDir, newest.path)})`,
    };
  }

  return {
    shouldBuild: false,
    reason: 'FrontendWorkbench baseline is current',
  };
}
