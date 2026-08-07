#!/usr/bin/env node
import { readFileSync } from 'fs';

const args = parseArgs(process.argv.slice(2));
const expected = requireArg(args, 'version');
const versions = new Map([
  ['package.json', readJsonVersion('package.json')],
  ['package-lock.json', readJsonVersion('package-lock.json')],
  ['Cargo.toml', readTomlVersion('Cargo.toml', /version = "([^"]+)" # x-release-please-version/)],
  ['BitFun-Installer/package.json', readJsonVersion('BitFun-Installer/package.json')],
  ['BitFun-Installer/package-lock.json', readJsonVersion('BitFun-Installer/package-lock.json')],
  ['BitFun-Installer/src-tauri/Cargo.toml', readTomlVersion('BitFun-Installer/src-tauri/Cargo.toml', /^version = "([^"]+)"/m)],
]);

const mismatches = [...versions].filter(([, version]) => version !== expected);
if (mismatches.length > 0) {
  for (const [file, version] of mismatches) {
    console.error(`[release-version] ${file}: expected ${expected}, found ${version}`);
  }
  process.exit(1);
}
console.log(`[release-version] OK: ${expected}`);

function readJsonVersion(file) {
  return JSON.parse(readFileSync(file, 'utf8')).version;
}

function readTomlVersion(file, pattern) {
  const match = pattern.exec(readFileSync(file, 'utf8'));
  if (!match) throw new Error(`Version was not found in ${file}`);
  return match[1];
}

function parseArgs(rawArgs) {
  const parsed = {};
  for (let i = 0; i < rawArgs.length; i += 1) {
    const arg = rawArgs[i];
    if (!arg.startsWith('--')) continue;
    parsed[arg.slice(2)] = rawArgs[i + 1];
    i += 1;
  }
  return parsed;
}

function requireArg(parsed, key) {
  if (!parsed[key]) throw new Error(`Missing required argument --${key}`);
  return parsed[key];
}
