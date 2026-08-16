#!/usr/bin/env node
import { appendFileSync, existsSync, readFileSync } from 'node:fs';
import { compareReleaseVersions } from './release-channel.mjs';

const args = parseArgs(process.argv.slice(2));
const candidatePath = requireArg(args, 'candidate');
const currentPath = args.current;
const candidate = readVersion(candidatePath);
const current = currentPath && existsSync(currentPath) ? readVersion(currentPath) : null;
const promote = current === null || compareReleaseVersions(candidate, current) >= 0;

console.log(
  current === null
    ? `[channel-promotion] Initial channel version: ${candidate}`
    : `[channel-promotion] current=${current} candidate=${candidate} promote=${promote}`,
);
if (args['github-output']) {
  appendFileSync(args['github-output'], `promote=${promote}\n`, 'utf8');
  appendFileSync(args['github-output'], `candidate_version=${candidate}\n`, 'utf8');
}

function readVersion(file) {
  const data = JSON.parse(readFileSync(file, 'utf8'));
  if (typeof data.version !== 'string') {
    throw new Error(`Manifest has no string version: ${file}`);
  }
  return data.version;
}

function parseArgs(rawArgs) {
  const parsed = {};
  for (let index = 0; index < rawArgs.length; index += 2) {
    const name = rawArgs[index];
    const value = rawArgs[index + 1];
    if (!name?.startsWith('--') || !value) {
      throw new Error(`Invalid argument near ${name || '<end>'}`);
    }
    parsed[name.slice(2)] = value;
  }
  return parsed;
}

function requireArg(parsed, name) {
  if (!parsed[name]) throw new Error(`Missing required --${name} argument`);
  return parsed[name];
}
