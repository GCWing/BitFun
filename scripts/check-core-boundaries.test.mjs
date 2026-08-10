import { access, readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  collectCargoMetadataGraph,
  collectCargoMetadataPackages,
  findCargoLayerViolations,
  findFeatureGatedTestTargetViolations,
  findProductEntrypointCoreFeatureViolations,
  findReqwestDependencyFeatureViolations,
  findRuntimeServicesTestSupportFeatureViolations,
  findResolvedReqwestNativeTlsViolations,
  findServicesIntegrationsReqwestFeatureViolations,
  findServicesIntegrationsTokioFeatureViolations,
  findTokioDependencyFeatureViolations,
} from './core-boundaries/cargo-dependency-boundaries.mjs';
import {
  checkCliIntegrationTestTopology,
  checkServicesCoreIntegrationTestTopology,
  checkServicesIntegrationsIntegrationTestTopology,
  cliIntegrationTestTargets,
  validateExplicitIntegrationTestTopology,
} from './core-boundaries/explicit-test-topology.mjs';
import { crateLayoutRules } from './core-boundaries/rules/crate-layout.mjs';
import {
  coreClosedFeatureProfileRules,
  coreProductFullFeatureAssemblyRule,
} from './core-boundaries/rules/feature-rules.mjs';

const ENTRYPOINT = new URL('./check-core-boundaries.mjs', import.meta.url);
const MODULES = [
  './core-boundaries/checker.mjs',
  './core-boundaries/cargo-dependency-boundaries.mjs',
  './core-boundaries/explicit-test-topology.mjs',
  './core-boundaries/manifest-feature-helpers.mjs',
  './core-boundaries/self-test.mjs',
  './core-boundaries/tui-boundary-ratchet.mjs',
  './core-boundaries/rules/crate-rules.mjs',
  './core-boundaries/rules/feature-rules.mjs',
  './core-boundaries/rules/source-rules.mjs',
  './core-boundaries/rules/source/facade-rules.mjs',
  './core-boundaries/rules/source/forbidden-rules.mjs',
  './core-boundaries/rules/source/public-api-rules.mjs',
  './core-boundaries/rules/source/required-rules.mjs',
];

const TEST_ROOT = join('C:', 'repo');

function parseManifestFeatures(manifest) {
  const section = manifest.match(/^\[features\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m)?.[1] ?? '';
  const features = {};

  for (const match of section.matchAll(/^([a-zA-Z0-9_-]+)\s*=\s*\[([\s\S]*?)\]/gm)) {
    features[match[1]] = [...match[2].matchAll(/["']([^"']+)["']/g)].map((value) => value[1]);
  }

  return features;
}

function removeFeatureValue(manifest, feature, value) {
  const featurePattern = new RegExp(`^${feature}\\s*=\\s*\\[([\\s\\S]*?)\\]`, 'm');
  return manifest.replace(featurePattern, (definition) =>
    definition.replace(new RegExp(`\\s*["']${value}["'],?`), ''));
}

function servicesIntegrationsPackage(manifest) {
  return {
    name: 'bitfun-services-integrations',
    manifest_path: join(TEST_ROOT, 'src', 'crates', 'services', 'services-integrations', 'Cargo.toml'),
    features: parseManifestFeatures(manifest),
  };
}

function packageAt(name, repoManifestPath, dependencies = []) {
  return {
    id: name,
    name,
    manifest_path: join(TEST_ROOT, ...repoManifestPath.split('/')),
    dependencies,
  };
}

function pathDependency(repoCratePath, options = {}) {
  return {
    name: options.name ?? repoCratePath.split('/').at(-1),
    rename: options.rename ?? null,
    path: join(TEST_ROOT, ...repoCratePath.split('/')),
    kind: options.kind ?? null,
    optional: options.optional ?? false,
    target: options.target ?? null,
    uses_default_features: options.usesDefaultFeatures ?? true,
    features: options.features ?? [],
  };
}

function integrationTarget(name, sourcePath, requiredFeatures = []) {
  return {
    kind: ['test'],
    name,
    src_path: sourcePath,
    'required-features': requiredFeatures,
  };
}

test('feature-gated integration targets require every positive crate feature', () => {
  const sourcePath = join(TEST_ROOT, 'tests', 'remote.rs');
  const pkg = {
    ...packageAt('example', 'src/crates/services/example/Cargo.toml'),
    targets: [integrationTarget('remote', sourcePath, ['remote-ssh'])],
  };
  const sources = new Map([[
    sourcePath,
    '#![cfg(all(feature = "remote-ssh", feature = "workspace-search", not(feature = "remote-ssh-concrete")))]\n',
  ]]);

  const violations = findFeatureGatedTestTargetViolations([pkg], {
    readSource: (path) => sources.get(path),
  });

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /workspace-search/);
  assert.doesNotMatch(violations[0].message, /remote-ssh-concrete.*missing/);
});

test('matching integration target requirements cover all positive crate features', () => {
  const sourcePath = join(TEST_ROOT, 'tests', 'remote.rs');
  const pkg = {
    ...packageAt('example', 'src/crates/services/example/Cargo.toml'),
    targets: [integrationTarget(
      'remote',
      sourcePath,
      ['remote-ssh', 'workspace-search'],
    )],
  };

  assert.deepEqual(
    findFeatureGatedTestTargetViolations([pkg], {
      readSource: () => '#![cfg(all(feature = "remote-ssh", feature = "workspace-search", not(feature = "remote-ssh-concrete")))]\n',
    }),
    [],
  );
});

test('runtime-services test support stays dev-only across dependency and feature edges', () => {
  const runtimeServicesPath = 'src/crates/execution/runtime-services';
  const packages = [
    packageAt('normal-consumer', 'src/apps/normal/Cargo.toml', [
      pathDependency(runtimeServicesPath, {
        name: 'bitfun-runtime-services',
        features: ['test-support'],
      }),
    ]),
    packageAt('build-consumer', 'src/apps/build/Cargo.toml', [
      pathDependency(runtimeServicesPath, {
        name: 'bitfun-runtime-services',
        kind: 'build',
        features: ['test-support'],
      }),
    ]),
    {
      ...packageAt('feature-forwarder', 'src/apps/forwarder/Cargo.toml'),
      features: {
        preview: ['bitfun-runtime-services/test-support'],
      },
    },
    {
      ...packageAt('weak-forwarder', 'src/apps/weak/Cargo.toml'),
      features: {
        preview: ['bitfun-runtime-services?/test-support'],
      },
    },
    {
      ...packageAt('renamed-forwarder', 'src/apps/renamed/Cargo.toml', [{
        ...pathDependency(runtimeServicesPath, {
          name: 'bitfun-runtime-services',
          optional: true,
        }),
        rename: 'runtime_services',
      }]),
      features: {
        preview: ['runtime_services?/test-support'],
      },
    },
    {
      ...packageAt(
        'bitfun-runtime-services',
        'src/crates/execution/runtime-services/Cargo.toml',
      ),
      features: {
        default: ['test-support'],
        'test-support': [],
      },
    },
    packageAt('test-consumer', 'src/apps/test/Cargo.toml', [
      pathDependency(runtimeServicesPath, {
        name: 'bitfun-runtime-services',
        kind: 'dev',
        features: ['test-support'],
      }),
    ]),
  ];

  const violations = findRuntimeServicesTestSupportFeatureViolations(packages);

  assert.equal(violations.length, 6);
  assert.match(violations[0].message, /normal-consumer.*normal dependency/);
  assert.match(violations[1].message, /build-consumer.*build dependency/);
  assert.match(violations[2].message, /feature-forwarder:preview/);
  assert.match(violations[3].message, /weak-forwarder:preview/);
  assert.match(violations[4].message, /renamed-forwarder:preview/);
  assert.match(violations[5].message, /bitfun-runtime-services:default/);
});

test('runtime-services feature aliases cannot hide test support from default builds', () => {
  const owner = {
    ...packageAt(
      'bitfun-runtime-services',
      'src/crates/execution/runtime-services/Cargo.toml',
    ),
    features: {
      default: ['testing'],
      testing: ['test-support'],
      'test-support': [],
    },
  };

  const messages = findRuntimeServicesTestSupportFeatureViolations([owner])
    .map((violation) => violation.message)
    .join('\n');

  assert.match(messages, /bitfun-runtime-services:default/);
  assert.match(messages, /default -> testing -> test-support/);
  assert.match(messages, /bitfun-runtime-services:testing/);
});

test('CLI integration tests keep the reviewed three-target topology', () => {
  const repositoryRoot = fileURLToPath(new URL('..', import.meta.url));

  assert.deepEqual(cliIntegrationTestTargets, [
    { name: 'acp_stdio_cli', path: 'tests/acp_stdio_cli.rs' },
    { name: 'cli_command_contracts', path: 'tests/cli_command_contracts.rs' },
    { name: 'terminal_process_contracts', path: 'tests/terminal_process_contracts.rs' },
  ]);
  assert.deepEqual(checkCliIntegrationTestTopology(repositoryRoot), []);
});

test('service integration tests keep their reviewed explicit target topology', () => {
  const repositoryRoot = fileURLToPath(new URL('..', import.meta.url));

  assert.deepEqual(checkServicesCoreIntegrationTestTopology(repositoryRoot), []);
  assert.deepEqual(checkServicesIntegrationsIntegrationTestTopology(repositoryRoot), []);
});

test('runtime-services test support is absent from ordinary library builds', async () => {
  const [manifest, library] = await Promise.all([
    readFile(
      new URL('../src/crates/execution/runtime-services/Cargo.toml', import.meta.url),
      'utf8',
    ),
    readFile(
      new URL('../src/crates/execution/runtime-services/src/lib.rs', import.meta.url),
      'utf8',
    ),
  ]);

  assert.match(manifest, /^test-support\s*=\s*\[\]\s*$/m);
  assert.doesNotMatch(manifest, /^required-features\s*=.*test-support.*$/m);
  assert.match(
    library,
    /#\[cfg\(any\(test, feature = "test-support"\)\)\]\s*pub mod test_support;/,
  );
  assert.match(library, /#\[cfg\(test\)\]\s*mod runtime_services_contracts;/);
  assert.equal((library.match(/^pub mod test_support;\s*$/gm) ?? []).length, 1);
});

test('feature-gated integration targets reject extra umbrella requirements', () => {
  const sourcePath = join(TEST_ROOT, 'tests', 'focused.rs');
  const pkg = {
    ...packageAt('example', 'src/crates/services/example/Cargo.toml'),
    targets: [integrationTarget('focused', sourcePath, ['focused', 'product-full'])],
  };

  const violations = findFeatureGatedTestTargetViolations([pkg], {
    readSource: () => '#![cfg(feature = "focused")]\n',
  });

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /unexpected required-features: product-full/);
});

test('target guard ignores module cfg and non-integration targets', () => {
  const moduleSourcePath = join(TEST_ROOT, 'tests', 'module.rs');
  const binarySourcePath = join(TEST_ROOT, 'src', 'main.rs');
  const pkg = {
    ...packageAt('example', 'src/crates/services/example/Cargo.toml'),
    targets: [
      integrationTarget('module', moduleSourcePath),
      {
        ...integrationTarget('binary', binarySourcePath),
        kind: ['bin'],
      },
    ],
  };
  const sources = new Map([
    [moduleSourcePath, '#[cfg(feature = "serde")]\nmod serde_tests {}\n'],
    [binarySourcePath, '#![cfg(feature = "cli")]\nfn main() {}\n'],
  ]);

  assert.deepEqual(
    findFeatureGatedTestTargetViolations([pkg], {
      readSource: (path) => sources.get(path),
    }),
    [],
  );
});

test('target guard ignores crate cfg examples in comments and strings', () => {
  const sourcePath = join(TEST_ROOT, 'tests', 'documented.rs');
  const pkg = {
    ...packageAt('example', 'src/crates/services/example/Cargo.toml'),
    targets: [integrationTarget('documented', sourcePath)],
  };

  assert.deepEqual(
    findFeatureGatedTestTargetViolations([pkg], {
      readSource: () => [
        '// Example: #![cfg(feature = "commented")]',
        'const EXAMPLE: &str = r#"',
        '#![cfg(feature = "string-literal")]',
        '"#;',
      ].join('\n'),
    }),
    [],
  );
});

test('target guard rejects feature OR gates that Cargo cannot express', () => {
  const sourcePath = join(TEST_ROOT, 'tests', 'provider.rs');
  const pkg = {
    ...packageAt('example', 'src/crates/services/example/Cargo.toml'),
    targets: [integrationTarget('provider', sourcePath, ['provider-a'])],
  };

  const violations = findFeatureGatedTestTargetViolations([pkg], {
    readSource: () => '#![cfg(any(feature = "provider-a", feature = "provider-b"))]\n',
  });

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /cannot express.*split the target/);
});

test('multiple crate feature gates combine as required feature AND conditions', () => {
  const sourcePath = join(TEST_ROOT, 'tests', 'combined.rs');
  const pkg = {
    ...packageAt('example', 'src/crates/services/example/Cargo.toml'),
    targets: [integrationTarget('combined', sourcePath, ['first'])],
  };

  const violations = findFeatureGatedTestTargetViolations([pkg], {
    readSource: () => '#![cfg(feature = "first")]\n#![cfg(feature = "second")]\n',
  });

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /second/);
});

test('product entrypoints must disable bitfun-core default features', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const app = packageAt('entry', 'src/apps/example/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      features: ['plugin-source'],
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [app, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /default-features = false/);
});

test('Core Agent Runtime baseline excludes concrete capability unions', () => {
  const agentRuntime = coreClosedFeatureProfileRules.find(
    (rule) => rule.featureName === 'agent-runtime',
  );
  assert.ok(agentRuntime, 'agent-runtime closed profile must exist');

  for (const forbidden of [
    'bitfun-services-integrations/browser-control',
    'bitfun-services-integrations/deep-research',
    'bitfun-services-integrations/mcp',
    'bitfun-services-integrations/models-dev',
    'bitfun-services-integrations/remote-connect',
    'bitfun-services-integrations/script-tool-runtime',
    'bitfun-services-integrations/web-tools',
    'bitfun-services-integrations/workspace-search',
    'dep:cron',
    'dep:semver',
    'dep:tokio-tungstenite',
    'git',
    'review-platform',
  ]) {
    assert.ok(
      !agentRuntime.requiredFeatureRefs.includes(forbidden),
      `agent-runtime must not own ${forbidden}`,
    );
  }
});

test('Core product-full explicitly assembles service and tool capability owners', () => {
  for (const required of [
    'model-catalog',
    'mcp-runtime',
    'remote-connect',
    'workspace-search',
    'browser-control',
    'web-tools',
    'deep-research',
    'scheduled-jobs',
    'tools-basic',
    'tools-git',
    'tools-mcp',
    'tools-browser-web',
    'tools-computer-use',
    'tools-image-analysis',
    'tools-miniapp',
    'tools-canvas',
    'tools-agent-control',
  ]) {
    assert.ok(
      coreProductFullFeatureAssemblyRule.requiredFeatureRefs.includes(required),
      `product-full must explicitly assemble ${required}`,
    );
  }
});

test('product entrypoints must select explicit bitfun-core features', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const interfacePackage = packageAt(
    'interface',
    'src/crates/interfaces/acp/Cargo.toml',
    [pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
    })],
  );

  const violations = findProductEntrypointCoreFeatureViolations(
    [interfacePackage, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /at least one explicit feature/);
});

test('explicit product entrypoint bitfun-core feature selections pass', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const consumers = [
    packageAt('app', 'src/apps/example/Cargo.toml'),
    packageAt('interface', 'src/crates/interfaces/acp/Cargo.toml'),
    packageAt('installer', 'BitFun-Installer/src-tauri/Cargo.toml'),
  ].map((pkg) => ({
    ...pkg,
    dependencies: [pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ['plugin-source'],
    })],
  }));

  assert.deepEqual(
    findProductEntrypointCoreFeatureViolations(
      [...consumers, core, packageAt('no-core', 'src/apps/no-core/Cargo.toml')],
      { root: TEST_ROOT, crateLayoutRules },
    ),
    [],
  );
});

const ACP_REVIEWED_CORE_FEATURES = [
  'agent-runtime',
  'deep-research',
  'lsp',
  'external-sources',
  'ssh-remote',
  'tools-basic',
  'tools-git',
  'tools-mcp',
  'tools-browser-web',
  'tools-computer-use',
  'tools-image-analysis',
  'tools-miniapp',
  'tools-canvas',
  'tools-agent-control',
];

const CLI_REVIEWED_CORE_FEATURES = [
  ...ACP_REVIEWED_CORE_FEATURES,
  'remote-connect',
  'plugin-runtime',
];

const APP_SERVER_REVIEWED_CORE_FEATURES = [
  'external-sources',
  'git',
  'remote-connect',
];

test('App Server Core capability closure keeps its production Git owner', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const appServer = packageAt(
    'bitfun-app-server',
    'src/crates/interfaces/app-server/Cargo.toml',
    [pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: APP_SERVER_REVIEWED_CORE_FEATURES.filter((feature) => feature !== 'git'),
    })],
  );

  const violations = findProductEntrypointCoreFeatureViolations(
    [appServer, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.deepEqual(violations.map((violation) => violation.message), [
    'bitfun-app-server Core capability closure must include git',
  ]);
});

test('App Server reviewed Core capability closure remains independently valid', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const appServer = packageAt(
    'bitfun-app-server',
    'src/crates/interfaces/app-server/Cargo.toml',
    [pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: APP_SERVER_REVIEWED_CORE_FEATURES,
    })],
  );

  assert.deepEqual(
    findProductEntrypointCoreFeatureViolations(
      [appServer, core],
      { root: TEST_ROOT, crateLayoutRules },
    ),
    [],
  );
});

test('ACP Core capability closure must retain its Canvas tool owner', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const acp = packageAt('bitfun-acp', 'src/crates/interfaces/acp/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ACP_REVIEWED_CORE_FEATURES.filter((feature) => feature !== 'tools-canvas'),
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [acp, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /must include tools-canvas/);
});

test('ACP Core capability closure validation cannot be disabled by removing an owner', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const acp = packageAt('bitfun-acp', 'src/crates/interfaces/acp/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ACP_REVIEWED_CORE_FEATURES.filter((feature) => feature !== 'agent-runtime'),
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [acp, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.deepEqual(violations.map((violation) => violation.message), [
    'bitfun-acp Core capability closure must include agent-runtime',
  ]);
});

test('CLI Core capability closure requires every reviewed owner', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: CLI_REVIEWED_CORE_FEATURES.filter((feature) => feature !== 'plugin-runtime'),
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /must include plugin-runtime/);
});

test('CLI entrypoint must not select the product-full Core feature', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ['product-full'],
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.ok(violations.some((violation) =>
    /bitfun-cli -> bitfun-core\/product-full/.test(violation.message)));
});

test('CLI entrypoint must not reach product-full through a Core owner feature', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: {
      'agent-runtime': ['runtime-services', 'product-full'],
      'runtime-services': ['dep:bitfun-runtime-services'],
      'product-full': ['dep:bitfun-agent-runtime'],
    },
  };
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ['agent-runtime'],
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.ok(violations.some((violation) =>
    /bitfun-cli -> bitfun-core\/product-full/.test(violation.message)));
});

test('CLI dependency closure must not re-enable product-full through an interface crate', () => {
  const core = packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml');
  const acp = packageAt('bitfun-acp', 'src/crates/interfaces/acp/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ['product-full'],
    }),
  ]);
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/interfaces/acp', { name: 'bitfun-acp' }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, acp, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.ok(violations.some((violation) =>
    /bitfun-cli -> bitfun-acp -> bitfun-core\/product-full/.test(violation.message)));
});

test('CLI dependency closure rejects indirect Core default features', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { default: ['product-full'], 'product-full': [] },
  };
  const bridge = packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
    pathDependency('src/crates/assembly/core', { name: 'bitfun-core' }),
  ]);
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', { name: 'bridge' }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-cli -> bridge -> bitfun-core\/product-full/);
});

test('CLI dependency closure resolves active intermediate feature forwarding', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { default: ['product-full'], 'product-full': [] },
  };
  const bridge = {
    ...packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        optional: true,
        usesDefaultFeatures: false,
      }),
    ]),
    features: {
      default: ['full'],
      full: ['dep:bitfun-core', 'bitfun-core/product-full'],
    },
  };
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', { name: 'bridge' }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-cli -> bridge -> bitfun-core\/product-full/);
});

test('CLI dependency closure resolves renamed optional dependency forwarding', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { 'product-full': [] },
  };
  const bridge = {
    ...packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        rename: 'core-alias',
        optional: true,
        usesDefaultFeatures: false,
      }),
    ]),
    features: {
      full: ['dep:core-alias', 'core-alias/product-full'],
    },
  };
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      usesDefaultFeatures: false,
      features: ['full'],
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/product-full/);
});

test('CLI dependency closure unions weak forwarding and optional activation per package', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { 'product-full': [] },
  };
  const bridge = {
    ...packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        optional: true,
        usesDefaultFeatures: false,
      }),
    ]),
    features: {
      forward: ['bitfun-core?/product-full'],
      activate: ['dep:bitfun-core'],
    },
  };
  const left = packageAt('left', 'src/crates/assembly/left/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      usesDefaultFeatures: false,
      features: ['forward'],
    }),
  ]);
  const right = packageAt('right', 'src/crates/assembly/right/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      usesDefaultFeatures: false,
      features: ['activate'],
    }),
  ]);
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/left', { name: 'left' }),
    pathDependency('src/crates/assembly/right', { name: 'right' }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, left, right, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/product-full/);
});

test('CLI dependency closure keeps normal and build feature unions separate', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { 'product-full': [] },
  };
  const bridge = {
    ...packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        optional: true,
        usesDefaultFeatures: false,
      }),
    ]),
    features: {
      forward: ['bitfun-core?/product-full'],
      activate: ['dep:bitfun-core'],
    },
  };
  const normalParent = packageAt('normal-parent', 'src/crates/assembly/normal-parent/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      usesDefaultFeatures: false,
      features: ['forward'],
    }),
  ]);
  const buildParent = packageAt('build-parent', 'src/crates/assembly/build-parent/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      usesDefaultFeatures: false,
      features: ['activate'],
    }),
  ]);
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/normal-parent', { name: 'normal-parent' }),
    pathDependency('src/crates/assembly/build-parent', {
      name: 'build-parent',
      kind: 'build',
    }),
  ]);

  assert.deepEqual(
    findProductEntrypointCoreFeatureViolations(
      [cli, normalParent, buildParent, bridge, core],
      { root: TEST_ROOT, crateLayoutRules },
    ),
    [],
  );
});

test('CLI dependency closure keeps proc-macro and normal feature unions separate', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { 'product-full': [] },
  };
  const shared = {
    ...packageAt('shared', 'src/crates/assembly/shared/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        optional: true,
        usesDefaultFeatures: false,
      }),
    ]),
    features: {
      forward: ['bitfun-core?/product-full'],
      activate: ['dep:bitfun-core'],
    },
  };
  const normalParent = packageAt('normal-parent', 'src/crates/assembly/normal-parent/Cargo.toml', [
    pathDependency('src/crates/assembly/shared', {
      name: 'shared',
      usesDefaultFeatures: false,
      features: ['forward'],
    }),
  ]);
  const macroParent = {
    ...packageAt('macro-parent', 'src/crates/assembly/macro-parent/Cargo.toml', [
      pathDependency('src/crates/assembly/shared', {
        name: 'shared',
        usesDefaultFeatures: false,
        features: ['activate'],
      }),
    ]),
    targets: [{ kind: ['proc-macro'] }],
  };
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/normal-parent', { name: 'normal-parent' }),
    pathDependency('src/crates/assembly/macro-parent', { name: 'macro-parent' }),
  ]);

  assert.deepEqual(
    findProductEntrypointCoreFeatureViolations(
      [cli, normalParent, macroParent, shared, core],
      { root: TEST_ROOT, crateLayoutRules },
    ),
    [],
  );
});

test('CLI dependency architecture closure cannot hide features behind target cfgs', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { 'product-full': [] },
  };
  const bridge = {
    ...packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        optional: true,
        usesDefaultFeatures: false,
      }),
    ]),
    features: {
      forward: ['bitfun-core?/product-full'],
      activate: ['dep:bitfun-core'],
    },
  };
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      target: 'cfg(windows)',
      usesDefaultFeatures: false,
      features: ['forward'],
    }),
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      target: 'cfg(unix)',
      usesDefaultFeatures: false,
      features: ['activate'],
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/product-full/);
});

test('CLI dependency architecture closure unions unconditional and target-specific declarations', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { 'product-full': [] },
  };
  const bridge = {
    ...packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        optional: true,
        usesDefaultFeatures: false,
      }),
    ]),
    features: {
      forward: ['bitfun-core?/product-full'],
      activate: ['dep:bitfun-core'],
    },
  };
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      usesDefaultFeatures: false,
      features: ['forward'],
    }),
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      target: 'cfg(windows)',
      usesDefaultFeatures: false,
      features: ['activate'],
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/product-full/);
});

function reviewedCoreFeaturesFor(rootName) {
  return rootName === 'bitfun-cli'
    ? CLI_REVIEWED_CORE_FEATURES
    : ACP_REVIEWED_CORE_FEATURES;
}

function targetedWeakForwardingGraph(rootName, forwardTarget, activateTarget, reverse = false) {
  const reviewedFeatures = reviewedCoreFeaturesFor(rootName);
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: Object.fromEntries([
      ...reviewedFeatures.map((feature) => [feature, []]),
      ['product-full', []],
    ]),
  };
  const bridge = {
    ...packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        optional: true,
        usesDefaultFeatures: false,
      }),
    ]),
    features: {
      forward: ['bitfun-core?/product-full'],
      activate: ['dep:bitfun-core'],
    },
  };
  const root = packageAt(rootName, rootName === 'bitfun-cli'
    ? 'src/apps/cli/Cargo.toml'
    : 'src/crates/interfaces/acp/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: reviewedFeatures,
    }),
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      target: forwardTarget,
      usesDefaultFeatures: false,
      features: [reverse ? 'activate' : 'forward'],
    }),
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      target: activateTarget,
      usesDefaultFeatures: false,
      features: [reverse ? 'forward' : 'activate'],
    }),
  ]);

  return { root, bridge, core };
}

test('CLI dependency architecture closure ignores Windows target spelling differences', () => {
  for (const reverse of [false, true]) {
    const { root, bridge, core } = targetedWeakForwardingGraph(
      'bitfun-cli',
      'cfg(windows)',
      'cfg(target_os = "windows")',
      reverse,
    );
    const violations = findProductEntrypointCoreFeatureViolations(
      [root, bridge, core],
      { root: TEST_ROOT, crateLayoutRules },
    );

    assert.equal(violations.length, 1);
    assert.match(violations[0].message, /bitfun-core\/product-full/);
  }
});

test('CLI dependency architecture closure includes Unix and not-Windows declarations', () => {
  for (const reverse of [false, true]) {
    const { root, bridge, core } = targetedWeakForwardingGraph(
      'bitfun-cli',
      'cfg(not(windows))',
      'cfg(unix)',
      reverse,
    );
    const violations = findProductEntrypointCoreFeatureViolations(
      [root, bridge, core],
      { root: TEST_ROOT, crateLayoutRules },
    );

    assert.equal(violations.length, 1);
    assert.match(violations[0].message, /bitfun-core\/product-full/);
  }
});

test('CLI dependency architecture closure includes nested target-specific declarations', () => {
  const reviewedFeatures = reviewedCoreFeaturesFor('bitfun-cli');
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: Object.fromEntries([
      ...reviewedFeatures.map((feature) => [feature, []]),
      ['product-full', []],
    ]),
  };
  const bridge = packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      target: 'cfg(unix)',
      usesDefaultFeatures: false,
      features: ['product-full'],
    }),
  ]);
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: reviewedFeatures,
    }),
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      target: 'cfg(windows)',
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/product-full/);
});

test('CLI dependency architecture closure includes target-specific build dependencies', () => {
  const reviewedFeatures = reviewedCoreFeaturesFor('bitfun-cli');
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: Object.fromEntries([
      ...reviewedFeatures.map((feature) => [feature, []]),
      ['product-full', []],
    ]),
  };
  const bridge = packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      kind: 'build',
      target: 'cfg(unix)',
      usesDefaultFeatures: false,
      features: ['product-full'],
    }),
  ]);
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: reviewedFeatures,
    }),
    pathDependency('src/crates/assembly/bridge', {
      name: 'bridge',
      target: 'cfg(windows)',
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/product-full/);
});

test('CLI dependency closure inspects non-default root features', () => {
  const reviewedFeatures = reviewedCoreFeaturesFor('bitfun-cli');
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: Object.fromEntries([
      ...reviewedFeatures.map((feature) => [feature, []]),
      ['product-full', []],
    ]),
  };
  const bridge = packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ['product-full'],
    }),
  ]);
  const cli = {
    ...packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
      pathDependency('src/crates/assembly/core', {
        name: 'bitfun-core',
        usesDefaultFeatures: false,
        features: reviewedFeatures,
      }),
      pathDependency('src/crates/assembly/bridge', {
        name: 'bridge',
        optional: true,
      }),
    ]),
    features: {
      default: [],
      bad: ['dep:bridge'],
    },
  };

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/product-full/);
});

test('ACP dependency closure rejects indirect unreviewed Core features', () => {
  const reviewedFeatures = reviewedCoreFeaturesFor('bitfun-acp');
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: Object.fromEntries([
      ...reviewedFeatures.map((feature) => [feature, []]),
      ['product-full', []],
    ]),
  };
  const bridge = packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ['product-full'],
    }),
  ]);
  const acp = packageAt('bitfun-acp', 'src/crates/interfaces/acp/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: reviewedFeatures,
    }),
    pathDependency('src/crates/assembly/bridge', { name: 'bridge' }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [acp, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/product-full/);
});

test('ACP active closure cannot be expanded by a reviewed owner definition', () => {
  const reviewedFeatures = reviewedCoreFeaturesFor('bitfun-acp');
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: {
      ...Object.fromEntries(reviewedFeatures.map((feature) => [feature, []])),
      'tools-canvas': ['plugin-runtime'],
      'plugin-runtime': [],
    },
  };
  const acp = packageAt('bitfun-acp', 'src/crates/interfaces/acp/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: reviewedFeatures,
    }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [acp, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/plugin-runtime/);
});

test('CLI dependency closure includes build dependencies and excluded capabilities', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: {
      'cli-everything': ['announcement', 'debug-log'],
      announcement: [],
      'debug-log': [],
    },
  };
  const bridge = packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      kind: 'build',
      usesDefaultFeatures: false,
      features: ['cli-everything'],
    }),
  ]);
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', { name: 'bridge' }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/announcement/);
});

test('CLI dependency closure excludes the Core dispatch store', () => {
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml'),
    features: { 'dispatch-store': [] },
  };
  const bridge = packageAt('bridge', 'src/crates/assembly/bridge/Cargo.toml', [
    pathDependency('src/crates/assembly/core', {
      name: 'bitfun-core',
      usesDefaultFeatures: false,
      features: ['dispatch-store'],
    }),
  ]);
  const cli = packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [
    pathDependency('src/crates/assembly/bridge', { name: 'bridge' }),
  ]);

  const violations = findProductEntrypointCoreFeatureViolations(
    [cli, bridge, core],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /bitfun-core\/dispatch-store/);
});

test('cargo layer checker rejects reverse edges across dependency kinds', () => {
  const packages = [
    packageAt('entry', 'src/apps/example/Cargo.toml'),
    packageAt('adapter', 'src/crates/adapters/transport/Cargo.toml'),
    packageAt('assembly', 'src/crates/assembly/core/Cargo.toml', [
      pathDependency('src/apps/example', { optional: true }),
    ]),
    packageAt('service', 'src/crates/services/services-core/Cargo.toml', [
      pathDependency('src/crates/adapters/transport'),
      pathDependency('src/crates/assembly/core', {
        kind: 'dev',
        target: 'cfg(windows)',
      }),
    ]),
    packageAt('runtime', 'src/crates/execution/agent-runtime/Cargo.toml', [
      pathDependency('src/crates/adapters/transport'),
      pathDependency('src/crates/services/services-core'),
    ]),
    packageAt('contract', 'src/crates/contracts/core-types/Cargo.toml', [
      pathDependency('src/crates/services/services-core', { kind: 'build' }),
    ]),
  ];

  const violations = findCargoLayerViolations(packages, {
    root: TEST_ROOT,
    crateLayoutRules,
  });

  assert.equal(violations.length, 6);
  assert.match(violations[0].message, /assembly.*->.*entry.*apps.*normal optional dependency/);
  assert.match(violations[1].message, /service.*services.*->.*adapter.*adapters.*normal dependency/);
  assert.match(violations[2].message, /service.*services.*->.*assembly.*dev dependency.*cfg\(windows\)/);
  assert.match(violations[3].message, /runtime.*execution.*->.*adapter.*adapters.*normal dependency/);
  assert.match(violations[4].message, /runtime.*execution.*->.*service.*services.*normal dependency/);
  assert.match(violations[5].message, /contract.*contracts.*->.*service.*services.*build dependency/);
});

test('workspace Tokio capabilities stay crate-owned', async () => {
  const repositoryRoot = fileURLToPath(new URL('..', import.meta.url));
  const workspaceManifest = await readFile(new URL('../Cargo.toml', import.meta.url), 'utf8');
  const workspaceTokio = workspaceManifest.match(/^tokio\s*=\s*\{[^}]+\}/m)?.[0];

  assert.ok(workspaceTokio, 'workspace dependencies must declare Tokio once');
  assert.match(workspaceTokio, /default-features\s*=\s*false/);
  assert.doesNotMatch(workspaceTokio, /(?:^|,\s*)features\s*=/);
  const packages = collectCargoMetadataPackages({ root: repositoryRoot });
  assert.deepEqual(findTokioDependencyFeatureViolations(packages), []);
});

test('services integrations Tokio owner contracts reject feature-union masking', async () => {
  const manifest = await readFile(
    new URL('../src/crates/services/services-integrations/Cargo.toml', import.meta.url),
    'utf8',
  );
  const mutations = [
    ['plugin-source', 'tokio/time', /plugin-source missing effective Tokio capabilities: time/],
    ['mcp', 'tokio/process', /mcp missing effective Tokio capabilities: process/],
    ['miniapp-market', 'miniapp-runtime', /miniapp-market missing effective Tokio capabilities: fs/],
    ['function-agents', 'git', /function-agents missing effective Tokio capabilities: fs/],
    ['remote-ssh-concrete', 'remote-ssh', /remote-ssh-concrete missing effective Tokio capabilities: fs/],
  ];

  for (const [feature, value, expected] of mutations) {
    const mutated = removeFeatureValue(manifest, feature, value);
    assert.notEqual(mutated, manifest, `${feature} must own ${value} in the fixture`);
    const messages = findServicesIntegrationsTokioFeatureViolations(
      servicesIntegrationsPackage(mutated),
    ).map((violation) => violation.message).join('\n');
    assert.match(messages, expected);
  }
});

test('services integrations Reqwest policy uses Cargo-decoded feature references', () => {
  const pkg = servicesIntegrationsPackage(`
[features]
reqwest = ["dep:reqwest"]
announcement = ["reqwest", "reqwest/rustls"]
file-watch = ["reqwest?/__native-tls"]
mcp = ["reqwest"]
models-dev = ["reqwest", "reqwest/rustls", "reqwest/system-proxy"]
speech = ["reqwest", "reqwest/rustls", "reqwest/http3"]
`);

  const messages = findServicesIntegrationsReqwestFeatureViolations(pkg)
    .map((violation) => violation.message)
    .join('\n');
  assert.match(messages, /file-watch.*outside its reviewed owner features/);
  assert.match(messages, /mcp.*missing reqwest\/rustls/);
  assert.doesNotMatch(messages, /models-dev.*system-proxy/);
  assert.match(messages, /speech.*unreviewed Reqwest feature reference reqwest\/http3/);
});

test('direct Reqwest clients reject extra decoded dependency and package features', () => {
  const pkg = {
    ...packageAt('bitfun-cli', 'src/apps/cli/Cargo.toml', [{
      name: 'reqwest',
      kind: null,
      optional: false,
      uses_default_features: false,
      features: [
        'http2',
        'json',
        'stream',
        'multipart',
        'query',
        'form',
        'rustls',
        '__native-tls',
      ],
    }]),
    features: { default: ['reqwest?/http3'] },
  };

  const messages = findReqwestDependencyFeatureViolations([pkg])
    .map((violation) => violation.message)
    .join('\n');
  assert.match(messages, /bitfun-cli.*unexpected dependency features: __native-tls/);
  assert.match(messages, /bitfun-cli:default.*unreviewed Reqwest feature reference reqwest\?\/http3/);

  const installerMessages = findReqwestDependencyFeatureViolations([{
    ...pkg,
    name: 'bitfun-installer',
    manifest_path: join(TEST_ROOT, 'BitFun-Installer', 'src-tauri', 'Cargo.toml'),
  }]).map((violation) => violation.message).join('\n');
  assert.match(installerMessages, /bitfun-installer.*missing a reviewed owner profile/);
});

test('AI adapters Reqwest profile owns the supported SOCKS transport', () => {
  const baseFeatures = ['http2', 'json', 'stream', 'multipart', 'query', 'form'];
  const valid = packageAt('bitfun-ai-adapters', 'src/crates/adapters/ai-adapters/Cargo.toml', [{
    name: 'reqwest',
    kind: null,
    optional: false,
    uses_default_features: false,
    features: [...baseFeatures, 'rustls', 'socks'],
  }]);
  const missingSocks = packageAt(
    'bitfun-ai-adapters',
    'src/crates/adapters/ai-adapters/Cargo.toml',
    [{
      name: 'reqwest',
      kind: null,
      optional: false,
      uses_default_features: false,
      features: [...baseFeatures, 'rustls'],
    }],
  );

  assert.deepEqual(findReqwestDependencyFeatureViolations([valid]), []);
  const messages = findReqwestDependencyFeatureViolations([missingSocks])
    .map((violation) => violation.message)
    .join('\n');
  assert.match(messages, /bitfun-ai-adapters.*missing features: socks/);
});

test('Reqwest metadata policy covers URL-only and future dependency owners', () => {
  const baseFeatures = ['http2', 'json', 'stream', 'multipart', 'query', 'form'];
  const core = {
    ...packageAt('bitfun-core', 'src/crates/assembly/core/Cargo.toml', [{
      name: 'reqwest',
      kind: null,
      optional: true,
      uses_default_features: false,
      features: baseFeatures,
    }]),
    features: { product: ['dep:reqwest', 'reqwest/__native-tls'] },
  };
  const future = packageAt('future-client', 'src/crates/services/future-client/Cargo.toml', [{
    name: 'reqwest',
    kind: null,
    optional: false,
    uses_default_features: false,
    features: [...baseFeatures, 'rustls'],
  }]);
  const duplicate = packageAt(
    'bitfun-services-integrations',
    'src/crates/services/services-integrations/Cargo.toml',
    [
      {
        name: 'reqwest',
        kind: null,
        optional: true,
        uses_default_features: false,
        features: baseFeatures,
      },
      {
        name: 'reqwest',
        rename: 'windows_reqwest',
        kind: null,
        optional: true,
        target: 'cfg(windows)',
        uses_default_features: false,
        features: [...baseFeatures, '__native-tls'],
      },
    ],
  );

  const messages = findReqwestDependencyFeatureViolations([core, future, duplicate])
    .map((violation) => violation.message)
    .join('\n');
  assert.match(messages, /bitfun-core:product.*reqwest\/__native-tls/);
  assert.match(messages, /future-client.*missing a reviewed owner profile/);
  assert.match(messages, /bitfun-services-integrations.*exactly one normal Reqwest dependency/);
});

test('resolved Reqwest feature union rejects every native TLS backend alias', () => {
  const violations = findResolvedReqwestNativeTlsViolations(
    [
      {
        name: 'reqwest',
        version: '0.13.4',
        features: ['rustls', 'rustls-no-provider', '__native-tls', 'native-tls-vendored-no-alpn'],
      },
      {
        name: 'reqwest',
        version: '0.12.28',
        features: ['rustls-tls', 'default-tls'],
      },
    ],
    { root: TEST_ROOT },
  );

  assert.equal(violations.length, 2);
  const messages = violations.map((violation) => violation.message).join('\n');
  assert.match(messages, /__native-tls, native-tls-vendored-no-alpn/);
  assert.match(messages, /reqwest 0\.12\.28.*default-tls/);
});

test('Cargo metadata Tokio policy catches table-style and renamed full dependencies', () => {
  const pkg = packageAt('table-style', 'src/crates/services/table-style/Cargo.toml', [{
    name: 'tokio',
    rename: 'async_runtime',
    kind: null,
    optional: false,
    features: ['full'],
  }]);
  const violations = findTokioDependencyFeatureViolations([pkg]);

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /table-style must not enable tokio\/full/);

  const installerViolations = findTokioDependencyFeatureViolations([{
    ...pkg,
    name: 'bitfun-installer',
    manifest_path: join(TEST_ROOT, 'BitFun-Installer', 'src-tauri', 'Cargo.toml'),
  }]);
  assert.equal(installerViolations.length, 1);
  assert.match(installerViolations[0].message, /bitfun-installer must not enable tokio\/full/);
});

test('cargo layer checker allows documented downward and peer dependencies', () => {
  const packages = [
    packageAt('entry', 'src/apps/example/Cargo.toml', [
      pathDependency('src/crates/interfaces/acp'),
      pathDependency('src/crates/assembly/core'),
    ]),
    packageAt('interface', 'src/crates/interfaces/acp/Cargo.toml', [
      pathDependency('src/crates/assembly/core'),
    ]),
    packageAt('assembly', 'src/crates/assembly/core/Cargo.toml', [
      pathDependency('src/crates/services/services-core'),
      pathDependency('src/crates/execution/agent-runtime'),
    ]),
    packageAt('service', 'src/crates/services/services-core/Cargo.toml', [
      pathDependency('src/crates/execution/agent-runtime'),
      pathDependency('src/crates/contracts/core-types'),
    ]),
    packageAt('runtime', 'src/crates/execution/agent-runtime/Cargo.toml', [
      pathDependency('src/crates/contracts/core-types'),
    ]),
    packageAt('contract', 'src/crates/contracts/core-types/Cargo.toml'),
  ];

  assert.deepEqual(
    findCargoLayerViolations(packages, {
      root: TEST_ROOT,
      crateLayoutRules,
    }),
    [],
  );
});

test('cargo layer checker uses resolved edges for locally patched dependencies', () => {
  const entry = packageAt('entry', 'src/apps/example/Cargo.toml');
  const assembly = packageAt('assembly', 'src/crates/assembly/core/Cargo.toml', [
    { name: 'entry', path: null, kind: null, optional: false, target: null },
  ]);

  const violations = findCargoLayerViolations(
    [entry, assembly],
    { root: TEST_ROOT, crateLayoutRules },
    [{
      sourceManifestPath: assembly.manifest_path,
      targetManifestPath: entry.manifest_path,
      name: 'entry',
      kind: null,
      optional: false,
      target: null,
    }],
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /assembly.*->.*entry.*apps.*normal dependency/);
});

test('cargo layer checker combines declared path dependencies with resolved edges', () => {
  const entry = packageAt('entry', 'src/apps/example/Cargo.toml');
  const assembly = packageAt('assembly', 'src/crates/assembly/core/Cargo.toml', [
    pathDependency('src/apps/example', { optional: true }),
  ]);

  const violations = findCargoLayerViolations(
    [entry, assembly],
    { root: TEST_ROOT, crateLayoutRules },
    [],
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /assembly.*->.*entry.*apps.*normal optional dependency/);
});

test('cargo layer checker deduplicates renamed declared and resolved edges', () => {
  const entry = packageAt('entry', 'src/apps/example/Cargo.toml');
  const assembly = packageAt('assembly', 'src/crates/assembly/core/Cargo.toml', [{
    ...pathDependency('src/apps/example', { name: 'entry', optional: true }),
    rename: 'legacy_entry',
  }]);

  const violations = findCargoLayerViolations(
    [entry, assembly],
    { root: TEST_ROOT, crateLayoutRules },
    [{
      sourceManifestPath: assembly.manifest_path,
      targetManifestPath: entry.manifest_path,
      name: 'legacy_entry',
      kind: null,
      optional: true,
      target: null,
    }],
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /assembly.*->.*entry.*apps.*normal optional dependency/);
});

test('cargo layer checker rejects repository packages without a known layer', () => {
  const violations = findCargoLayerViolations(
    [packageAt('mystery', 'tools/mystery/Cargo.toml')],
    { root: TEST_ROOT, crateLayoutRules },
  );

  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /unknown crate layer.*tools\/mystery\/Cargo\.toml/);
});

test('cargo metadata collection scans standalone manifests not covered by the workspace', () => {
  const workspaceManifest = join(TEST_ROOT, 'Cargo.toml');
  const memberManifest = join(TEST_ROOT, 'src', 'apps', 'example', 'Cargo.toml');
  const installerManifest = join(TEST_ROOT, 'BitFun-Installer', 'src-tauri', 'Cargo.toml');
  const calls = [];

  const packages = collectCargoMetadataPackages({
    root: TEST_ROOT,
    manifestPaths: [workspaceManifest, memberManifest, installerManifest],
    loadMetadata(manifestPath, options) {
      calls.push([manifestPath, options]);
      if (manifestPath === workspaceManifest) {
        const entry = packageAt('entry', 'src/apps/example/Cargo.toml');
        return { packages: [entry], workspace_members: [entry.id] };
      }
      if (manifestPath === installerManifest) {
        return { packages: [packageAt('installer', 'BitFun-Installer/src-tauri/Cargo.toml')] };
      }
      throw new Error(`workspace member metadata should not be loaded twice: ${manifestPath}`);
    },
  });

  assert.deepEqual(calls, [
    [workspaceManifest, { noDeps: false }],
    [installerManifest, { noDeps: true }],
  ]);
  assert.deepEqual(packages.map((pkg) => pkg.name), ['entry', 'installer']);
});

test('cargo metadata collection rescans standalone packages discovered by the workspace', () => {
  const workspaceManifest = join(TEST_ROOT, 'Cargo.toml');
  const serviceManifest = join(TEST_ROOT, 'src', 'crates', 'services', 'services-core', 'Cargo.toml');
  const assembly = packageAt('assembly', 'src/crates/assembly/core/Cargo.toml', [
    pathDependency('src/crates/services/services-core'),
  ]);
  const service = packageAt('service', 'src/crates/services/services-core/Cargo.toml', [
    pathDependency('src/apps/example', { optional: true }),
  ]);
  const entry = packageAt('example', 'src/apps/example/Cargo.toml');
  const calls = [];

  const graph = collectCargoMetadataGraph({
    root: TEST_ROOT,
    manifestPaths: [workspaceManifest, serviceManifest],
    loadMetadata(manifestPath, options) {
      calls.push([manifestPath, options]);
      if (manifestPath === workspaceManifest) {
        return {
          packages: [assembly, service, entry],
          workspace_members: [assembly.id],
          resolve: {
            nodes: [{
              id: assembly.id,
              deps: [{
                name: 'service',
                pkg: service.id,
                dep_kinds: [{ kind: null, target: null }],
              }],
            }],
          },
        };
      }
      return {
        packages: [service, entry],
        workspace_members: [service.id],
        resolve: {
          nodes: [{
            id: service.id,
            deps: [{
              name: 'example',
              pkg: entry.id,
              dep_kinds: [{ kind: null, target: null }],
            }],
          }],
        },
      };
    },
  });

  const violations = findCargoLayerViolations(
    graph.packages,
    { root: TEST_ROOT, crateLayoutRules },
    graph.resolvedDependencies,
  );

  assert.deepEqual(calls, [
    [workspaceManifest, { noDeps: false }],
    [serviceManifest, { noDeps: true }],
  ]);
  assert.equal(violations.length, 1);
  assert.match(violations[0].message, /service.*services.*->.*example.*apps.*normal optional dependency/);
});

test('cargo metadata collection preserves resolved repository edges', () => {
  const workspaceManifest = join(TEST_ROOT, 'Cargo.toml');
  const assembly = packageAt('assembly', 'src/crates/assembly/core/Cargo.toml', [{
    name: 'entry',
    rename: null,
    path: null,
    kind: 'dev',
    optional: true,
    target: 'cfg(windows)',
  }]);
  const entry = packageAt('entry', 'src/apps/example/Cargo.toml');

  const graph = collectCargoMetadataGraph({
    root: TEST_ROOT,
    manifestPaths: [workspaceManifest],
    loadMetadata() {
      return {
        packages: [assembly, entry],
        resolve: {
          nodes: [{
            id: assembly.id,
            deps: [{
              name: 'entry',
              pkg: entry.id,
              dep_kinds: [{ kind: 'dev', target: 'cfg(windows)' }],
            }],
          }],
        },
      };
    },
  });

  assert.deepEqual(graph.packages.map((pkg) => pkg.name), ['assembly', 'entry']);
  assert.equal(graph.resolvedDependencies.length, 1);
  assert.equal(graph.resolvedDependencies[0].sourceManifestPath, assembly.manifest_path);
  assert.equal(graph.resolvedDependencies[0].targetManifestPath, entry.manifest_path);
  assert.equal(graph.resolvedDependencies[0].kind, 'dev');
  assert.equal(graph.resolvedDependencies[0].optional, true);
  assert.equal(graph.resolvedDependencies[0].target, 'cfg(windows)');
});

test('core boundary check is split into focused modules', async () => {
  const entrypoint = await readFile(ENTRYPOINT, 'utf8');
  assert.ok(
    entrypoint.split(/\r?\n/).length <= 20,
    'entrypoint should stay a thin wrapper around core-boundaries modules',
  );
  assert.match(entrypoint, /core-boundaries\/checker\.mjs/);

  for (const modulePath of MODULES) {
    await access(new URL(modulePath, import.meta.url));
  }

  const checker = await readFile(new URL('./core-boundaries/checker.mjs', import.meta.url), 'utf8');
  assert.ok(
    checker.split(/\r?\n/).length <= 1200,
    'checker should stay focused on orchestration and shared check helpers',
  );

  const sourceRuleEntry = await readFile(
    new URL('./core-boundaries/rules/source-rules.mjs', import.meta.url),
    'utf8',
  );
  assert.ok(
    sourceRuleEntry.split(/\r?\n/).length <= 40,
    'source rule entrypoint should delegate to focused source-rule modules',
  );
});

test('core checker runs the unified cargo dependency boundary check', async () => {
  const checker = await readFile(
    new URL('./core-boundaries/checker.mjs', import.meta.url),
    'utf8',
  );

  assert.match(checker, /checkCargoDependencyBoundariesSafely/);
  assert.doesNotMatch(checker, /checkCargoDependencyLayersSafely/);
});

test('product entrypoint feature policy does not pin product-full per manifest', async () => {
  const [checker, featureRules] = await Promise.all([
    readFile(new URL('./core-boundaries/checker.mjs', import.meta.url), 'utf8'),
    readFile(
      new URL('./core-boundaries/rules/feature-rules.mjs', import.meta.url),
      'utf8',
    ),
  ]);

  assert.doesNotMatch(checker, /checkProductCoreFeatureAssembly/);
  assert.doesNotMatch(featureRules, /productCoreFeatureAssemblyRules/);
});

test('Rust build dependency boundary policy stays discoverable', async () => {
  const policyUrl = new URL(
    '../docs/architecture/rust-build-dependency-boundaries.md',
    import.meta.url,
  );
  await assert.doesNotReject(
    () => access(policyUrl),
    'the Rust build dependency boundary policy must exist',
  );
  const [agents, productArchitecture, policy] = await Promise.all([
    readFile(new URL('../AGENTS.md', import.meta.url), 'utf8'),
    readFile(new URL('../docs/architecture/product-architecture.md', import.meta.url), 'utf8'),
    readFile(policyUrl, 'utf8'),
  ]);

  assert.match(agents, /docs\/architecture\/rust-build-dependency-boundaries\.md/);
  assert.match(productArchitecture, /rust-build-dependency-boundaries\.md/);
  assert.match(policy, /Cargo feature/);
  assert.match(policy, /Delivery Profile/);
  assert.match(policy, /Runtime Config/);
  assert.match(policy, /Capability Availability/);
  assert.match(policy, /pnpm run check:core-boundaries:test/);
});

test('transport contract stays limited to current delivery needs', async () => {
  const [workspaceManifest, transportTrait] = await Promise.all([
      readFile(new URL('../Cargo.toml', import.meta.url), 'utf8'),
      readFile(
        new URL('../src/crates/adapters/transport/src/traits.rs', import.meta.url),
        'utf8',
      ),
    ]);

  assert.doesNotMatch(workspaceManifest, /src\/crates\/adapters\/api-layer/);
  assert.doesNotMatch(
    transportTrait,
    /\b(?:emit_text_chunk|emit_tool_event|emit_stream_start|emit_stream_end|adapter_type|TextChunk|ToolEventPayload|ToolEventType|StreamEvent)\b/,
  );
  assert.doesNotMatch(
    transportTrait,
    /emit_event\s*\(\s*&self,\s*session_id:\s*&str/,
  );
});

test('public event projection stays limited to current host needs', async () => {
  const frontendProjection = await readFile(
    new URL(
      '../src/crates/contracts/events/src/frontend_projection.rs',
      import.meta.url,
    ),
    'utf8',
  );

  assert.doesNotMatch(
    frontendProjection,
    /\b(?:into_)?legacy_flat_message\b|\bpub event_type\b/,
  );
});

test('embedded relay concrete lifecycle stays desktop-owned', async () => {
  const [coreManifest, corePort, desktopManifest, desktopHost] = await Promise.all([
    readFile(new URL('../src/crates/assembly/core/Cargo.toml', import.meta.url), 'utf8'),
    readFile(
      new URL(
        '../src/crates/assembly/core/src/service/remote_connect/embedded_relay_host.rs',
        import.meta.url,
      ),
      'utf8',
    ),
    readFile(new URL('../src/apps/desktop/Cargo.toml', import.meta.url), 'utf8'),
    readFile(
      new URL('../src/apps/desktop/src/embedded_relay_host.rs', import.meta.url),
      'utf8',
    ),
  ]);

  assert.doesNotMatch(coreManifest, /bitfun-relay-service/);
  assert.doesNotMatch(corePort, /\b(?:axum|TcpListener|ServeDir|build_relay_router)\b/);
  assert.match(desktopManifest, /bitfun-relay-service/);
  assert.match(desktopHost, /impl EmbeddedRelayHost for DesktopEmbeddedRelayHost/);
  assert.match(desktopHost, /TcpListener::bind/);
  assert.match(desktopHost, /ServeDir::new/);
});

test('desktop preview rebuild inputs use the current crate layout', async () => {
  const devScript = await readFile(new URL('./dev.cjs', import.meta.url), 'utf8');

  assert.match(
    devScript,
    /path\.join\(ROOT_DIR, 'src', 'crates'\)/,
  );
  assert.doesNotMatch(
    devScript,
    /'src', 'crates', '(?:core|transport|events|ai-adapters|webdriver|api-layer|assembly|adapters|contracts|execution|interfaces|services)'/,
  );
});

test('split core boundary check keeps self-test and default execution behavior', () => {
  const selfTest = spawnSync(
    process.execPath,
    ['scripts/check-core-boundaries.mjs'],
    {
      cwd: new URL('..', import.meta.url),
      env: { ...process.env, BITFUN_BOUNDARY_CHECK_SELF_TEST: '1' },
      encoding: 'utf8',
    },
  );
  assert.equal(selfTest.status, 0, selfTest.stderr || selfTest.stdout);
  assert.match(selfTest.stdout, /Core boundary check self-test passed\./);

  const defaultRun = spawnSync(process.execPath, ['scripts/check-core-boundaries.mjs'], {
    cwd: new URL('..', import.meta.url),
    encoding: 'utf8',
  });
  assert.equal(defaultRun.status, 0, defaultRun.stderr || defaultRun.stdout);
  assert.match(defaultRun.stdout, /Core boundary check passed\./);
});

test('optional dependency ownership rejects undeclared direct feature owners', async () => {
  const { unexpectedDependencyOwnerFeatures } = await import(
    './core-boundaries/manifest-feature-helpers.mjs'
  );
  const features = new Map([
    ['declared', { refs: ['dep:example'], line: 1 }],
    ['missing', { refs: ['example'], line: 2 }],
    ['feature-ref', { refs: ['example/subfeature'], line: 3 }],
    ['weak-ref', { refs: ['example?/subfeature'], line: 4 }],
    ['unrelated', { refs: ['other'], line: 5 }],
  ]);

  assert.deepEqual(
    unexpectedDependencyOwnerFeatures(features, {
      depName: 'example',
      ownerFeatures: ['declared'],
    }).map(([featureName]) => featureName),
    ['missing', 'feature-ref'],
  );
});

test('services-core capability profiles keep heavy owners out of the empty profile', async () => {
  const { coreClosedFeatureProfileRules } = await import(
    './core-boundaries/rules/feature-rules.mjs'
  );
  const { dependencyProfileRules } = await import(
    './core-boundaries/rules/crate-rules.mjs'
  );
  const { requiredContentRules } = await import(
    './core-boundaries/rules/source/required-rules.mjs'
  );
  const serviceManifest = 'src/crates/services/services-core/Cargo.toml';
  const profiles = new Map(
    coreClosedFeatureProfileRules
      .filter((rule) => rule.manifestPath === serviceManifest)
      .map((rule) => [rule.featureName, rule.requiredFeatureRefs]),
  );

  assert.deepEqual(profiles.get('filesystem'), [
    'dep:base64',
    'dep:chrono',
    'dep:ignore',
    'dep:sha2',
    'tokio/fs',
  ]);
  assert.deepEqual(profiles.get('json-io'), [
    'dep:fs2',
    'dep:windows',
    'tokio/fs',
    'tokio/sync',
    'windows/Win32_Foundation',
    'windows/Win32_Storage_FileSystem',
  ]);
  assert.deepEqual(profiles.get('local-storage'), [
    'dep:bitfun-core-types',
    'dep:bitfun-events',
    'dep:chrono',
    'dep:fs2',
    'dep:libc',
    'dep:sha2',
    'dep:windows',
    'tokio/fs',
    'tokio/sync',
    'windows/Win32_Foundation',
    'windows/Win32_Storage_FileSystem',
  ]);
  assert.deepEqual(profiles.get('process-runtime'), [
    'dep:libc',
    'dep:which',
    'dep:win32job',
    'dep:windows',
    'tokio/io-util',
    'tokio/process',
    'windows/Win32_Foundation',
    'windows/Win32_System_Diagnostics_ToolHelp',
    'windows/Win32_System_Threading',
  ]);
  assert.deepEqual(profiles.get('workspace-instructions'), [
    'dep:globset',
    'dep:serde_yaml',
    'tokio/fs',
    'tokio/io-util',
  ]);
  assert.deepEqual(profiles.get('lsp'), [
    'dep:anyhow',
    'dep:bitfun-core-types',
    'dep:notify',
    'dep:zip',
    'process-runtime',
    'tokio/fs',
    'tokio/io-util',
    'tokio/sync',
  ]);
  assert.deepEqual(profiles.get('workspace-runtime'), [
    'dep:anyhow',
    'dep:async-trait',
    'dep:bitfun-runtime-ports',
    'dep:dunce',
    'process-runtime',
    'tokio/fs',
    'tokio/io-util',
    'tokio/sync',
  ]);

  const defaultProfile = dependencyProfileRules.find(
    (rule) => rule.crateName === 'services-core',
  );
  for (const dependency of [
    'base64',
    'bitfun-core-types',
    'bitfun-events',
    'chrono',
    'fs2',
    'globset',
    'ignore',
    'libc',
    'sha2',
    'which',
    'win32job',
    'windows',
  ]) {
    assert.ok(
      defaultProfile?.forbiddenNonOptionalDeps.includes(dependency),
      `services-core empty profile must reject ambient ${dependency}`,
    );
  }

  const sourceRule = requiredContentRules.find(
    (rule) => rule.path === 'src/crates/services/services-core/src/lib.rs',
  );
  const sourceContracts = sourceRule?.patterns.map((pattern) => pattern.regex.source).join('\n') ?? '';
  for (const moduleName of [
    'filesystem',
    'json_store',
    'managed_runtime',
    'persistence',
    'process_manager',
    'process_tree',
    'session',
    'session_usage',
    'storage_cleanup',
    'system',
    'token_usage',
    'workspace_instructions',
  ]) {
    assert.match(
      sourceContracts,
      new RegExp(`pub mod ${moduleName}`),
      `services-core source rule must protect the ${moduleName} capability gate`,
    );
  }
});

test('services-core Tokio capabilities stay owner-scoped', () => {
  const invalidPackage = {
    name: 'bitfun-services-core',
    manifest_path: 'src/crates/services/services-core/Cargo.toml',
    dependencies: [
      {
        name: 'tokio',
        kind: null,
        optional: false,
        features: ['fs', 'io-util', 'process', 'rt', 'sync', 'time'],
      },
    ],
    features: {
      filesystem: [],
      'json-io': [],
      'local-storage': [],
      'process-runtime': [],
      'workspace-instructions': [],
      lsp: [],
      'workspace-runtime': [],
    },
  };

  const messages = findTokioDependencyFeatureViolations([invalidPackage]).map(
    (violation) => violation.message,
  );
  assert.ok(
    messages.some((message) => message.includes('unexpected base Tokio capabilities')),
    'services-core must reject ambient fs/io/process/sync Tokio capabilities',
  );
  assert.ok(
    messages.some((message) => message.includes('filesystem missing effective Tokio capabilities: fs')),
    'services-core must require filesystem to own tokio/fs',
  );
  assert.ok(
    messages.some((message) => message.includes('lsp missing effective Tokio capabilities')),
    'services-core must require lsp to declare its complete effective Tokio profile',
  );
});

test('services-core Windows API capabilities stay feature-owned', async () => {
  const { findServicesCorePlatformDependencyFeatureViolations } = await import(
    './core-boundaries/cargo-dependency-boundaries.mjs'
  );
  assert.equal(
    typeof findServicesCorePlatformDependencyFeatureViolations,
    'function',
    'Cargo boundary checker must expose the services-core platform dependency policy',
  );
  const packageWithAmbientWindowsApis = {
    name: 'bitfun-services-core',
    manifest_path: 'src/crates/services/services-core/Cargo.toml',
    dependencies: [
      {
        name: 'windows',
        kind: null,
        optional: true,
        target: 'cfg(windows)',
        features: ['Win32_Storage_FileSystem', 'Win32_System_Threading'],
      },
    ],
  };

  const violations = findServicesCorePlatformDependencyFeatureViolations([
    packageWithAmbientWindowsApis,
  ]);
  assert.equal(violations.length, 1);
  assert.match(
    violations[0].message,
    /windows API capabilities must be selected by services-core owner features/,
  );

  assert.deepEqual(
    findServicesCorePlatformDependencyFeatureViolations([
      {
        ...packageWithAmbientWindowsApis,
        dependencies: [{ ...packageWithAmbientWindowsApis.dependencies[0], features: [] }],
      },
    ]),
    [],
  );
});

test('closed feature profiles reject product-full hidden behind a child feature', async () => {
  const { unexpectedReachableLocalFeatures } = await import(
    './core-boundaries/manifest-feature-helpers.mjs'
  );
  const features = new Map([
    ['service-integrations', { refs: ['announcement'], line: 1 }],
    [
      'announcement',
      {
        refs: ['bitfun-services-integrations/announcement', 'product-full'],
        line: 2,
      },
    ],
    ['product-full', { refs: ['dep:rmcp'], line: 3 }],
  ]);

  assert.deepEqual(
    unexpectedReachableLocalFeatures(
      features,
      'service-integrations',
      new Set(['announcement']),
    ),
    [
      {
        featureName: 'product-full',
        path: ['service-integrations', 'announcement', 'product-full'],
      },
    ],
  );
});
