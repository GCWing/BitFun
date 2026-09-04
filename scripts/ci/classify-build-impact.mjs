import { appendFileSync, existsSync, readFileSync, statSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { rustWebUiSourceBoundaryRule } from '../core-boundaries/rules/source-rules.mjs';
import { scanForbiddenContentUnder } from '../core-boundaries/source-content-checks.mjs';

function readArg(args, name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : undefined;
}

function changedPaths(base, head, rangeMode) {
  const rangeArgs = rangeMode === 'merge-base'
    ? [`${base}...${head}`]
    : [base, head];
  const result = spawnSync(
    'git',
    ['diff', '--no-renames', '--name-only', '-z', ...rangeArgs, '--'],
    { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
  );
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || 'git diff failed');
  }
  return result.stdout.split('\0').filter(Boolean);
}

const ALL_DESKTOP_PLATFORMS = [
  'linux-x64',
  'linux-arm64',
  'macos-arm64',
  'macos-x64',
  'windows-x64',
];

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

// These inputs define repository-level release metadata, signing, staging, or
// scheduling contracts shared by more than one artifact producer. A focused
// platform match is not sufficient for them: validate the complete producer
// set so a release-contract change cannot merge on compile-only evidence.
const ALL_PACKAGE_INPUTS = new Set([
  'scripts/ci/classify-build-impact.mjs',
  'scripts/collect-tauri-updater-assets.mjs',
  'scripts/generate-linux-binaries-manifest.mjs',
  'scripts/generate-tauri-latest-json.mjs',
  'scripts/generate-version.cjs',
  'scripts/openbitfun-release-sync.sh',
  'scripts/prepare-windows-installer-asset.mjs',
  'scripts/release-channel.mjs',
  'scripts/set-build-version.mjs',
  'scripts/sign-release-assets.sh',
  'scripts/stage-github-release-assets.mjs',
  'scripts/verify-release-version-sync.mjs',
  'scripts/verify-tauri-latest-json.mjs',
  'scripts/write-minisign-public-key.mjs',
]);

// Inputs consumed by the Desktop/Tauri package producer itself. Keep this
// separate from ALL_PACKAGE_INPUTS: these paths do not affect the standalone
// Linux CLI/Relay archives, so selecting those producers would add cost
// without validating the changed input.
const DESKTOP_PACKAGE_INPUTS = collectLocalModuleInputs([
  'scripts/desktop-tauri-build.mjs',
  // Tauri runs this entrypoint through beforeBuildCommand rather than an
  // import from desktop-tauri-build.mjs.
  'scripts/frontend-build-all.mjs',
]);

const DESKTOP_PACKAGE_PREFIXES = [
  'products/',
  'resources/flashgrep/',
  'src/apps/desktop/capabilities/',
  'src/apps/desktop/dmg/',
  'src/apps/desktop/icons/',
  'src/apps/desktop/resources/',
  'src/apps/desktop/scripts/',
];

// These product surfaces are validated by the frontend gate and do not need
// the Rust/CLI matrices on their own. Native mobile is intentionally filtered
// as neutral below because it has no supported CI toolchain or producer here.
const FRONTEND_ONLY_PREFIXES = [
  'design-system/',
  'src/web-ui/',
  'src/mobile-web/',
];

const FRONTEND_ONLY_INPUTS = new Set([
  'scripts/mobile-web-build.cjs',
]);

function collectLocalModuleInputs(entryPaths) {
  const inputs = new Set();
  const pending = entryPaths.map((entry) => resolve(REPO_ROOT, entry));

  while (pending.length > 0) {
    const absolutePath = pending.pop();
    const repoPath = relative(REPO_ROOT, absolutePath).split(sep).join('/');
    if (repoPath === '..' || repoPath.startsWith('../') || inputs.has(repoPath)) {
      continue;
    }
    if (!existsSync(absolutePath)) {
      throw new Error(`Missing package producer module: ${repoPath}`);
    }
    inputs.add(repoPath);

    const source = readFileSync(absolutePath, 'utf8');
    const localImport = /(?:\b(?:import|export)\s+(?:[^'"\n]*?\s+from\s*)?|\bimport\s*\(\s*)['"](\.[^'"]+)['"]/g;
    for (const match of source.matchAll(localImport)) {
      const importedPath = resolveLocalModule(dirname(absolutePath), match[1]);
      if (importedPath) {
        pending.push(importedPath);
      }
    }
  }

  return inputs;
}

function resolveLocalModule(parentDirectory, specifier) {
  const base = resolve(parentDirectory, specifier);
  const candidates = [
    base,
    `${base}.mjs`,
    `${base}.js`,
    `${base}.cjs`,
    resolve(base, 'index.mjs'),
    resolve(base, 'index.js'),
    resolve(base, 'index.cjs'),
  ];
  return candidates.find((candidate) =>
    existsSync(candidate) && statSync(candidate).isFile());
}

export function classifyBuildImpact(paths) {
  const result = ({
    rustRequired,
    frontendRequired,
    desktopPlatforms = [],
    linuxBinariesRequired = false,
    relayImageRequired = false,
    dshProfileRequired = false,
    reason,
  }) => ({
    rustRequired,
    frontendRequired,
    desktopPackagesImpacted: desktopPlatforms.length > 0,
    desktopPlatforms,
    linuxBinariesRequired,
    relayImageRequired,
    dshProfileRequired,
    // Desktop producers consume too many concurrent hosted runners for PRs.
    // Keep their impact visible for Nightly ownership, while limiting automatic
    // PR producers to the standalone Linux/Relay delivery contracts.
    prProducerRequired: linuxBinariesRequired || relayImageRequired,
    reason,
    changedCount: paths.length,
  });
  const failClosed = (reason) => result({
    rustRequired: true,
    frontendRequired: true,
    desktopPlatforms: ALL_DESKTOP_PLATFORMS,
    linuxBinariesRequired: true,
    relayImageRequired: true,
    dshProfileRequired: true,
    reason,
  });
  if (paths.length === 0) {
    return failClosed('no-changes');
  }
  if (paths.some((file) => !isValidRepositoryPath(file))) {
    return failClosed('invalid-path');
  }

  const activePaths = paths.filter((file) => !isKnownNeutralPath(file));
  if (activePaths.length === 0) {
    return result({
      rustRequired: false,
      frontendRequired: false,
      reason: 'ci-ignored-only',
    });
  }

  const rustRequired = activePaths.some(isRustBuildInput)
    || !activePaths.every(isFrontendOnlyInput);
  if (activePaths.some(isFullPackageInput)) {
    return result({
      rustRequired,
      frontendRequired: true,
      desktopPlatforms: ALL_DESKTOP_PLATFORMS,
      linuxBinariesRequired: true,
      relayImageRequired: true,
      dshProfileRequired: true,
      reason: 'full-package-input',
    });
  }

  const desktopPlatforms = new Set();
  let linuxBinariesRequired = false;
  let relayImageRequired = false;
  for (const file of activePaths) {
    if (isDesktopPackageInput(file)) {
      for (const platform of ALL_DESKTOP_PLATFORMS) {
        desktopPlatforms.add(platform);
      }
    }
    if (isWindowsPackageInput(file)) {
      desktopPlatforms.add('windows-x64');
    }
    if (isMacPackageInput(file)) {
      desktopPlatforms.add('macos-arm64');
      desktopPlatforms.add('macos-x64');
    }
    if (isLinuxDesktopPackageInput(file)) {
      desktopPlatforms.add('linux-x64');
      desktopPlatforms.add('linux-arm64');
    }
    if (isLinuxBinaryPackageInput(file)) {
      linuxBinariesRequired = true;
    }
    if (isRelayImageInput(file)) {
      // The image validation consumes the archive produced in the same matrix
      // job, so selecting it necessarily selects Linux binary packaging too.
      linuxBinariesRequired = true;
      relayImageRequired = true;
    }
  }

  const selectedPlatforms = ALL_DESKTOP_PLATFORMS.filter((platform) =>
    desktopPlatforms.has(platform));
  const dshProfileRequired = activePaths.some(isDshProfileInput);
  const hasPackageImpact = selectedPlatforms.length > 0
    || linuxBinariesRequired
    || relayImageRequired;
  const reason = hasPackageImpact
    ? 'platform-package-input'
    : rustRequired
      ? (activePaths.some(isRustBuildInput) ? 'rust-build-input' : 'outside-web-ui')
      : 'frontend-only';
  return result({
    rustRequired,
    frontendRequired: true,
    desktopPlatforms: selectedPlatforms,
    linuxBinariesRequired,
    relayImageRequired,
    dshProfileRequired,
    reason,
  });
}

function isValidRepositoryPath(file) {
  return typeof file === 'string'
    && file.length > 0
    && !file.startsWith('/')
    && !/^[A-Za-z]:/.test(file)
    && !file.includes('\\')
    && !/[\r\n\0]/.test(file)
    && file.split('/').every((segment) => segment !== '' && segment !== '.' && segment !== '..');
}

function isRustBuildInput(file) {
  const segments = file.split('/');
  const name = segments.at(-1);
  return file.endsWith('.rs')
    || name === 'Cargo.toml'
    || name === 'Cargo.lock'
    || name === 'build.rs'
    || name === 'rust-toolchain'
    || name === 'rust-toolchain.toml'
    || segments.includes('.cargo');
}

function isFullPackageInput(file) {
  const name = file.split('/').at(-1);
  if (
    file === 'Cargo.toml'
    || file === 'Cargo.lock'
    || file === 'package.json'
    || file === 'package-lock.json'
    || file === 'pnpm-lock.yaml'
    || file === 'pnpm-workspace.yaml'
    || name === 'rust-toolchain'
    || name === 'rust-toolchain.toml'
    || file.startsWith('.cargo/')
  ) {
    return true;
  }
  if (name === 'Cargo.toml' && !file.startsWith('OpenBitFun-Installer/')) {
    return true;
  }
  if (
    file.startsWith('.github/workflows/')
    || ALL_PACKAGE_INPUTS.has(file)
  ) {
    return true;
  }
  return false;
}

function isDesktopPackageInput(file) {
  return DESKTOP_PACKAGE_INPUTS.has(file)
    || DESKTOP_PACKAGE_PREFIXES.some((prefix) => file.startsWith(prefix))
    || file === 'src/apps/desktop/build.rs'
    || file.startsWith('src/apps/desktop/tauri.')
    || file === 'src/web-ui/package.json';
}

function isFrontendOnlyInput(file) {
  return FRONTEND_ONLY_INPUTS.has(file)
    || FRONTEND_ONLY_PREFIXES.some((prefix) => file.startsWith(prefix));
}

function isWindowsPackageInput(file) {
  return file.startsWith('OpenBitFun-Installer/')
    || file === 'scripts/cli/package-windows.ps1'
    || file.startsWith('scripts/windows/')
    || file.includes('/windows/')
    || /(?:^|\/)(?:nsis|wix)(?:\/|\.|$)/i.test(file);
}

function isMacPackageInput(file) {
  return file === 'scripts/cli/package-unix.sh'
    || file.startsWith('scripts/ci/setup-macos-signing.')
    || file.startsWith('scripts/ci/verify-macos-signing.')
    || file.startsWith('scripts/macos/')
    || file.includes('/macos/')
    || /(?:entitlements|Info\.plist)$/i.test(file);
}

function isLinuxDesktopPackageInput(file) {
  return file === 'scripts/ci/verify-appimage-fcitx.sh'
    || file.startsWith('scripts/linux/')
    || file.includes('/linux/')
    || /(?:appimage|linuxdeploy|\.deb\b|\.rpm\b)/i.test(file);
}

function isLinuxBinaryPackageInput(file) {
  return file === 'scripts/ci/check-glibc-floor.sh'
    || file.startsWith('scripts/cli/package-unix.')
    || file.startsWith('scripts/relay/package-unix.')
    || file.startsWith('scripts/cli/test-install-unix.');
}

function isRelayImageInput(file) {
  return file === 'src/apps/relay-server/Dockerfile.release'
    || file.startsWith('scripts/relay/package-unix.');
}

function isDshProfileInput(file) {
  return file.startsWith('packages/dsh-acp/')
    || file === 'scripts/prepare-dsh-profile.mjs';
}

function isKnownNeutralPath(file) {
  const isKnownDocumentation = file.endsWith('.md')
    && (!file.includes('/') || file.startsWith('docs/'));
  return isKnownDocumentation
    || file.startsWith('png/')
    || file.startsWith('src/apps/mobile/');
}

export function run(args = process.argv.slice(2), env = process.env) {
  const base = readArg(args, '--base');
  const head = readArg(args, '--head');
  const rangeMode = readArg(args, '--range-mode') ?? 'direct';
  if (!base || !head) {
    throw new Error(
      'Usage: classify-build-impact.mjs --base <sha> --head <sha> '
      + '[--range-mode direct|merge-base]',
    );
  }

  const boundaryFindings = scanForbiddenContentUnder(
    process.cwd(),
    rustWebUiSourceBoundaryRule,
  );
  if (boundaryFindings.length > 0) {
    const details = boundaryFindings
      .slice(0, 20)
      .map((finding) => `${finding.repoPath}:${finding.line}: ${finding.message}`)
      .join('\n');
    throw new Error(`${rustWebUiSourceBoundaryRule.reason}\n${details}`);
  }

  let paths = [];
  let result;
  if (
    !isUsableCommitSha(base)
    || !isUsableCommitSha(head)
    || !['direct', 'merge-base'].includes(rangeMode)
  ) {
    result = { rustRequired: true, reason: 'invalid-range', changedCount: 0 };
  } else {
    try {
      paths = changedPaths(base, head, rangeMode);
      result = classifyBuildImpact(paths);
    } catch {
      result = classifyBuildImpact([]);
      result.reason = 'unavailable-range';
    }
  }
  if (result?.reason === 'invalid-range') {
    result = classifyBuildImpact([]);
    result.reason = 'invalid-range';
  }
  const lines = [
    `rust_required=${result.rustRequired}`,
    `frontend_required=${result.frontendRequired}`,
    `desktop_packages_impacted=${result.desktopPackagesImpacted}`,
    `desktop_platforms=${JSON.stringify(result.desktopPlatforms)}`,
    `linux_binaries_required=${result.linuxBinariesRequired}`,
    `relay_image_required=${result.relayImageRequired}`,
    `dsh_profile_required=${result.dshProfileRequired}`,
    `pr_producer_required=${result.prProducerRequired}`,
    `reason=${result.reason}`,
    `changed_count=${result.changedCount}`,
  ];
  if (env.GITHUB_OUTPUT) {
    appendFileSync(env.GITHUB_OUTPUT, `${lines.join('\n')}\n`);
  } else {
    process.stdout.write(`${lines.join('\n')}\n`);
  }
  if (env.GITHUB_STEP_SUMMARY) {
    appendFileSync(env.GITHUB_STEP_SUMMARY, renderSummary(result, paths));
  }
  return result;
}

function isUsableCommitSha(value) {
  return /^[0-9a-f]{40}$/i.test(value) && !/^0{40}$/.test(value);
}

function renderSummary(result, paths) {
  const shownPaths = paths.slice(0, 20);
  const lines = [
    '### Build impact classification',
    '',
    `- <strong>Required:</strong> ${result.rustRequired}`,
    `- <strong>Frontend:</strong> ${result.frontendRequired}`,
    `- <strong>Desktop package impact (Nightly-owned):</strong> ${result.desktopPlatforms.join(', ') || 'none'}`,
    `- <strong>Linux binaries:</strong> ${result.linuxBinariesRequired}`,
    `- <strong>Relay image:</strong> ${result.relayImageRequired}`,
    `- <strong>PR producer:</strong> ${result.prProducerRequired}`,
    `- <strong>DSH profile:</strong> ${result.dshProfileRequired}`,
    `- <strong>Reason:</strong> ${result.reason}`,
    `- <strong>Changed files:</strong> ${result.changedCount}`,
  ];
  if (shownPaths.length > 0) {
    lines.push('', ...shownPaths.map((file) => `- <code>${escapeHtml(file)}</code>`));
  }
  if (paths.length > shownPaths.length) {
    lines.push(`- ${paths.length - shownPaths.length} additional changed file(s) omitted`);
  }
  return `${lines.join('\n')}\n`;
}

function escapeHtml(value) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('\r', '\\r')
    .replaceAll('\n', '\\n');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    run();
  } catch (error) {
    process.stderr.write(`${error.message || String(error)}\n`);
    process.exitCode = 1;
  }
}
