#!/usr/bin/env node
// Formats only the Rust files that are already changed, staged, or untracked.
// This avoids `cargo fmt` sweeping the whole workspace, while also compensating
// for rustfmt's stable behavior of recursively formatting out-of-line modules
// referenced by an entry file. After formatting the intended target set, the
// script restores any additional `.rs` files that became dirty only because
// rustfmt expanded into child modules, so the final diff stays scoped to the
// files the user actually touched.
import { existsSync, readFileSync } from 'fs';
import { spawnSync } from 'child_process';
import { dirname, relative, resolve } from 'path';
import { fileURLToPath } from 'url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, '..');
const cargoTomlPath = resolve(repoRoot, 'Cargo.toml');
const args = new Set(process.argv.slice(2));
const checkMode = args.has('--check');
const verbose = args.has('--verbose');
const rustEdition = resolveRustEdition();
const changedFiles = collectChangedRustFiles();

if (changedFiles.length === 0) {
  console.log('[format-changed-rust] No changed Rust files found in workspace or index.');
  process.exit(0);
}

console.log(
  `[format-changed-rust] ${checkMode ? 'Checking' : 'Formatting'} ${changedFiles.length} Rust file(s).`
);

for (const batch of buildBatches(changedFiles, 6000)) {
  const commandArgs = ['--edition', rustEdition];
  if (checkMode) {
    commandArgs.push('--check');
  }
  if (!verbose) {
    commandArgs.push('--quiet');
  }
  commandArgs.push(...batch);

  const result = spawnSync('rustfmt', commandArgs, {
    cwd: repoRoot,
    stdio: 'inherit',
  });

  if (result.error) {
    fail(`Failed to run rustfmt: ${result.error.message}`);
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

if (!checkMode) {
  restoreCollateralRustFiles(changedFiles);
}

function collectChangedRustFiles() {
  const candidates = new Set();

  for (const file of runGit([
    'diff',
    '--name-only',
    '--diff-filter=ACMR',
    '--',
    '*.rs',
  ])) {
    candidates.add(normalizeFile(file));
  }

  for (const file of runGit([
    'diff',
    '--cached',
    '--name-only',
    '--diff-filter=ACMR',
    '--',
    '*.rs',
  ])) {
    candidates.add(normalizeFile(file));
  }

  for (const file of runGit([
    'ls-files',
    '--others',
    '--exclude-standard',
    '--',
    '*.rs',
  ])) {
    candidates.add(normalizeFile(file));
  }

  return [...candidates]
    .filter((file) => file.endsWith('.rs'))
    .filter((file) => existsSync(resolve(repoRoot, file)))
    .sort();
}

function collectDirtyRustFiles() {
  const candidates = new Set();

  for (const file of runGit([
    'diff',
    '--name-only',
    '--diff-filter=ACMR',
    '--',
    '*.rs',
  ])) {
    candidates.add(normalizeFile(file));
  }

  for (const file of runGit([
    'ls-files',
    '--others',
    '--exclude-standard',
    '--',
    '*.rs',
  ])) {
    candidates.add(normalizeFile(file));
  }

  return [...candidates]
    .filter((file) => file.endsWith('.rs'))
    .filter((file) => existsSync(resolve(repoRoot, file)))
    .sort();
}

function restoreCollateralRustFiles(targetFiles) {
  const targetSet = new Set(targetFiles);
  const collateralFiles = collectDirtyRustFiles().filter((file) => !targetSet.has(file));

  if (collateralFiles.length === 0) {
    return;
  }

  console.log(
    `[format-changed-rust] Restoring ${collateralFiles.length} collateral Rust file(s) touched through module expansion.`
  );

  for (const batch of buildBatches(collateralFiles, 6000)) {
    const result = spawnSync('git', ['restore', '--worktree', '--', ...batch], {
      cwd: repoRoot,
      stdio: 'inherit',
    });

    if (result.error) {
      fail(`Failed to restore collateral Rust files: ${result.error.message}`);
    }

    if (result.status !== 0) {
      process.exit(result.status ?? 1);
    }
  }
}

function normalizeFile(file) {
  return file.replace(/\\/g, '/').trim();
}

function runGit(commandArgs) {
  const result = spawnSync('git', commandArgs, {
    cwd: repoRoot,
    encoding: 'utf8',
  });

  if (result.error) {
    fail(`Failed to run git: ${result.error.message}`);
  }

  if (result.status !== 0) {
    const stderr = result.stderr?.trim();
    if (stderr) {
      console.error(`[format-changed-rust] ${stderr}`);
    }
    process.exit(result.status ?? 1);
  }

  return result.stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function resolveRustEdition() {
  if (!existsSync(cargoTomlPath)) {
    return '2021';
  }

  const cargoToml = readFileSync(cargoTomlPath, 'utf8');
  const workspacePackageMatch = cargoToml.match(
    /\[workspace\.package\][\s\S]*?^\s*edition\s*=\s*"(\d{4})"/m
  );
  if (workspacePackageMatch) {
    return workspacePackageMatch[1];
  }

  const packageMatch = cargoToml.match(/^\s*edition\s*=\s*"(\d{4})"/m);
  return packageMatch?.[1] ?? '2021';
}

function buildBatches(files, maxCommandLength) {
  const batches = [];
  let currentBatch = [];
  let currentLength = 0;

  for (const file of files) {
    const relativePath = relative(repoRoot, resolve(repoRoot, file)).replace(/\\/g, '/');
    const nextLength = currentLength + relativePath.length + 1;
    if (currentBatch.length > 0 && nextLength > maxCommandLength) {
      batches.push(currentBatch);
      currentBatch = [];
      currentLength = 0;
    }

    currentBatch.push(relativePath);
    currentLength += relativePath.length + 1;
  }

  if (currentBatch.length > 0) {
    batches.push(currentBatch);
  }

  return batches;
}

function fail(message) {
  console.error(`[format-changed-rust] ${message}`);
  process.exit(1);
}
