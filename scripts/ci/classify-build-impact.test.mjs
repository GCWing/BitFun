import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { classifyBuildImpact } from './classify-build-impact.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const scriptPath = path.join(repoRoot, 'scripts/ci/classify-build-impact.mjs');
const allPlatforms = [
  'linux-x64',
  'linux-arm64',
  'macos-arm64',
  'macos-x64',
  'windows-x64',
];

function expected({
  rustRequired,
  frontendRequired = true,
  desktopPlatforms = [],
  linuxBinariesRequired = false,
  relayImageRequired = false,
  dshProfileRequired = false,
  reason,
  changedCount = 1,
}) {
  return {
    rustRequired,
    frontendRequired,
    desktopPackagesImpacted: desktopPlatforms.length > 0,
    desktopPlatforms,
    linuxBinariesRequired,
    relayImageRequired,
    dshProfileRequired,
    prProducerRequired: linuxBinariesRequired || relayImageRequired,
    reason,
    changedCount,
  };
}

function git(root, args) {
  const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}

function commit(root, message) {
  git(root, ['add', '--all']);
  git(root, [
    '-c', 'user.name=CI Impact Test',
    '-c', 'user.email=ci-impact@example.invalid',
    'commit', '-m', message,
  ]);
  return git(root, ['rev-parse', 'HEAD']);
}

function parseOutputs(file) {
  return Object.fromEntries(
    readFileSync(file, 'utf8').trim().split(/\r?\n/).map((line) => line.split('=', 2)),
  );
}

function runClassifier(root, base, head, rangeMode = 'direct') {
  const output = path.join(root, `github-output-${Date.now()}-${Math.random()}.txt`);
  const summary = path.join(root, `github-summary-${Date.now()}-${Math.random()}.md`);
  const result = spawnSync(
    process.execPath,
    [scriptPath, '--base', base, '--head', head, '--range-mode', rangeMode],
    {
      cwd: root,
      env: { ...process.env, GITHUB_OUTPUT: output, GITHUB_STEP_SUMMARY: summary },
      encoding: 'utf8',
    },
  );
  return {
    ...result,
    outputs: result.status === 0 ? parseOutputs(output) : undefined,
    summary: existsSync(summary) ? readFileSync(summary, 'utf8') : undefined,
  };
}

test('maps representative changes to the smallest predictive validation', () => {
  const cases = [
    {
      paths: ['docs/review-notes.md', 'png/example.png'],
      result: expected({
        rustRequired: false,
        frontendRequired: false,
        reason: 'ci-ignored-only',
        changedCount: 2,
      }),
    },
    {
      paths: ['src/web-ui/src/example.ts'],
      result: expected({ rustRequired: false, reason: 'frontend-only' }),
    },
    {
      paths: ['src/mobile-web/src/App.tsx'],
      result: expected({ rustRequired: false, reason: 'frontend-only' }),
    },
    {
      paths: ['design-system/packages/ui/src/components/Input/Input.tsx'],
      result: expected({ rustRequired: false, reason: 'frontend-only' }),
    },
    {
      paths: [
        'src/apps/mobile/android/app/src/main/kotlin/com/openbitfun/MainActivity.kt',
        'src/apps/mobile/shared/core-feature/src/commonMain/kotlin/Feature.kt',
        'src/apps/mobile/ios/OpenBitFun/App.swift',
        'src/apps/mobile/harmonyos/entry/src/main/ets/pages/Index.ets',
      ],
      result: expected({
        rustRequired: false,
        frontendRequired: false,
        reason: 'ci-ignored-only',
        changedCount: 4,
      }),
    },
    {
      paths: ['src/crates/services/services-core/src/lib.rs'],
      result: expected({ rustRequired: true, reason: 'rust-build-input' }),
    },
    {
      paths: ['OpenBitFun-Installer/scripts/build-installer.cjs'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: ['windows-x64'],
        reason: 'platform-package-input',
      }),
    },
    {
      paths: ['scripts/ci/setup-macos-signing.sh'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: ['macos-arm64', 'macos-x64'],
        reason: 'platform-package-input',
      }),
    },
    {
      paths: ['scripts/ci/verify-appimage-fcitx.sh'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: ['linux-x64', 'linux-arm64'],
        reason: 'platform-package-input',
      }),
    },
    ...[
      'scripts/prepare-flashgrep-resource.mjs',
      'resources/flashgrep/VERSION.json',
      'src/apps/desktop/scripts/post-install-icons.sh',
      'src/apps/desktop/dmg/background.png',
      'scripts/product-customization/projections.mjs',
      'products/openbitfun/product.jsonc',
    ].map((desktopInput) => ({
      paths: [desktopInput],
      result: expected({
        rustRequired: true,
        desktopPlatforms: allPlatforms,
        reason: 'platform-package-input',
      }),
    })),
    {
      paths: ['src/mobile-web/package.json'],
      result: expected({
        rustRequired: false,
        reason: 'frontend-only',
      }),
    },
    {
      paths: ['scripts/mobile-web-build.cjs'],
      result: expected({
        rustRequired: false,
        reason: 'frontend-only',
      }),
    },
    {
      paths: ['src/apps/relay-server/Dockerfile.release'],
      result: expected({
        rustRequired: true,
        linuxBinariesRequired: true,
        relayImageRequired: true,
        reason: 'platform-package-input',
      }),
    },
    {
      paths: ['scripts/cli/package-unix.sh'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: ['macos-arm64', 'macos-x64'],
        linuxBinariesRequired: true,
        reason: 'platform-package-input',
      }),
    },
    {
      paths: ['packages/dsh-acp/src/index.ts'],
      result: expected({
        rustRequired: true,
        dshProfileRequired: true,
        reason: 'outside-web-ui',
      }),
    },
    {
      paths: ['Cargo.lock'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: allPlatforms,
        linuxBinariesRequired: true,
        relayImageRequired: true,
        dshProfileRequired: true,
        reason: 'full-package-input',
      }),
    },
    {
      paths: ['.github/workflows/nightly.yml'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: allPlatforms,
        linuxBinariesRequired: true,
        relayImageRequired: true,
        dshProfileRequired: true,
        reason: 'full-package-input',
      }),
    },
    {
      paths: ['scripts/sign-release-assets.sh'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: allPlatforms,
        linuxBinariesRequired: true,
        relayImageRequired: true,
        dshProfileRequired: true,
        reason: 'full-package-input',
      }),
    },
    {
      paths: ['scripts/ci/classify-build-impact.mjs'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: allPlatforms,
        linuxBinariesRequired: true,
        relayImageRequired: true,
        dshProfileRequired: true,
        reason: 'full-package-input',
      }),
    },
    {
      paths: ['scripts/cli/package-windows.ps1'],
      result: expected({
        rustRequired: true,
        desktopPlatforms: ['windows-x64'],
        reason: 'platform-package-input',
      }),
    },
  ];

  for (const { paths, result } of cases) {
    assert.deepEqual(classifyBuildImpact(paths), result, paths.join(', '));
  }
});

test('combines mobile-web with other surfaces without native-mobile noise', () => {
  assert.deepEqual(
    classifyBuildImpact([
      'src/mobile-web/src/App.tsx',
      'src/apps/mobile/android/app/src/main/kotlin/com/openbitfun/MainActivity.kt',
    ]),
    expected({
      rustRequired: false,
      reason: 'frontend-only',
      changedCount: 2,
    }),
  );

  assert.deepEqual(
    classifyBuildImpact([
      'src/mobile-web/src/App.tsx',
      'src/crates/services/services-core/src/lib.rs',
    ]),
    expected({
      rustRequired: true,
      reason: 'rust-build-input',
      changedCount: 2,
    }),
  );

  assert.deepEqual(
    classifyBuildImpact([
      'src/mobile-web/src/App.tsx',
      'src/apps/mobile/shared/core-feature/src/commonMain/kotlin/Feature.kt',
      'src/apps/relay-server/Dockerfile.release',
    ]),
    expected({
      rustRequired: true,
      linuxBinariesRequired: true,
      relayImageRequired: true,
      reason: 'platform-package-input',
      changedCount: 3,
    }),
  );
});

test('keeps nested Markdown fail-closed because it may be a compile-time input', () => {
  assert.deepEqual(
    classifyBuildImpact([
      'src/web-ui/src/example.ts',
      'src/crates/assembly/agent-content/prompts/agents/example.md',
    ]),
    expected({
      rustRequired: true,
      reason: 'outside-web-ui',
      changedCount: 2,
    }),
  );
});

test('uses a merge-base range for pull requests and emits every workflow output', (t) => {
  const root = mkdtempSync(path.join(tmpdir(), 'openbitfun-build-impact-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  git(root, ['init', '--initial-branch=main']);
  writeFileSync(path.join(root, 'README.md'), 'baseline\n');
  commit(root, 'baseline');

  git(root, ['switch', '-c', 'feature']);
  const installer = path.join(root, 'OpenBitFun-Installer/scripts/build-installer.cjs');
  mkdirSync(path.dirname(installer), { recursive: true });
  writeFileSync(installer, 'export {};\n');
  const head = commit(root, 'installer change');

  git(root, ['switch', 'main']);
  writeFileSync(path.join(root, 'Cargo.lock'), 'version = 4\n');
  const currentBase = commit(root, 'base dependency change');

  const pullRequest = runClassifier(root, currentBase, head, 'merge-base');
  assert.equal(pullRequest.status, 0, pullRequest.stderr);
  assert.deepEqual(pullRequest.outputs, {
    rust_required: 'true',
    frontend_required: 'true',
    desktop_packages_impacted: 'true',
    desktop_platforms: '["windows-x64"]',
    linux_binaries_required: 'false',
    relay_image_required: 'false',
    dsh_profile_required: 'false',
    pr_producer_required: 'false',
    reason: 'platform-package-input',
    changed_count: '1',
  });
  assert.match(pullRequest.summary, /Build impact classification/);
  assert.match(
    pullRequest.summary,
    /Desktop package impact \(Nightly-owned\):<\/strong> windows-x64/,
  );

  const push = runClassifier(root, currentBase, head, 'direct');
  assert.deepEqual(JSON.parse(push.outputs.desktop_platforms), allPlatforms);
  assert.equal(push.outputs.pr_producer_required, 'true');
  assert.equal(push.outputs.reason, 'full-package-input');
});

test('fails closed when paths or event ranges are invalid or unavailable', (t) => {
  for (const paths of [[], ['src/web-ui/../apps/desktop/src/lib.rs'], ['src\\web-ui\\x.ts']]) {
    const result = classifyBuildImpact(paths);
    assert.equal(result.rustRequired, true);
    assert.equal(result.frontendRequired, true);
    assert.deepEqual(result.desktopPlatforms, allPlatforms);
    assert.equal(result.linuxBinariesRequired, true);
    assert.equal(result.relayImageRequired, true);
    assert.equal(result.dshProfileRequired, true);
  }

  const root = mkdtempSync(path.join(tmpdir(), 'openbitfun-build-impact-range-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  git(root, ['init', '--initial-branch=main']);
  writeFileSync(path.join(root, 'README.md'), 'baseline\n');
  const head = commit(root, 'baseline');

  for (const [base, reason] of [
    ['0'.repeat(40), 'invalid-range'],
    ['f'.repeat(40), 'unavailable-range'],
  ]) {
    const result = runClassifier(root, base, head);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.outputs.reason, reason);
    assert.equal(result.outputs.pr_producer_required, 'true');
    assert.deepEqual(JSON.parse(result.outputs.desktop_platforms), allPlatforms);
  }
});

test('rejects tracked Rust sources that reference the Web UI source tree', (t) => {
  const root = mkdtempSync(path.join(tmpdir(), 'openbitfun-build-impact-boundary-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  git(root, ['init', '--initial-branch=main']);
  writeFileSync(path.join(root, 'README.md'), 'baseline\n');
  const base = commit(root, 'baseline');
  const rustFile = path.join(root, 'src/lib.rs');
  mkdirSync(path.dirname(rustFile), { recursive: true });
  writeFileSync(rustFile, 'const WEB: &str = include_dir!("../web-ui/src");\n');
  const head = commit(root, 'forbidden Rust input');

  const result = runClassifier(root, base, head);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Rust source must not reference the Web UI source tree/);
});
