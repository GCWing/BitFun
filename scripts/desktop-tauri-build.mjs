#!/usr/bin/env node
/**
 * Runs `tauri build` from src/apps/desktop with CI=true.
 * On Windows: shared OpenSSL bootstrap (see ensure-openssl-windows.mjs).
 */
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { ensureOpenSslWindows } from './ensure-openssl-windows.mjs';

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

  const desktopDir = join(ROOT, 'src', 'apps', 'desktop');
  // Tauri CLI reads CI and rejects numeric "1" (common in CI providers).
  process.env.CI = 'true';

  const r = spawnSync('pnpm', ['exec', 'tauri', 'build', ...forward], {
    cwd: desktopDir,
    env: process.env,
    stdio: 'inherit',
    shell: true,
  });

  if (r.error) {
    console.error(r.error);
    process.exit(1);
  }
  process.exit(r.status ?? 1);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
