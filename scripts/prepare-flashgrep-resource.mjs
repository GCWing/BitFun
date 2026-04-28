import { chmodSync, existsSync, statSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const RESOURCE_DIR = join(ROOT, 'resources', 'flashgrep');

export function flashgrepBinaryName() {
  return process.platform === 'win32' ? 'flashgrep.exe' : 'flashgrep';
}

export function flashgrepBinaryPath() {
  return join(RESOURCE_DIR, flashgrepBinaryName());
}

export function ensureFlashgrepBinary() {
  const binaryPath = flashgrepBinaryPath();
  if (!existsSync(binaryPath)) {
    throw new Error(
      `flashgrep binary not found: ${binaryPath}. Put the prebuilt daemon binary at resources/flashgrep/${flashgrepBinaryName()}`
    );
  }

  if (process.platform !== 'win32') {
    chmodSync(binaryPath, statSync(binaryPath).mode | 0o111);
  }
  return binaryPath;
}
