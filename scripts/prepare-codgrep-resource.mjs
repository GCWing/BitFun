import { chmodSync, existsSync, statSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const RESOURCE_DIR = join(ROOT, 'resources', 'codgrep');

export function codgrepBinaryName() {
  return process.platform === 'win32' ? 'cg.exe' : 'cg';
}

export function codgrepBinaryPath() {
  return join(RESOURCE_DIR, codgrepBinaryName());
}

export function ensureCodgrepBinary() {
  const binaryPath = codgrepBinaryPath();
  if (!existsSync(binaryPath)) {
    throw new Error(
      `codgrep binary not found: ${binaryPath}. Put the prebuilt daemon binary at resources/codgrep/${codgrepBinaryName()}`
    );
  }

  if (process.platform !== 'win32') {
    chmodSync(binaryPath, statSync(binaryPath).mode | 0o111);
  }
  return binaryPath;
}
