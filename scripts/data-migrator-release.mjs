#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const ROOT = resolve(import.meta.dirname, '..');
export const PLATFORMS = {
  'windows-x64': { target: 'x86_64-pc-windows-msvc', extension: 'zip' },
  'macos-arm64': { target: 'aarch64-apple-darwin', extension: 'dmg', folder: 'dmg' },
  'macos-x64': { target: 'x86_64-apple-darwin', extension: 'dmg', folder: 'dmg' },
  'linux-x64': { target: 'x86_64-unknown-linux-gnu', extension: 'AppImage', folder: 'appimage' },
};
export function releaseVersion(root = ROOT) {
  const manifest = readFileSync(join(root, 'src/apps/data-migrator/Cargo.toml'), 'utf8');
  const version = manifest.match(/^version = "([^"]+)"/m)?.[1];
  if (!version || !/^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$/.test(version)) throw new Error('Invalid independent migrator version');
  const config = JSON.parse(readFileSync(join(root, 'src/apps/data-migrator/tauri.conf.json'), 'utf8'));
  if (config.version !== version) throw new Error('Migrator version differs between Cargo and Tauri');
  return version;
}

export function validateReleaseTag(tag, version) {
  if (tag !== `data-migrator-v${version}`) throw new Error('Tag must match the independent migrator version');
}

function stage(platform) {
  const metadata = PLATFORMS[platform];
  if (!metadata) throw new Error('Unsupported migrator release platform');
  const version = releaseVersion();
  if (process.env.GITHUB_REF_TYPE === 'tag') validateReleaseTag(process.env.GITHUB_REF_NAME, version);
  const target = resolve(ROOT, process.env.CARGO_TARGET_DIR || 'target', metadata.target, 'release');
  const output = join(ROOT, 'target/data-migrator-release', platform);
  mkdirSync(output, { recursive: true });
  const asset = `openbitfun-data-migrator-v${version}-${platform}.${metadata.extension}`;
  const destination = join(output, asset);
  if (platform === 'windows-x64') {
    const portable = join(ROOT, 'target/data-migrator-portable', version);
    mkdirSync(portable, { recursive: true });
    copyFileSync(join(target, 'openbitfun-data-migrator.exe'), join(portable, 'openbitfun-data-migrator.exe'));
    for (const file of ['README.md', 'README.zh-CN.md']) copyFileSync(join(ROOT, 'src/apps/data-migrator', file), join(portable, file));
    copyFileSync(join(ROOT, 'THIRD_PARTY_NOTICES.md'), join(portable, 'THIRD_PARTY_NOTICES.md'));
    const result = spawnSync('tar', ['-a', '-cf', destination, '-C', portable, 'openbitfun-data-migrator.exe', 'README.md', 'README.zh-CN.md', 'THIRD_PARTY_NOTICES.md'], { stdio: 'inherit', windowsHide: true });
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error('Could not archive the portable migrator');
  } else {
    const folder = join(target, 'bundle', metadata.folder);
    const files = readdirSync(folder).filter((name) => name.startsWith(`OpenBitFun Data Migrator_${version}_`) && name.endsWith(`.${metadata.extension}`));
    if (files.length !== 1) throw new Error('Expected exactly one migrator package in the bundle output');
    copyFileSync(join(folder, files[0]), destination);
  }
  const sha256 = createHash('sha256').update(readFileSync(destination)).digest('hex');
  writeFileSync(`${destination}.sha256`, `${sha256}  ${asset}\n`);
  console.log(destination);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv[2] === '--check-version') {
    const version = releaseVersion();
    if (process.env.GITHUB_REF_TYPE === 'tag') validateReleaseTag(process.env.GITHUB_REF_NAME, version);
    console.log(version);
  } else stage(process.argv[2]);
}
