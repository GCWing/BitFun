import { existsSync, mkdirSync, readFileSync, renameSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';

const ARCHIVES = {
  'aarch64-apple-darwin': 'osx-arm64-static-lib',
  'x86_64-apple-darwin': 'osx-x64-static-lib',
  'x86_64-unknown-linux-gnu': 'linux-x64-static-lib',
  'aarch64-unknown-linux-gnu': 'linux-aarch64-static-lib',
  'x86_64-pc-windows-msvc': 'win-x64-static-MT-Release-lib',
};

export function prepareSherpaDev(root, env = process.env, runtime = {}) {
  if (env.SHERPA_ONNX_LIB_DIR || env.SHERPA_ONNX_ARCHIVE_DIR) return;
  const run = runtime.run || spawnSync;
  const commandOptions = { cwd: root, encoding: 'utf8', windowsHide: true, shell: false };
  const target = runtime.target || env.CARGO_BUILD_TARGET
    || String(run('rustc', ['-vV'], commandOptions).stdout).match(/^host:\s*(\S+)$/m)?.[1];
  if (!ARCHIVES[target]) throw new Error(`Unsupported Sherpa development target: ${target}`);
  const version = readFileSync(join(root, 'Cargo.lock'), 'utf8')
    .match(/name = "sherpa-onnx-sys"\r?\nversion = "([\d.]+)"/)?.[1];
  if (!version) throw new Error('Cannot find the locked sherpa-onnx-sys version');
  const stem = `sherpa-onnx-v${version}-${ARCHIVES[target]}`;
  const archive = `${stem}.tar.bz2`;
  const cache = join(resolve(root, env.CARGO_TARGET_DIR || 'target'), 'sherpa-onnx-prebuilt');
  const common = run('git', ['rev-parse', '--git-common-dir'], commandOptions);
  const sharedRoot = common.status === 0 ? dirname(resolve(root, common.stdout.trim())) : root;
  for (const directory of [cache, join(sharedRoot, 'target', 'sherpa-onnx-prebuilt')]) {
    const library = join(directory, stem, 'lib');
    const libraryName = target.includes('windows') ? 'sherpa-onnx-c-api.lib' : 'libsherpa-onnx-c-api.a';
    if (existsSync(join(library, libraryName))) {
      env.SHERPA_ONNX_LIB_DIR = library;
      return;
    }
    if (existsSync(join(directory, archive))) {
      env.SHERPA_ONNX_ARCHIVE_DIR = directory;
      return;
    }
  }
  // Download through curl, which supports the user's HTTP and SOCKS proxies.
  // sherpa-onnx-sys retains ownership of archive extraction and linking.
  mkdirSync(cache, { recursive: true });
  const temporary = join(cache, `${archive}.download-${randomUUID()}`);
  try {
    const result = run(process.platform === 'win32' ? 'curl.exe' : 'curl', [
      '--fail', '--location', '--silent', '--show-error', '--proto', '=https', '--proto-redir', '=https',
      '--retry', '3', '--connect-timeout', '20', '--max-time', '300',
      '--output', temporary,
      `https://github.com/k2-fsa/sherpa-onnx/releases/download/v${version}/${archive}`,
    ], commandOptions);
    if (result.error || result.status !== 0) {
      throw new Error(`Failed to prepare Sherpa libraries: ${result.error?.message || result.stderr || result.status}`);
    }
    renameSync(temporary, join(cache, archive));
    env.SHERPA_ONNX_ARCHIVE_DIR = cache;
  } finally {
    rmSync(temporary, { force: true });
  }
}
