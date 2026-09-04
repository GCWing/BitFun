import { mkdirSync, realpathSync } from 'node:fs';
import { isAbsolute, join, relative, resolve, sep } from 'node:path';

import { ProductDefinitionError } from './resolver.mjs';

function inside(root, candidate) {
  const path = relative(root, candidate);
  return path === '' || (!path.startsWith(`..${sep}`) && path !== '..' && !isAbsolute(path));
}

export function ensureProductOutputDirectory(resolution) {
  const target = join(resolution.rootDir, 'target');
  mkdirSync(target, { recursive: true });
  const canonicalTarget = realpathSync.native(target);
  if (!inside(resolution.rootDir, canonicalTarget)) {
    throw new ProductDefinitionError(
      'generated_path_escape',
      'The generated target directory resolves outside the repository.',
      'Remove the escaping link before resolving product output.',
    );
  }
  const segments = ['product-assembly', resolution.assembly.assemblyDigest, resolution.assembly.member];
  let current = target;
  for (const segment of segments) {
    current = join(current, segment);
    mkdirSync(current, { recursive: true });
    const canonical = realpathSync.native(current);
    if (!inside(canonicalTarget, canonical)) {
      throw new ProductDefinitionError(
        'generated_path_escape',
        'A generated product directory resolves outside target.',
        'Remove the escaping link before resolving product output.',
      );
    }
  }
  return current;
}

export function productBuildEnvironment(resolution) {
  const fallbackName =
    resolution.productNames[resolution.assembly.fallbackLocale]
    ?? resolution.productNames[resolution.assembly.defaultLocale];
  const environment = {
    OPENBITFUN_PRODUCT_ID: resolution.assembly.productId,
    OPENBITFUN_DATA_NAMESPACE: resolution.assembly.dataNamespace,
    OPENBITFUN_HIDDEN_DATA_DIRECTORY: `.${resolution.assembly.dataNamespace}`,
    OPENBITFUN_PRODUCT_BINARY_NAME: resolution.assembly.binaryName,
    OPENBITFUN_PRODUCT_DISPLAY_NAME: fallbackName,
  };
  if (!resolution.isDefaultProduct) {
    const cargoTargetRoot = process.env.CARGO_TARGET_DIR
      ? resolve(resolution.rootDir, process.env.CARGO_TARGET_DIR)
      : join(resolution.rootDir, 'target');
    environment.CARGO_TARGET_DIR = join(
      cargoTargetRoot,
      '.product-cache',
      resolution.assembly.assemblyDigest.slice(0, 24),
      resolution.assembly.member,
    );
  }
  return environment;
}
