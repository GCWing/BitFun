#!/usr/bin/env node
/**
 * Windows release asset packager (三件套: installer exe + zip 便携版 + SHA256SUMS).
 *
 * Usage:
 *   node scripts/package-windows-assets.mjs \
 *     --installer <path-to-bitfun-installer.exe> \
 *     --app-release-dir <path-to-target/release> \
 *     --version 0.2.16 \
 *     --out-dir release-assets
 *
 * Produces under --out-dir:
 *   BitFun_<version>_windows-x86_64-installer.exe   (copied installer)
 *   BitFun_<version>_windows-x86_64-portable.zip    (portable app: exe + runtime dirs)
 *   SHA256SUMS                                      (sha256 of every asset)
 *
 * The portable zip mirrors the installer payload layout: the main app exe plus
 * the runtime siblings the app needs at startup (mobile-web, resources,
 * third-party, THIRD_PARTY_NOTICES.md). It is a no-install distribution.
 *
 * Windows-native: uses tar.exe bsdtar to create the zip (available on Windows
 * 10+); falls back to PowerShell Compress-Archive if bsdtar is unavailable.
 */
import { createHash } from 'crypto';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'fs';
import { basename, dirname, join, resolve as resolvePath } from 'path';
import { fileURLToPath } from 'url';
import { spawnSync } from 'child_process';

if (isMain()) {
  try {
    const args = parseArgs(process.argv.slice(2));
    await main(args);
  } catch (error) {
    process.exit(error?.exitCode ?? 1);
  }
}

function isMain() {
  // Under `node --test` the module is imported with a bare entry argv
  // (argv[1] is the test runner's shim or undefined), so a pure argv
  // comparison would treat imports as the CLI. Compare against the resolved
  // module path instead.
  if (!process.argv[1]) {
    return false;
  }
  try {
    return resolvePath(process.argv[1]) === resolvePath(fileURLToPath(import.meta.url));
  } catch {
    return false;
  }
}

export async function main(argv = []) {
  // Accept either raw CLI args (["--installer", ...]) or a parsed object
  // ({ installer, ... }). The CLI passes raw argv; tests pass raw argv too,
  // so normalize once here.
  const parsed = Array.isArray(argv) ? parseArgs(argv) : argv;
  const installerPath = requireArg(parsed, 'installer');
  const appReleaseDir = requireArg(parsed, 'app-release-dir');
  const version = requireArg(parsed, 'version');
  const outDir = requireArg(parsed, 'out-dir');

  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    fail(`Version is not safe for a release asset name: ${version}`);
  }
  if (!existsSync(installerPath)) {
    fail(`Installer does not exist: ${installerPath}`);
  }
  if (!existsSync(appReleaseDir)) {
    fail(`App release dir does not exist: ${appReleaseDir}`);
  }

  const exeName = 'bitfun-desktop.exe';
  const exePath = join(appReleaseDir, exeName);
  if (!existsSync(exePath)) {
    fail(`Main app exe not found in release dir: ${exePath}`);
  }

  // Never rm -rf an unexpected path: --out-dir must be a directory that does
  // not exist yet, or one whose contents are all old release assets (files or
  // the SHA256SUMS manifest). Everything else is refused so a mistyped path
  // like E:/ or src can never be recursively deleted (d8-P2-1).
  if (existsSync(outDir)) {
    assertSafeOutDir(outDir);
  }

  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });

  const baseName = `BitFun_${version}_windows-x86_64`;
  const installerOut = join(outDir, `${baseName}-installer.exe`);
  const zipOut = join(outDir, `${baseName}-portable.zip`);
  const sumsOut = join(outDir, 'SHA256SUMS');

  // 1. Copy installer exe.
  copyFile(installerPath, installerOut);
  log(`Copied installer: ${installerPath} -> ${installerOut}`);

  // 2. Create portable zip from the app release dir.
  // Only copy the runtime-relevant entries (mirrors build-installer.cjs payload
  // selection, plus the notice file); exclude build metadata and debug symbols.
  const portableEntries = collectPortableEntries(appReleaseDir, exeName);
  log(`Portable zip will contain ${portableEntries.length} file(s) from ${appReleaseDir}`);
  createZip(portableEntries, zipOut);

  // 3. Write SHA256SUMS over every produced asset.
  const assets = [installerOut, zipOut].sort();
  const lines = assets
    .map((file) => `${sha256File(file)}  ${basename(file)}`)
    .join('\n');
  writeFileSync(sumsOut, `${lines}\n`);
  log(`Wrote ${sumsOut}:`);
  for (const line of lines.split('\n')) log(`  ${line}`);

  console.log(`\n[package-windows-assets] Done. Output in ${outDir}`);
  console.log(`  ${installerOut}`);
  console.log(`  ${zipOut}`);
  console.log(`  ${sumsOut}`);
}

export function collectPortableEntries(releaseDir, exeName) {
  const entries = [];
  const runtimeDirs = ['mobile-web', 'resources', 'third-party'];
  for (const entry of readdirSync(releaseDir, { withFileTypes: true })) {
    const src = join(releaseDir, entry.name);
    if (entry.isFile()) {
      if (entry.name === exeName) entries.push(src);
      else if (entry.name === 'THIRD_PARTY_NOTICES.md') entries.push(src);
      // .pdb / .d / .cargo-lock are build metadata, not runtime files.
    } else if (entry.isDirectory() && runtimeDirs.includes(entry.name)) {
      entries.push(src);
    }
  }
  return entries;
}

function createZip(entries, zipPath) {
  // Build a bsdtar include list of the source paths. tar.exe on Windows
  // (C:\Windows\System32\tar.exe) uses libarchive and can write zip archives.
  //
  // IMPORTANT: entries are absolute paths; archives must store RELATIVE
  // entry names rooted at the release directory, otherwise the zip unpacks
  // into the full temp path nesting (Users/.../Temp/...) and the Windows
  // portable distribution is unusable (d8-P1-1). bsdtar supports `-C <dir>`
  // to chdir before reading entries, which stores the basenames relative to
  // that directory. The Compress-Archive fallback passes `-Path` absolute
  // paths whose file names are stored relative to the first component, so
  // the two branches already produce the same relative layout.
  const cwd = process.cwd();
  let tar = spawnSync('tar', ['--version'], { encoding: 'utf8' });
  if (tar.status === 0) {
    const releaseDir = dirname(entries[0]);
    const args = ['-a', '-c', '-f', zipPath];
    for (const entry of entries) args.push('-C', releaseDir, basename(entry));
    const result = spawnSync('tar', args, { stdio: 'inherit', encoding: 'utf8' });
    if (result.status === 0) {
      log(`Created zip via bsdtar: ${zipPath}`);
      return;
    }
    log('bsdtar zip creation failed, falling back to Compress-Archive');
  }
  // Fallback: PowerShell Compress-Archive (slower, but always present).
  // `-Path` with absolute paths stores entries relative to the leaf
  // directory's parent (the release dir), matching the bsdtar layout.
  const psScript = [
    '$ErrorActionPreference = "Stop"',
    `$dest = '${zipPath.replace(/'/g, "''")}'`,
    'if (Test-Path $dest) { Remove-Item $dest -Force }',
    `$items = @(${entries
      .map((entry) => `'${entry.replace(/'/g, "''")}'`)
      .join(', ')})`,
    'Compress-Archive -Path $items -DestinationPath $dest -CompressionLevel Optimal',
  ].join('; ');
  const result = spawnSync('powershell.exe', ['-NoProfile', '-Command', psScript], {
    stdio: 'inherit',
    encoding: 'utf8',
  });
  if (result.status !== 0) fail(`Failed to create zip: ${zipPath}`);
  log(`Created zip via Compress-Archive: ${zipPath}`);
}

function sha256File(filePath) {
  return createHash('sha256').update(readFileSync(filePath)).digest('hex');
}

function copyFile(src, dest) {
  mkdirSync(join(dest, '..'), { recursive: true });
  copyFileSync(src, dest);
}

function parseArgs(rawArgs) {
  const parsed = {};
  for (let i = 0; i < rawArgs.length; i += 1) {
    const arg = rawArgs[i];
    if (!arg.startsWith('--')) continue;
    const key = arg.slice(2);
    const value = rawArgs[i + 1];
    if (!value || value.startsWith('--')) fail(`Missing value for --${key}`);
    parsed[key] = value;
    i += 1;
  }
  return parsed;
}

function requireArg(parsed, key) {
  const value = parsed[key];
  if (!value) fail(`Missing required argument --${key}`);
  return value;
}

function log(message) {
  console.log(`\x1b[36m[package-windows-assets]\x1b[0m ${message}`);
}

function fail(message) {
  console.error(`\x1b[31m[package-windows-assets]\x1b[0m ${message}`);
  // Throw instead of process.exit so the error is catchable by test runners;
  // the CLI entry wraps main() and exits with a non-zero code.
  const error = new Error(message);
  error.exitCode = 1;
  throw error;
}

/**
 * Refuse to delete a directory that is not a prior release-asset output.
 * Allowed contents: files whose names look like release assets
 * (BitFun_*_windows-x86_64-* / SHA256SUMS) or their manifest, plus nothing
 * else (no subdirectories). This protects against a mistyped --out-dir
 * wiping an unrelated directory tree (d8-P2-1).
 */
function assertSafeOutDir(dir) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch (error) {
    fail(`Cannot read --out-dir ${dir}: ${error.message || String(error)}`);
  }
  const assetName = /^BitFun_\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?_windows-x86_64-(installer\.exe|portable\.zip)$/;
  const manifestName = /^SHA256SUMS$/;
  for (const entry of entries) {
    if (entry.isDirectory()) {
      fail(
        `Refusing to delete --out-dir ${dir}: contains subdirectory "${entry.name}" (not a release-assets output)`,
      );
    }
    if (!assetName.test(entry.name) && !manifestName.test(entry.name)) {
      fail(
        `Refusing to delete --out-dir ${dir}: contains unexpected file "${entry.name}"`,
      );
    }
  }
}
