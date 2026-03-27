#!/usr/bin/env node
/**
 * Runs `tauri build` from src/apps/desktop with CI=true.
 * On Windows: shared OpenSSL bootstrap (see ensure-openssl-windows.mjs).
 */
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'fs';
import { ensureOpenSslWindows } from './ensure-openssl-windows.mjs';
import { ensureFlashgrepBinary } from './prepare-flashgrep-resource.mjs';

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

async function main() {
  const forward = tauriBuildArgsFromArgv();

  await ensureOpenSslWindows();
  process.env.FLASHGREP_DAEMON_BIN = ensureFlashgrepBinary();

  const desktopDir = join(ROOT, 'src', 'apps', 'desktop');
  // Tauri CLI reads CI and rejects numeric "1" (common in CI providers).
  process.env.CI = 'true';

  const tauriConfig = prepareTauriConfig(join(desktopDir, 'tauri.conf.json'));
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

function prepareTauriConfig(baseConfigPath) {
  const enabled = ['1', 'true', 'yes'].includes(
    String(process.env.BITFUN_ENABLE_UPDATER_ARTIFACTS || '').toLowerCase()
  );
  if (!enabled) {
    return baseConfigPath;
  }

  const pubkey = process.env.TAURI_UPDATER_PUBKEY;
  if (!pubkey) {
    console.error('BITFUN_ENABLE_UPDATER_ARTIFACTS is set, but TAURI_UPDATER_PUBKEY is missing.');
    process.exit(1);
  }
  if (!process.env.TAURI_SIGNING_PRIVATE_KEY) {
    console.error('BITFUN_ENABLE_UPDATER_ARTIFACTS is set, but TAURI_SIGNING_PRIVATE_KEY is missing.');
    process.exit(1);
  }

  const endpoint =
    process.env.TAURI_UPDATER_ENDPOINT ||
    'https://github.com/GCWing/BitFun/releases/latest/download/latest.json';

  const config = JSON.parse(readFileSync(baseConfigPath, 'utf8'));
  config.bundle = {
    ...(config.bundle || {}),
    createUpdaterArtifacts: true,
  };
  config.plugins = {
    ...(config.plugins || {}),
    updater: {
      endpoints: [endpoint],
      pubkey,
      windows: {
        installMode: 'passive',
      },
    },
  };

  const generatedDir = join(ROOT, '.bitfun', 'build');
  mkdirSync(generatedDir, { recursive: true });
  const generatedConfig = join(generatedDir, 'tauri.updater.conf.json');
  writeFileSync(generatedConfig, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
  console.log(`[tauri-build] Updater artifacts enabled: ${endpoint}`);
  return generatedConfig;
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
