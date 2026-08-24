import assert from 'node:assert/strict';
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const scriptPath = path.join(repoRoot, 'scripts/check-github-config.mjs');
const requireFromWebUi = createRequire(
  path.join(repoRoot, 'src/web-ui/package.json'),
);
const yaml = requireFromWebUi('yaml');

function createRepo({ workflow, nodeVersionFile }) {
  const root = mkdtempSync(path.join(tmpdir(), 'bitfun-github-config-'));
  mkdirSync(path.join(root, '.github/workflows'), { recursive: true });
  writeFileSync(
    path.join(root, 'package.json'),
    `${JSON.stringify({ engines: { node: '>=22.12.0' } }, null, 2)}\n`,
  );
  writeFileSync(path.join(root, '.github/workflows/ci.yml'), workflow);

  if (nodeVersionFile) {
    writeFileSync(path.join(root, nodeVersionFile.path), `${nodeVersionFile.value}\n`);
  }

  return root;
}

function enableProductControlContract(root) {
  const manifestPath = path.join(root, 'package.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  manifest.scripts = { 'capabilities:check': 'node scripts/check.mjs' };
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}

function runCheck(root) {
  return spawnSync(process.execPath, [scriptPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      BITFUN_GITHUB_CONFIG_TEST_ROOT: root,
    },
    encoding: 'utf8',
  });
}

test('rejects setup-node node-version-file below the project baseline', (t) => {
  const root = createRepo({
    nodeVersionFile: { path: '.node-version', value: '20' },
    workflow: `
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v5
        with:
          node-version-file: .node-version
`,
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /node-version-file \.node-version resolves to 20/);
  assert.match(result.stderr, /Node\.js 22\.12\.0 or newer/);
});

test('rejects removal or weakening of the ProductControl and Playbook gates', (t) => {
  const root = createRepo({
    workflow: `
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Validate interactive capability contract
        continue-on-error: true
        run: pnpm run capabilities:check
`,
  });
  enableProductControlContract(root);
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /interactive capability gate must run exactly/u);
  assert.match(result.stderr, /interactive capability gate must remain blocking/u);
  assert.match(result.stderr, /ProductControl owner\/delivery-profile gate is missing/u);
  assert.match(result.stderr, /CLI ProductControl self-control coverage requires/u);
});

test('rejects explicit setup-node node-version below the project baseline when node-version-file is valid', (t) => {
  const root = createRepo({
    nodeVersionFile: { path: '.node-version', value: '22' },
    workflow: `
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v5
        with:
          node-version: 20
          node-version-file: .node-version
`,
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /node-version resolves to 20/);
});

test('accepts package.json node-version-file from engines.node', (t) => {
  const root = createRepo({
    workflow: `
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v5
        with:
          node-version-file: package.json
`,
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root);

  assert.equal(result.status, 0, result.stderr);
});

test('accepts tool-versions node-version-file syntax', (t) => {
  const root = createRepo({
    nodeVersionFile: { path: '.tool-versions', value: 'nodejs 22.12.0' },
    workflow: `
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v5
        with:
          node-version-file: .tool-versions
`,
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root);

  assert.equal(result.status, 0, result.stderr);
});

test('rejects floating setup-node minor below the project baseline', (t) => {
  const root = createRepo({
    workflow: `
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v5
        with:
          node-version: "22.11.x"
`,
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /node-version resolves to 22.11.x/);
});

test('accepts floating setup-node minor at the project baseline', (t) => {
  const root = createRepo({
    workflow: `
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v5
        with:
          node-version: "22.12.x"
`,
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root);

  assert.equal(result.status, 0, result.stderr);
});

test('accepts explicit setup-node semver range at the project baseline', (t) => {
  const root = createRepo({
    workflow: `
name: CI
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/setup-node@v5
        with:
          node-version: ">=22.12.0"
`,
  });
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const result = runCheck(root);

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /GitHub YAML config check passed/);
});

test('keeps Rust CI independent, restore-only on PRs, and target-focused', () => {
  const workflow = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8'),
  );
  const rustJob = workflow.jobs['rust-build-check'];
  const frontendJob = workflow.jobs['frontend-build'];
  const trustedMain =
    "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}";

  assert.equal(
    rustJob.needs,
    undefined,
    'Rust validation must not wait for the frontend build',
  );
  assert.equal(
    rustJob.steps.some((step) => step.uses?.startsWith('actions/download-artifact@')),
    false,
    'Rust validation must not download frontend artifacts',
  );
  assert.match(
    rustJob.steps.find((step) => step.name === 'Create Tauri resource directories')
      ?.run ?? '',
    /mkdir -p dist src\/mobile-web\/dist/,
  );
  assert.equal(
    frontendJob.steps.some(
      (step) =>
        step.uses?.startsWith('actions/upload-artifact@') &&
        step.with?.name === 'frontend-dist',
    ),
    false,
    'The frontend build must not upload an artifact with no consumer',
  );

  for (const jobName of ['cli-test', 'rust-build-check']) {
    const job = workflow.jobs[jobName];
    const cache = job.steps.find((step) =>
      step.uses?.startsWith('swatinem/rust-cache@'),
    );
    assert.equal(
      job.steps.some((step) => step.run?.includes('cargo generate-lockfile')),
      false,
      `${jobName} must consume the committed Cargo.lock`,
    );
    assert.equal(cache?.with?.['save-if'], trustedMain);
    assert.equal(cache?.with?.['cache-on-failure'], trustedMain);
  }

  const cliJob = workflow.jobs['cli-test'];
  assert.ok(
    cliJob.strategy.matrix.include.some((entry) => entry.os === 'windows-latest'),
    'Windows ConPTY contracts must run before Nightly',
  );
  assert.equal(
    cliJob.steps.find((step) => step.name === 'Run Windows CLI terminal contracts')?.run,
    'cargo test --locked -p bitfun-cli --test terminal_process_contracts -- --test-threads=1',
  );

  const rustCache = rustJob.steps.find((step) =>
    step.uses?.startsWith('swatinem/rust-cache@'),
  );
  assert.equal(
    rustCache?.with?.['cache-directories'],
    undefined,
    'Rust cache cleanup must not own native libraries stored under target',
  );

  const restoreSherpaCache = rustJob.steps.find(
    (step) => step.name === 'Restore Sherpa native libraries',
  );
  const repairSherpaState = rustJob.steps.find(
    (step) => step.name === 'Repair missing Sherpa native state',
  );
  const checkCompilation = rustJob.steps.find(
    (step) => step.name === 'Check compilation',
  );
  const saveSherpaCache = rustJob.steps.find(
    (step) => step.name === 'Save Sherpa native libraries',
  );
  const sherpaCacheKey =
    'sherpa-onnx-v1-${{ runner.os }}-${{ runner.arch }}-1.13.4-static';

  assert.equal(restoreSherpaCache?.uses, 'actions/cache/restore@v5');
  assert.equal(restoreSherpaCache?.with?.path, 'target/sherpa-onnx-prebuilt');
  assert.equal(restoreSherpaCache?.with?.key, sherpaCacheKey);
  assert.match(
    repairSherpaState?.run ?? '',
    /rm -rf target\/sherpa-onnx-prebuilt/,
  );
  assert.match(repairSherpaState?.run ?? '', /cargo clean -p sherpa-onnx-sys/);
  assert.equal(saveSherpaCache?.uses, 'actions/cache/save@v5');
  assert.equal(saveSherpaCache?.with?.path, 'target/sherpa-onnx-prebuilt');
  assert.equal(saveSherpaCache?.with?.key, sherpaCacheKey);
  assert.equal(
    saveSherpaCache?.if,
    "github.event_name == 'push' && github.ref == 'refs/heads/main' && steps.sherpa-native-cache.outputs.cache-hit != 'true'",
  );
  assert.ok(
    rustJob.steps.indexOf(restoreSherpaCache) <
      rustJob.steps.indexOf(checkCompilation),
    'Sherpa native libraries must be restored before cargo check',
  );
  assert.ok(
    rustJob.steps.indexOf(checkCompilation) <
      rustJob.steps.indexOf(saveSherpaCache),
    'Sherpa native libraries must be saved before rust-cache post cleanup',
  );

  const commandByStep = new Map(
    rustJob.steps.map((step) => [step.name, step.run]),
  );
  const verifyMetadata = rustJob.steps.find(
    (step) => step.name === 'Verify committed Cargo metadata',
  );
  assert.equal(verifyMetadata?.run, 'cargo metadata --locked --no-deps');
  assert.ok(
    rustJob.steps.indexOf(verifyMetadata) < rustJob.steps.indexOf(checkCompilation),
    'CI must validate the committed Cargo.lock before the workspace check',
  );
  assert.equal(
    commandByStep.get('Run subscription authentication tests'),
    'cargo test --locked -p bitfun-ai-adapters --features subscription-auth --lib subscription_auth',
  );
  const installerCheck = rustJob.steps.find(
    (step) => step.name === 'Check installer compilation',
  );
  assert.equal(installerCheck?.if, "runner.os == 'Windows'");
  assert.equal(
    installerCheck?.run,
    'cargo check --locked --manifest-path BitFun-Installer/src-tauri/Cargo.toml',
  );
  const coreLibraryTests = rustJob.steps.find(
    (step) => step.name === 'Run core library tests',
  );
  const desktopLibraryTests = rustJob.steps.find(
    (step) => step.name === 'Run desktop library tests',
  );
  const windowsDesktopProbe = rustJob.steps.find(
    (step) => step.name === 'Probe Windows desktop library tests',
  );
  const productControlContracts = rustJob.steps.find(
    (step) => step.name === 'Run product-control domain and delivery-profile contracts',
  );
  assert.equal(
    coreLibraryTests?.run,
    'cargo test --locked -p bitfun-core --lib',
  );
  assert.equal(desktopLibraryTests?.if, "runner.os != 'Windows'");
  assert.equal(
    desktopLibraryTests?.run,
    'cargo test --locked -p bitfun-desktop --lib',
  );
  assert.equal(windowsDesktopProbe?.if, "runner.os == 'Windows'");
  assert.equal(windowsDesktopProbe?.shell, 'pwsh');
  assert.match(
    windowsDesktopProbe?.run ?? '',
    /cargo test --locked -p bitfun-desktop --lib 2>&1/,
  );
  assert.match(windowsDesktopProbe?.run ?? '', /0xc0000139/);
  assert.match(windowsDesktopProbe?.run ?? '', /STATUS_ENTRYPOINT_NOT_FOUND/);
  assert.match(windowsDesktopProbe?.run ?? '', /test result: FAILED/);
  assert.match(windowsDesktopProbe?.run ?? '', /exit \$testExitCode/);
  assert.equal(
    productControlContracts?.if,
    undefined,
    'product-control contracts must run on every supported CI OS',
  );
  assert.match(
    productControlContracts?.run ?? '',
    /bitfun-product-domains --no-default-features product_control/,
  );
  assert.match(
    productControlContracts?.run ?? '',
    /bitfun-product-capabilities every_agent_runtime_delivery_profile_includes_product_control_discovery/,
  );
  const fileWatchContracts = rustJob.steps.find(
    (step) => step.name === 'Run file watch contract tests',
  );
  assert.equal(
    fileWatchContracts?.run,
    'cargo test --locked -p bitfun-services-integrations --no-default-features --features file-watch --test file_watch_contracts',
  );
  assert.equal(
    fileWatchContracts?.if,
    undefined,
    'file-watch contracts must exercise FSEvents on macOS',
  );
  assert.equal(
    commandByStep.get('Run search tool tests'),
    'cargo test --locked -p tool-runtime --lib search::',
  );
});

test('ordinary CI requires the exact Nightly artifact producers', () => {
  const workflow = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8'),
  );
  const buildJob = workflow.jobs['nightly-build-contract'];

  assert.equal(buildJob.name, 'Nightly Build Contract');
  assert.equal(buildJob.needs, undefined);
  assert.equal(buildJob.uses, './.github/workflows/nightly-artifacts.yml');
  assert.deepEqual(buildJob.permissions, { contents: 'read' });
  assert.deepEqual(buildJob.with, {
    checkout_ref: '${{ github.sha }}',
    version: '0.0.0-nightly.ci.${{ github.run_id }}',
    artifact_prefix: 'ci-${{ github.run_id }}',
    artifact_retention_days: 1,
  });
});

test('nightly validates generated inputs and projected lockfiles before packaging', () => {
  const workflow = yaml.parse(
    readFileSync(
      path.join(repoRoot, '.github/workflows/nightly-artifacts.yml'),
      'utf8',
    ),
  );
  const callInputs = workflow.on.workflow_call.inputs;
  const packageJob = workflow.jobs.package;
  const steps = packageJob.steps;
  const committedMetadataIndex = steps.findIndex(
    (step) => step.name === 'Verify committed Cargo metadata',
  );
  const generationIndex = steps.findIndex(
    (step) => step.name === 'Generate web API bindings',
  );
  const typeCheckIndex = steps.findIndex(
    (step) => step.name === 'Type-check web UI',
  );
  const patchIndex = steps.findIndex(
    (step) => step.name === 'Patch nightly version',
  );
  const tauriAlignmentIndex = steps.findIndex(
    (step) => step.name === 'Verify Installer Tauri package alignment',
  );
  const metadataIndex = steps.findIndex(
    (step) => step.name === 'Verify projected Cargo metadata',
  );
  const buildIndex = steps.findIndex(
    (step) => step.name === 'Build desktop app',
  );

  assert.equal(callInputs.checkout_ref.required, true);
  assert.equal(callInputs.version.required, true);
  assert.equal(callInputs.artifact_prefix.required, true);
  assert.equal(callInputs.artifact_retention_days.default, 1);
  assert.equal(workflow.permissions.contents, 'read');

  assert.notEqual(committedMetadataIndex, -1);
  assert.notEqual(tauriAlignmentIndex, -1);
  assert.notEqual(generationIndex, -1);
  assert.notEqual(typeCheckIndex, -1);
  assert.equal(
    steps[generationIndex].run,
    'pnpm --dir src/web-ui run gen:types',
  );
  assert.ok(
    generationIndex < typeCheckIndex,
    'nightly must generate web API bindings before type-checking the web UI',
  );
  assert.ok(
    committedMetadataIndex < patchIndex &&
      tauriAlignmentIndex < patchIndex &&
      typeCheckIndex < patchIndex &&
      patchIndex < metadataIndex &&
      metadataIndex < buildIndex,
    'nightly must verify the projected lockfile before nested locked build hooks run',
  );
  const expectedMetadata =
    'cargo metadata --locked --no-deps\n' +
    'cargo metadata --locked --no-deps --manifest-path BitFun-Installer/src-tauri/Cargo.toml\n';
  assert.equal(steps[committedMetadataIndex].run, expectedMetadata);
  assert.equal(steps[metadataIndex].run, expectedMetadata);
  assert.equal(steps[tauriAlignmentIndex].if, "runner.os == 'Windows'");
  assert.match(
    steps[tauriAlignmentIndex].run,
    /Found version mismatched Tauri packages/,
  );
  assert.equal(
    steps.some((step) => step.run?.includes('cargo generate-lockfile')),
    false,
    'nightly must not hide stale committed lockfiles by regenerating them ad hoc',
  );
  assert.equal(
    steps.find((step) => step.name === 'Run Windows CLI terminal contracts')?.run,
    'cargo test --locked -p bitfun-cli --test terminal_process_contracts -- --test-threads=1',
  );
});

test('nightly orchestrates the shared build before the separately privileged publish', () => {
  const workflow = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/nightly.yml'), 'utf8'),
  );
  const build = workflow.jobs['build-artifacts'];
  const publish = workflow.jobs['publish-nightly'];

  assert.equal(workflow.on.workflow_call, undefined);
  assert.equal(build.uses, './.github/workflows/nightly-artifacts.yml');
  assert.deepEqual(build.permissions, { contents: 'read' });
  assert.deepEqual(build.with, {
    checkout_ref: '${{ github.sha }}',
    version: '${{ needs.check-changes.outputs.nightly_version }}',
    artifact_prefix: 'nightly',
    artifact_retention_days: '${{ inputs.artifact_retention_days || 7 }}',
  });
  assert.deepEqual(publish.needs, ['check-changes', 'build-artifacts']);
  assert.match(publish.if, /inputs\.build_only != true/);
  assert.deepEqual(publish.permissions, {
    contents: 'write',
    packages: 'write',
  });
});

test('Linux binary packaging uses the shared locked version projection contract', () => {
  const workflow = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/linux-binaries.yml'), 'utf8'),
  );
  const inputs = workflow.on.workflow_call.inputs;
  const steps = workflow.jobs.build.steps;
  const nodeIndex = steps.findIndex(
    (step) => step.name === 'Setup Node.js',
  );
  const committedIndex = steps.findIndex(
    (step) => step.name === 'Verify committed Cargo metadata',
  );
  const patchIndex = steps.findIndex(
    (step) => step.name === 'Patch build version',
  );
  const projectedIndex = steps.findIndex(
    (step) => step.name === 'Verify projected Cargo metadata',
  );
  const buildIndex = steps.findIndex(
    (step) => step.name === 'Build CLI and Relay Server',
  );

  assert.equal(inputs.artifact_retention_days.default, 7);
  assert.equal(steps[nodeIndex].uses, 'actions/setup-node@v5');
  assert.equal(steps[nodeIndex].with['node-version-file'], 'package.json');
  assert.ok(
    nodeIndex < patchIndex &&
      committedIndex < patchIndex &&
      patchIndex < projectedIndex &&
      projectedIndex < buildIndex,
  );
  assert.match(steps[patchIndex].run, /node scripts\/set-build-version\.mjs/);
  assert.doesNotMatch(steps[patchIndex].run, /sed -i/);
  assert.equal(steps[committedIndex].run, 'cargo metadata --locked --no-deps');
  assert.equal(steps[projectedIndex].run, 'cargo metadata --locked --no-deps');
  assert.match(steps[buildIndex].run, /cargo build --locked --release/);
  const upload = steps.find((step) => step.uses?.startsWith('actions/upload-artifact@'));
  assert.equal(
    upload?.with?.['retention-days'],
    '${{ inputs.artifact_retention_days }}',
  );
});

test('nightly publishes and verifies the Relay image in the current repository owner scope', () => {
  const workflow = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/nightly.yml'), 'utf8'),
  );
  const steps = workflow.jobs['publish-nightly'].steps;
  const metadata = steps.find(
    (step) => step.name === 'Resolve nightly image metadata',
  );
  const publish = steps.find(
    (step) => step.name === 'Build and push multi-platform Relay image',
  );
  const smoke = steps.find(
    (step) => step.name === 'Smoke-test published Relay image on both platforms',
  );
  const manifest = steps.find(
    (step) => step.name === 'Generate Linux binaries manifest',
  );
  const verifyDescriptor = steps.find(
    (step) => step.name === 'Verify published Relay image descriptor',
  );
  const verifyMacCli = steps.find(
    (step) => step.name === 'Verify published macOS CLI assets',
  );
  const image = '${{ steps.nightly-image-meta.outputs.image }}';

  assert.match(
    metadata?.run ?? '',
    /image=ghcr\.io\/\$\{GITHUB_REPOSITORY_OWNER,,\}\/bitfun-relay-server/,
  );
  assert.equal(
    publish?.with?.tags,
    `${image}:${'${{ env.NIGHTLY_TAG }}'}\n${image}:${'${{ steps.nightly-image-meta.outputs.asset_version }}'}\n`,
  );
  assert.equal(
    smoke?.run,
    `bash scripts/relay/smoke-image.sh \\\n  "${image}@\${IMAGE_DIGEST}"\n`,
  );
  assert.match(manifest?.run ?? '', /--repo "\$\{\{ github\.repository \}\}"/);
  assert.match(
    verifyDescriptor?.run ?? '',
    /\$\{GITHUB_SERVER_URL\}\/\$\{GITHUB_REPOSITORY\}\/releases\/download/,
  );
  assert.match(
    verifyMacCli?.run ?? '',
    /\$\{GITHUB_SERVER_URL\}\/\$\{GITHUB_REPOSITORY\}\/releases\/download/,
  );
});

test('passes the verification key when signing the versioned Windows installer', () => {
  const workflow = yaml.parse(
    readFileSync(
      path.join(repoRoot, '.github/workflows/desktop-package.yml'),
      'utf8',
    ),
  );
  const signingStep = workflow.jobs['upload-release-assets'].steps.find(
    (step) => step.name === 'Sign versioned Windows installer',
  );

  assert.equal(
    signingStep?.env?.BITFUN_SIGNING_PUBKEY,
    '${{ secrets.TAURI_UPDATER_PUBKEY }}',
    'release signatures must be self-verified with the configured public key',
  );
});

test('stages unique release asset names before publishing', () => {
  const workflow = yaml.parse(
    readFileSync(
      path.join(repoRoot, '.github/workflows/desktop-package.yml'),
      'utf8',
    ),
  );
  const steps = workflow.jobs['upload-release-assets'].steps;
  const stagingIndexes = [
    steps.findIndex((step) => step.name === 'Stage stable release assets'),
    steps.findIndex((step) => step.name === 'Stage beta release assets'),
  ];
  const uploadIndex = steps.findIndex((step) => step.name === 'Upload to release');

  assert.equal(stagingIndexes.every((index) => index >= 0), true);
  assert.notEqual(uploadIndex, -1);
  for (const stagingIndex of stagingIndexes) {
    assert.ok(stagingIndex < uploadIndex);
    assert.match(
      steps[stagingIndex].run,
      /node scripts\/stage-github-release-assets\.mjs/,
    );
    assert.doesNotMatch(
      steps[stagingIndex].run,
      /release-assets\/\*\*\/\*\.sig(?:\s|\\)/,
      'raw updater signatures have colliding names across macOS architectures',
    );
  }
  assert.equal(steps[uploadIndex].with.files, 'release-upload-assets/*');
});

test('Desktop packaging keeps beta identity explicit and stable-safe', () => {
  const workflow = yaml.parse(
    readFileSync(
      path.join(repoRoot, '.github/workflows/desktop-package.yml'),
      'utf8',
    ),
  );
  const inputs = workflow.on.workflow_dispatch.inputs;
  assert.deepEqual(inputs.release_channel.options, ['stable', 'beta']);
  assert.equal(inputs.release_channel.default, 'stable');

  const prepareStep = workflow.jobs.prepare.steps.find(
    (step) => step.name === 'Resolve version metadata',
  );
  assert.match(prepareStep.run, /GITHUB_REPOSITORY.*GCWing\/BitFun/);
  assert.match(prepareStep.run, /merge-base --is-ancestor/);
  assert.match(prepareStep.run, /rev-parse --verify --quiet/);

  const packageJob = workflow.jobs.package;
  assert.equal(
    packageJob.env.BITFUN_RELEASE_CHANNEL,
    '${{ needs.prepare.outputs.release_channel }}',
  );
  assert.match(packageJob.env.TAURI_UPDATER_ENDPOINT, /github\.repository/);
  assert.match(packageJob.env.TAURI_UPDATER_ENDPOINT, /channel-beta/);
  assert.match(packageJob.env.BITFUN_RELEASE_PUBKEY, /BITFUN_RELEASE_PUBKEY/);
  const appleSetupIndex = packageJob.steps.findIndex(
    (step) => step.name === 'Configure Apple Developer ID signing and notarization',
  );
  const desktopBuildIndex = packageJob.steps.findIndex(
    (step) => step.name === 'Build desktop app',
  );
  const appleVerifyIndex = packageJob.steps.findIndex(
    (step) => step.name === 'Verify Apple signature and notarization',
  );
  assert.ok(
    appleSetupIndex >= 0 &&
      appleSetupIndex < desktopBuildIndex &&
      desktopBuildIndex < appleVerifyIndex,
    'Apple credentials must be configured before packaging and verified afterwards',
  );
  assert.equal(packageJob.steps[appleSetupIndex].if, "runner.os == 'macOS'");
  assert.equal(
    packageJob.steps[appleSetupIndex].env.BITFUN_REQUIRE_APPLE_SIGNING,
    '${{ needs.prepare.outputs.upload_to_release }}',
  );
  assert.equal(packageJob.steps[appleVerifyIndex].if, "runner.os == 'macOS'");
  const patchIndex = packageJob.steps.findIndex(
    (step) => step.name === 'Project beta build version',
  );
  const committedMetadataIndex = packageJob.steps.findIndex(
    (step) => step.name === 'Verify committed Cargo metadata',
  );
  const buildMetadataIndex = packageJob.steps.findIndex(
    (step) => step.name === 'Verify build Cargo metadata',
  );
  const verifyIndex = packageJob.steps.findIndex(
    (step) => step.name === 'Verify release version metadata',
  );
  assert.ok(
    committedMetadataIndex >= 0 &&
      committedMetadataIndex < patchIndex &&
      patchIndex < buildMetadataIndex &&
      buildMetadataIndex < verifyIndex,
  );
  const expectedMetadata =
    'cargo metadata --locked --no-deps\n' +
    'cargo metadata --locked --no-deps --manifest-path BitFun-Installer/src-tauri/Cargo.toml\n';
  assert.equal(packageJob.steps[committedMetadataIndex].run, expectedMetadata);
  assert.equal(packageJob.steps[buildMetadataIndex].run, expectedMetadata);
  assert.equal(
    packageJob.steps[patchIndex].if,
    "needs.prepare.outputs.release_channel == 'beta'",
  );

  const uploadSteps = workflow.jobs['upload-release-assets'].steps;
  const release = uploadSteps.find((step) => step.name === 'Upload to release');
  assert.equal(
    release.with.prerelease,
    "${{ needs.prepare.outputs.release_channel == 'beta' }}",
  );
  const verifyIndexPublished = uploadSteps.findIndex(
    (step) => step.name === 'Verify published updater manifest',
  );
  const promoteIndex = uploadSteps.findIndex(
    (step) => step.name === 'Publish beta channel manifest',
  );
  assert.ok(verifyIndexPublished >= 0 && verifyIndexPublished < promoteIndex);
  assert.match(workflow.jobs['linux-binaries'].if, /release_channel == 'stable'/);
  assert.equal(
    uploadSteps.find((step) => step.name === 'Stage beta release assets').if,
    "needs.prepare.outputs.release_channel == 'beta'",
  );
  assert.match(
    uploadSteps.find((step) => step.name === 'Generate updater manifest').run,
    /github\.repository/,
  );
  const signingStep = uploadSteps.find(
    (step) => step.name === 'Sign installer packages',
  );
  assert.match(signingStep.run, /write-minisign-public-key\.mjs/);
  assert.doesNotMatch(signingStep.run, /BITFUN_SIGNING_PUBKEY.*base64 -d/);
  const promotionStep = uploadSteps.find(
    (step) => step.name === 'Resolve beta channel promotion',
  );
  assert.doesNotMatch(promotionStep.run, /current\.beta\.json \|\| true/);
  assert.match(promotionStep.run, /case "\$\{channel_status\}" in/);
  assert.match(promotionStep.run, /404\)/);
  assert.match(promotionStep.run, /GitHub API returned/);
  const publishStep = uploadSteps.find(
    (step) => step.name === 'Publish beta channel manifest',
  );
  assert.equal(
    publishStep.env.CHANNEL_EXISTS,
    '${{ steps.beta-channel.outputs.channel_exists }}',
  );
});

test('beta publishing cannot advance the Relay latest image tag', () => {
  const workflow = yaml.parse(
    readFileSync(
      path.join(repoRoot, '.github/workflows/desktop-package.yml'),
      'utf8',
    ),
  );
  const imageTags = workflow.jobs['publish-relay-image'].steps.find(
    (step) => step.name === 'Resolve image tags',
  );
  assert.equal(
    imageTags.env.RELEASE_CHANNEL,
    '${{ needs.prepare.outputs.release_channel }}',
  );
  assert.match(imageTags.run, /RELEASE_CHANNEL.*stable/);
  assert.doesNotMatch(imageTags.run, /RELEASE_PRERELEASE/);
});

test('nightly and beta use the shared build-version projection', () => {
  const artifacts = yaml.parse(
    readFileSync(
      path.join(repoRoot, '.github/workflows/nightly-artifacts.yml'),
      'utf8',
    ),
  );
  const nightly = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/nightly.yml'), 'utf8'),
  );
  const patch = artifacts.jobs.package.steps.find(
    (step) => step.name === 'Patch nightly version',
  );
  assert.match(patch.run, /node scripts\/set-build-version\.mjs/);
  assert.equal(artifacts.jobs.package.env.BITFUN_RELEASE_CHANNEL, 'nightly');
  assert.equal(
    artifacts.jobs.package.env.TAURI_UPDATER_ENDPOINT,
    'https://github.com/GCWing/BitFun/releases/latest/download/latest.json',
  );
  assert.equal(
    artifacts.jobs.package.env.TAURI_UPDATER_FALLBACK_ENDPOINT,
    'https://openbitfun.com/release/latest.json',
  );
  assert.equal(artifacts.jobs.package.env.BITFUN_ENABLE_UPDATER_ARTIFACTS, undefined);
  const signingStep = nightly.jobs['publish-nightly'].steps.find(
    (step) => step.name === 'Sign installer packages',
  );
  assert.match(signingStep.run, /write-minisign-public-key\.mjs/);
});

test('Installer packaging consumes its committed Cargo.lock', () => {
  const installer = JSON.parse(
    readFileSync(path.join(repoRoot, 'BitFun-Installer/package.json'), 'utf8'),
  );
  for (const scriptName of [
    'tauri:build',
    'tauri:build:fast',
    'tauri:build:exe',
    'tauri:build:exe:fast',
  ]) {
    assert.match(
      installer.scripts[scriptName],
      /tauri build(?: --no-bundle)? -- --locked(?: |$)/,
      `${scriptName} must reject installer lockfile drift`,
    );
  }
});
