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

// GitHub rewrites characters outside this set when it stores a release asset
// (`+` becomes `.`, spaces become `.`), which would silently turn every URL in
// this manifest into a 404. Fail at generation time instead.
const GITHUB_SAFE_ASSET_NAME = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

function asset(filename) {
  if (!GITHUB_SAFE_ASSET_NAME.test(filename)) {
    throw new Error(
      `Release asset name is not preserved verbatim by GitHub: ${filename}. ` +
        'Use only alphanumerics, dot, dash and underscore (strip SemVer build metadata).'
    );
  }
  const absolutePath = path.join(assetsDir, filename);
  if (!fs.existsSync(absolutePath)) {
    throw new Error(`Required Linux release asset was not found: ${absolutePath}`);
  }
  const checksum = `${filename}.sha256`;
  const checksumPath = path.join(assetsDir, checksum);
  if (!fs.existsSync(checksumPath)) {
    throw new Error(`Required Linux checksum was not found: ${checksumPath}`);
  }
  const entry = {
    filename,
    url: `${releaseBase}/${filename}`,
    sha256Url: `${releaseBase}/${checksum}`,
  };
  const checksumSignature = `${checksum}.sig`;
  if (fs.existsSync(path.join(assetsDir, checksumSignature))) {
    entry.sha256SigUrl = `${releaseBase}/${checksumSignature}`;
  }
  // Signature is optional: forks build without the release key. A checksum
  // proves only that the transfer was intact, since whoever serves the archive
  // serves the checksum too; the signature is what a mirror cannot forge.
  const signature = `${filename}.sig`;
  if (fs.existsSync(path.join(assetsDir, signature))) {
    entry.sigUrl = `${releaseBase}/${signature}`;
  }
  return entry;
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
        cli: asset(`openbitfun-cli-${version}-${target}.tar.gz`),
        relay: asset(`openbitfun-relay-server-${target}.tar.gz`),
      },
    ])
  ),
};

fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
console.log(`Generated Linux binaries manifest: ${out}`);
