import { existsSync, readdirSync, readFileSync, statSync } from 'fs';
import { join, relative } from 'path';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

import {
  dependencyProfileRules,
  lightweightBoundaryRules,
  noCoreDependencyCrates,
} from './rules/crate-rules.mjs';
import {
  crateLayoutLayerNames,
  crateLayoutRules,
  cratePathForName,
} from './rules/crate-layout.mjs';
import {
  coreProductFullFeatureAssemblyRule,
  optionalDependencyFeatureOwnerRules,
  ownerCrateFeatureAssemblyRules,
  productCoreFeatureAssemblyRules,
  productCoreFeatureAssemblyScanRoots,
} from './rules/feature-rules.mjs';
import {
  facadeOnlyFiles,
  forbiddenContentRules,
  forbiddenContentUnderRules,
  requiredContentRules,
} from './rules/source-rules.mjs';
import { runManifestParserSelfTest } from './self-test.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..', '..');
const failures = [];

function toRepoPath(path) {
  return relative(ROOT, path).replace(/\\/g, '/');
}

function readText(path) {
  return readFileSync(path, 'utf8');
}

function repoPathToFsPath(repoPath) {
  return join(ROOT, ...repoPath.split('/'));
}

function crateDirForName(crateName) {
  const repoPath = cratePathForName(crateName);
  if (!repoPath) {
    failures.push({
      path: ROOT,
      line: 1,
      message: `missing crate layout rule for ${crateName}`,
    });
    return join(ROOT, 'src', 'crates', crateName);
  }
  return repoPathToFsPath(repoPath);
}

function walkFiles(dir, visit) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      walkFiles(path, visit);
      continue;
    }
    visit(path);
  }
}

function rustImportName(depName) {
  return depName.replace(/-/g, '_');
}

function escapeRegex(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function regexSourceContainsContract(ruleText, contract) {
  const slashEscapedContract = contract.replaceAll('/', '\\/');
  const escapedContract = escapeRegex(contract);
  const slashEscapedRegexContract = escapedContract.replaceAll('/', '\\/');
  return (
    ruleText.includes(contract) ||
    ruleText.includes(slashEscapedContract) ||
    ruleText.includes(escapedContract) ||
    ruleText.includes(slashEscapedRegexContract)
  );
}

function manifestDependencyHeaderPattern(depName) {
  const depPattern = `(?:${escapeRegex(depName)}|"${escapeRegex(depName)}")`;
  return new RegExp(
    `^\\[(?:target\\.[^\\]]+\\.)?(?:dependencies|dev-dependencies|build-dependencies)\\.${depPattern}\\]$`,
  );
}

function isManifestDependencyDeclaration(trimmedLine, depName) {
  const isInlineDependency = new RegExp(`^${escapeRegex(depName)}\\s*=`).test(trimmedLine);
  const isDependencyTable = manifestDependencyHeaderPattern(depName).test(trimmedLine);
  return isInlineDependency || isDependencyTable;
}

function isDependencyListHeader(trimmedLine) {
  return /^\[(?:target\.[^\]]+\.)?(?:dependencies|dev-dependencies|build-dependencies)\]$/.test(
    trimmedLine,
  );
}

function parseManifestDependencies(lines) {
  const deps = [];
  let inDependencyList = false;
  let currentTable = null;
  let currentInline = null;

  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') {
      return;
    }

    if (currentInline) {
      currentInline.text.push(trimmed);
      if (/\boptional\s*=\s*true\b/.test(trimmed)) {
        currentInline.optional = true;
      }
      if (trimmed.includes('}')) {
        currentInline = null;
      }
      return;
    }

    const headerMatch = trimmed.match(/^\[(.+)]$/);
    if (headerMatch) {
      inDependencyList = isDependencyListHeader(trimmed);
      currentTable = null;
      for (const depName of collectKnownDependencyNames()) {
        if (manifestDependencyHeaderPattern(depName).test(trimmed)) {
          currentTable = {
            name: depName,
            line: index + 1,
            optional: false,
            text: [trimmed],
          };
          deps.push(currentTable);
          break;
        }
      }
      return;
    }

    if (currentTable) {
      currentTable.text.push(trimmed);
      if (/\boptional\s*=\s*true\b/.test(trimmed)) {
        currentTable.optional = true;
      }
      return;
    }

    if (!inDependencyList) {
      return;
    }

    const inlineMatch = trimmed.match(/^([A-Za-z0-9_-]+|"[A-Za-z0-9_-]+")\s*=/);
    if (inlineMatch) {
      const name = inlineMatch[1].replace(/^"|"$/g, '');
      deps.push({
        name,
        line: index + 1,
        optional: /\boptional\s*=\s*true\b/.test(trimmed),
        text: [trimmed],
      });
      if (trimmed.includes('{') && !trimmed.includes('}')) {
        currentInline = deps[deps.length - 1];
      }
      return;
    }

  });

  return deps;
}

function manifestDependencyText(dep) {
  return dep?.text?.join('\n') ?? '';
}

function manifestDependencyDisablesDefaultFeatures(dep) {
  return /\bdefault-features\s*=\s*false\b/.test(manifestDependencyText(dep));
}

function parseManifestDependencyFeatureNames(dep) {
  const features = new Set();
  const text = manifestDependencyText(dep);
  for (const match of text.matchAll(/\bfeatures\s*=\s*\[([\s\S]*?)\]/g)) {
    for (const featureMatch of match[1].matchAll(/"([^"]+)"/g)) {
      features.add(featureMatch[1]);
    }
  }
  return features;
}

function collectProductCoreDependencyManifestPaths(manifestEntries) {
  return manifestEntries
    .filter((entry) => {
      const deps = parseManifestDependencies(entry.text.split(/\r?\n/));
      return deps.some((dep) => dep.name === 'bitfun-core');
    })
    .map((entry) => entry.manifestPath)
    .sort();
}

function collectProductCoreDependencyManifests(scanRoots = productCoreFeatureAssemblyScanRoots) {
  const manifestEntries = [];
  for (const repoDir of scanRoots) {
    const dir = join(ROOT, ...repoDir.split('/'));
    walkFiles(dir, (path) => {
      if (!path.endsWith('Cargo.toml')) {
        return;
      }
      manifestEntries.push({
        manifestPath: toRepoPath(path),
        text: readText(path),
      });
    });
  }
  return collectProductCoreDependencyManifestPaths(manifestEntries);
}

function parseManifestFeatures(lines) {
  const features = new Map();
  let inFeatures = false;
  let currentFeature = null;

  const appendRefs = (feature, text) => {
    const refs = [...text.matchAll(/"([^"]+)"/g)].map((match) => match[1]);
    feature.refs.push(...refs);
  };

  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('#') || trimmed === '') {
      return;
    }

    const headerMatch = trimmed.match(/^\[(.+)]$/);
    if (headerMatch) {
      inFeatures = trimmed === '[features]';
      currentFeature = null;
      return;
    }

    if (!inFeatures) {
      return;
    }

    if (currentFeature) {
      appendRefs(currentFeature, trimmed);
      if (trimmed.includes(']')) {
        currentFeature = null;
      }
      return;
    }

    const featureMatch = trimmed.match(/^([A-Za-z0-9_-]+)\s*=\s*(.*)$/);
    if (!featureMatch) {
      return;
    }

    const feature = {
      name: featureMatch[1],
      line: index + 1,
      refs: [],
    };
    appendRefs(feature, featureMatch[2]);
    features.set(feature.name, feature);
    if (featureMatch[2].includes('[') && !featureMatch[2].includes(']')) {
      currentFeature = feature;
    }
  });

  return features;
}

function collectKnownDependencyNames() {
  return Array.from(
    new Set([
      'bitfun-core',
      ...lightweightBoundaryRules.flatMap((rule) => rule.forbiddenDeps),
      ...dependencyProfileRules.flatMap((rule) => rule.forbiddenNonOptionalDeps),
      ...optionalDependencyFeatureOwnerRules.flatMap((rule) =>
        rule.dependencies.map((dependency) => dependency.depName),
      ),
      ...productCoreFeatureAssemblyRules.map((rule) => rule.dependencyName),
    ]),
  );
}

function parseWorkspaceMembers() {
  const manifestPath = join(ROOT, 'Cargo.toml');
  const lines = readText(manifestPath).split(/\r?\n/);
  const members = [];
  let inMembers = false;
  for (const line of lines) {
    const trimmed = line.trim();
    if (!inMembers) {
      if (trimmed === 'members = [' || trimmed.startsWith('members = [')) {
        inMembers = true;
      }
    }
    if (!inMembers) {
      continue;
    }
    for (const match of trimmed.matchAll(/"([^"]+)"/g)) {
      members.push(match[1]);
    }
    if (trimmed.includes(']')) {
      break;
    }
  }
  return members;
}

function checkCrateLayoutRules() {
  const manifestPath = join(ROOT, 'Cargo.toml');
  const requiredCrateNames = new Set(['core', ...noCoreDependencyCrates]);
  const layoutCrateNames = new Set();
  const layoutPaths = new Set();
  const workspaceMembers = new Set(parseWorkspaceMembers());
  const expectedWorkspaceCratePaths = new Set(crateLayoutRules.map((rule) => rule.path));

  for (const rule of crateLayoutRules) {
    if (!crateLayoutLayerNames.includes(rule.layer)) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `crate layout rule for ${rule.crateName} uses unknown layer ${rule.layer}`,
      });
    }
    if (layoutCrateNames.has(rule.crateName)) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `duplicate crate layout rule for ${rule.crateName}`,
      });
    }
    if (layoutPaths.has(rule.path)) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `duplicate crate layout path ${rule.path}`,
      });
    }
    layoutCrateNames.add(rule.crateName);
    layoutPaths.add(rule.path);

    const crateManifestPath = repoPathToFsPath(`${rule.path}/Cargo.toml`);
    if (!existsSync(crateManifestPath)) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `crate layout path for ${rule.crateName} is missing Cargo.toml: ${rule.path}`,
      });
    }
    if (!workspaceMembers.has(rule.path)) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `workspace members must include crate layout path: ${rule.path}`,
      });
    }
  }

  for (const crateName of requiredCrateNames) {
    if (!layoutCrateNames.has(crateName)) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `crate layout rules must cover workspace owner crate: ${crateName}`,
      });
    }
  }

  for (const member of workspaceMembers) {
    if (!member.startsWith('src/crates/')) {
      continue;
    }
    if (!expectedWorkspaceCratePaths.has(member)) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `workspace crate member must use an approved layered path: ${member}`,
      });
    }
  }

  const cratesRoot = join(ROOT, 'src', 'crates');
  const allowedRootEntries = new Set([...crateLayoutLayerNames, 'LOGGING.md']);
  for (const entry of readdirSync(cratesRoot)) {
    if (allowedRootEntries.has(entry)) {
      continue;
    }
    const entryPath = join(cratesRoot, entry);
    if (statSync(entryPath).isDirectory() && existsSync(join(entryPath, 'Cargo.toml'))) {
      failures.push({
        path: entryPath,
        line: 1,
        message: `workspace crate must live under a layer directory, not directly under src/crates: ${entry}`,
      });
    }
  }
}

function forbiddenRuleTextForPath(path) {
  return forbiddenContentRules
    .filter((rule) => rule.path === path)
    .flatMap((rule) => rule.patterns)
    .map((pattern) => pattern.regex.source)
    .join('\n');
}

function checkCargoManifest(crateDir) {
  checkForbiddenManifestDeps(crateDir, ['bitfun-core'], () => {
    return 'extracted crate must not depend on bitfun-core';
  });
}

function checkForbiddenManifestDeps(crateDir, forbiddenDeps, messageForDep) {
  const manifestPath = join(crateDir, 'Cargo.toml');
  const lines = readText(manifestPath).split(/\r?\n/);
  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('#')) {
      return;
    }
    for (const dep of forbiddenDeps) {
      if (isManifestDependencyDeclaration(trimmed, dep)) {
        failures.push({
          path: manifestPath,
          line: index + 1,
          message: messageForDep(dep),
        });
      }
    }
  });
}

function checkForbiddenNonOptionalManifestDeps(crateDir, forbiddenDeps, messageForDep) {
  const manifestPath = join(crateDir, 'Cargo.toml');
  const deps = parseManifestDependencies(readText(manifestPath).split(/\r?\n/));
  for (const dep of deps) {
    if (!dep.optional && forbiddenDeps.includes(dep.name)) {
      failures.push({
        path: manifestPath,
        line: dep.line,
        message: messageForDep(dep.name),
      });
    }
  }
}

function featureReferencesDependency(feature, depName) {
  if (!feature) {
    return false;
  }
  return feature.refs.includes(`dep:${depName}`) || feature.refs.includes(depName);
}

function featureReferencesFeature(feature, featureName) {
  if (!feature) {
    return false;
  }
  return feature.refs.includes(featureName);
}

function checkOptionalDependencyFeatureOwners(crateDir, rule) {
  const manifestPath = join(crateDir, 'Cargo.toml');
  const lines = readText(manifestPath).split(/\r?\n/);
  const deps = parseManifestDependencies(lines);
  const depsByName = new Map(deps.map((dep) => [dep.name, dep]));
  const features = parseManifestFeatures(lines);
  const declaredOwnerDeps = new Set(rule.dependencies.map((dependency) => dependency.depName));

  for (const dependency of rule.dependencies) {
    const dep = depsByName.get(dependency.depName);
    if (!dep) {
      failures.push({
        path: manifestPath,
        line: 1,
        message: `${rule.reason}; missing optional dependency: ${dependency.depName}`,
      });
      continue;
    }
    if (!dep.optional) {
      failures.push({
        path: manifestPath,
        line: dep.line,
        message: `${rule.reason}; dependency must be optional: ${dependency.depName}`,
      });
    }
    for (const featureName of dependency.ownerFeatures) {
      const feature = features.get(featureName);
      if (!feature) {
        failures.push({
          path: manifestPath,
          line: dep?.line ?? 1,
          message: `${rule.reason}; missing owner feature ${featureName} for ${dependency.depName}`,
        });
        continue;
      }
      if (!featureReferencesDependency(feature, dependency.depName)) {
        failures.push({
          path: manifestPath,
          line: feature.line,
          message: `${rule.reason}; ${featureName} must explicitly enable ${dependency.depName}`,
        });
      }
    }
  }

  const profileRule = dependencyProfileRules.find((profile) => profile.crateName === rule.crateName);
  const depsRequiringOwner = new Set(profileRule?.forbiddenNonOptionalDeps ?? []);
  const uncoveredDeps = new Map();
  for (const dep of deps) {
    if (!dep.optional || !depsRequiringOwner.has(dep.name) || declaredOwnerDeps.has(dep.name)) {
      continue;
    }
    if (!uncoveredDeps.has(dep.name)) {
      uncoveredDeps.set(dep.name, dep);
    }
  }
  for (const [depName, dep] of uncoveredDeps.entries()) {
    failures.push({
      path: manifestPath,
      line: dep.line,
      message: `${rule.reason}; optional runtime dependency must declare owner feature coverage: ${depName}`,
    });
  }
}

function checkProductCoreFeatureAssembly(rule) {
  const manifestPath = repoPathToFsPath(rule.manifestPath);
  const deps = parseManifestDependencies(readText(manifestPath).split(/\r?\n/));
  const dep = deps.find((candidate) => candidate.name === rule.dependencyName);
  if (!dep) {
    failures.push({
      path: manifestPath,
      line: 1,
      message: `${rule.reason}; missing dependency: ${rule.dependencyName}`,
    });
    return;
  }

  if (!manifestDependencyDisablesDefaultFeatures(dep)) {
    failures.push({
      path: manifestPath,
      line: dep.line,
      message: `${rule.reason}; ${rule.dependencyName} must set default-features = false`,
    });
  }

  const enabledFeatures = parseManifestDependencyFeatureNames(dep);
  for (const featureName of rule.requiredFeatures) {
    if (!enabledFeatures.has(featureName)) {
      failures.push({
        path: manifestPath,
        line: dep.line,
        message: `${rule.reason}; ${rule.dependencyName} must enable feature ${featureName}`,
      });
    }
  }
}

function checkProductCoreFeatureAssemblyCoverage() {
  const rulePaths = new Set(productCoreFeatureAssemblyRules.map((rule) => rule.manifestPath));
  for (const manifestPath of collectProductCoreDependencyManifests()) {
    if (!rulePaths.has(manifestPath)) {
      failures.push({
        path: join(ROOT, ...manifestPath.split('/')),
        line: 1,
        message:
          'product entry crate depends on bitfun-core but is not covered by product-full assembly rules',
      });
    }
  }
}

function checkCoreDefaultProductFullFeature() {
  const manifestPath = join(crateDirForName('core'), 'Cargo.toml');
  const features = parseManifestFeatures(readText(manifestPath).split(/\r?\n/));
  if (!featureReferencesFeature(features.get('default'), 'product-full')) {
    failures.push({
      path: manifestPath,
      line: features.get('default')?.line ?? 1,
      message:
        'bitfun-core default feature must remain product-full until a separate product matrix review changes it',
    });
  }
}

function checkCoreProductFullFeatureAssembly(rule) {
  const manifestPath = repoPathToFsPath(rule.manifestPath);
  const features = parseManifestFeatures(readText(manifestPath).split(/\r?\n/));
  const productFull = features.get(rule.featureName);
  if (!productFull) {
    failures.push({
      path: manifestPath,
      line: 1,
      message: `${rule.reason}; missing ${rule.featureName} feature declaration`,
    });
    return;
  }
  for (const featureName of rule.requiredFeatureRefs) {
    if (!featureReferencesFeature(productFull, featureName)) {
      failures.push({
        path: manifestPath,
        line: productFull.line,
        message: `${rule.reason}; ${rule.featureName} must explicitly enable ${featureName}`,
      });
    }
  }
}

function checkOwnerCrateFeatureAssembly(rule) {
  const manifestPath = repoPathToFsPath(rule.manifestPath);
  const features = parseManifestFeatures(readText(manifestPath).split(/\r?\n/));
  const allowedProductFullFeatures = new Set(rule.requiredProductFullFeatures);
  const defaultFeature = features.get('default');
  if (!defaultFeature) {
    failures.push({
      path: manifestPath,
      line: 1,
      message: `${rule.reason}; missing default feature declaration`,
    });
  } else if (defaultFeature.refs.length > 0) {
    failures.push({
      path: manifestPath,
      line: defaultFeature.line,
      message: `${rule.reason}; default feature must remain empty`,
    });
  }

  const productFull = features.get('product-full');
  if (!productFull) {
    failures.push({
      path: manifestPath,
      line: 1,
      message: `${rule.reason}; missing product-full feature declaration`,
    });
    return;
  }

  for (const featureName of rule.requiredProductFullFeatures) {
    if (!featureReferencesFeature(productFull, featureName)) {
      failures.push({
        path: manifestPath,
        line: productFull.line,
        message: `${rule.reason}; product-full must explicitly enable ${featureName}`,
      });
    }
  }
  for (const featureName of productFull.refs) {
    if (!allowedProductFullFeatures.has(featureName)) {
      failures.push({
        path: manifestPath,
        line: productFull.line,
        message: `${rule.reason}; product-full must not include undeclared feature group ${featureName}`,
      });
    }
  }
}

function checkRustImports(crateDir) {
  const srcDir = join(crateDir, 'src');
  try {
    if (!statSync(srcDir).isDirectory()) {
      return;
    }
  } catch {
    return;
  }

  walkFiles(srcDir, (path) => {
    if (!path.endsWith('.rs')) {
      return;
    }
    const lines = readText(path).split(/\r?\n/);
    lines.forEach((line, index) => {
      if (/\bbitfun_core::/.test(line)) {
        failures.push({
          path,
          line: index + 1,
          message: 'extracted crate must not import bitfun_core',
        });
      }
    });
  });
}

function checkForbiddenRustImports(crateDir, forbiddenDeps, messageForDep) {
  const srcDir = join(crateDir, 'src');
  try {
    if (!statSync(srcDir).isDirectory()) {
      return;
    }
  } catch {
    return;
  }

  const forbiddenImports = forbiddenDeps.map((dep) => ({
    dep,
    pattern: new RegExp(`\\b${escapeRegex(rustImportName(dep))}::`),
  }));

  walkFiles(srcDir, (path) => {
    if (!path.endsWith('.rs')) {
      return;
    }
    const lines = readText(path).split(/\r?\n/);
    lines.forEach((line, index) => {
      for (const forbidden of forbiddenImports) {
        if (forbidden.pattern.test(line)) {
          failures.push({
            path,
            line: index + 1,
            message: messageForDep(forbidden.dep),
          });
        }
      }
    });
  });
}

function createFacadeLineChecker(importPrefix) {
  let inPubUseBlock = false;
  const escapedPrefix = escapeRegex(importPrefix);
  const singleReexportPattern = new RegExp(
    `^pub use ${escapedPrefix}(?:::[A-Za-z_][A-Za-z0-9_]*)*(?:::\\*)?;$`,
  );
  const blockItemPattern = /^[A-Za-z_][A-Za-z0-9_]*(?:,\s*[A-Za-z_][A-Za-z0-9_]*)*,?$/;
  const blockStart = `pub use ${importPrefix}::{`;

  const checker = (line) => {
    const trimmed = line.trim();
    if (
      trimmed === '' ||
      trimmed.startsWith('//') ||
      trimmed.startsWith('/*') ||
      trimmed.startsWith('*') ||
      trimmed.startsWith('*/')
    ) {
      return true;
    }

    if (inPubUseBlock) {
      if (trimmed === '};') {
        inPubUseBlock = false;
        return true;
      }
      return blockItemPattern.test(trimmed);
    }

    if (singleReexportPattern.test(trimmed)) {
      return true;
    }

    if (trimmed.startsWith(blockStart)) {
      if (trimmed.endsWith('};')) {
        return true;
      }
      if (trimmed.endsWith('{')) {
        inPubUseBlock = true;
        return true;
      }
    }

    return false;
  };

  checker.isComplete = () => !inPubUseBlock;
  return checker;
}

function checkFacadeOnlyFile(repoPath, importPrefix, reason) {
  const path = repoPathToFsPath(repoPath);
  const acceptsLine = createFacadeLineChecker(importPrefix);
  const lines = readText(path).split(/\r?\n/);
  lines.forEach((line, index) => {
    if (!acceptsLine(line)) {
      failures.push({
        path,
        line: index + 1,
        message: reason,
      });
    }
  });

  if (!acceptsLine.isComplete()) {
    failures.push({
      path,
      line: lines.length,
      message: `${reason}; unterminated pub use block`,
    });
  }
}

function checkForbiddenContent(repoPath, patterns) {
  const path = repoPathToFsPath(repoPath);
  const lines = readText(path).split(/\r?\n/);
  lines.forEach((line, index) => {
    for (const pattern of patterns) {
      if (pattern.regex.test(line)) {
        failures.push({
          path,
          line: index + 1,
          message: pattern.message,
        });
      }
    }
  });
}

function checkRequiredContent(repoPath, patterns, reason) {
  const path = repoPathToFsPath(repoPath);
  const text = readText(path);
  for (const pattern of patterns) {
    if (!pattern.regex.test(text)) {
      failures.push({
        path,
        line: 1,
        message: `${reason}; ${pattern.message}`,
      });
    }
  }
}

function checkForbiddenContentUnder(repoDir, patterns, reason) {
  const dir = repoPathToFsPath(repoDir);
  walkFiles(dir, (path) => {
    if (!path.endsWith('.rs')) {
      return;
    }
    const repoPath = toRepoPath(path);
    const lines = readText(path).split(/\r?\n/);
    lines.forEach((line, index) => {
      for (const pattern of patterns) {
        if (pattern.allowPaths?.includes(repoPath)) {
          continue;
        }
        if (pattern.regex.test(line)) {
          failures.push({
            path,
            line: index + 1,
            message: `${reason}; ${pattern.message}`,
          });
        }
      }
    });
  });
}

export function runCoreBoundaryCheck() {
  failures.length = 0;

  if (process.env.BITFUN_BOUNDARY_CHECK_SELF_TEST === '1') {
    runManifestParserSelfTest({
      isManifestDependencyDeclaration,
      parseManifestDependencies,
      manifestDependencyDisablesDefaultFeatures,
      parseManifestDependencyFeatureNames,
      productCoreFeatureAssemblyRules,
      coreProductFullFeatureAssemblyRule,
      collectProductCoreDependencyManifestPaths,
      ownerCrateFeatureAssemblyRules,
      parseManifestFeatures,
      optionalDependencyFeatureOwnerRules,
      lightweightBoundaryRules,
      dependencyProfileRules,
      noCoreDependencyCrates,
      requiredContentRules,
      forbiddenContentRules,
      forbiddenContentUnderRules,
      facadeOnlyFiles,
      forbiddenRuleTextForPath,
      regexSourceContainsContract,
      createFacadeLineChecker,
      escapeRegex,
    });
    console.log('Core boundary check self-test passed.');
    return;
  }

  checkCrateLayoutRules();

  for (const crateName of noCoreDependencyCrates) {
    const crateDir = crateDirForName(crateName);
    checkCargoManifest(crateDir);
    checkRustImports(crateDir);
  }

  for (const rule of lightweightBoundaryRules) {
    const crateDir = crateDirForName(rule.crateName);
    const messageForDep = (dep) => `${rule.reason}; forbidden dependency: ${dep}`;
    checkForbiddenManifestDeps(crateDir, rule.forbiddenDeps, messageForDep);
    checkForbiddenRustImports(crateDir, rule.forbiddenDeps, messageForDep);
  }

  for (const rule of dependencyProfileRules) {
    const crateDir = crateDirForName(rule.crateName);
    const messageForDep = (dep) =>
      `${rule.reason}; ${rule.profileName} forbids non-optional dependency: ${dep}`;
    checkForbiddenNonOptionalManifestDeps(crateDir, rule.forbiddenNonOptionalDeps, messageForDep);
  }

  for (const rule of optionalDependencyFeatureOwnerRules) {
    const crateDir = crateDirForName(rule.crateName);
    checkOptionalDependencyFeatureOwners(crateDir, rule);
  }

  for (const rule of productCoreFeatureAssemblyRules) {
    checkProductCoreFeatureAssembly(rule);
  }
  checkProductCoreFeatureAssemblyCoverage();
  checkCoreDefaultProductFullFeature();
  checkCoreProductFullFeatureAssembly(coreProductFullFeatureAssemblyRule);
  for (const rule of ownerCrateFeatureAssemblyRules) {
    checkOwnerCrateFeatureAssembly(rule);
  }

  for (const facade of facadeOnlyFiles) {
    checkFacadeOnlyFile(facade.path, facade.importPrefix, facade.reason);
  }

  for (const rule of forbiddenContentRules) {
    checkForbiddenContent(rule.path, rule.patterns);
  }

  for (const rule of forbiddenContentUnderRules) {
    checkForbiddenContentUnder(rule.path, rule.patterns, rule.reason);
  }

  for (const rule of requiredContentRules) {
    checkRequiredContent(rule.path, rule.patterns, rule.reason);
  }

  if (failures.length > 0) {
    console.error('Core boundary check failed.');
    for (const failure of failures) {
      console.error(`${toRepoPath(failure.path)}:${failure.line}: ${failure.message}`);
    }
    process.exit(1);
  }

  console.log('Core boundary check passed.');
}
