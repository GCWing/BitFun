#!/usr/bin/env node
import { readFileSync } from 'fs';

const args = parseArgs(process.argv.slice(2));
const manifestPath = requireArg(args, 'manifest');
const version = args.version;
const requiredPlatforms = parseListArg(args['required-platforms'] || '');
const checkUrls = ['1', 'true', 'yes'].includes(String(args['check-urls'] || '').toLowerCase());

const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
if (version && manifest.version !== version) {
  fail(`Manifest version ${manifest.version} does not match expected ${version}`);
}

if (!manifest.platforms || typeof manifest.platforms !== 'object') {
  fail('Manifest does not contain a platforms object');
}

const missing = requiredPlatforms.filter((platform) => !manifest.platforms[platform]);
if (missing.length > 0) {
  fail(`Missing required updater platforms: ${missing.join(', ')}`);
}

for (const [platform, entry] of Object.entries(manifest.platforms)) {
  if (!entry || typeof entry !== 'object') {
    fail(`Invalid platform entry for ${platform}`);
  }
  if (!entry.url || typeof entry.url !== 'string') {
    fail(`Missing URL for ${platform}`);
  }
  if (!entry.signature || typeof entry.signature !== 'string') {
    fail(`Missing signature for ${platform}`);
  }
}

if (checkUrls) {
  for (const [platform, entry] of Object.entries(manifest.platforms)) {
    await assertUrlAvailable(platform, entry.url);
  }
}

console.log(`[verify-latest-json] OK: ${Object.keys(manifest.platforms).sort().join(', ')}`);

async function assertUrlAvailable(platform, url) {
  let response = await fetch(url, { method: 'HEAD', redirect: 'follow' });
  if (response.ok) {
    return;
  }

  response = await fetch(url, {
    method: 'GET',
    redirect: 'follow',
    headers: { Range: 'bytes=0-0' },
  });
  if (!response.ok) {
    fail(`URL for ${platform} is not available: ${url} (${response.status})`);
  }
}

function parseArgs(rawArgs) {
  const parsed = {};
  for (let i = 0; i < rawArgs.length; i += 1) {
    const arg = rawArgs[i];
    if (!arg.startsWith('--')) {
      continue;
    }
    const key = arg.slice(2);
    const value = rawArgs[i + 1];
    if (!value || value.startsWith('--')) {
      fail(`Missing value for --${key}`);
    }
    parsed[key] = value;
    i += 1;
  }
  return parsed;
}

function requireArg(parsed, key) {
  const value = parsed[key];
  if (!value) {
    fail(`Missing required argument --${key}`);
  }
  return value;
}

function parseListArg(value) {
  return String(value)
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

function fail(message) {
  console.error(`[verify-latest-json] ${message}`);
  process.exit(1);
}
