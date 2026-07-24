import { access, readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  collectCargoMetadataGraph,
  collectCargoMetadataPackages,
  findCargoLayerViolations,
} from './core-boundaries/cargo-dependency-boundaries.mjs';
import { crateLayoutRules } from './core-boundaries/rules/crate-layout.mjs';

const ENTRYPOINT = new URL('./check-core-boundaries.mjs', import.meta.url);
const MODULES = [
  './core-boundaries/checker.mjs',
  './core-boundaries/cargo-dependency-boundaries.mjs',
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
  };
}

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
    loadMetadata(manifestPath) {
      calls.push(manifestPath);
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

  assert.deepEqual(calls, [workspaceManifest, installerManifest]);
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
    loadMetadata(manifestPath) {
      calls.push(manifestPath);
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

  assert.deepEqual(calls, [workspaceManifest, serviceManifest]);
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
