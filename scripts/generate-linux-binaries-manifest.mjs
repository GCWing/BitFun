#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

function option(name) {
  const index = process.argv.indexOf(name);
  if (index < 0 || !process.argv[index + 1]) {
    throw new Error(`Missing required option: ${name}`);
  }
  return process.argv[index + 1];
}

const assetsDir = path.resolve(option('--assets-dir'));
const version = option('--version');
const tag = option('--tag');
const repo = option('--repo');
const out = path.resolve(option('--out'));
const releaseBase = `https://github.com/${repo}/releases/download/${tag}`;

const platforms = [
  {
    key: 'linux-x86_64',
    target: 'x86_64-unknown-linux-gnu',
  },
  {
    key: 'linux-aarch64',
    target: 'aarch64-unknown-linux-gnu',
  },
];

function asset(filename) {
  const absolutePath = path.join(assetsDir, filename);
  if (!fs.existsSync(absolutePath)) {
    throw new Error(`Required Linux release asset was not found: ${absolutePath}`);
  }
  const checksum = `${filename}.sha256`;
  const checksumPath = path.join(assetsDir, checksum);
  if (!fs.existsSync(checksumPath)) {
    throw new Error(`Required Linux checksum was not found: ${checksumPath}`);
  }
  return {
    filename,
    url: `${releaseBase}/${filename}`,
    sha256Url: `${releaseBase}/${checksum}`,
  };
}

const manifest = {
  schemaVersion: 1,
  version,
  tag,
  platforms: Object.fromEntries(
    platforms.map(({ key, target }) => [
      key,
      {
        target,
        cli: asset(`bitfun-cli-${version}-${target}.tar.gz`),
        relay: asset(`bitfun-relay-server-${target}.tar.gz`),
      },
    ])
  ),
};

fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
console.log(`Generated Linux binaries manifest: ${out}`);
