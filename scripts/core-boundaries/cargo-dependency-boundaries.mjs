import { readdirSync } from 'node:fs';
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

function loadCargoMetadata(manifestPath, root) {
  const result = spawnSync(
    'cargo',
    ['metadata', '--format-version', '1', '--all-features', '--manifest-path', manifestPath],
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
  loadMetadata = (manifestPath) => loadCargoMetadata(manifestPath, root),
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

    const metadata = loadMetadata(manifestPath);
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
