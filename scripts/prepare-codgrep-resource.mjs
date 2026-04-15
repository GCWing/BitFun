import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  statSync,
} from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const RESOURCE_DIR = join(ROOT, 'build-resources', 'codgrep');

export function codgrepBinaryName() {
  return process.platform === 'win32' ? 'cg.exe' : 'cg';
}

export function codgrepBinaryPath(profile) {
  return join(ROOT, 'target', profile, codgrepBinaryName());
}

export function ensureCodgrepResource(profile) {
  const binaryPath = codgrepBinaryPath(profile);
  if (!existsSync(binaryPath)) {
    throw new Error(`codgrep binary not found: ${binaryPath}`);
  }

  mkdirSync(RESOURCE_DIR, { recursive: true });
  const bundledPath = join(RESOURCE_DIR, codgrepBinaryName());
  copyFileSync(binaryPath, bundledPath);
  if (process.platform !== 'win32') {
    chmodSync(bundledPath, statSync(binaryPath).mode);
  }
  return bundledPath;
}
