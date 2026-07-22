import { chmodSync, existsSync, statSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const RESOURCE_DIR = join(ROOT, 'resources', 'flashgrep');

export function flashgrepBinaryNames() {
  if (process.platform === 'win32' && process.arch === 'x64') {
    return ['flashgrep-x86_64-pc-windows-msvc.exe'];
  }
  if (process.platform === 'win32' && process.arch === 'arm64') {
    return ['flashgrep-aarch64-pc-windows-msvc.exe'];
  }
  if (process.platform === 'darwin' && process.arch === 'x64') {
    return ['flashgrep-x86_64-apple-darwin'];
  }
  if (process.platform === 'darwin' && process.arch === 'arm64') {
    return ['flashgrep-aarch64-apple-darwin'];
  }
  if (process.platform === 'linux' && process.arch === 'x64') {
    return [
      'flashgrep-x86_64-unknown-linux-musl',
      'flashgrep-x86_64-unknown-linux-gnu',
    ];
  }
  if (process.platform === 'linux' && process.arch === 'arm64') {
    return [
      'flashgrep-aarch64-unknown-linux-musl',
      'flashgrep-aarch64-unknown-linux-gnu',
    ];
  }
  return [process.platform === 'win32' ? 'flashgrep.exe' : 'flashgrep'];
}

export function flashgrepBinaryName() {
  return flashgrepBinaryNames()[0];
}

export function flashgrepBinaryPath() {
  const availableBinaryName =
    flashgrepBinaryNames().find((binaryName) => existsSync(join(RESOURCE_DIR, binaryName))) ??
    flashgrepBinaryName();
  return join(RESOURCE_DIR, availableBinaryName);
}

export function ensureFlashgrepBinary() {
  for (const binaryName of flashgrepBinaryNames()) {
    const binaryPath = join(RESOURCE_DIR, binaryName);
    if (!existsSync(binaryPath)) {
      continue;
    }

    if (process.platform !== 'win32') {
      chmodSync(binaryPath, statSync(binaryPath).mode | 0o111);
    }
    return binaryPath;
  }

  throw new Error(
    `flashgrep binary not found for ${process.platform}/${process.arch}. Expected one of: ${flashgrepBinaryNames()
      .map((name) => `resources/flashgrep/${name}`)
      .join(', ')}`
  );
}
