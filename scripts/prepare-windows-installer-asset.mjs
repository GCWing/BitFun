#!/usr/bin/env node
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync } from 'fs';
import { basename, join } from 'path';

const args = parseArgs(process.argv.slice(2));
const assetsDir = requireArg(args, 'assets-dir');
const version = requireArg(args, 'version');
const outDir = requireArg(args, 'out-dir');

if (!existsSync(assetsDir)) {
  fail(`Assets directory does not exist: ${assetsDir}`);
}
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  fail(`Version is not safe for a release asset name: ${version}`);
}

const candidates = walkFiles(assetsDir).filter(
  (file) => basename(file).toLowerCase() === 'openbitfun-installer.exe'
);
if (candidates.length !== 1) {
  fail(`Expected exactly one openbitfun-installer.exe, found ${candidates.length}`);
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const outputName = `OpenBitFun_${version}_windows-x86_64-installer.exe`;
const outputPath = join(outDir, outputName);
copyFileSync(candidates[0], outputPath);
console.log(`[manual-installer] ${candidates[0]} -> ${outputPath}`);

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

function walkFiles(dir) {
  const files = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(fullPath));
    else if (entry.isFile()) files.push(fullPath);
  }
  return files;
}

function fail(message) {
  console.error(`[manual-installer] ${message}`);
  process.exit(1);
}
