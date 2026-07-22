/**
 * Windows: ensure FireDaemon prebuilt OpenSSL for Cargo (russh / libgit2).
 * - Cached under .bitfun/cache/firedaemon-openssl-<version>/x64 (gitignored).
 * - Skips download if OPENSSL_DIR already points at a valid tree, or cache hit, or BITFUN_SKIP_OPENSSL_BOOTSTRAP=1.
 * Mutates `process.env` by default so child processes (tauri, cargo) inherit OPENSSL_*.
 */
import { spawnSync } from 'child_process';
import { createWriteStream, existsSync, mkdirSync, realpathSync } from 'fs';
import { pipeline } from 'stream/promises';
import { Readable } from 'stream';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const LOG = '[bitfun-openssl]';

// Keep in sync with $Version in scripts/ci/setup-openssl-windows.ps1.
export const OPENSSL_VERSION = '3.5.5';
const OPENSSL_URL = `https://download.firedaemon.com/FireDaemon-OpenSSL/openssl-${OPENSSL_VERSION}.zip`;
export const CACHE_ROOT = join(ROOT, '.bitfun', 'cache', `firedaemon-openssl-${OPENSSL_VERSION}`);

function libcryptoPath(opensslDir) {
  return join(opensslDir, 'lib', 'libcrypto.lib');
}

function opensslDirLooksValid(dir) {
  return Boolean(dir && existsSync(libcryptoPath(dir)));
}

async function downloadToFile(url, filePath) {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`OpenSSL download failed: HTTP ${res.status} ${res.statusText}`);
  }
  if (!res.body) {
    throw new Error('OpenSSL download failed: empty body');
  }
  await pipeline(Readable.fromWeb(res.body), createWriteStream(filePath));
}

function extractZipWindows(zipPath, destDir) {
  const esc = (p) => p.replace(/'/g, "''");
  const ps = `Expand-Archive -LiteralPath '${esc(zipPath)}' -DestinationPath '${esc(destDir)}' -Force`;
  const r = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', ps], {
    stdio: 'inherit',
    cwd: ROOT,
  });
  if (r.error) {
    throw r.error;
  }
  if (r.status !== 0) {
    throw new Error('Expand-Archive failed (PowerShell)');
  }
}

/**
 * No-op on non-Windows. On Windows, sets process.env OPENSSL_DIR / OPENSSL_LIB_DIR / OPENSSL_STATIC when needed.
 */
export async function ensureOpenSslWindows() {
  if (process.platform !== 'win32') {
    return;
  }

  if (process.env.BITFUN_SKIP_OPENSSL_BOOTSTRAP === '1') {
    console.log(`${LOG} BITFUN_SKIP_OPENSSL_BOOTSTRAP=1, skipping bootstrap`);
    return;
  }

  if (opensslDirLooksValid(process.env.OPENSSL_DIR)) {
    const dir = process.env.OPENSSL_DIR;
    if (!process.env.OPENSSL_LIB_DIR) {
      process.env.OPENSSL_LIB_DIR = join(dir, 'lib');
    }
    if (!process.env.OPENSSL_STATIC) {
      process.env.OPENSSL_STATIC = '1';
    }
    console.log(`${LOG} Using existing OPENSSL_DIR:`, dir);
    return;
  }

  mkdirSync(CACHE_ROOT, { recursive: true });
  const x64 = join(CACHE_ROOT, 'x64');

  if (existsSync(libcryptoPath(x64))) {
    process.env.OPENSSL_DIR = x64;
    process.env.OPENSSL_LIB_DIR = join(x64, 'lib');
    process.env.OPENSSL_STATIC = '1';
    console.log(`${LOG} Using cached OpenSSL:`, x64);
    return;
  }

  const zipFile = join(CACHE_ROOT, 'dist.zip');
  if (!existsSync(zipFile)) {
    console.log(`${LOG} Downloading prebuilt OpenSSL (cached for future builds)...`);
    await downloadToFile(OPENSSL_URL, zipFile);
  } else {
    console.log(`${LOG} Re-using cached dist.zip, extracting...`);
  }
  extractZipWindows(zipFile, CACHE_ROOT);

  if (!existsSync(libcryptoPath(x64))) {
    throw new Error(
      `${LOG} Unexpected layout after extract (missing ${libcryptoPath(x64)}). Delete ${CACHE_ROOT} and retry.`,
    );
  }

  process.env.OPENSSL_DIR = x64;
  process.env.OPENSSL_LIB_DIR = join(x64, 'lib');
  process.env.OPENSSL_STATIC = '1';
  console.log(`${LOG} OpenSSL ready:`, x64);
}

function isExecutedAsCli() {
  const entry = process.argv[1];
  if (!entry) return false;
  try {
    const selfPath = realpathSync(fileURLToPath(import.meta.url));
    const entryPath = realpathSync(entry);
    return selfPath === entryPath;
  } catch {
    return false;
  }
}

function printShellEnvHint() {
  const dir = process.env.OPENSSL_DIR;
  const lib = process.env.OPENSSL_LIB_DIR;
  if (!dir || !lib) {
    console.log(
      `${LOG} No OPENSSL_DIR set in this process (skipped or use your own install). Raw cargo needs these in the shell.`,
    );
    return;
  }
  console.log(`${LOG} For shells that do not inherit Node env (e.g. raw cargo), run in PowerShell before build:`);
  console.log(`  $env:OPENSSL_DIR="${dir}"`);
  console.log(`  $env:OPENSSL_LIB_DIR="${lib}"`);
  console.log(`  $env:OPENSSL_STATIC="1"`);
}

if (isExecutedAsCli()) {
  ensureOpenSslWindows()
    .then(() => {
      if (process.platform !== 'win32') {
        console.log(`${LOG} Not Windows; nothing to do.`);
        return;
      }
      printShellEnvHint();
    })
    .catch((e) => {
      console.error(e);
      process.exit(1);
    });
}
