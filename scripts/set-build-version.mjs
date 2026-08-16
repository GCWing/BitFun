#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export function setBuildVersion(root, version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid build version: ${version}`);
  }

  for (const relative of [
    'package.json',
    'package-lock.json',
    'BitFun-Installer/package.json',
    'BitFun-Installer/package-lock.json',
  ]) {
    const file = path.join(root, relative);
    const data = JSON.parse(readFileSync(file, 'utf8'));
    data.version = version;
    if (data.packages?.['']) {
      data.packages[''].version = version;
    }
    writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`, 'utf8');
  }

  replaceVersion(
    path.join(root, 'Cargo.toml'),
    /^version = "[^"]+" # x-release-please-version$/m,
    `version = "${version}" # x-release-please-version`,
  );
  replaceVersion(
    path.join(root, 'src/apps/relay-server/Cargo.toml'),
    /^version = "[^"]+" # x-release-please-version$/m,
    `version = "${version}" # x-release-please-version`,
  );
  replaceVersion(
    path.join(root, 'BitFun-Installer/src-tauri/Cargo.toml'),
    /^version = "[^"]+"$/m,
    `version = "${version}"`,
  );
}

function replaceVersion(file, pattern, replacement) {
  const source = readFileSync(file, 'utf8');
  if (!pattern.test(source)) {
    throw new Error(`Version marker was not found in ${file}`);
  }
  writeFileSync(file, source.replace(pattern, replacement), 'utf8');
}

function readArg(args, name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const version = readArg(process.argv.slice(2), '--version');
    if (!version) {
      throw new Error('Missing required --version argument');
    }
    setBuildVersion(ROOT, version);
    console.log(`[build-version] Updated release metadata to ${version}`);
  } catch (error) {
    console.error(`[build-version] ${error.message || error}`);
    process.exit(1);
  }
}
