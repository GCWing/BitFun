#!/usr/bin/env node
import { readFileSync } from 'fs';

const args = parseArgs(process.argv.slice(2));
const expected = args.version || readJsonVersion('package.json');
const versions = new Map([
  ['package.json', readJsonVersion('package.json')],
  ['package-lock.json', readJsonVersion('package-lock.json')],
  ['Cargo.toml', readTomlVersion('Cargo.toml', /version = "([^"]+)" # x-release-please-version/)],
  ['OpenBitFun-Installer/package.json', readJsonVersion('OpenBitFun-Installer/package.json')],
  ['OpenBitFun-Installer/package-lock.json', readJsonVersion('OpenBitFun-Installer/package-lock.json')],
  ['OpenBitFun-Installer/src-tauri/Cargo.toml', readTomlVersion('OpenBitFun-Installer/src-tauri/Cargo.toml', /^version = "([^"]+)"/m)],
  ['src/web-ui/package.json', readJsonVersion('src/web-ui/package.json')],
  ['src/mobile-web/package.json', readJsonVersion('src/mobile-web/package.json')],
  ['src/mobile-web/package-lock.json', readJsonVersion('src/mobile-web/package-lock.json')],
  ['src/miniapp-market-web/package.json', readJsonVersion('src/miniapp-market-web/package.json')],
  ['src/skin-market-web/package.json', readJsonVersion('src/skin-market-web/package.json')],
  ['src/apps/relay-server/Cargo.toml', readTomlVersion('src/apps/relay-server/Cargo.toml', /version = "([^"]+)" # x-release-please-version/)],
  ['src/crates/services/relay-service/Cargo.toml', readTomlVersion('src/crates/services/relay-service/Cargo.toml', /^version = "([^"]+)"/m)],
  ['src/crates/services/page-function-runtime/Cargo.toml', readTomlVersion('src/crates/services/page-function-runtime/Cargo.toml', /^version = "([^"]+)"/m)],
  ['src/apps/mobile/android/app/build.gradle.kts', readTextVersion('src/apps/mobile/android/app/build.gradle.kts', /versionName\s*=\s*"([^"]+)"/)],
  ['src/apps/mobile/ios/OpenBitFun/Info.plist', readTextVersion('src/apps/mobile/ios/OpenBitFun/Info.plist', /<key>CFBundleShortVersionString<\/key>\s*<string>([^<]+)<\/string>/)],
  ['src/apps/mobile/harmonyos/AppScope/app.json5', readTextVersion('src/apps/mobile/harmonyos/AppScope/app.json5', /"versionName"\s*:\s*"([^"]+)"/)],
]);

if (!expected.includes('-')) {
  const releasePleaseManifest = JSON.parse(readFileSync('.release-please-manifest.json', 'utf8'));
  versions.set('.release-please-manifest.json', releasePleaseManifest['.']);
}

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
  return readTextVersion(file, pattern);
}

function readTextVersion(file, pattern) {
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
