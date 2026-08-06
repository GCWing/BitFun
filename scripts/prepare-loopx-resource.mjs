/**
 * Prepare the LoopX sidecar binary for desktop packaging.
 *
 * Mirrors prepare-flashgrep-resource.mjs: the packaged app bundles a
 * platform-specific single-file `loopx` binary under resources/loopx/, which
 * the desktop host points LOOPX_BIN at when the user has not set it.
 *
 * Unlike flashgrep (a prebuilt download), LoopX is a zero-dependency Python
 * package, so the binary can be produced locally with PyInstaller when a
 * LoopX checkout is available:
 *
 *   node scripts/prepare-loopx-resource.mjs --build [--loopx-dir <path>]
 *
 * The checkout defaults to ../loopx next to this repository, or LOOPX_SRC_DIR.
 * CI can instead drop a prebuilt binary into resources/loopx/ before packaging.
 *
 * The sidecar is OPTIONAL: when no binary is present the desktop build
 * continues without it and Issue-Fix falls back to LOOPX_BIN / PATH at
 * runtime, keeping `pnpm run desktop:build` working for contributors who do
 * not touch Issue-Fix.
 */
import { execFileSync } from 'child_process';
import {
  chmodSync,
  existsSync,
  mkdirSync,
  statSync,
  writeFileSync,
} from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const RESOURCE_DIR = join(ROOT, 'resources', 'loopx');

export function loopxBinaryNames() {
  if (process.platform === 'win32' && process.arch === 'x64') {
    return ['loopx-x86_64-pc-windows-msvc.exe'];
  }
  if (process.platform === 'win32' && process.arch === 'arm64') {
    return ['loopx-aarch64-pc-windows-msvc.exe'];
  }
  if (process.platform === 'darwin' && process.arch === 'x64') {
    return ['loopx-x86_64-apple-darwin'];
  }
  if (process.platform === 'darwin' && process.arch === 'arm64') {
    return ['loopx-aarch64-apple-darwin'];
  }
  if (process.platform === 'linux' && process.arch === 'x64') {
    return ['loopx-x86_64-unknown-linux-musl', 'loopx-x86_64-unknown-linux-gnu'];
  }
  if (process.platform === 'linux' && process.arch === 'arm64') {
    return ['loopx-aarch64-unknown-linux-musl', 'loopx-aarch64-unknown-linux-gnu'];
  }
  return [process.platform === 'win32' ? 'loopx.exe' : 'loopx'];
}

export function loopxBinaryName() {
  return loopxBinaryNames()[0];
}

/** The bundled sidecar path when present, else null (sidecar is optional). */
export function findLoopxBinary() {
  for (const binaryName of loopxBinaryNames()) {
    const binaryPath = join(RESOURCE_DIR, binaryName);
    if (!existsSync(binaryPath)) {
      continue;
    }
    if (process.platform !== 'win32') {
      chmodSync(binaryPath, statSync(binaryPath).mode | 0o111);
    }
    return binaryPath;
  }
  return null;
}

function resolveLoopxSourceDir(explicitDir) {
  const candidates = [
    explicitDir,
    process.env.LOOPX_SRC_DIR,
    join(ROOT, '..', 'loopx'),
  ].filter(Boolean);
  for (const candidate of candidates) {
    if (existsSync(join(candidate, 'pyproject.toml')) && existsSync(join(candidate, 'loopx'))) {
      return candidate;
    }
  }
  throw new Error(
    `LoopX checkout not found. Tried: ${candidates.join(', ')}. ` +
      'Pass --loopx-dir <path> or set LOOPX_SRC_DIR.'
  );
}

/**
 * Build the sidecar with PyInstaller from a LoopX checkout.
 *
 * LoopX has zero runtime dependencies (pyproject `dependencies = []`), which
 * keeps the one-file build deterministic; --collect-submodules covers its
 * dynamically imported command modules.
 */
export function buildLoopxBinary({ loopxDir, python = process.env.PYTHON || 'python' } = {}) {
  const sourceDir = resolveLoopxSourceDir(loopxDir);
  const binaryName = loopxBinaryName();
  const workDir = join(ROOT, 'target', 'loopx-sidecar');
  mkdirSync(RESOURCE_DIR, { recursive: true });
  mkdirSync(workDir, { recursive: true });

  // LoopX exposes its CLI as the console script `loopx.cli:main`; PyInstaller
  // wants a script file, so generate a tiny launcher for it.
  const launcher = join(workDir, 'loopx_sidecar_entry.py');
  writeFileSync(launcher, 'from loopx.cli import main\n\nif __name__ == "__main__":\n    main()\n');

  console.log(`[loopx-sidecar] building ${binaryName} from ${sourceDir}`);
  execFileSync(
    python,
    [
      '-m', 'PyInstaller',
      '--onefile',
      '--name', binaryName.replace(/\.exe$/, ''),
      '--distpath', RESOURCE_DIR,
      '--workpath', join(workDir, 'build'),
      '--specpath', join(workDir, 'spec'),
      '--collect-submodules', 'loopx',
      '--console',
      '--noconfirm',
      launcher,
    ],
    { stdio: 'inherit', cwd: sourceDir, env: { ...process.env, PYTHONUTF8: '1' } }
  );

  const built = findLoopxBinary();
  if (!built) {
    throw new Error(`PyInstaller finished but ${binaryName} is missing under resources/loopx/`);
  }
  const version = execFileSync(built, ['--version'], { encoding: 'utf8' }).trim();
  console.log(`[loopx-sidecar] built ${built} (${version})`);
  return built;
}

const invokedDirectly =
  process.argv[1] && fileURLToPath(import.meta.url) === join(process.argv[1]);
if (invokedDirectly) {
  const args = process.argv.slice(2);
  const dirIndex = args.indexOf('--loopx-dir');
  const loopxDir = dirIndex >= 0 ? args[dirIndex + 1] : undefined;
  if (args.includes('--build')) {
    buildLoopxBinary({ loopxDir });
  } else {
    const existing = findLoopxBinary();
    console.log(
      existing
        ? `[loopx-sidecar] present: ${existing}`
        : '[loopx-sidecar] absent (optional); run with --build to produce it'
    );
  }
}
