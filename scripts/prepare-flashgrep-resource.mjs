import { createHash, randomUUID } from 'node:crypto';
import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const RESOURCE_DIR = join(ROOT, 'resources', 'flashgrep');
const HOST_TARGETS = {
  'darwin/x64': 'x86_64-apple-darwin',
  'darwin/arm64': 'aarch64-apple-darwin',
  'win32/x64': 'x86_64-pc-windows-msvc',
  'win32/arm64': 'aarch64-pc-windows-msvc',
  'linux/x64': 'x86_64-unknown-linux-musl',
  'linux/arm64': 'aarch64-unknown-linux-musl',
};

export function flashgrepBinaryNames({ target, platform = process.platform, arch = process.arch } = {}) {
  const triple = (target || HOST_TARGETS[`${platform}/${arch}`] || '')
    .replace(/-unknown-linux-gnu$/, '-unknown-linux-musl');
  if (!Object.values(HOST_TARGETS).includes(triple)) {
    throw new Error(`Unsupported Flashgrep target: ${target || `${platform}/${arch}`}`);
  }
  return [`flashgrep-${triple}${triple.includes('windows') ? '.exe' : ''}`];
}

export function flashgrepBinaryName(options) {
  return flashgrepBinaryNames(options)[0];
}

export function flashgrepBinaryPath(options = {}) {
  return join(options.resourceDir || RESOURCE_DIR, flashgrepBinaryName(options));
}

export function downloadFlashgrep(url, destination) {
  // curl supports system proxy settings on all desktop build hosts, including Windows.
  const result = spawnSync(process.platform === 'win32' ? 'curl.exe' : 'curl', [
    '--fail', '--location', '--silent', '--show-error',
    '--proto', '=https', '--proto-redir', '=https',
    '--retry', '3', '--connect-timeout', '20', '--max-time', '180',
    '--output', destination, url,
  ], { encoding: 'utf8', windowsHide: true, shell: false });
  if (result.error || result.status !== 0) {
    throw new Error(`Failed to download Flashgrep: ${result.error?.message || result.stderr || result.status}`);
  }
}

export function ensureFlashgrepBinary(options = {}) {
  const resourceDir = options.resourceDir || RESOURCE_DIR;
  const manifest = options.manifest || JSON.parse(readFileSync(join(RESOURCE_DIR, 'VERSION.json'), 'utf8'));
  const name = flashgrepBinaryName(options);
  const asset = manifest.assets?.[name];
  if (!asset || !/^[a-f0-9]{64}$/.test(asset.sha256) || !Number.isSafeInteger(asset.size) || asset.size <= 0) {
    throw new Error(`Missing or invalid pinned Flashgrep asset: ${name}`);
  }
  if (!/^[\w.-]+\/[\w.-]+$/.test(manifest.repo) || !/^v[\w.-]+$/.test(manifest.tag)) {
    throw new Error('Invalid pinned Flashgrep release');
  }
  const binaryPath = join(resourceDir, name);
  const valid = (path) => {
    if (!existsSync(path)) return false;
    const bytes = readFileSync(path);
    return bytes.length === asset.size && createHash('sha256').update(bytes).digest('hex') === asset.sha256;
  };
  if (!valid(binaryPath)) {
    mkdirSync(resourceDir, { recursive: true });
    const temporary = `${binaryPath}.download-${randomUUID()}`;
    try {
      const url = `https://github.com/${manifest.repo}/releases/download/${manifest.tag}/${name}`;
      (options.download || downloadFlashgrep)(url, temporary);
      if (!valid(temporary)) throw new Error(`Flashgrep SHA-256 or size mismatch: ${name}`);
      renameSync(temporary, binaryPath);
    } finally {
      rmSync(temporary, { force: true });
    }
  }
  if (!name.endsWith('.exe')) chmodSync(binaryPath, 0o755);
  return binaryPath;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const inlineTarget = args.find((arg) => arg.startsWith('--target='));
  const targetIndex = args.indexOf('--target');
  const target = inlineTarget ? inlineTarget.slice('--target='.length) : targetIndex >= 0 ? args[targetIndex + 1] : undefined;
  if (targetIndex >= 0 && !target) throw new Error('--target requires a Rust target triple');
  console.log(ensureFlashgrepBinary({ target }));
}
