#!/usr/bin/env node
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export function decodeMinisignPublicKey(value) {
  const input = String(value || '').trim();
  if (!input) {
    throw new Error('OPENBITFUN_SIGNING_PUBKEY is required');
  }

  const raw = input.startsWith('untrusted comment:')
    ? input
    : decodeBase64(input);
  const lines = raw.trim().split(/\r?\n/);
  if (!lines[0]?.startsWith('untrusted comment:') || lines.length < 2 || !lines[1]) {
    throw new Error('Public key is not a minisign public key file');
  }
  return `${raw.trim()}\n`;
}

function decodeBase64(value) {
  const compact = value.replace(/\s/g, '');
  if (
    compact.length === 0
    || compact.length % 4 !== 0
    || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(compact)
  ) {
    throw new Error('Public key is neither raw minisign text nor valid base64');
  }
  return Buffer.from(compact, 'base64').toString('utf8');
}

function readArg(args, name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const output = readArg(process.argv.slice(2), '--out');
    if (!output) throw new Error('Missing required --out argument');
    writeFileSync(output, decodeMinisignPublicKey(process.env.OPENBITFUN_SIGNING_PUBKEY), 'utf8');
  } catch (error) {
    console.error(`[release-key] ${error.message || error}`);
    process.exit(1);
  }
}
