#!/usr/bin/env node

/**
 * Shared mobile-web build helpers for desktop dev/build flows.
 *
 * We must clean copied resources in Tauri target directories before rebuilding,
 * otherwise old Vite hashed assets remain in target profile mobile-web folders and get bundled
 * or uploaded by remote connect along with the latest files.
 */

const { execSync } = require('child_process');
const path = require('path');
const {
  printInfo,
  printSuccess,
  printError,
} = require('./console-style.cjs');

const ROOT_DIR = path.resolve(__dirname, '..');

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

function runSilent(command, cwd = ROOT_DIR) {
  try {
    const stdout = execSync(command, {
      cwd,
      stdio: 'pipe',
      encoding: 'buffer',
    });
    return { ok: true, stdout: decodeOutput(stdout), stderr: '' };
  } catch (error) {
    const stdout = error.stdout ? decodeOutput(error.stdout) : '';
    const stderr = error.stderr ? decodeOutput(error.stderr) : '';
    return { ok: false, stdout, stderr, error };
  }
}

function runInherit(command, cwd = ROOT_DIR) {
  try {
    execSync(command, { cwd, stdio: 'inherit' });
    return { ok: true, error: null };
  } catch (error) {
    return { ok: false, error };
  }
}

function cleanStaleMobileWebResources(logInfo = printInfo) {
  const fs = require('fs');
  const targetDir = path.join(ROOT_DIR, 'target');
  if (!fs.existsSync(targetDir)) return 0;

  let cleaned = 0;
  for (const profile of fs.readdirSync(targetDir)) {
    const mobileWebDir = path.join(targetDir, profile, 'mobile-web');
    if (fs.existsSync(mobileWebDir) && fs.statSync(mobileWebDir).isDirectory()) {
      fs.rmSync(mobileWebDir, { recursive: true, force: true });
      cleaned++;
    }
  }

  if (cleaned > 0) {
    logInfo(`Cleaned stale mobile-web resources from ${cleaned} target profile(s)`);
  }

  return cleaned;
}

function buildMobileWeb(options = {}) {
  const {
    install = false,
    logInfo = printInfo,
    logSuccess = printSuccess,
    logError = printError,
  } = options;

  const mobileWebDir = path.join(ROOT_DIR, 'src/mobile-web');

  cleanStaleMobileWebResources(logInfo);

  if (install) {
    const installResult = runSilent('pnpm install --silent', mobileWebDir);
    if (!installResult.ok) {
      logError('mobile-web pnpm install failed');
      const output = tailOutput(installResult.stderr || installResult.stdout);
      if (output) {
        logError(output);
      } else if (installResult.error?.message) {
        logError(installResult.error.message);
      }
      return { ok: false };
    }
  }

  const buildResult = runInherit('pnpm run build', mobileWebDir);
  if (!buildResult.ok) {
    logError('mobile-web build failed');
    if (buildResult.error?.message) {
      logError(buildResult.error.message);
    }
    return { ok: false };
  }

  logSuccess('mobile-web build complete');
  return { ok: true };
}

if (require.main === module) {
  const result = buildMobileWeb();
  process.exit(result.ok ? 0 : 1);
}

module.exports = {
  buildMobileWeb,
  cleanStaleMobileWebResources,
};
