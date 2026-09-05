#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const repositoryRoot = resolve(dirname(scriptPath), '..');

function git(repository, args, input) {
  return execFileSync('git', ['-c', 'core.quotePath=false', ...args], {
    cwd: repository,
    encoding: 'utf8',
    input,
    maxBuffer: 64 * 1024 * 1024,
    windowsHide: true,
    stdio: ['pipe', 'pipe', 'pipe'],
  }).trimEnd();
}

export function inspectGitObjectSizes({ repository, head = 'HEAD', base, policy }) {
  if (!Number.isSafeInteger(policy.maxBlobBytes) || policy.maxBlobBytes <= 0) {
    throw new Error('maxBlobBytes must be a positive integer.');
  }
  const allowed = new Set(policy.allowedBlobs.map((entry) => entry.oid));
  const headCommit = git(repository, ['rev-parse', '--verify', '--end-of-options', `${head}^{commit}`]);
  const revisions = [headCommit];
  if (base) {
    const baseCommit = git(repository, ['rev-parse', '--verify', '--end-of-options', `${base}^{commit}`]);
    revisions.push('--not', baseCommit);
  }

  // Traverse the entire introduced history, including files deleted before HEAD.
  const entries = git(repository, ['rev-list', '--objects', ...revisions]);
  if (!entries) return { checkedBlobs: 0, violations: [] };
  const paths = new Map(entries.split('\n').map((entry) => {
    const separator = entry.indexOf(' ');
    return separator < 0 ? [entry, ''] : [entry.slice(0, separator), entry.slice(separator + 1)];
  }));
  const metadata = git(repository, ['cat-file', '--batch-check=%(objectname) %(objecttype) %(objectsize)'],
    `${[...paths.keys()].join('\n')}\n`);
  let checkedBlobs = 0;
  const violations = [];
  for (const line of metadata.split('\n')) {
    const [oid, type, rawSize] = line.split(' ');
    const bytes = Number(rawSize);
    if (!Number.isSafeInteger(bytes)) throw new Error(`Cannot read Git object ${oid}.`);
    if (type !== 'blob') continue;
    checkedBlobs += 1;
    if (bytes > policy.maxBlobBytes && !allowed.has(oid)) {
      violations.push({ oid, bytes, path: paths.get(oid) });
    }
  }
  return { checkedBlobs, violations };
}

function main() {
  const options = { repository: repositoryRoot };
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 2) {
    const option = args[index];
    if (!['--base', '--head'].includes(option) || !args[index + 1]) {
      throw new Error('Usage: node scripts/check-git-object-sizes.mjs [--base COMMIT] [--head COMMIT]');
    }
    options[option.slice(2)] = args[index + 1];
  }
  const policy = JSON.parse(readFileSync(new URL('./git-object-size-policy.json', import.meta.url), 'utf8'));
  const result = inspectGitObjectSizes({ ...options, policy });
  if (result.violations.length > 0) {
    for (const violation of result.violations) {
      console.error(`${JSON.stringify(violation.path)}: ${violation.bytes} bytes (${violation.oid}) exceeds the ${policy.maxBlobBytes}-byte Git object limit.`);
    }
    throw new Error('Store large build artifacts outside Git. Removing a file only from the latest commit does not remove it from history.');
  }
  console.log(`Checked ${result.checkedBlobs} Git blobs; no unapproved objects exceed ${policy.maxBlobBytes} bytes.`);
}

if (process.argv[1] && resolve(process.argv[1]) === scriptPath) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
