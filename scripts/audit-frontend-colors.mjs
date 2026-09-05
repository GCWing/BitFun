#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const REPOSITORY_ROOT = path.resolve(path.dirname(SCRIPT_PATH), '..');
const DEFAULT_REGISTRY_PATH = path.join(REPOSITORY_ROOT, 'scripts/frontend-color-surface-registry.json');
const THEME_AUDITOR_PATH = path.join(REPOSITORY_ROOT, 'scripts/audit-theme-colors.mjs');
const CLI_AUDITOR_PATH = path.join(REPOSITORY_ROOT, 'scripts/audit-cli-theme-colors.mjs');
const MINIAPP_SOURCE_EXTENSIONS = new Set(['.css', '.html', '.js', '.jsx', '.mjs', '.scss', '.svg', '.ts', '.tsx']);
const SPECIALIZED_COLOR_OWNER_KINDS = new Set(['bespoke-theme', 'data-viz', 'game-renderer', 'slide-renderer']);
const SURFACE_KINDS = new Set(['canonical-web', 'contract-owner', 'miniapp', 'native-mobile', 'terminal']);
const AUDIT_ENGINES = new Set(['cli', 'miniapp', 'native', 'theme']);
const CANONICAL_ZERO_METRICS = [
  'colorScopes.appUi.occurrences',
  'colorScopes.appUi.uniqueColors',
  'colorScopes.token.occurrences',
  'colorScopes.exception.occurrences',
  'fallbackOccurrences',
  'fallbackUniqueTokens',
  'fallbackContracts.uncontractedUnique',
  'compatibilityAliases.usedUnique',
  'compatibilityAliases.occurrences',
  'compatibilityAliases.familyUsedUnique',
  'compatibilityAliases.familyOccurrences',
  'compatibilityAliases.missingCanonicalUnique',
  'surfaceTokenRenames.activeUnique',
  'surfaceTokenRenames.activeOccurrences',
  'surfaceTokenRenames.missingCanonicalUnique',
  'tokenAliasLiterals.occurrences',
  'tokenAliasLiterals.uniqueColors',
  'cssVarDefinitions.unresolvedUnique',
  'cssVarDefinitions.unresolvedRequiredUnique',
  'cssVarDefinitions.fallbackOnlyUnique',
  'cssVarDefinitions.nonContractCrossFileUnique',
  'cssVarDefinitions.nonContractDynamicInputUnique',
  'cssVarDefinitions.nonContractCssPrivateUnique',
  'cssVarDefinitions.unregisteredDynamicFamilyUnique',
  'cssVarDefinitions.dynamicFamilyUnexportedUnique',
  'cssVarDefinitions.staleRegisteredDynamicFamilyUnique',
  'nearPairs.indistinguishableTotal',
  'nearPairs.nearTotal'
];
const HEX_COLOR_PATTERN = /(?<!&)#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})\b/g;
const FUNCTION_COLOR_PATTERN = /\b(?:rgba?|hsla?)\(\s*[^)]+\)/gi;
const HOST_VARIABLE_PATTERN = /--openbitfun-[a-z0-9-]+/g;
const HOST_VARIABLE_FALLBACK_PATTERN = /var\(\s*(--openbitfun-[a-z0-9-]+)\s*,/g;
const HOST_VARIABLE_DEFINITION_PATTERN = /(?:^|[;{\s])(--openbitfun-[a-z0-9-]+)\s*:/gm;
const CSS_NAMED_COLORS = new Set([
  'aliceblue', 'antiquewhite', 'aqua', 'aquamarine', 'azure', 'beige', 'bisque', 'black', 'blue',
  'blueviolet', 'brown', 'burlywood', 'cadetblue', 'chartreuse', 'chocolate', 'coral', 'cornflowerblue',
  'cornsilk', 'crimson', 'cyan', 'darkblue', 'darkcyan', 'darkgoldenrod', 'darkgray', 'darkgreen',
  'darkgrey', 'darkkhaki', 'darkmagenta', 'darkolivegreen', 'darkorange', 'darkorchid', 'darkred',
  'darksalmon', 'darkseagreen', 'darkslateblue', 'darkslategray', 'darkslategrey', 'darkturquoise',
  'darkviolet', 'deeppink', 'deepskyblue', 'dimgray', 'dimgrey', 'dodgerblue', 'firebrick', 'floralwhite',
  'forestgreen', 'fuchsia', 'gainsboro', 'ghostwhite', 'gold', 'goldenrod', 'gray', 'green', 'greenyellow',
  'grey', 'honeydew', 'hotpink', 'indianred', 'indigo', 'ivory', 'khaki', 'lavender', 'lavenderblush',
  'lawngreen', 'lemonchiffon', 'lightblue', 'lightcoral', 'lightcyan', 'lightgoldenrodyellow', 'lightgray',
  'lightgreen', 'lightgrey', 'lightpink', 'lightsalmon', 'lightseagreen', 'lightskyblue', 'lightslategray',
  'lightslategrey', 'lightsteelblue', 'lightyellow', 'lime', 'limegreen', 'linen', 'magenta', 'maroon',
  'mediumaquamarine', 'mediumblue', 'mediumorchid', 'mediumpurple', 'mediumseagreen', 'mediumslateblue',
  'mediumspringgreen', 'mediumturquoise', 'mediumvioletred', 'midnightblue', 'mintcream', 'mistyrose',
  'moccasin', 'navajowhite', 'navy', 'oldlace', 'olive', 'olivedrab', 'orange', 'orangered', 'orchid',
  'palegoldenrod', 'palegreen', 'paleturquoise', 'palevioletred', 'papayawhip', 'peachpuff', 'peru',
  'pink', 'plum', 'powderblue', 'purple', 'rebeccapurple', 'red', 'rosybrown', 'royalblue',
  'saddlebrown', 'salmon', 'sandybrown', 'seagreen', 'seashell', 'sienna', 'silver', 'skyblue',
  'slateblue', 'slategray', 'slategrey', 'snow', 'springgreen', 'steelblue', 'tan', 'teal', 'thistle',
  'tomato', 'turquoise', 'violet', 'wheat', 'white', 'whitesmoke', 'yellow', 'yellowgreen'
]);
const CSS_NAMED_COLOR_PATTERN = new RegExp(
  `(?:^|[^\\w-])(${Array.from(CSS_NAMED_COLORS).join('|')})(?![\\w-])`,
  'gi'
);
const CSS_DECLARATION_PATTERN = /(?:^|[;{])\s*(?:--|\$)?[a-zA-Z_][a-zA-Z0-9_-]*\s*:\s*([^;{}]+)/gm;
const CSS_COLOR_ATTRIBUTE_PATTERN = new RegExp(
  `(?:color|fill|stroke|stop-color|flood-color|lighting-color)\\s*=\\s*['\"](${Array.from(CSS_NAMED_COLORS).join('|')})['\"]`,
  'gi'
);
const SCRIPT_COLOR_VALUE_PATTERN = new RegExp(
  `(?:color|backgroundColor|borderColor|outlineColor|caretColor|textDecorationColor|fill|stroke)\\s*:\\s*['\"\x60](${Array.from(CSS_NAMED_COLORS).join('|')})['\"\x60]`,
  'gi'
);

function normalizePath(value) {
  return value.split(path.sep).join('/');
}

function normalizeRelativePath(value) {
  return normalizePath(path.normalize(value)).replace(/^\.\//, '').replace(/\/$/, '');
}

function getPathValue(object, expression) {
  return expression.split('.').reduce((value, key) => value?.[key], object);
}

function pathIsWithinRoot(repositoryRoot, candidate) {
  const relative = path.relative(repositoryRoot, candidate);
  return relative !== '' && !relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative);
}

function resolveRegistryPath(repositoryRoot, relativePath, label, failures, { allowFile = true, allowDirectory = true } = {}) {
  if (typeof relativePath !== 'string' || relativePath.trim() === '' || path.isAbsolute(relativePath)) {
    failures.push(`${label} must be a non-empty repository-relative path.`);
    return null;
  }
  const resolved = path.resolve(repositoryRoot, relativePath);
  if (!pathIsWithinRoot(repositoryRoot, resolved)) {
    failures.push(`${label} escapes the repository root: ${relativePath}`);
    return null;
  }
  if (!fs.existsSync(resolved)) {
    failures.push(`${label} does not exist: ${relativePath}`);
    return null;
  }
  const stat = fs.statSync(resolved);
  if ((!allowFile && stat.isFile()) || (!allowDirectory && stat.isDirectory())) {
    failures.push(`${label} has the wrong path kind: ${relativePath}`);
  }
  return resolved;
}

export function validateRegistry(registry, { repositoryRoot = REPOSITORY_ROOT } = {}) {
  const failures = [];
  if (!registry || registry.version !== 1) {
    return ['frontend color surface registry must use version 1.'];
  }
  if (!registry.contracts || typeof registry.contracts !== 'object') {
    failures.push('registry.contracts must be an object.');
  } else {
    for (const [name, contractPath] of Object.entries(registry.contracts)) {
      resolveRegistryPath(repositoryRoot, contractPath, `contracts.${name}`, failures, { allowDirectory: false });
    }
  }
  if (!Array.isArray(registry.surfaces) || registry.surfaces.length === 0) {
    failures.push('registry.surfaces must be a non-empty array.');
    return failures;
  }

  const surfaceIds = new Set();
  for (const [index, surface] of registry.surfaces.entries()) {
    const label = `surfaces[${index}]`;
    if (!/^[a-z0-9-]+$/.test(surface?.id ?? '')) failures.push(`${label}.id must be kebab-case.`);
    if (surfaceIds.has(surface?.id)) failures.push(`${label}.id duplicates ${surface.id}.`);
    surfaceIds.add(surface?.id);
    if (typeof surface?.label !== 'string' || surface.label.trim() === '') failures.push(`${label}.label is required.`);
    if (!SURFACE_KINDS.has(surface?.kind)) failures.push(`${label}.kind is invalid: ${String(surface?.kind)}`);
    if (typeof surface?.owner !== 'string' || surface.owner.trim() === '') failures.push(`${label}.owner is required.`);

    const roots = surface?.roots ?? (surface?.root ? [surface.root] : []);
    if (!Array.isArray(roots) || roots.length === 0) failures.push(`${label} must declare root or roots.`);
    for (const [rootIndex, root] of roots.entries()) {
      resolveRegistryPath(repositoryRoot, root, `${label}.roots[${rootIndex}]`, failures, { allowFile: false });
    }

    if (surface.kind === 'contract-owner') {
      if (surface.audit !== undefined) failures.push(`${label} contract owners must not declare an audit engine.`);
      continue;
    }
    if (!surface.audit || !AUDIT_ENGINES.has(surface.audit.engine)) {
      failures.push(`${label}.audit.engine is required and must be registered.`);
      continue;
    }
    const expectedEngine = surface.kind === 'terminal'
      ? 'cli'
      : surface.kind === 'native-mobile'
        ? 'native'
        : surface.kind === 'miniapp'
          ? 'miniapp'
          : 'theme';
    if (surface.audit.engine !== expectedEngine) {
      failures.push(`${label}.audit.engine must be ${expectedEngine} for ${surface.kind}.`);
    }
    if (surface.audit.baseline) {
      resolveRegistryPath(repositoryRoot, surface.audit.baseline, `${label}.audit.baseline`, failures, { allowDirectory: false });
    }
    for (const [excludeIndex, excludePath] of (surface.audit.excludePaths ?? []).entries()) {
      resolveRegistryPath(
        repositoryRoot,
        path.join(surface.root, excludePath),
        `${label}.audit.excludePaths[${excludeIndex}]`,
        failures
      );
    }
    for (const [excludeIndex, excludeFile] of (surface.audit.excludeFiles ?? []).entries()) {
      resolveRegistryPath(
        repositoryRoot,
        path.join(surface.root, excludeFile),
        `${label}.audit.excludeFiles[${excludeIndex}]`,
        failures,
        { allowDirectory: false }
      );
    }
    for (const [ownerIndex, owner] of (surface.audit.rawColorOwners ?? []).entries()) {
      const ownerLabel = `${label}.audit.rawColorOwners[${ownerIndex}]`;
      if (!SPECIALIZED_COLOR_OWNER_KINDS.has(owner?.kind)) failures.push(`${ownerLabel}.kind is invalid.`);
      if (typeof owner?.reason !== 'string' || owner.reason.trim().length < 24) failures.push(`${ownerLabel}.reason must explain the narrow owner.`);
      const ownerFiles = [owner.file, ...(owner.files ?? []), ...(owner.pathPrefixes ?? [])].filter(Boolean);
      if (ownerFiles.length === 0) failures.push(`${ownerLabel} must declare file, files, or pathPrefixes.`);
      for (const [fileIndex, ownerFile] of ownerFiles.entries()) {
        resolveRegistryPath(
          repositoryRoot,
          path.join(surface.root, ownerFile),
          `${ownerLabel}.paths[${fileIndex}]`,
          failures
        );
      }
      if ((owner.startMarker && !owner.endMarker) || (!owner.startMarker && owner.endMarker)) {
        failures.push(`${ownerLabel} must declare both startMarker and endMarker.`);
      }
      if (owner.linePattern) {
        try {
          new RegExp(owner.linePattern);
        } catch (error) {
          failures.push(`${ownerLabel}.linePattern is invalid: ${error instanceof Error ? error.message : String(error)}`);
        }
      }
    }
    for (const [bundleIndex, bundle] of (surface.audit.generatedBundles ?? []).entries()) {
      const bundleLabel = `${label}.audit.generatedBundles[${bundleIndex}]`;
      resolveRegistryPath(repositoryRoot, path.join(surface.root, bundle.output), `${bundleLabel}.output`, failures, { allowDirectory: false });
      if (!Array.isArray(bundle.inputs) || bundle.inputs.length === 0) failures.push(`${bundleLabel}.inputs must be non-empty.`);
      for (const [inputIndex, input] of (bundle.inputs ?? []).entries()) {
        resolveRegistryPath(repositoryRoot, path.join(surface.root, input), `${bundleLabel}.inputs[${inputIndex}]`, failures, { allowDirectory: false });
      }
    }
  }

  const mirrorPaths = new Set();
  for (const [index, mirror] of (registry.mirrors ?? []).entries()) {
    const label = `mirrors[${index}]`;
    const surface = registry.surfaces.find(candidate => candidate.id === mirror?.surfaceId);
    if (!surface || surface.kind !== 'miniapp') failures.push(`${label}.surfaceId must name a MiniApp surface.`);
    if (mirrorPaths.has(mirror?.path)) failures.push(`${label}.path duplicates ${mirror.path}.`);
    mirrorPaths.add(mirror?.path);
    resolveRegistryPath(repositoryRoot, mirror?.path, `${label}.path`, failures, { allowFile: false });
  }

  const discoveryParents = registry.discovery?.miniappParents;
  if (!Array.isArray(discoveryParents) || discoveryParents.length === 0) {
    failures.push('registry.discovery.miniappParents must be non-empty.');
  } else {
    for (const [index, parent] of discoveryParents.entries()) {
      resolveRegistryPath(repositoryRoot, parent, `discovery.miniappParents[${index}]`, failures, { allowFile: false });
    }
  }

  const generatedCheckIds = new Set();
  for (const [index, check] of (registry.generatedChecks ?? []).entries()) {
    const label = `generatedChecks[${index}]`;
    if (!/^[a-z0-9-]+$/.test(check?.id ?? '')) failures.push(`${label}.id must be kebab-case.`);
    if (generatedCheckIds.has(check?.id)) failures.push(`${label}.id duplicates ${check.id}.`);
    generatedCheckIds.add(check?.id);
    if (!Array.isArray(check?.command) || check.command.length < 2 || check.command.some(part => typeof part !== 'string' || part === '')) {
      failures.push(`${label}.command must be a non-empty string array.`);
    }
    for (const surfaceId of check?.surfaceIds ?? []) {
      if (!surfaceIds.has(surfaceId)) failures.push(`${label}.surfaceIds references unknown surface ${surfaceId}.`);
    }
    for (const kind of check?.surfaceKinds ?? []) {
      if (!SURFACE_KINDS.has(kind)) failures.push(`${label}.surfaceKinds references unknown kind ${kind}.`);
    }
  }

  const exclusionIds = new Set();
  for (const [index, exclusion] of (registry.exclusions ?? []).entries()) {
    const label = `exclusions[${index}]`;
    if (!/^[a-z0-9-]+$/.test(exclusion?.id ?? '')) failures.push(`${label}.id must be kebab-case.`);
    if (exclusionIds.has(exclusion?.id)) failures.push(`${label}.id duplicates ${exclusion.id}.`);
    exclusionIds.add(exclusion?.id);
    resolveRegistryPath(repositoryRoot, exclusion?.path, `${label}.path`, failures);
    if (typeof exclusion?.kind !== 'string' || exclusion.kind.trim() === '') failures.push(`${label}.kind is required.`);
    if (typeof exclusion?.owner !== 'string' || exclusion.owner.trim() === '') failures.push(`${label}.owner is required.`);
    if (typeof exclusion?.reason !== 'string' || exclusion.reason.trim().length < 24) failures.push(`${label}.reason must explain the boundary.`);
  }
  return failures;
}

function walkFiles(root, { extensions, excludePaths = [], excludeFiles = [] } = {}) {
  const files = [];
  const normalizedExcludePaths = excludePaths.map(normalizeRelativePath);
  const normalizedExcludeFiles = new Set(excludeFiles.map(normalizeRelativePath));
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name);
      const relativePath = normalizePath(path.relative(root, fullPath));
      const excluded = normalizedExcludeFiles.has(relativePath) || normalizedExcludePaths.some(prefix => (
        relativePath === prefix || relativePath.startsWith(`${prefix}/`)
      ));
      if (excluded) continue;
      if (entry.isDirectory()) {
        if (entry.name === 'node_modules' || entry.name === '.git') continue;
        stack.push(fullPath);
      } else if (entry.isFile() && (!extensions || extensions.has(path.extname(entry.name).toLowerCase()))) {
        files.push(fullPath);
      }
    }
  }
  return files.sort((left, right) => left.localeCompare(right));
}

function addFinding(findings, seen, { file, content, index, value, type }) {
  const key = `${file}:${index}:${type}:${value.toLowerCase()}`;
  if (seen.has(key)) return;
  seen.add(key);
  const line = content.slice(0, index).split(/\r?\n/).length;
  const lineText = content.split(/\r?\n/)[line - 1] ?? '';
  findings.push({ file, index, line, lineText, type, value });
}

function collectRawColorFindings(relativePath, content) {
  const findings = [];
  const seen = new Set();
  for (const pattern of [HEX_COLOR_PATTERN, FUNCTION_COLOR_PATTERN]) {
    pattern.lastIndex = 0;
    for (const match of content.matchAll(pattern)) {
      addFinding(findings, seen, {
        file: relativePath,
        content,
        index: match.index,
        value: match[0],
        type: match[0].startsWith('#') ? 'hex' : 'color-function'
      });
    }
  }

  const extension = path.extname(relativePath).toLowerCase();
  if (['.css', '.html', '.scss', '.svg'].includes(extension)) {
    CSS_DECLARATION_PATTERN.lastIndex = 0;
    for (const declaration of content.matchAll(CSS_DECLARATION_PATTERN)) {
      const value = declaration[1].replace(/url\((?:\"[^\"]*\"|'[^']*'|[^)]*)\)/gi, ' ');
      CSS_NAMED_COLOR_PATTERN.lastIndex = 0;
      for (const match of value.matchAll(CSS_NAMED_COLOR_PATTERN)) {
        addFinding(findings, seen, {
          file: relativePath,
          content,
          index: declaration.index + declaration[0].indexOf(declaration[1]) + match.index + match[0].lastIndexOf(match[1]),
          value: match[1],
          type: 'named-color'
        });
      }
    }
    CSS_COLOR_ATTRIBUTE_PATTERN.lastIndex = 0;
    for (const match of content.matchAll(CSS_COLOR_ATTRIBUTE_PATTERN)) {
      addFinding(findings, seen, {
        file: relativePath,
        content,
        index: match.index + match[0].lastIndexOf(match[1]),
        value: match[1],
        type: 'named-color'
      });
    }
  } else {
    SCRIPT_COLOR_VALUE_PATTERN.lastIndex = 0;
    for (const match of content.matchAll(SCRIPT_COLOR_VALUE_PATTERN)) {
      addFinding(findings, seen, {
        file: relativePath,
        content,
        index: match.index + match[0].lastIndexOf(match[1]),
        value: match[1],
        type: 'named-color'
      });
    }
  }
  return findings.sort((left, right) => left.index - right.index);
}

function ownerMatchesFinding(owner, finding, content) {
  const ownerPaths = [owner.file, ...(owner.files ?? []), ...(owner.pathPrefixes ?? [])]
    .filter(Boolean)
    .map(normalizeRelativePath);
  const pathMatches = ownerPaths.some(ownerPath => (
    finding.file === ownerPath || finding.file.startsWith(`${ownerPath}/`)
  ));
  if (!pathMatches) return false;
  if (owner.startMarker) {
    const start = content.indexOf(owner.startMarker);
    const end = start < 0 ? -1 : content.indexOf(owner.endMarker, start + owner.startMarker.length);
    if (start < 0 || end < 0 || finding.index < start || finding.index > end + owner.endMarker.length) return false;
  }
  if (owner.linePattern && !new RegExp(owner.linePattern).test(finding.lineText)) return false;
  return true;
}

function normalizeLineEndings(content) {
  return content.replace(/\r\n?/g, '\n');
}

function checkGeneratedBundles(surface, repositoryRoot) {
  const failures = [];
  for (const bundle of surface.audit.generatedBundles ?? []) {
    const outputRelative = normalizeRelativePath(bundle.output);
    const outputDirectory = path.posix.dirname(outputRelative);
    const expected = bundle.inputs.map((input) => {
      const normalizedInput = normalizeRelativePath(input);
      const label = path.posix.relative(outputDirectory, normalizedInput);
      const content = fs.readFileSync(path.join(repositoryRoot, surface.root, normalizedInput), 'utf8');
      return `/* ${label} */\n${content}\n`;
    }).join('');
    const actual = fs.readFileSync(path.join(repositoryRoot, surface.root, outputRelative), 'utf8');
    if (normalizeLineEndings(actual) !== normalizeLineEndings(expected)) {
      failures.push(`${surface.id} generated bundle ${bundle.output} is stale; run its source/build.js.`);
    }
  }
  return failures;
}

export function auditMiniappSurface(surface, contract, { repositoryRoot = REPOSITORY_ROOT } = {}) {
  const root = path.join(repositoryRoot, surface.root);
  const files = walkFiles(root, {
    extensions: MINIAPP_SOURCE_EXTENSIONS,
    excludePaths: surface.audit.excludePaths ?? [],
    excludeFiles: surface.audit.excludeFiles ?? []
  });
  const allowedHostVariables = new Set(contract.variables.map(variable => variable.name));
  const rawOwnerMatches = new Map((surface.audit.rawColorOwners ?? []).map((owner, index) => [index, 0]));
  const failures = [];
  const rawFindings = [];
  const hostVariables = new Set();
  let hostVariableOccurrences = 0;

  for (const file of files) {
    const relativePath = normalizePath(path.relative(root, file));
    const content = fs.readFileSync(file, 'utf8');
    for (const finding of collectRawColorFindings(relativePath, content)) {
      const ownerIndex = (surface.audit.rawColorOwners ?? []).findIndex(owner => ownerMatchesFinding(owner, finding, content));
      if (ownerIndex < 0) {
        rawFindings.push(finding);
        failures.push(`${surface.id}:${finding.file}:${finding.line} has unowned ${finding.type} ${finding.value}.`);
      } else {
        rawOwnerMatches.set(ownerIndex, (rawOwnerMatches.get(ownerIndex) ?? 0) + 1);
      }
    }

    HOST_VARIABLE_PATTERN.lastIndex = 0;
    for (const match of content.matchAll(HOST_VARIABLE_PATTERN)) {
      hostVariables.add(match[0]);
      hostVariableOccurrences += 1;
      if (!allowedHostVariables.has(match[0])) {
        const line = content.slice(0, match.index).split(/\r?\n/).length;
        failures.push(`${surface.id}:${relativePath}:${line} uses unregistered MiniApp appearance variable ${match[0]}.`);
      }
    }
    HOST_VARIABLE_FALLBACK_PATTERN.lastIndex = 0;
    for (const match of content.matchAll(HOST_VARIABLE_FALLBACK_PATTERN)) {
      const line = content.slice(0, match.index).split(/\r?\n/).length;
      failures.push(`${surface.id}:${relativePath}:${line} adds a fallback to public host variable ${match[1]}.`);
    }
    HOST_VARIABLE_DEFINITION_PATTERN.lastIndex = 0;
    for (const match of content.matchAll(HOST_VARIABLE_DEFINITION_PATTERN)) {
      const line = content.slice(0, match.index).split(/\r?\n/).length;
      failures.push(`${surface.id}:${relativePath}:${line} redefines host-owned MiniApp variable ${match[1]}.`);
    }
  }

  for (const [ownerIndex, count] of rawOwnerMatches) {
    if (count === 0) {
      const owner = surface.audit.rawColorOwners[ownerIndex];
      failures.push(`${surface.id} specialized owner ${owner.kind} is stale; remove or narrow the registry entry.`);
    }
  }
  failures.push(...checkGeneratedBundles(surface, repositoryRoot));
  return {
    surfaceId: surface.id,
    engine: 'miniapp',
    filesScanned: files.length,
    hostVariableOccurrences,
    hostVariables: Array.from(hostVariables).sort(),
    unownedRawColors: rawFindings,
    specializedOwners: (surface.audit.rawColorOwners ?? []).map((owner, index) => ({
      kind: owner.kind,
      occurrences: rawOwnerMatches.get(index) ?? 0
    })),
    failures
  };
}

const NATIVE_PATTERNS = {
  android: [
    { type: 'named-native-color', pattern: /\bColor\.(?:Black|White|Red|Green|Blue|Yellow|Gray|Magenta|Cyan|Transparent)\b/g },
    { type: 'native-color-constructor', pattern: /\bColor\s*\(\s*0x[0-9a-fA-F_]+\s*\)/g },
    { type: 'native-color-function', pattern: /\b(?:android\.graphics\.)?Color\.(?:argb|rgb|parseColor)\s*\(/g },
    { type: 'hex-string', pattern: /['\"]#[0-9a-fA-F]{3,8}\b/g }
  ],
  ios: [
    { type: 'named-native-color', pattern: /(?<![a-zA-Z0-9_])\.(?:white|black|clear|red|green|blue|gray|orange|yellow|purple|pink)\b/g },
    { type: 'native-color-constructor', pattern: /\b(?:Color|UIColor)\s*\(\s*(?:red|white|hue|displayP3Red)\s*:/g },
    { type: 'hex-string', pattern: /['\"]#[0-9a-fA-F]{3,8}\b/g }
  ],
  harmonyos: [
    { type: 'named-native-color', pattern: /\bColor\.(?:Black|White|Red|Green|Blue|Yellow|Gray|Transparent)\b/g },
    { type: 'color-string', pattern: /['\"]#[0-9a-fA-F]{3,8}\b/g },
    { type: 'color-function', pattern: /['\"](?:rgba?|hsla?)\([^'\"]+\)['\"]/gi },
    { type: 'numeric-ui-color', pattern: /(?:backgroundColor|fontColor|statusBarColor|navigationBarColor|color)\s*:\s*0x[0-9a-fA-F]{6,8}\b/g }
  ]
};

export function auditNativeSurface(surface, { repositoryRoot = REPOSITORY_ROOT } = {}) {
  const root = path.join(repositoryRoot, surface.root);
  const files = walkFiles(root, {
    extensions: new Set(surface.audit.extensions),
    excludePaths: surface.audit.excludePaths ?? [],
    excludeFiles: surface.audit.excludeFiles ?? []
  });
  const findings = [];
  for (const file of files) {
    const relativePath = normalizePath(path.relative(root, file));
    const content = fs.readFileSync(file, 'utf8');
    for (const { type, pattern } of NATIVE_PATTERNS[surface.audit.platform] ?? []) {
      pattern.lastIndex = 0;
      for (const match of content.matchAll(pattern)) {
        findings.push({
          file: relativePath,
          line: content.slice(0, match.index).split(/\r?\n/).length,
          type,
          value: match[0]
        });
      }
    }
  }
  const failures = findings.map(finding => (
    `${surface.id}:${finding.file}:${finding.line} has raw native color ${finding.value}.`
  ));
  if (files.length === 0) failures.push(`${surface.id} scanned no native source files.`);
  return {
    surfaceId: surface.id,
    engine: 'native',
    platform: surface.audit.platform,
    filesScanned: files.length,
    rawColorOccurrences: findings.length,
    findings,
    failures
  };
}

function listTreeFiles(root) {
  return walkFiles(root).map(file => normalizePath(path.relative(root, file)));
}

export function compareMirrorTrees(sourceRoot, mirrorRoot) {
  const sourceFiles = listTreeFiles(sourceRoot);
  const mirrorFiles = listTreeFiles(mirrorRoot);
  const sourceSet = new Set(sourceFiles);
  const mirrorSet = new Set(mirrorFiles);
  const failures = [];
  for (const file of sourceFiles) {
    if (!mirrorSet.has(file)) {
      failures.push(`mirror is missing ${file}`);
      continue;
    }
    const source = fs.readFileSync(path.join(sourceRoot, file));
    const mirror = fs.readFileSync(path.join(mirrorRoot, file));
    if (!source.equals(mirror)) failures.push(`mirror differs at ${file}`);
  }
  for (const file of mirrorFiles) {
    if (!sourceSet.has(file)) failures.push(`mirror has extra file ${file}`);
  }
  return failures;
}

export function discoverMiniappRoots(registry, { repositoryRoot = REPOSITORY_ROOT } = {}) {
  const discovered = [];
  for (const parentPath of registry.discovery.miniappParents) {
    const parent = path.join(repositoryRoot, parentPath);
    for (const entry of fs.readdirSync(parent, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const appRoot = path.join(parent, entry.name);
      if (fs.existsSync(path.join(appRoot, 'meta.json'))) {
        discovered.push(normalizePath(path.relative(repositoryRoot, appRoot)));
      }
    }
  }
  return discovered.sort();
}

function auditMiniappDiscovery(registry, repositoryRoot) {
  const registered = new Set([
    ...registry.surfaces.filter(surface => surface.kind === 'miniapp').map(surface => normalizeRelativePath(surface.root)),
    ...(registry.mirrors ?? []).map(mirror => normalizeRelativePath(mirror.path))
  ]);
  const discovered = discoverMiniappRoots(registry, { repositoryRoot });
  const discoveredSet = new Set(discovered);
  const failures = [];
  for (const root of discovered) {
    if (!registered.has(root)) failures.push(`Unregistered MiniApp discovered at ${root}.`);
  }
  for (const root of registered) {
    if (!discoveredSet.has(root)) failures.push(`Registered MiniApp path is not discoverable: ${root}.`);
  }
  return { discovered, registered: Array.from(registered).sort(), failures };
}

function runSubprocess(command, args, repositoryRoot) {
  return spawnSync(command, args, {
    cwd: repositoryRoot,
    encoding: 'utf8',
    env: process.env,
    maxBuffer: 64 * 1024 * 1024
  });
}

function parseJsonOutput(result, label) {
  if (!result.stdout?.trim()) {
    return { report: null, failures: [`${label} returned no JSON output. ${result.stderr?.trim() ?? ''}`.trim()] };
  }
  try {
    return { report: JSON.parse(result.stdout), failures: [] };
  } catch (error) {
    return {
      report: null,
      failures: [`${label} returned invalid JSON: ${error instanceof Error ? error.message : String(error)}`]
    };
  }
}

function auditThemeSurface(surface, repositoryRoot) {
  const args = [THEME_AUDITOR_PATH, '--root', surface.root, '--json', '--top', '0'];
  if (surface.audit.baseline) args.push('--baseline', surface.audit.baseline);
  else args.push('--no-baseline');
  for (const packageName of surface.audit.packageContracts ?? []) args.push('--package-contract', packageName);
  for (const excludePath of surface.audit.excludePaths ?? []) args.push('--exclude', excludePath);
  const result = runSubprocess(process.execPath, args, repositoryRoot);
  const parsed = parseJsonOutput(result, `${surface.id} theme audit`);
  const failures = [...parsed.failures];
  if (result.status !== 0) {
    failures.push(`${surface.id} theme audit failed: ${result.stderr?.trim() || `exit ${result.status}`}`);
  }
  if (parsed.report?.filesScanned === 0) failures.push(`${surface.id} theme audit scanned no files.`);
  if (surface.audit.policy === 'canonical-ui-zero' && parsed.report) {
    for (const metric of CANONICAL_ZERO_METRICS) {
      const actual = getPathValue(parsed.report, metric);
      if (typeof actual !== 'number') failures.push(`${surface.id} canonical policy references missing metric ${metric}.`);
      else if (actual !== 0) failures.push(`${surface.id} ${metric} must be 0, found ${actual}.`);
    }
  }
  return { surfaceId: surface.id, engine: 'theme', report: parsed.report, failures };
}

function auditCliSurface(surface, repositoryRoot) {
  const args = [CLI_AUDITOR_PATH, '--root', surface.root, '--json'];
  if (surface.audit.baseline) args.push('--baseline', surface.audit.baseline);
  else args.push('--no-baseline');
  const result = runSubprocess(process.execPath, args, repositoryRoot);
  const parsed = parseJsonOutput(result, `${surface.id} CLI audit`);
  const failures = [...parsed.failures];
  if (result.status !== 0) failures.push(`${surface.id} CLI audit failed: ${result.stderr?.trim() || `exit ${result.status}`}`);
  return { surfaceId: surface.id, engine: 'cli', report: parsed.report, failures };
}

function generatedCheckApplies(check, selectedSurfaces) {
  const selectedIds = new Set(selectedSurfaces.map(surface => surface.id));
  const selectedKinds = new Set(selectedSurfaces.map(surface => surface.kind));
  return (check.surfaceIds ?? []).some(id => selectedIds.has(id))
    || (check.surfaceKinds ?? []).some(kind => selectedKinds.has(kind));
}

function runGeneratedCheck(check, repositoryRoot) {
  const [executable, ...args] = check.command;
  const command = executable === 'node' ? process.execPath : executable;
  const result = runSubprocess(command, args, repositoryRoot);
  return {
    id: check.id,
    status: result.status,
    output: [result.stdout, result.stderr].filter(Boolean).join('\n').trim(),
    failures: result.status === 0
      ? []
      : [`Generated artifact check ${check.id} failed: ${[result.stdout, result.stderr].filter(Boolean).join('\n').trim()}`]
  };
}

export function loadRegistry(registryPath = DEFAULT_REGISTRY_PATH) {
  return JSON.parse(fs.readFileSync(registryPath, 'utf8'));
}

export function runFrontendColorAudit({
  registry,
  repositoryRoot = REPOSITORY_ROOT,
  surfaceIds = [],
  surfaceKinds = [],
  runGeneratedChecks = true
}) {
  const failures = validateRegistry(registry, { repositoryRoot });
  const requestedIds = new Set(surfaceIds);
  const requestedKinds = new Set(surfaceKinds);
  const hasFilter = requestedIds.size > 0 || requestedKinds.size > 0;
  for (const id of requestedIds) {
    if (!registry.surfaces.some(surface => surface.id === id)) failures.push(`Unknown frontend color surface: ${id}.`);
  }
  for (const kind of requestedKinds) {
    if (!SURFACE_KINDS.has(kind)) failures.push(`Unknown frontend color surface kind: ${kind}.`);
  }
  if (failures.length > 0) {
    return { registryVersion: registry.version, surfaces: [], discovery: null, mirrors: [], generatedChecks: [], failures };
  }

  const selectedSurfaces = registry.surfaces.filter(surface => (
    !hasFilter || requestedIds.has(surface.id) || requestedKinds.has(surface.kind)
  ));
  const miniappContract = JSON.parse(fs.readFileSync(path.join(repositoryRoot, registry.contracts.miniappAppearance), 'utf8'));
  const surfaceReports = [];
  for (const surface of selectedSurfaces) {
    let report;
    if (surface.kind === 'contract-owner') {
      report = { surfaceId: surface.id, engine: 'contract-owner', roots: surface.roots, failures: [] };
    } else if (surface.audit.engine === 'theme') {
      report = auditThemeSurface(surface, repositoryRoot);
    } else if (surface.audit.engine === 'cli') {
      report = auditCliSurface(surface, repositoryRoot);
    } else if (surface.audit.engine === 'native') {
      report = auditNativeSurface(surface, { repositoryRoot });
    } else {
      report = auditMiniappSurface(surface, miniappContract, { repositoryRoot });
    }
    surfaceReports.push(report);
    failures.push(...report.failures);
  }

  const selectedMiniapps = selectedSurfaces.filter(surface => surface.kind === 'miniapp');
  const shouldAuditAllMiniappDiscovery = !hasFilter || requestedKinds.has('miniapp');
  const discovery = shouldAuditAllMiniappDiscovery ? auditMiniappDiscovery(registry, repositoryRoot) : null;
  if (discovery) failures.push(...discovery.failures);

  const selectedMiniappIds = new Set(selectedMiniapps.map(surface => surface.id));
  const mirrorReports = (registry.mirrors ?? [])
    .filter(mirror => !hasFilter || selectedMiniappIds.has(mirror.surfaceId) || requestedKinds.has('miniapp'))
    .map((mirror) => {
      const surface = registry.surfaces.find(candidate => candidate.id === mirror.surfaceId);
      const mirrorFailures = compareMirrorTrees(
        path.join(repositoryRoot, surface.root),
        path.join(repositoryRoot, mirror.path)
      ).map(failure => `${mirror.surfaceId} reference ${mirror.path}: ${failure}.`);
      failures.push(...mirrorFailures);
      return { surfaceId: mirror.surfaceId, path: mirror.path, failures: mirrorFailures };
    });

  const generatedReports = runGeneratedChecks
    ? (registry.generatedChecks ?? [])
      .filter(check => generatedCheckApplies(check, selectedSurfaces))
      .map((check) => {
        const report = runGeneratedCheck(check, repositoryRoot);
        failures.push(...report.failures);
        return report;
      })
    : [];
  return {
    registryVersion: registry.version,
    selectedSurfaceIds: selectedSurfaces.map(surface => surface.id),
    surfaces: surfaceReports,
    discovery,
    mirrors: mirrorReports,
    generatedChecks: generatedReports,
    failures
  };
}

function parseArgs(argv) {
  const options = {
    registryPath: DEFAULT_REGISTRY_PATH,
    surfaceIds: [],
    surfaceKinds: [],
    json: false,
    list: false,
    runGeneratedChecks: true
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--registry') options.registryPath = path.resolve(argv[++index] ?? '');
    else if (arg.startsWith('--registry=')) options.registryPath = path.resolve(arg.slice('--registry='.length));
    else if (arg === '--surface') options.surfaceIds.push(argv[++index] ?? '');
    else if (arg.startsWith('--surface=')) options.surfaceIds.push(arg.slice('--surface='.length));
    else if (arg === '--kind') options.surfaceKinds.push(argv[++index] ?? '');
    else if (arg.startsWith('--kind=')) options.surfaceKinds.push(arg.slice('--kind='.length));
    else if (arg === '--json') options.json = true;
    else if (arg === '--list') options.list = true;
    else if (arg === '--skip-generated-checks') options.runGeneratedChecks = false;
    else if (arg === '--help' || arg === '-h') {
      console.log('Usage: node scripts/audit-frontend-colors.mjs [--surface <id>] [--kind <kind>] [--json] [--list]');
      process.exit(0);
    } else throw new Error(`Unknown argument: ${arg}`);
  }
  return options;
}

function formatSurfaceSummary(surfaceReport) {
  if (surfaceReport.engine === 'theme') {
    const report = surfaceReport.report;
    return `${report?.filesScanned ?? 0} files, app raw=${report?.colorScopes?.appUi?.occurrences ?? '?'}, fallbacks=${report?.fallbackOccurrences ?? '?'}`;
  }
  if (surfaceReport.engine === 'cli') {
    return `${surfaceReport.report?.presetFiles ?? 0} presets, runtime colors=${surfaceReport.report?.runtimePresetUniqueColors ?? '?'}`;
  }
  if (surfaceReport.engine === 'native') {
    return `${surfaceReport.filesScanned} files, raw=${surfaceReport.rawColorOccurrences}`;
  }
  if (surfaceReport.engine === 'miniapp') {
    const specialized = surfaceReport.specializedOwners.map(owner => `${owner.kind}:${owner.occurrences}`).join(', ') || 'none';
    return `${surfaceReport.filesScanned} files, host vars=${surfaceReport.hostVariableOccurrences}, specialized=${specialized}`;
  }
  return `${surfaceReport.roots.length} owner roots`;
}

function formatGitHubCommandValue(value) {
  return value.replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const registry = loadRegistry(options.registryPath);
  if (options.list) {
    for (const surface of registry.surfaces) console.log(`${surface.id}\t${surface.kind}\t${surface.label}`);
    return;
  }
  const report = runFrontendColorAudit({
    registry,
    repositoryRoot: REPOSITORY_ROOT,
    surfaceIds: options.surfaceIds,
    surfaceKinds: options.surfaceKinds,
    runGeneratedChecks: options.runGeneratedChecks
  });
  if (options.json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    for (const surfaceReport of report.surfaces) {
      const mark = surfaceReport.failures.length === 0 ? 'ok' : 'failed';
      console.log(`[frontend-colors] ${mark} ${surfaceReport.surfaceId}: ${formatSurfaceSummary(surfaceReport)}`);
    }
    if (report.discovery) {
      console.log(`[frontend-colors] ${report.discovery.failures.length === 0 ? 'ok' : 'failed'} MiniApp discovery: ${report.discovery.discovered.length} registered roots`);
    }
    for (const mirror of report.mirrors) {
      console.log(`[frontend-colors] ${mirror.failures.length === 0 ? 'ok' : 'failed'} mirror ${mirror.surfaceId}`);
    }
    for (const check of report.generatedChecks) {
      console.log(`[frontend-colors] ${check.failures.length === 0 ? 'ok' : 'failed'} generated ${check.id}`);
    }
    if (report.failures.length > 0) {
      console.error('\nFrontend color governance failures:');
      for (const failure of report.failures) {
        console.error(`- ${failure}`);
        if (process.env.GITHUB_ACTIONS === 'true') {
          console.error(`::error title=Frontend color governance::${formatGitHubCommandValue(failure)}`);
        }
      }
    } else {
      console.log(`[frontend-colors] all ${report.selectedSurfaceIds.length} selected surfaces passed.`);
    }
  }
  if (report.failures.length > 0) process.exitCode = 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_PATH) {
  main();
}
