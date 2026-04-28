#!/usr/bin/env node

/**
 * Development environment startup script
 * Manages pre-build tasks and dev server startup
 */

const fs = require('fs');
const net = require('net');
const { execSync, spawn } = require('child_process');
const path = require('path');
const { pathToFileURL } = require('url');
const {
  printHeader,
  printSuccess,
  printInfo,
  printError,
  printStep,
  printComplete,
  printBlank,
} = require('./console-style.cjs');
const { buildMobileWeb } = require('./mobile-web-build.cjs');

const ROOT_DIR = path.resolve(__dirname, '..');
const DEV_SERVER_PORT = 1422;
const DEV_SERVER_HOSTS = ['localhost', '127.0.0.1', '::1'];
const DESKTOP_PREVIEW_REBUILD_INPUTS = [
  path.join(ROOT_DIR, 'Cargo.toml'),
  path.join(ROOT_DIR, 'src', 'apps', 'desktop'),
  path.join(ROOT_DIR, 'src', 'crates', 'core'),
  path.join(ROOT_DIR, 'src', 'crates', 'transport'),
  path.join(ROOT_DIR, 'src', 'crates', 'api-layer'),
  path.join(ROOT_DIR, 'src', 'crates', 'events'),
  path.join(ROOT_DIR, 'src', 'crates', 'ai-adapters'),
  path.join(ROOT_DIR, 'src', 'crates', 'webdriver'),
];
const DESKTOP_PREVIEW_REBUILD_IGNORED_DIRS = new Set([
  '.bitfun',
  '.git',
  'coverage',
  'dist',
  'node_modules',
  'target',
]);
const DESKTOP_PREVIEW_REBUILD_RELEVANT_EXTENSIONS = new Set([
  '.ftl',
  '.json',
  '.md',
  '.rs',
  '.toml',
  '.yaml',
  '.yml',
]);
const DESKTOP_PREVIEW_REBUILD_IGNORED_BASENAMES = new Set([
  'AGENTS-CN.md',
  'AGENTS.md',
  'CONTRIBUTING.md',
  'CONTRIBUTING_CN.md',
  'README.md',
  'README.zh-CN.md',
  'README_CN.md',
]);

function isDesktopMode(mode) {
  return mode === 'desktop' || mode === 'desktop-preview';
}

function getDesktopBinaryPath() {
  const suffix = process.platform === 'win32' ? '.exe' : '';
  const binaryName = `bitfun-desktop${suffix}`;

  if (process.platform === 'darwin') {
    return path.join(ROOT_DIR, 'target', 'debug', 'BitFun.app', 'Contents', 'MacOS', 'BitFun');
  }

  return path.join(ROOT_DIR, 'target', 'debug', binaryName);
}

/**
 * Run command synchronously (silent mode)
 */
function runSilent(command, cwd = ROOT_DIR) {
  try {
    const stdout = execSync(command, { 
      cwd, 
      stdio: 'pipe',
      encoding: 'buffer'
    });
    return { ok: true, stdout: decodeOutput(stdout), stderr: '' };
  } catch (error) {
    const stdout = error.stdout ? decodeOutput(error.stdout) : '';
    const stderr = error.stderr ? decodeOutput(error.stderr) : '';
    return { ok: false, stdout, stderr, error };
  }
}

function decodeOutput(output) {
  if (!output) return '';
  if (typeof output === 'string') return output;
  const buffer = Buffer.isBuffer(output) ? output : Buffer.from(output);
  if (process.platform !== 'win32') return buffer.toString('utf-8');

  const utf8 = buffer.toString('utf-8');
  if (!utf8.includes('�')) return utf8;

  try {
    const { TextDecoder } = require('util');
    const decoder = new TextDecoder('gbk');
    const gbk = decoder.decode(buffer);
    if (gbk && !gbk.includes('�')) return gbk;
    return gbk || utf8;
  } catch (error) {
    return utf8;
  }
}

function tailOutput(output, maxLines = 12) {
  if (!output) return '';
  const lines = output
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim() !== '');
  if (lines.length <= maxLines) return lines.join('\n');
  return lines.slice(-maxLines).join('\n');
}

/**
 * Run command with inherited output
 */
function runInherit(command, cwd = ROOT_DIR) {
  try {
    execSync(command, { cwd, stdio: 'inherit' });
    return { ok: true, error: null };
  } catch (error) {
    return { ok: false, error };
  }
}

/**
 * Run command and show output
 */
function runCommand(command, cwd = ROOT_DIR) {
  return new Promise((resolve, reject) => {
    const isWindows = process.platform === 'win32';
    const shell = isWindows ? 'cmd.exe' : '/bin/sh';
    const shellArgs = isWindows ? ['/c', command] : ['-c', command];
    
    const child = spawn(shell, shellArgs, {
      cwd,
      stdio: 'inherit'
    });
    
    child.on('close', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`Command failed with code ${code}`));
      }
    });
    
    child.on('error', reject);
  });
}

/**
 * Spawn a command with explicit args array (no shell interpolation, safe for paths with spaces)
 */
function spawnCommand(cmd, args, cwd = ROOT_DIR, envOverrides = {}, shell = false) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, {
      cwd,
      stdio: 'inherit',
      shell,
      env: {
        ...process.env,
        ...envOverrides,
      },
    });

    child.on('close', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`Command failed with code ${code}`));
      }
    });

    child.on('error', reject);
  });
}

function spawnBackgroundCommand(cmd, args, cwd = ROOT_DIR, env = process.env) {
  return spawn(cmd, args, {
    cwd,
    stdio: 'inherit',
    env,
  });
}

function spawnWindowsCommand(command, cwd = ROOT_DIR, env = process.env) {
  return spawn(process.env.ComSpec || 'C:\\Windows\\System32\\cmd.exe', ['/d', '/s', '/c', command], {
    cwd,
    stdio: 'inherit',
    env,
  });
}

function spawnWindowsCommandArgs(command, args, cwd = ROOT_DIR, env = process.env) {
  return spawn(process.env.ComSpec || 'C:\\Windows\\System32\\cmd.exe', ['/d', '/s', '/c', command, ...args], {
    cwd,
    stdio: 'inherit',
    env,
  });
}

function runWindowsCommandArgs(command, args, cwd = ROOT_DIR, env = process.env) {
  return new Promise((resolve, reject) => {
    const child = spawnWindowsCommandArgs(command, args, cwd, env);

    child.on('close', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`Command failed with code ${code}`));
      }
    });

    child.on('error', reject);
  });
}

function stopChildProcess(child) {
  if (!child || child.exitCode !== null) {
    return;
  }

  if (process.platform === 'win32') {
    try {
      execSync(`taskkill /pid ${child.pid} /T /F >nul 2>&1`);
      return;
    } catch (error) {
      // Fall through to a best-effort kill below.
    }
  }

  try {
    child.kill('SIGTERM');
  } catch (error) {
    // Ignore cleanup failures on shutdown paths.
  }
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function isPortOpen(port, hosts = DEV_SERVER_HOSTS) {
  return Promise.any(hosts.map((host) => {
    return new Promise((resolve, reject) => {
      const client = new net.Socket();
      client.setTimeout(1500);
      client.connect(port, host, () => {
        client.destroy();
        resolve(true);
      });
      client.on('error', (error) => {
        client.destroy();
        reject(error);
      });
      client.on('timeout', () => {
        client.destroy();
        reject(new Error(`Timeout connecting to ${host}:${port}`));
      });
    });
  })).then(() => true).catch(() => false);
}

async function waitForPort(port, hosts = DEV_SERVER_HOSTS, timeoutMs = 30000) {
  const startedAt = Date.now();

  while (Date.now() - startedAt < timeoutMs) {
    if (await isPortOpen(port, hosts)) {
      return;
    }
    await wait(500);
  }

  throw new Error(`Port ${port} did not become ready within ${timeoutMs}ms`);
}

async function ensureDesktopOpenSslIfNeeded() {
  if (process.platform !== 'win32') {
    return;
  }

  printInfo('Windows: ensuring prebuilt OpenSSL (cached under .bitfun/cache/)');
  try {
    const { ensureOpenSslWindows } = await import(
      pathToFileURL(path.join(__dirname, 'ensure-openssl-windows.mjs')).href
    );
    await ensureOpenSslWindows();
  } catch (error) {
    printError('OpenSSL bootstrap failed');
    printError(error.message || String(error));
    process.exit(1);
  }
}

async function rebuildDesktopDebugBinary() {
  await ensureDesktopOpenSslIfNeeded();

  const buildEnv = {
    ...process.env,
    CARGO_PROFILE_DEV_DEBUG: process.env.CARGO_PROFILE_DEV_DEBUG || '0',
    CARGO_PROFILE_DEV_INCREMENTAL: process.env.CARGO_PROFILE_DEV_INCREMENTAL || 'true',
    CARGO_PROFILE_DEV_CODEGEN_UNITS: process.env.CARGO_PROFILE_DEV_CODEGEN_UNITS || '256',
  };

  printInfo('Building bitfun-desktop in dev mode with reduced debug info for faster local relink');
  printInfo(
    `Fast local build env: CARGO_PROFILE_DEV_DEBUG=${buildEnv.CARGO_PROFILE_DEV_DEBUG}, ` +
    `CARGO_PROFILE_DEV_CODEGEN_UNITS=${buildEnv.CARGO_PROFILE_DEV_CODEGEN_UNITS}`
  );

  await spawnCommand(
    process.platform === 'win32' ? 'cargo.exe' : 'cargo',
    ['build', '-p', 'bitfun-desktop'],
    ROOT_DIR,
    buildEnv,
  );
}

function getNewestTrackedInput(entryPath) {
  if (!fs.existsSync(entryPath)) {
    return null;
  }

  const stat = fs.lstatSync(entryPath);
  if (stat.isSymbolicLink()) {
    return null;
  }

  if (stat.isFile()) {
    const basename = path.basename(entryPath);
    if (DESKTOP_PREVIEW_REBUILD_IGNORED_BASENAMES.has(basename)) {
      return null;
    }

    const ext = path.extname(entryPath).toLowerCase();
    if (!DESKTOP_PREVIEW_REBUILD_RELEVANT_EXTENSIONS.has(ext)) {
      return null;
    }

    if (ext === '.md' && !entryPath.includes(`${path.sep}prompts${path.sep}`)) {
      return null;
    }

    return {
      path: entryPath,
      mtimeMs: stat.mtimeMs,
    };
  }

  if (!stat.isDirectory()) {
    return null;
  }

  let newest = null;
  for (const entry of fs.readdirSync(entryPath, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) {
      continue;
    }
    if (entry.isDirectory() && DESKTOP_PREVIEW_REBUILD_IGNORED_DIRS.has(entry.name)) {
      continue;
    }

    const candidate = getNewestTrackedInput(path.join(entryPath, entry.name));
    if (candidate && (!newest || candidate.mtimeMs > newest.mtimeMs)) {
      newest = candidate;
    }
  }

  return newest;
}

function getDesktopPreviewRebuildPlan(desktopBinary, forceRebuild = false) {
  if (forceRebuild) {
    return {
      shouldRebuild: true,
      reason: 'Force rebuild requested for desktop preview',
    };
  }

  if (!fs.existsSync(desktopBinary)) {
    return {
      shouldRebuild: true,
      reason: 'Debug desktop binary is missing',
    };
  }

  const binaryMtimeMs = fs.statSync(desktopBinary).mtimeMs;
  let newestInput = null;

  for (const input of DESKTOP_PREVIEW_REBUILD_INPUTS) {
    const candidate = getNewestTrackedInput(input);
    if (candidate && (!newestInput || candidate.mtimeMs > newestInput.mtimeMs)) {
      newestInput = candidate;
    }
  }

  if (newestInput && newestInput.mtimeMs > binaryMtimeMs) {
    return {
      shouldRebuild: true,
      reason: `Rust / Tauri inputs changed since the last preview (${path.relative(ROOT_DIR, newestInput.path)})`,
    };
  }

  return {
    shouldRebuild: false,
    reason: `Reusing debug desktop binary: ${path.relative(ROOT_DIR, desktopBinary)}`,
  };
}

async function ensureDesktopDebugBinaryForPreview(forceRebuild = false) {
  const desktopBinary = getDesktopBinaryPath();
  const rebuildPlan = getDesktopPreviewRebuildPlan(desktopBinary, forceRebuild);

  if (!rebuildPlan.shouldRebuild) {
    printInfo(rebuildPlan.reason);
    return desktopBinary;
  }

  printInfo(`${rebuildPlan.reason}; rebuilding before preview`);
  await rebuildDesktopDebugBinary();
  printSuccess('Debug desktop binary rebuilt for preview');
  return desktopBinary;
}

async function startDesktopPreview() {
  const desktopBinary = getDesktopBinaryPath();

  if (!fs.existsSync(desktopBinary)) {
    printError(`Debug desktop binary not found: ${desktopBinary}`);
    printInfo('Retry with `pnpm run desktop:preview:debug -- --force-rebuild` or build it with `cargo build -p bitfun-desktop`');
    process.exit(1);
  }

  let appProcess = null;
  let devServerProcess = null;
  let ownsDevServer = false;
  let shuttingDown = false;

  const cleanup = () => {
    stopChildProcess(appProcess);
    if (ownsDevServer) {
      stopChildProcess(devServerProcess);
    }
  };

  const shutdown = (exitCode = 0) => {
    if (shuttingDown) {
      return;
    }

    shuttingDown = true;
    cleanup();
    process.exit(exitCode);
  };

  process.on('SIGINT', () => {
    printInfo('Stopping desktop preview...');
    shutdown(0);
  });
  process.on('SIGTERM', () => {
    printInfo('Stopping desktop preview...');
    shutdown(0);
  });

  if (await isPortOpen(DEV_SERVER_PORT)) {
    printInfo(`Reusing web UI dev server on http://localhost:${DEV_SERVER_PORT}`);
  } else {
    printInfo(`Starting web UI dev server on http://localhost:${DEV_SERVER_PORT}`);
    const viteArgs = ['--dir', 'src/web-ui', 'exec', 'vite', '--host', 'localhost', '--port', String(DEV_SERVER_PORT)];
    const viteEnv = {
      ...process.env,
      TAURI_DEV_HOST: 'localhost',
    };

    devServerProcess = process.platform === 'win32'
      ? spawnWindowsCommand(`pnpm ${viteArgs.join(' ')}`, ROOT_DIR, viteEnv)
      : spawnBackgroundCommand('pnpm', viteArgs, ROOT_DIR, viteEnv);
    ownsDevServer = true;

    devServerProcess.on('error', (error) => {
      printError(`Web UI dev server failed to start: ${error.message || String(error)}`);
      shutdown(1);
    });

    devServerProcess.on('exit', (code, signal) => {
      devServerProcess = null;
      ownsDevServer = false;
      if (!appProcess && !shuttingDown && code !== 0) {
        printError(`Web UI dev server exited before desktop launch (code=${code ?? 'null'}, signal=${signal ?? 'null'})`);
        shutdown(code ?? 1);
        return;
      }
      if (appProcess && appProcess.exitCode === null && !shuttingDown) {
        printError(`Web UI dev server exited unexpectedly (code=${code ?? 'null'}, signal=${signal ?? 'null'})`);
        shutdown(code ?? 1);
      }
    });

    try {
      await waitForPort(DEV_SERVER_PORT);
    } catch (error) {
      printError(error.message || String(error));
      shutdown(1);
    }

    printSuccess(`Web UI dev server is ready on http://localhost:${DEV_SERVER_PORT}`);
  }

  printInfo(`Launching debug desktop binary: ${desktopBinary}`);

  appProcess = spawnBackgroundCommand(desktopBinary, [], ROOT_DIR, {
    ...process.env,
  });

  appProcess.on('error', (error) => {
    printError(`Desktop preview failed to start: ${error.message || String(error)}`);
    shutdown(1);
  });

  appProcess.on('exit', (code, signal) => {
    if (!shuttingDown) {
      printInfo(`Desktop preview exited (code=${code ?? 'null'}, signal=${signal ?? 'null'})`);
    }
    shutdown(code ?? 0);
  });

  printSuccess('Desktop preview is running');
  printInfo('Front-end edits continue to use Vite HMR; rebuild Rust only when desktop-side code changes');

  await new Promise(() => {});
}

function flashgrepBinaryName() {
  return process.platform === 'win32' ? 'flashgrep.exe' : 'flashgrep';
}

function flashgrepBinaryPath() {
  return path.join(ROOT_DIR, 'resources', 'flashgrep', flashgrepBinaryName());
}

function ensureFlashgrepBinary() {
  const binaryPath = flashgrepBinaryPath();
  if (!fs.existsSync(binaryPath)) {
    return {
      ok: false,
      error: new Error(
        `flashgrep binary not found: ${binaryPath}. Put the prebuilt daemon binary at resources/flashgrep/${flashgrepBinaryName()}`
      ),
    };
  }

  return { ok: true, binaryPath };
}

async function ensureFlashgrepBundleResource() {
  const helperUrl = pathToFileURL(path.join(__dirname, 'prepare-flashgrep-resource.mjs')).href;
  const helper = await import(helperUrl);
  return helper.ensureFlashgrepBinary();
}

/**
 * Main entry
 */
async function main() {
  const startTime = Date.now();
  let mode = process.argv[2] || 'web'; // web | desktop
  const extraArgs = process.argv.slice(3);
  let forceDesktopPreviewRebuild = extraArgs.includes('--force-rebuild');

  if (mode === 'desktop-preview-rebuild') {
    mode = 'desktop-preview';
    forceDesktopPreviewRebuild = true;
  }

  const desktopMode = isDesktopMode(mode);
  const modeLabelMap = {
    desktop: 'Desktop',
    'desktop-preview': 'Desktop Debug Preview',
    web: 'Web',
  };
  const modeLabel = modeLabelMap[mode] || 'Web';
  
  printHeader(`BitFun ${modeLabel} Development`);
  printBlank();

  const totalSteps = desktopMode ? 5 : 3;
  let currentStep = 1;

  // Step 1: Copy resources
  printStep(currentStep++, totalSteps, 'Copy resources');
  const copyResult = runSilent('pnpm run copy-monaco --silent');
  if (copyResult.ok) {
    printSuccess('Monaco Editor resources ready');
  } else {
    printError('Copy resources failed');
    const output = tailOutput(copyResult.stderr || copyResult.stdout);
    if (output) {
      printError(output);
    } else if (copyResult.error) {
      printError(copyResult.error.message);
    }
    if (copyResult.error && copyResult.error.status !== undefined) {
      printError(`Exit code: ${copyResult.error.status}`);
    }
    printInfo('Hint: run `pnpm install` in repo root if dependencies are missing');
    process.exit(1);
  }
  
  // Step 2: Generate version info
  printStep(currentStep++, totalSteps, 'Generate version info');
  const versionResult = runInherit('node scripts/generate-version.cjs');
  if (!versionResult.ok) {
    printError('Generate version info failed');
    if (versionResult.error && versionResult.error.message) {
      printError(versionResult.error.message);
    }
    if (versionResult.error && versionResult.error.status !== undefined) {
      printError(`Exit code: ${versionResult.error.status}`);
    }
    process.exit(1);
  }
  
  const prepTime = ((Date.now() - startTime) / 1000).toFixed(1);
  
  // Step 3: Build mobile-web (desktop only)
  if (desktopMode) {
    printStep(currentStep++, totalSteps, 'Build mobile-web');
    const mobileWebResult = buildMobileWeb({
      install: true,
      logInfo: printInfo,
      logSuccess: printSuccess,
      logError: printError,
    });
    if (!mobileWebResult.ok) {
      process.exit(1);
    }

    printStep(currentStep++, totalSteps, 'Build workspace search daemon');
    const flashgrepResult = ensureFlashgrepBinary();
    if (!flashgrepResult.ok) {
      printError('Workspace search daemon is missing');
      if (flashgrepResult.error && flashgrepResult.error.message) {
        printError(flashgrepResult.error.message);
      }
      if (flashgrepResult.error && flashgrepResult.error.status !== undefined) {
        printError(`Exit code: ${flashgrepResult.error.status}`);
      }
      process.exit(1);
    }
    process.env.FLASHGREP_DAEMON_BIN = flashgrepResult.binaryPath;

    try {
      await ensureFlashgrepBundleResource();
    } catch (error) {
      printError('Validate workspace search daemon failed');
      printError(error instanceof Error ? error.message : String(error));
      process.exit(1);
    }
  }

  // Final step: Start dev server
  const startStepLabel = mode === 'desktop-preview'
    ? 'Start desktop preview'
    : 'Start dev server';
  printStep(currentStep, totalSteps, startStepLabel);
  printInfo(`Prep took ${prepTime}s`);
  
  printComplete('Initialization complete');
  
  try {
    if (mode === 'desktop') {
      await ensureDesktopOpenSslIfNeeded();
      const desktopDir = path.join(ROOT_DIR, 'src/apps/desktop');
      const tauriConfig = path.join(desktopDir, 'tauri.conf.json');
      if (process.platform === 'win32') {
        // Running the generated .cmd shim directly via spawn is flaky on Windows.
        // Use cmd.exe with an explicit args array so the desktop app directory
        // stays the Tauri project root without pnpm workspace path rewriting.
        const tauriBin = path.join(ROOT_DIR, 'node_modules', '.bin', 'tauri.cmd');
        await runWindowsCommandArgs(tauriBin, ['dev', '--config', tauriConfig], desktopDir, process.env);
      } else {
        const tauriBin = path.join(ROOT_DIR, 'node_modules', '.bin', 'tauri');
        await spawnCommand(tauriBin, ['dev', '--config', tauriConfig], desktopDir);
      }
    } else if (mode === 'desktop-preview') {
      await ensureDesktopDebugBinaryForPreview(forceDesktopPreviewRebuild);
      await startDesktopPreview();
    } else {
      await runCommand('pnpm exec vite', path.join(ROOT_DIR, 'src/web-ui'));
    }
  } catch (error) {
    printError('Dev server failed to start');
    if (error?.message) {
      printError(error.message);
    }
    process.exit(1);
  }
}

main().catch((error) => {
  printError('Startup failed: ' + error.message);
  process.exit(1);
});
