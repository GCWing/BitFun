#!/usr/bin/env node
/**
 * Runs `tauri build` from src/apps/desktop with CI=true.
 * On Windows: shared OpenSSL bootstrap (see ensure-openssl-windows.mjs).
 */
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { existsSync, readdirSync } from 'fs';
import { ensureOpenSslWindows } from './ensure-openssl-windows.mjs';
import {
  codgrepBinaryPath,
  ensureCodgrepResource,
} from './prepare-codgrep-resource.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

function tauriBuildArgsFromArgv() {
  const args = process.argv.slice(2);
  // `node script.mjs -- --foo` leaves a leading `--`; strip so `tauri build` sees the same argv as before.
  let i = 0;
  while (i < args.length && args[i] === '--') {
    i += 1;
  }
  return args.slice(i);
}

function codgrepProfileFromTauriArgs(args) {
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--debug') {
      return 'debug';
    }
    if (arg === '--profile' && args[i + 1]) {
      return args[i + 1];
    }
    if (arg.startsWith('--profile=')) {
      return arg.slice('--profile='.length);
    }
  }
  return 'release';
}

function ensureCodgrepBinary(profile) {
  const cargoArgs = ['build', '-p', 'codgrep', '--bin', 'cg'];
  if (profile !== 'debug') {
    cargoArgs.push('--profile', profile);
  }

  const result = spawnSync('cargo', cargoArgs, {
    cwd: ROOT,
    env: process.env,
    stdio: 'inherit',
    shell: true,
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }

  const binaryPath = codgrepBinaryPath(profile);
  if (!existsSync(binaryPath)) {
    throw new Error(`codgrep binary not found after build: ${binaryPath}`);
  }

  return binaryPath;
}

async function main() {
  const forward = tauriBuildArgsFromArgv();
  const codgrepProfile = codgrepProfileFromTauriArgs(forward);

  await ensureOpenSslWindows();
  process.env.CODGREP_DAEMON_BIN = ensureCodgrepBinary(codgrepProfile);
  ensureCodgrepResource(codgrepProfile);

  const desktopDir = join(ROOT, 'src', 'apps', 'desktop');
  // Tauri CLI reads CI and rejects numeric "1" (common in CI providers).
  process.env.CI = 'true';

  const tauriConfig = join(desktopDir, 'tauri.conf.json');
  const tauriBin = join(ROOT, 'node_modules', '.bin', 'tauri');
  const r = spawnSync(tauriBin, ['build', '--config', tauriConfig, ...forward], {
    cwd: desktopDir,
    env: process.env,
    stdio: 'inherit',
    shell: true,
  });

  if (r.error) {
    console.error(r.error);
    process.exit(1);
  }

  if (r.status === 0 && process.platform === 'darwin') {
    patchDmgExtras(ROOT);
  }

  process.exit(r.status ?? 1);
}

// Find all .dmg files under target/ and inject the helper TXT files
// (quarantine removal instructions) into each one.
function patchDmgExtras(root) {
  const patchScript = join(root, 'scripts', 'patch-dmg-extras.sh');
  const targetDir = join(root, 'target');

  const dmgFiles = findDmgFiles(targetDir);
  if (dmgFiles.length === 0) {
    console.log('[patch-dmg] No .dmg files found — skipping.');
    return;
  }

  for (const dmg of dmgFiles) {
    console.log(`[patch-dmg] Patching ${dmg}`);
    const p = spawnSync('bash', [patchScript, dmg], {
      stdio: 'inherit',
      shell: false,
    });
    if (p.status !== 0) {
      console.error(`[patch-dmg] Failed to patch ${dmg}`);
      process.exit(1);
    }
  }
}

function findDmgFiles(dir) {
  const results = [];
  try {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        results.push(...findDmgFiles(full));
      } else if (entry.name.endsWith('.dmg')) {
        results.push(full);
      }
    }
  } catch {
    // directory may not exist for some targets
  }
  return results;
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
