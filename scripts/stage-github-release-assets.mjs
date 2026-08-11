#!/usr/bin/env node

import {
  copyFileSync,
  mkdirSync,
  rmSync,
  statSync,
} from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const outDirIndex = args.indexOf('--out-dir');
if (outDirIndex === -1 || !args[outDirIndex + 1]) {
  fail('Missing required --out-dir argument');
}

const outDir = path.resolve(args[outDirIndex + 1]);
const inputs = args.filter(
  (_, index) => index !== outDirIndex && index !== outDirIndex + 1,
);

if (inputs.length === 0) {
  fail('No release assets were provided');
}

const byName = new Map();
for (const input of inputs) {
  const source = path.resolve(input);
  let stats;
  try {
    stats = statSync(source);
  } catch {
    fail(`Release asset was not found: ${input}`);
  }
  if (!stats.isFile()) {
    fail(`Release asset is not a file: ${input}`);
  }

  const name = path.basename(source);
  const previous = byName.get(name);
  if (previous) {
    fail(`Duplicate release asset name ${name}: ${previous} conflicts with ${source}`);
  }
  byName.set(name, source);
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

for (const [name, source] of byName) {
  copyFileSync(source, path.join(outDir, name));
}

console.log(`Staged ${byName.size} uniquely named GitHub release assets in ${outDir}`);

function fail(message) {
  console.error(`[stage-release-assets] ${message}`);
  process.exit(1);
}
