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
    'rust-impact',
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
  assert.equal(cliJob['timeout-minutes'], 30);
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

test('gates Rust and CLI validation behind one fail-closed impact decision', (t) => {
  const workflow = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8'),
  );
  for (const eventName of ['pull_request', 'push']) {
    assert.deepEqual(
      workflow.on[eventName]?.['paths-ignore'],
      ['png/**'],
      'nested Markdown may be a Rust compile-time input and must trigger classification',
    );
  }
  const impactJob = workflow.jobs['rust-impact'];
  const cliJob = workflow.jobs['cli-test'];
  const rustJob = workflow.jobs['rust-build-check'];
  const buildJob = workflow.jobs['nightly-build-contract'];
  const resultJob = workflow.jobs['rust-validation-result'];
  const frontendJob = workflow.jobs['frontend-build'];

  assert.equal(
    frontendJob.steps.find((step) => step.name === 'Test core boundary contracts')?.run,
    'pnpm run check:core-boundaries:test',
  );
  assert.equal(
    frontendJob.steps.find((step) => step.name === 'Check core boundaries')?.run,
    'pnpm run check:core-boundaries',
  );

  assert.equal(impactJob.name, 'Rust / CLI Impact');
  assert.equal(impactJob['timeout-minutes'], 5);
  assert.equal(
    impactJob.outputs.rust_required,
    '${{ steps.classify.outputs.rust_required }}',
  );
  const checkout = impactJob.steps.find((step) => step.uses?.startsWith('actions/checkout@'));
  assert.equal(checkout?.with?.['fetch-depth'], 0);
  const classify = impactJob.steps.find((step) => step.id === 'classify');
  assert.match(classify?.run ?? '', /scripts\/ci\/classify-rust-impact\.mjs/);
  assert.equal(
    classify?.env?.BASE_SHA,
    '${{ github.event.pull_request.base.sha || github.event.before }}',
  );
  assert.equal(
    classify?.env?.HEAD_SHA,
    '${{ github.event.pull_request.head.sha || github.sha }}',
  );
  assert.equal(
    classify?.env?.RANGE_MODE,
    "${{ github.event_name == 'pull_request' && 'merge-base' || 'direct' }}",
  );
  assert.match(classify?.run ?? '', /--range-mode "\$RANGE_MODE"/);

  for (const job of [cliJob, rustJob]) {
    assert.equal(job.needs, 'rust-impact');
    assert.match(job.if, /!cancelled\(\)/);
    assert.doesNotMatch(job.if, /always\(\)/);
    assert.match(job.if, /rust_required != 'false'/);
  }

  assert.equal(buildJob.name, 'Nightly Build Contract');
  assert.equal(buildJob.needs, 'rust-impact');
  assert.equal(buildJob.uses, './.github/workflows/nightly.yml');
  assert.match(buildJob.if, /rust-impact\.result == 'success'/);
  assert.doesNotMatch(buildJob.if, /rust_required/);
  assert.deepEqual(buildJob.permissions, { contents: 'read' });
  assert.deepEqual(buildJob.with, {
    force_build: true,
    build_only: true,
    artifact_retention_days: 1,
  });

  assert.equal(resultJob.name, 'Rust / CLI Validation');
  assert.equal(resultJob.if, '${{ always() }}');
  assert.deepEqual(
    [...resultJob.needs].sort(),
    ['cli-test', 'nightly-build-contract', 'rust-build-check', 'rust-impact'],
  );
  const verify = resultJob.steps.find((step) => step.name === 'Verify Rust and CLI result');
  assert.equal(verify?.env?.RUST_REQUIRED, '${{ needs.rust-impact.outputs.rust_required }}');
  assert.equal(verify?.env?.IMPACT_RESULT, '${{ needs.rust-impact.result }}');
  assert.equal(verify?.env?.CLI_RESULT, '${{ needs.cli-test.result }}');
  assert.equal(verify?.env?.RUST_RESULT, '${{ needs.rust-build-check.result }}');
  assert.equal(
    verify?.env?.BUILD_RESULT,
    '${{ needs.nightly-build-contract.result }}',
  );
  assert.equal(verify?.shell, 'pwsh');
  assert.match(verify?.run ?? '', /successful Nightly build contract/i);
  assert.match(verify?.run ?? '', /successful Rust, CLI, and Nightly build jobs/i);

  const statuses = ['success', 'skipped', 'failure', 'cancelled'];
  const cases = [];
  for (const impactResult of statuses.filter((status) => status !== 'success')) {
    cases.push({
      rustRequired: 'true',
      impactResult,
      cliResult: 'success',
      rustResult: 'success',
      buildResult: 'success',
      expectedSuccess: false,
    });
  }
  for (const rustRequired of ['false', 'true']) {
    for (const cliResult of statuses) {
      for (const rustResult of statuses) {
        for (const buildResult of statuses) {
          cases.push({
            rustRequired,
            impactResult: 'success',
            cliResult,
            rustResult,
            buildResult,
            expectedSuccess: buildResult === 'success' && (rustRequired === 'false'
              ? cliResult === 'skipped' && rustResult === 'skipped'
              : cliResult === 'success' && rustResult === 'success'),
          });
        }
      }
    }
  }
  cases.push({
    rustRequired: '',
    impactResult: 'success',
    cliResult: 'skipped',
    rustResult: 'skipped',
    buildResult: 'success',
    expectedSuccess: false,
  });
  const truthTable = spawnSync(
    'pwsh',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      `$cases = ConvertFrom-Json @'
${JSON.stringify(cases)}
'@
$verify = {
${verify.run}
}
foreach ($case in $cases) {
  $env:RUST_REQUIRED = [string]$case.rustRequired
  $env:IMPACT_RESULT = [string]$case.impactResult
  $env:CLI_RESULT = [string]$case.cliResult
  $env:RUST_RESULT = [string]$case.rustResult
  $env:BUILD_RESULT = [string]$case.buildResult
  $succeeded = $true
  try { & $verify } catch { $succeeded = $false }
  if ($succeeded -ne [bool]$case.expectedSuccess) {
    throw "Unexpected result: $($case | ConvertTo-Json -Compress) succeeded=$succeeded"
  }
}`,
    ],
    {
      cwd: repoRoot,
      env: process.env,
      encoding: 'utf8',
    },
  );
  if (truthTable.error?.code === 'ENOENT') {
    t.skip('pwsh is not installed; GitHub-hosted runners execute this truth table');
    return;
  }
  assert.equal(truthTable.status, 0, `${truthTable.stdout}${truthTable.stderr}`);
});

test('nightly validates generated inputs and projected lockfiles before packaging', () => {
  const workflow = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/nightly.yml'), 'utf8'),
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
  const metadataIndex = steps.findIndex(
    (step) => step.name === 'Verify projected Cargo metadata',
  );
  const buildIndex = steps.findIndex(
    (step) => step.name === 'Build desktop app',
  );

  assert.equal(callInputs.force_build.default, true);
  assert.equal(callInputs.build_only.default, true);
  assert.equal(callInputs.artifact_retention_days.default, 1);
  assert.equal(workflow.permissions.contents, 'read');
  assert.match(
    workflow.jobs['publish-nightly'].if,
    /inputs\.build_only != true/,
  );
  assert.deepEqual(workflow.jobs['publish-nightly'].permissions, {
    contents: 'write',
    packages: 'write',
  });

  assert.notEqual(committedMetadataIndex, -1);
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
  const nightly = yaml.parse(
    readFileSync(path.join(repoRoot, '.github/workflows/nightly.yml'), 'utf8'),
  );
  const patch = nightly.jobs.package.steps.find(
    (step) => step.name === 'Patch nightly version',
  );
  assert.match(patch.run, /node scripts\/set-build-version\.mjs/);
  assert.equal(nightly.jobs.package.env.BITFUN_RELEASE_CHANNEL, 'nightly');
  assert.equal(
    nightly.jobs.package.env.TAURI_UPDATER_ENDPOINT,
    'https://github.com/GCWing/BitFun/releases/latest/download/latest.json',
  );
  assert.equal(
    nightly.jobs.package.env.TAURI_UPDATER_FALLBACK_ENDPOINT,
    'https://openbitfun.com/release/latest.json',
  );
  assert.equal(nightly.jobs.package.env.BITFUN_ENABLE_UPDATER_ARTIFACTS, undefined);
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

test('Linux Rust workflows do not install an unused native OpenSSL toolchain', () => {
  for (const workflowPath of [
    '.github/workflows/ci.yml',
    '.github/workflows/cli-package-manual.yml',
    '.github/workflows/linux-binaries.yml',
  ]) {
    const workflow = readFileSync(path.join(repoRoot, workflowPath), 'utf8');
    assert.doesNotMatch(
      workflow,
      /\blibssl-dev\b/,
      `${workflowPath} must rely on the reviewed Cargo-owned Git2 build profile`,
    );
  }
});
