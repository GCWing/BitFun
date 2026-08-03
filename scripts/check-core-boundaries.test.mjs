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
  findResolvedReqwestNativeTlsViolations,
  findServicesIntegrationsReqwestFeatureViolations,
  findServicesIntegrationsTokioFeatureViolations,
  findTokioDependencyFeatureViolations,
} from './core-boundaries/cargo-dependency-boundaries.mjs';
import { crateLayoutRules } from './core-boundaries/rules/crate-layout.mjs';

const ENTRYPOINT = new URL('./check-core-boundaries.mjs', import.meta.url);
const MODULES = [
  './core-boundaries/checker.mjs',
  './core-boundaries/cargo-dependency-boundaries.mjs',
  './core-boundaries/explicit-test-topology.mjs',
  './core-boundaries/manifest-feature-helpers.mjs',
  './core-boundaries/self-test.mjs',
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
speech = ["reqwest", "reqwest/rustls", "reqwest/http3"]
`);

  const messages = findServicesIntegrationsReqwestFeatureViolations(pkg)
    .map((violation) => violation.message)
    .join('\n');
  assert.match(messages, /file-watch.*outside its reviewed owner features/);
  assert.match(messages, /mcp.*missing reqwest\/rustls/);
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
