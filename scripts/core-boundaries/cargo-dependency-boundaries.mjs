import { readFileSync, readdirSync } from 'node:fs';
import { isAbsolute, join, relative, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const SKIPPED_DIRECTORIES = new Set([
  '.git',
  '.targets',
  '.worktrees',
  'node_modules',
  'target',
]);

const ALLOWED_TARGET_LAYERS = new Map([
  ['apps', new Set(['interfaces', 'assembly', 'adapters', 'services', 'execution', 'contracts'])],
  ['interfaces', new Set(['interfaces', 'assembly', 'adapters', 'services', 'execution', 'contracts'])],
  ['assembly', new Set(['assembly', 'adapters', 'services', 'execution', 'contracts'])],
  ['adapters', new Set(['adapters', 'services', 'execution', 'contracts'])],
  ['services', new Set(['services', 'execution', 'contracts'])],
  ['execution', new Set(['execution', 'contracts'])],
  ['contracts', new Set(['contracts'])],
]);

function normalizedPath(path) {
  const normalized = resolve(path).replace(/\\/g, '/');
  return process.platform === 'win32' ? normalized.toLowerCase() : normalized;
}

function repositoryPath(root, path) {
  const result = relative(resolve(root), resolve(path)).replace(/\\/g, '/');
  if (result === '' || result === '.') {
    return '';
  }
  if (result === '..' || result.startsWith('../') || isAbsolute(result)) {
    return null;
  }
  return result;
}

function layerForManifest(manifestPath, { root, crateLayoutRules }) {
  const repoManifestPath = repositoryPath(root, manifestPath);
  if (repoManifestPath === null) {
    return null;
  }
  const cratePath = repoManifestPath.replace(/\/Cargo\.toml$/, '');

  if (cratePath.startsWith('src/apps/') || cratePath === 'BitFun-Installer/src-tauri') {
    return 'apps';
  }

  return crateLayoutRules.find((rule) => rule.path === cratePath)?.layer ?? null;
}

function dependencyDescription(dependency) {
  const kind = dependency.kind ?? 'normal';
  const optional = dependency.optional ? ' optional' : '';
  const target = dependency.target ? ` for ${dependency.target}` : '';
  return `${kind}${optional} dependency${target}`;
}

const SERVICES_INTEGRATIONS_TOKIO_FEATURES = new Map([
  ['announcement', ['fs', 'sync']],
  ['browser-control', ['time']],
  ['canvas-runtime', ['fs']],
  ['debug-log', ['rt']],
  ['deep-research', ['fs']],
  ['git', ['fs', 'io-util', 'macros', 'rt', 'time']],
  ['file-watch', ['rt', 'sync']],
  ['function-agents', ['fs', 'io-util', 'macros', 'rt', 'time']],
  ['mcp', ['fs', 'io-util', 'net', 'process', 'rt', 'sync', 'time']],
  ['miniapp-runtime', ['fs', 'io-util', 'net', 'process', 'rt', 'sync', 'time']],
  ['miniapp-market', ['fs', 'io-util', 'net', 'process', 'rt', 'sync', 'time']],
  ['plugin-source', ['fs', 'rt', 'sync', 'time']],
  ['hook-import', ['fs', 'sync']],
  ['remote-connect', ['fs', 'io-util', 'net', 'process', 'rt', 'sync', 'time']],
  ['remote-ssh', ['fs', 'io-util', 'macros', 'net', 'process', 'rt', 'sync', 'time']],
  ['remote-ssh-concrete', ['fs', 'io-util', 'macros', 'net', 'process', 'rt', 'sync', 'time']],
  ['review-platform', ['fs', 'io-util', 'sync']],
  ['speech', ['fs', 'io-util', 'macros', 'rt', 'sync']],
  ['workspace-search', ['io-util', 'rt', 'sync', 'time']],
  ['script-tool-runtime', ['io-util', 'process', 'rt', 'sync', 'time']],
]);

const SERVICES_CORE_TOKIO_FEATURES = new Map([
  ['filesystem', ['fs']],
  ['local-storage', ['fs', 'sync']],
  ['process-runtime', ['io-util', 'process']],
  ['workspace-instructions', ['fs', 'io-util']],
  ['lsp', ['fs', 'io-util', 'process', 'sync']],
  ['workspace-runtime', ['fs', 'io-util', 'process', 'sync']],
]);
const SERVICES_CORE_BASE_TOKIO_FEATURES = ['rt', 'time'];

// The installer is an excluded standalone workspace with its own Rust checks
// and packaging lifecycle; this policy governs the root product workspace.
const TOKIO_DEPENDENCY_POLICY_EXCLUDED_PACKAGES = new Set(['bitfun-installer']);

function effectiveTokioCapabilities(feature, featureGraph, visiting = new Set()) {
  if (visiting.has(feature)) {
    return new Set();
  }
  visiting.add(feature);

  const capabilities = new Set();
  for (const value of featureGraph[feature] ?? []) {
    if (value.startsWith('tokio/')) {
      capabilities.add(value.slice('tokio/'.length));
    } else if (Object.hasOwn(featureGraph, value)) {
      for (const capability of effectiveTokioCapabilities(value, featureGraph, visiting)) {
        capabilities.add(capability);
      }
    }
  }

  visiting.delete(feature);
  return capabilities;
}

function findOwnedTokioFeatureViolations(pkg, ownerProfiles) {
  const violations = [];
  const featureGraph = pkg.features ?? {};

  for (const [feature, expectedCapabilities] of ownerProfiles) {
    if (!Object.hasOwn(featureGraph, feature)) {
      violations.push({
        path: pkg.manifest_path,
        line: 1,
        message: `${pkg.name}:${feature} governed Tokio feature is missing`,
      });
      continue;
    }

    const actualCapabilities = [...effectiveTokioCapabilities(feature, featureGraph)].sort();
    const expected = [...expectedCapabilities].sort();
    const missing = expected.filter((capability) => !actualCapabilities.includes(capability));
    const unexpected = actualCapabilities.filter((capability) => !expected.includes(capability));
    if (missing.length > 0) {
      violations.push({
        path: pkg.manifest_path,
        line: 1,
        message: `${pkg.name}:${feature} missing effective Tokio capabilities: ${missing.join(', ')}`,
      });
    }
    if (unexpected.length > 0) {
      violations.push({
        path: pkg.manifest_path,
        line: 1,
        message: `${pkg.name}:${feature} has unexpected effective Tokio capabilities: ${unexpected.join(', ')}`,
      });
    }
  }

  for (const [feature, values] of Object.entries(featureGraph)) {
    if (ownerProfiles.has(feature)) {
      continue;
    }
    if (values.some((value) => value.startsWith('tokio/'))) {
      violations.push({
        path: pkg.manifest_path,
        line: 1,
        message: `${pkg.name}:${feature} Tokio capabilities require an explicit owner contract`,
      });
    }
  }

  return violations;
}

export function findServicesIntegrationsTokioFeatureViolations(pkg) {
  return findOwnedTokioFeatureViolations(pkg, SERVICES_INTEGRATIONS_TOKIO_FEATURES);
}

export function findServicesCoreTokioFeatureViolations(pkg) {
  return findOwnedTokioFeatureViolations(pkg, SERVICES_CORE_TOKIO_FEATURES);
}

export function findServicesCorePlatformDependencyFeatureViolations(packages) {
  const violations = [];

  for (const pkg of packages) {
    if (pkg.name !== 'bitfun-services-core') {
      continue;
    }
    for (const dependency of pkg.dependencies ?? []) {
      if (dependency.name !== 'windows' || (dependency.features ?? []).length === 0) {
        continue;
      }
      violations.push({
        path: pkg.manifest_path,
        line: 1,
        message:
          'windows API capabilities must be selected by services-core owner features, not the dependency declaration',
      });
    }
  }

  return violations;
}

export function findTokioDependencyFeatureViolations(packages) {
  const violations = [];

  for (const pkg of packages) {
    if (TOKIO_DEPENDENCY_POLICY_EXCLUDED_PACKAGES.has(pkg.name)) {
      continue;
    }
    for (const dependency of pkg.dependencies ?? []) {
      if (dependency.name !== 'tokio') {
        continue;
      }
      const features = dependency.features ?? [];
      if (features.includes('full')) {
        violations.push({
          path: pkg.manifest_path,
          line: 1,
          message: `${pkg.name} must not enable tokio/full for its ${dependencyDescription(dependency)}`,
        });
      }
      const featureOwnedIntegrationRuntime =
        pkg.name === 'bitfun-services-integrations'
        && (dependency.kind ?? null) === null;
      const featureOwnedServicesCoreRuntime =
        pkg.name === 'bitfun-services-core'
        && (dependency.kind ?? null) === null;
      if (featureOwnedServicesCoreRuntime) {
        const actual = [...features].sort();
        const expected = [...SERVICES_CORE_BASE_TOKIO_FEATURES].sort();
        const missing = expected.filter((feature) => !actual.includes(feature));
        const unexpected = actual.filter((feature) => !expected.includes(feature));
        if (missing.length > 0) {
          violations.push({
            path: pkg.manifest_path,
            line: 1,
            message: `${pkg.name} missing base Tokio capabilities: ${missing.join(', ')}`,
          });
        }
        if (unexpected.length > 0) {
          violations.push({
            path: pkg.manifest_path,
            line: 1,
            message: `${pkg.name} has unexpected base Tokio capabilities: ${unexpected.join(', ')}`,
          });
        }
      } else if (features.length === 0 && !featureOwnedIntegrationRuntime) {
        violations.push({
          path: pkg.manifest_path,
          line: 1,
          message: `${pkg.name} must declare explicit Tokio capabilities for its ${dependencyDescription(dependency)}`,
        });
      }
    }

    if (pkg.name === 'bitfun-services-integrations') {
      violations.push(...findServicesIntegrationsTokioFeatureViolations(pkg));
    }
    if (pkg.name === 'bitfun-services-core') {
      violations.push(...findServicesCoreTokioFeatureViolations(pkg));
    }
  }

  return violations;
}

export function findCargoLayerViolations(
  packages,
  { root, crateLayoutRules },
  resolvedDependencies = null,
) {
  const packageByManifest = new Map(
    packages.map((pkg) => [normalizedPath(pkg.manifest_path), pkg]),
  );
  const layerByManifest = new Map();
  const violations = [];

  for (const pkg of packages) {
    const layer = layerForManifest(pkg.manifest_path, { root, crateLayoutRules });
    layerByManifest.set(normalizedPath(pkg.manifest_path), layer);
    if (!layer) {
      const repoManifestPath = repositoryPath(root, pkg.manifest_path) ?? pkg.manifest_path;
      violations.push({
        path: pkg.manifest_path,
        line: 1,
        message: `unknown crate layer for repository package ${pkg.name} at ${repoManifestPath}`,
      });
    }
  }

  const declaredDependencies = [];
  for (const sourcePackage of packages) {
    for (const dependency of sourcePackage.dependencies ?? []) {
      if (!dependency.path || repositoryPath(root, dependency.path) === null) {
        continue;
      }

      const targetManifestKey = normalizedPath(join(dependency.path, 'Cargo.toml'));
      const targetPackage = packageByManifest.get(targetManifestKey);
      if (!targetPackage) {
        violations.push({
          path: sourcePackage.manifest_path,
          line: 1,
          message: `cargo metadata did not discover internal path dependency ${dependency.name} at ${repositoryPath(root, dependency.path)}`,
        });
        continue;
      }

      declaredDependencies.push({
        sourceManifestPath: sourcePackage.manifest_path,
        targetManifestPath: targetPackage.manifest_path,
        name: dependency.name,
        kind: dependency.kind,
        optional: dependency.optional,
        target: dependency.target,
      });
    }
  }

  const dependenciesToCheck = new Map();
  for (const dependency of [
    ...declaredDependencies,
    ...(resolvedDependencies ?? []),
  ]) {
    const key = [
      normalizedPath(dependency.sourceManifestPath),
      normalizedPath(dependency.targetManifestPath),
      dependency.kind ?? 'normal',
      dependency.target ?? '',
    ].join('|');
    const existing = dependenciesToCheck.get(key);
    dependenciesToCheck.set(key, existing
      ? {
          ...existing,
          optional: existing.optional && dependency.optional,
        }
      : dependency);
  }

  for (const dependency of dependenciesToCheck.values()) {
    const sourceManifestKey = normalizedPath(dependency.sourceManifestPath);
    const targetManifestKey = normalizedPath(dependency.targetManifestPath);
    const sourcePackage = packageByManifest.get(sourceManifestKey);
    const targetPackage = packageByManifest.get(targetManifestKey);
    if (!sourcePackage || !targetPackage) {
      continue;
    }

    const sourceLayer = layerByManifest.get(sourceManifestKey);
    const targetLayer = layerByManifest.get(targetManifestKey);
    if (!sourceLayer || !targetLayer || ALLOWED_TARGET_LAYERS.get(sourceLayer)?.has(targetLayer)) {
      continue;
    }

    violations.push({
      path: sourcePackage.manifest_path,
      line: 1,
      message: `cargo dependency layer violation: ${sourcePackage.name} (${sourceLayer}) -> ${targetPackage.name} (${targetLayer}) via ${dependencyDescription(dependency)}`,
    });
  }

  return violations;
}

export function findProductEntrypointCoreFeatureViolations(
  packages,
  { root, crateLayoutRules },
) {
  const packageByManifest = new Map(
    packages.map((pkg) => [normalizedPath(pkg.manifest_path), pkg]),
  );
  const violations = [];

  for (const sourcePackage of packages) {
    const sourceLayer = layerForManifest(sourcePackage.manifest_path, {
      root,
      crateLayoutRules,
    });
    if (sourceLayer !== 'apps' && sourceLayer !== 'interfaces') {
      continue;
    }

    for (const dependency of sourcePackage.dependencies ?? []) {
      if (!dependency.path || repositoryPath(root, dependency.path) === null) {
        continue;
      }
      const targetPackage = packageByManifest.get(
        normalizedPath(join(dependency.path, 'Cargo.toml')),
      );
      if (targetPackage?.name !== 'bitfun-core') {
        continue;
      }
      if (dependency.uses_default_features !== false) {
        violations.push({
          path: sourcePackage.manifest_path,
          line: 1,
          message: `product entrypoint ${sourcePackage.name} must set default-features = false for its bitfun-core ${dependencyDescription(dependency)}`,
        });
      }
      if (!Array.isArray(dependency.features) || dependency.features.length === 0) {
        violations.push({
          path: sourcePackage.manifest_path,
          line: 1,
          message: `product entrypoint ${sourcePackage.name} must select at least one explicit feature for its bitfun-core ${dependencyDescription(dependency)}`,
        });
      }
    }
  }

  return violations;
}

function matchingClosingDelimiter(
  source,
  openingIndex,
  openingCharacter,
  closingCharacter,
) {
  let depth = 0;
  let quote = null;
  let escaped = false;

  for (let index = openingIndex; index < source.length; index += 1) {
    const character = source[index];
    if (quote !== null) {
      if (escaped) {
        escaped = false;
      } else if (character === '\\') {
        escaped = true;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
    } else if (character === openingCharacter) {
      depth += 1;
    } else if (character === closingCharacter) {
      depth -= 1;
      if (depth === 0) {
        return index;
      }
    }
  }

  return -1;
}

function matchingClosingParenthesis(source, openingIndex) {
  return matchingClosingDelimiter(source, openingIndex, '(', ')');
}

function crateCfgBodies(source) {
  const bodies = [];
  let index = source.charCodeAt(0) === 0xFEFF ? 1 : 0;

  while (index < source.length) {
    if (/\s/.test(source[index])) {
      index += 1;
      continue;
    }
    if (source.startsWith('//', index)) {
      const lineEnd = source.indexOf('\n', index + 2);
      index = lineEnd === -1 ? source.length : lineEnd + 1;
      continue;
    }
    if (source.startsWith('/*', index)) {
      let depth = 1;
      index += 2;
      while (index < source.length && depth > 0) {
        if (source.startsWith('/*', index)) {
          depth += 1;
          index += 2;
        } else if (source.startsWith('*/', index)) {
          depth -= 1;
          index += 2;
        } else {
          index += 1;
        }
      }
      continue;
    }
    if (!source.startsWith('#![', index)) {
      break;
    }

    const closingBracket = matchingClosingDelimiter(source, index + 2, '[', ']');
    if (closingBracket === -1) {
      break;
    }
    const attribute = source.slice(index, closingBracket + 1);
    const cfgStart = /^#!\s*\[\s*cfg\s*\(/.exec(attribute);
    if (cfgStart !== null) {
      const openingIndex = cfgStart[0].length - 1;
      const closingIndex = matchingClosingParenthesis(attribute, openingIndex);
      if (closingIndex !== -1) {
        bodies.push(attribute.slice(openingIndex + 1, closingIndex));
      }
    }
    index = closingBracket + 1;
  }

  return bodies;
}

function removeCfgBranches(expression, branchName) {
  const branchStart = new RegExp(`\\b${branchName}\\s*\\(`, 'g');
  let result = expression;
  for (let match = branchStart.exec(result); match !== null; match = branchStart.exec(result)) {
    const openingIndex = branchStart.lastIndex - 1;
    const closingIndex = matchingClosingParenthesis(result, openingIndex);
    if (closingIndex === -1) {
      break;
    }
    result = `${result.slice(0, match.index)}${' '.repeat(closingIndex + 1 - match.index)}${result.slice(closingIndex + 1)}`;
    branchStart.lastIndex = match.index;
  }
  return result;
}

function containsFeatureAny(expression) {
  const anyStart = /\bany\s*\(/g;
  for (let match = anyStart.exec(expression); match !== null; match = anyStart.exec(expression)) {
    const openingIndex = anyStart.lastIndex - 1;
    const closingIndex = matchingClosingParenthesis(expression, openingIndex);
    if (closingIndex === -1) {
      return false;
    }
    const body = expression.slice(openingIndex + 1, closingIndex);
    if (/\bfeature\s*=\s*"[^"]+"/.test(body)) {
      return true;
    }
    anyStart.lastIndex = closingIndex + 1;
  }
  return false;
}

function crateFeatureCfgFacts(source) {
  const positiveFeatures = new Set();
  let unsupportedAny = false;

  for (const body of crateCfgBodies(source)) {
    const positiveExpression = removeCfgBranches(body, 'not');
    unsupportedAny ||= containsFeatureAny(positiveExpression);
    for (const match of positiveExpression.matchAll(/\bfeature\s*=\s*"([^"]+)"/g)) {
      positiveFeatures.add(match[1]);
    }
  }

  return { positiveFeatures, unsupportedAny };
}

export function findFeatureGatedTestTargetViolations(
  packages,
  { readSource = (path) => readFileSync(path, 'utf8') } = {},
) {
  const violations = [];

  for (const pkg of packages) {
    for (const target of pkg.targets ?? []) {
      if (!(target.kind ?? []).includes('test')) {
        continue;
      }
      const cfgFacts = crateFeatureCfgFacts(readSource(target.src_path));
      if (cfgFacts.unsupportedAny) {
        violations.push({
          path: target.src_path,
          line: 1,
          message: `integration test target ${pkg.name}:${target.name} uses feature any(...), which Cargo required-features cannot express; split the target`,
        });
        continue;
      }
      const declared = new Set(target['required-features'] ?? []);
      const missing = [...cfgFacts.positiveFeatures]
        .filter((feature) => !declared.has(feature));
      const unexpected = cfgFacts.positiveFeatures.size === 0
        ? []
        : [...declared].filter((feature) => !cfgFacts.positiveFeatures.has(feature));
      if (missing.length > 0 && unexpected.length > 0) {
        violations.push({
          path: target.src_path,
          line: 1,
          message: `feature-gated integration test target ${pkg.name}:${target.name} must align required-features with crate-level cfg; missing: ${missing.join(', ')}; unexpected: ${unexpected.join(', ')}`,
        });
      } else if (missing.length > 0) {
        violations.push({
          path: target.src_path,
          line: 1,
          message: `feature-gated integration test target ${pkg.name}:${target.name} must declare required-features for: ${missing.join(', ')}`,
        });
      } else if (unexpected.length > 0) {
        violations.push({
          path: target.src_path,
          line: 1,
          message: `feature-gated integration test target ${pkg.name}:${target.name} has unexpected required-features: ${unexpected.join(', ')}`,
        });
      }
    }
  }

  return violations;
}

export function discoverCargoManifestPaths(root) {
  const manifests = [];

  function visit(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!SKIPPED_DIRECTORIES.has(entry.name)) {
          visit(join(directory, entry.name));
        }
        continue;
      }
      if (entry.isFile() && entry.name === 'Cargo.toml') {
        manifests.push(join(directory, entry.name));
      }
    }
  }

  visit(root);
  const workspaceManifest = normalizedPath(join(root, 'Cargo.toml'));
  return manifests.sort((left, right) => {
    if (normalizedPath(left) === workspaceManifest) {
      return -1;
    }
    if (normalizedPath(right) === workspaceManifest) {
      return 1;
    }
    return left.localeCompare(right);
  });
}

function loadCargoMetadata(manifestPath, root, { noDeps = false } = {}) {
  const args = ['metadata', '--format-version', '1', '--all-features'];
  if (noDeps) {
    args.push('--no-deps');
  }
  args.push('--manifest-path', manifestPath);
  const result = spawnSync(
    'cargo',
    args,
    {
      cwd: root,
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  if (result.status !== 0) {
    const detail = (result.stderr || result.stdout || `exit code ${result.status}`).trim();
    throw new Error(`cargo metadata failed for ${manifestPath}: ${detail}`);
  }
  return JSON.parse(result.stdout);
}

function resolvedDependencyRecords(metadata, root) {
  const packageById = new Map((metadata.packages ?? []).map((pkg) => [pkg.id, pkg]));
  const records = [];

  for (const node of metadata.resolve?.nodes ?? []) {
    const sourcePackage = packageById.get(node.id);
    if (!sourcePackage || repositoryPath(root, sourcePackage.manifest_path) === null) {
      continue;
    }

    for (const dependency of node.deps ?? []) {
      const targetPackage = packageById.get(dependency.pkg);
      if (!targetPackage || repositoryPath(root, targetPackage.manifest_path) === null) {
        continue;
      }

      const declarations = (sourcePackage.dependencies ?? []).filter((candidate) =>
        candidate.name === targetPackage.name
        && (candidate.rename ?? candidate.name) === dependency.name
      );
      const dependencyKinds = dependency.dep_kinds?.length > 0
        ? dependency.dep_kinds
        : [{ kind: null, target: null }];

      for (const dependencyKind of dependencyKinds) {
        const kind = dependencyKind.kind ?? null;
        const declaration = declarations.find((candidate) =>
          (candidate.kind ?? null) === kind
          && (candidate.target ?? null) === (dependencyKind.target ?? null)
        ) ?? declarations.find((candidate) => (candidate.kind ?? null) === kind)
          ?? declarations[0];

        records.push({
          sourceManifestPath: sourcePackage.manifest_path,
          targetManifestPath: targetPackage.manifest_path,
          name: dependency.name,
          kind,
          optional: declaration?.optional ?? false,
          target: dependencyKind.target ?? null,
        });
      }
    }
  }

  return records;
}

export function collectCargoMetadataGraph({
  root,
  manifestPaths = discoverCargoManifestPaths(root),
  loadMetadata = (manifestPath, options) => loadCargoMetadata(manifestPath, root, options),
}) {
  const packagesByManifest = new Map();
  const dependenciesByKey = new Map();
  const coveredManifests = new Set();
  const workspaceManifest = normalizedPath(join(root, 'Cargo.toml'));
  const orderedManifests = [...manifestPaths].sort((left, right) => {
    if (normalizedPath(left) === workspaceManifest) {
      return -1;
    }
    if (normalizedPath(right) === workspaceManifest) {
      return 1;
    }
    return left.localeCompare(right);
  });

  for (const manifestPath of orderedManifests) {
    const manifestKey = normalizedPath(manifestPath);
    if (manifestKey !== workspaceManifest && coveredManifests.has(manifestKey)) {
      continue;
    }

    const metadata = loadMetadata(manifestPath, {
      noDeps: manifestKey !== workspaceManifest,
    });
    const workspaceMemberIds = new Set(metadata.workspace_members ?? []);
    for (const pkg of metadata.packages ?? []) {
      if (repositoryPath(root, pkg.manifest_path) === null) {
        continue;
      }
      const packageManifestKey = normalizedPath(pkg.manifest_path);
      if (workspaceMemberIds.has(pkg.id)) {
        coveredManifests.add(packageManifestKey);
      }
      packagesByManifest.set(packageManifestKey, pkg);
    }
    for (const dependency of resolvedDependencyRecords(metadata, root)) {
      const key = [
        normalizedPath(dependency.sourceManifestPath),
        normalizedPath(dependency.targetManifestPath),
        dependency.name,
        dependency.kind ?? 'normal',
        dependency.optional,
        dependency.target ?? '',
      ].join('|');
      dependenciesByKey.set(key, dependency);
    }
  }

  return {
    packages: [...packagesByManifest.values()],
    resolvedDependencies: [...dependenciesByKey.values()],
  };
}

export function collectCargoMetadataPackages(options) {
  return collectCargoMetadataGraph(options).packages;
}

export function checkCargoDependencyLayers({ root, crateLayoutRules }) {
  const { packages, resolvedDependencies } = collectCargoMetadataGraph({ root });
  return findCargoLayerViolations(
    packages,
    { root, crateLayoutRules },
    resolvedDependencies,
  );
}

export function checkCargoDependencyLayersSafely({ root, crateLayoutRules }) {
  try {
    return checkCargoDependencyLayers({ root, crateLayoutRules });
  } catch (error) {
    return [{
      path: join(root, 'Cargo.toml'),
      line: 1,
      message: `cargo dependency layer check failed to run: ${error.message}`,
    }];
  }
}

export function checkCargoDependencyBoundaries({ root, crateLayoutRules }) {
  const { packages, resolvedDependencies } = collectCargoMetadataGraph({ root });
  return [
    ...findCargoLayerViolations(
      packages,
      { root, crateLayoutRules },
      resolvedDependencies,
    ),
    ...findProductEntrypointCoreFeatureViolations(
      packages,
      { root, crateLayoutRules },
    ),
    ...findFeatureGatedTestTargetViolations(packages),
    ...findTokioDependencyFeatureViolations(packages),
    ...findServicesCorePlatformDependencyFeatureViolations(packages),
  ];
}

export function checkCargoDependencyBoundariesSafely({ root, crateLayoutRules }) {
  try {
    return checkCargoDependencyBoundaries({ root, crateLayoutRules });
  } catch (error) {
    return [{
      path: join(root, 'Cargo.toml'),
      line: 1,
      message: `cargo dependency boundary check failed to run: ${error.message}`,
    }];
  }
}
