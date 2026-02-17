/**
 * BitFun Installer Build Script
 *
 * This script automates the full installer build process:
 * 1. Build the BitFun main application (without bundling)
 * 2. Package the app files into a payload archive
 * 3. Build the installer Tauri application
 *
 * Usage:
 *   node scripts/build-installer.cjs [--skip-app-build] [--dev]
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const BITFUN_ROOT = path.resolve(ROOT, '..');
const PAYLOAD_DIR = path.join(ROOT, 'src-tauri', 'payload');

const args = process.argv.slice(2);
const skipAppBuild = args.includes('--skip-app-build');
const isDev = args.includes('--dev');

function log(msg) {
  console.log(`\x1b[36m[installer]\x1b[0m ${msg}`);
}

function error(msg) {
  console.error(`\x1b[31m[installer]\x1b[0m ${msg}`);
  process.exit(1);
}

function run(cmd, cwd = ROOT) {
  log(`> ${cmd}`);
  try {
    execSync(cmd, { cwd, stdio: 'inherit' });
  } catch (e) {
    error(`Command failed: ${cmd}`);
  }
}

// ── Step 1: Build the main BitFun application ──
if (!skipAppBuild) {
  log('Step 1: Building BitFun main application...');
  run('npm run desktop:build:exe', BITFUN_ROOT);
} else {
  log('Step 1: Skipped (--skip-app-build)');
}

// ── Step 2: Prepare payload ──
log('Step 2: Preparing installer payload...');

// Locate the built application
const possiblePaths = [
  path.join(BITFUN_ROOT, 'src', 'apps', 'desktop', 'target', 'release', 'bitfun-desktop.exe'),
  path.join(BITFUN_ROOT, 'src', 'apps', 'desktop', 'target', 'release', 'BitFun.exe'),
  path.join(BITFUN_ROOT, 'target', 'release', 'bitfun-desktop.exe'),
];

let appExePath = null;
for (const p of possiblePaths) {
  if (fs.existsSync(p)) {
    appExePath = p;
    break;
  }
}

if (!appExePath && !skipAppBuild) {
  error('Could not find built BitFun executable. Check the build output.');
}

if (appExePath) {
  // Create payload directory
  if (fs.existsSync(PAYLOAD_DIR)) {
    fs.rmSync(PAYLOAD_DIR, { recursive: true });
  }
  fs.mkdirSync(PAYLOAD_DIR, { recursive: true });

  // Copy the executable
  const destExe = path.join(PAYLOAD_DIR, 'BitFun.exe');
  fs.copyFileSync(appExePath, destExe);
  log(`Copied: ${appExePath} -> ${destExe}`);

  // Copy WebView2 resources and other runtime files if they exist
  const releaseDir = path.dirname(appExePath);
  const runtimeFiles = fs.readdirSync(releaseDir).filter((f) => {
    return f.endsWith('.dll') || f === 'WebView2Loader.dll';
  });
  for (const file of runtimeFiles) {
    const src = path.join(releaseDir, file);
    const dest = path.join(PAYLOAD_DIR, file);
    fs.copyFileSync(src, dest);
    log(`Copied runtime: ${file}`);
  }
} else {
  log('No app executable found. Payload directory will be empty (dev mode).');
  fs.mkdirSync(PAYLOAD_DIR, { recursive: true });
}

// ── Step 3: Build the installer ──
log('Step 3: Building installer...');

if (isDev) {
  run('npm run tauri:dev');
} else {
  run('npm run tauri:build');
}

log('✓ Installer build complete!');
log(`Output: ${path.join(ROOT, 'src-tauri', 'target', 'release', 'bundle')}`);
