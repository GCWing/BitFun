#!/usr/bin/env node

import { readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { pathToFileURL } from 'node:url';

const repositoryRoot = path.resolve(import.meta.dirname, '..');
const defaultSourceRoot = path.join(repositoryRoot, 'src/web-ui/src');
const defaultBaselinePath = path.join(
  repositoryRoot,
  'scripts/design-system-adoption-baseline.json',
);
const sourceExtensions = new Set(['.js', '.jsx', '.ts', '.tsx']);
const nativeControlTags = ['button', 'input', 'select', 'textarea'];
const excludedDirectories = new Set([
  path.join(defaultSourceRoot, 'component-library'),
  path.join(defaultSourceRoot, 'generated'),
]);

function sortObject(record) {
  return Object.fromEntries(
    Object.entries(record).sort(([left], [right]) => left.localeCompare(right)),
  );
}

function stripComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, comment => comment.replace(/[^\n]/g, ' '))
    .replace(/(^|\s)\/\/[^\n]*/g, comment => comment.replace(/[^\n]/g, ' '));
}

function collectModuleSpecifiers(source) {
  const expressions = [
    /\bfrom\s*['"]([^'"]+)['"]/g,
    /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g,
    /\bimport\s*['"]([^'"]+)['"]/g,
    /\b(?:vi|jest)\.mock\(\s*['"]([^'"]+)['"]/g,
  ];
  return expressions.flatMap(expression =>
    [...source.matchAll(expression)].map(match => match[1])
  );
}

function isLegacySpecifier(specifier) {
  return specifier === '@/component-library'
    || specifier.startsWith('@/component-library/')
    || specifier.startsWith('@components/')
    || /(?:^|\/)component-library(?:\/|$)/.test(specifier);
}

function collectNamedLegacyImports(source) {
  const symbols = [];
  const expression = /\b(?:import|export)\s+(?:type\s+)?\{([^}]+)\}\s+from\s*['"]([^'"]+)['"]/g;

  for (const match of source.matchAll(expression)) {
    if (!isLegacySpecifier(match[2])) continue;
    for (const item of match[1].split(',')) {
      const symbol = item
        .trim()
        .replace(/^type\s+/, '')
        .split(/\s+as\s+/)[0]
        .trim();
      if (symbol) symbols.push(symbol);
    }
  }

  return symbols;
}

function countNativeControls(source) {
  const counts = {};
  for (const tag of nativeControlTags) {
    const count = [...source.matchAll(new RegExp(`<${tag}\\b`, 'g'))].length;
    if (count > 0) counts[tag] = count;
  }
  return counts;
}

export function analyzeSourceEntries(entries, { nativeControlAllowlist = [] } = {}) {
  const allowedNativeFiles = new Set(nativeControlAllowlist);
  const legacyByModule = {};
  const legacyBySymbol = {};
  const nativeByFile = {};
  let legacyModuleReferences = 0;
  let legacyAliasReferences = 0;
  let newLibraryModuleReferences = 0;

  for (const [file, originalSource] of entries) {
    const source = stripComments(originalSource);
    const specifiers = collectModuleSpecifiers(source);

    for (const specifier of specifiers) {
      if (specifier === '@bitfun/ui' || specifier.startsWith('@bitfun/ui/')) {
        newLibraryModuleReferences += 1;
      }
      if (!isLegacySpecifier(specifier)) continue;

      legacyModuleReferences += 1;
      legacyByModule[specifier] = (legacyByModule[specifier] ?? 0) + 1;
      if (specifier.startsWith('@components/')) legacyAliasReferences += 1;
    }

    for (const symbol of collectNamedLegacyImports(source)) {
      legacyBySymbol[symbol] = (legacyBySymbol[symbol] ?? 0) + 1;
    }

    if (path.extname(file).toLowerCase() === '.tsx' && !allowedNativeFiles.has(file)) {
      const counts = countNativeControls(source);
      if (Object.keys(counts).length > 0) nativeByFile[file] = counts;
    }
  }

  return {
    schemaVersion: 1,
    legacy: {
      aliasReferences: legacyAliasReferences,
      byModule: sortObject(legacyByModule),
      bySymbol: sortObject(legacyBySymbol),
      moduleReferences: legacyModuleReferences,
    },
    nativeControls: {
      allowlist: [...nativeControlAllowlist].sort(),
      byFile: sortObject(nativeByFile),
      total: Object.values(nativeByFile).reduce(
        (total, counts) => total + Object.values(counts).reduce((sum, count) => sum + count, 0),
        0,
      ),
    },
    newLibrary: {
      moduleReferences: newLibraryModuleReferences,
    },
  };
}

function debtDimensions(inventory) {
  const dimensions = new Map([
    ['legacy.moduleReferences', inventory.legacy.moduleReferences],
    ['legacy.aliasReferences', inventory.legacy.aliasReferences],
  ]);

  for (const [specifier, count] of Object.entries(inventory.legacy.byModule)) {
    dimensions.set(`legacy.byModule.${specifier}`, count);
  }
  for (const [symbol, count] of Object.entries(inventory.legacy.bySymbol)) {
    dimensions.set(`legacy.bySymbol.${symbol}`, count);
  }
  for (const [file, counts] of Object.entries(inventory.nativeControls.byFile)) {
    for (const [tag, count] of Object.entries(counts)) {
      dimensions.set(`nativeControls.byFile.${file}.<${tag}>`, count);
    }
  }

  return dimensions;
}

export function compareInventories(current, baseline) {
  const currentDimensions = debtDimensions(current);
  const baselineDimensions = debtDimensions(baseline);
  const keys = new Set([...currentDimensions.keys(), ...baselineDimensions.keys()]);
  const increases = [];
  const reductions = [];

  for (const key of [...keys].sort()) {
    const actual = currentDimensions.get(key) ?? 0;
    const allowed = baselineDimensions.get(key) ?? 0;
    if (actual > allowed) increases.push({ actual, allowed, key });
    if (actual < allowed) reductions.push({ actual, allowed, key });
  }

  return { increases, reductions };
}

async function collectSourceEntries(sourceRoot) {
  async function visit(directory) {
    const entries = await readdir(directory, { withFileTypes: true });
    const nested = await Promise.all(entries.map(async entry => {
      const absolutePath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        return excludedDirectories.has(absolutePath) ? [] : visit(absolutePath);
      }
      if (!sourceExtensions.has(path.extname(entry.name).toLowerCase())) return [];
      const relativePath = path.relative(repositoryRoot, absolutePath).replaceAll('\\', '/');
      return [[relativePath, await readFile(absolutePath, 'utf8')]];
    }));
    return nested.flat();
  }

  return visit(sourceRoot);
}

function printInventory(inventory) {
  const topSymbols = Object.entries(inventory.legacy.bySymbol)
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, 12);

  console.log('BitFun design-system adoption inventory');
  console.log(`Legacy module references: ${inventory.legacy.moduleReferences}`);
  console.log(`Legacy @components alias references: ${inventory.legacy.aliasReferences}`);
  console.log(`@bitfun/ui module references: ${inventory.newLibrary.moduleReferences}`);
  console.log(`Independent native control elements: ${inventory.nativeControls.total}`);
  if (topSymbols.length > 0) {
    console.log('Most referenced legacy symbols:');
    for (const [symbol, count] of topSymbols) console.log(`  ${symbol}: ${count}`);
  }
}

async function main() {
  const updateBaseline = process.argv.includes('--update-baseline');
  const baselineArgumentIndex = process.argv.indexOf('--baseline');
  const baselinePath = baselineArgumentIndex >= 0
    ? path.resolve(repositoryRoot, process.argv[baselineArgumentIndex + 1])
    : defaultBaselinePath;
  let baseline;

  try {
    baseline = JSON.parse(await readFile(baselinePath, 'utf8'));
  } catch (error) {
    if (!updateBaseline || error?.code !== 'ENOENT') throw error;
  }

  const allowlist = baseline?.nativeControls?.allowlist ?? [];
  const current = analyzeSourceEntries(await collectSourceEntries(defaultSourceRoot), {
    nativeControlAllowlist: allowlist,
  });
  printInventory(current);

  if (!baseline) {
    await writeFile(baselinePath, `${JSON.stringify(current, null, 2)}\n`);
    console.log(`Created adoption baseline at ${path.relative(repositoryRoot, baselinePath)}.`);
    return;
  }

  if (baseline.schemaVersion !== current.schemaVersion) {
    throw new Error(`Unsupported adoption baseline schema: ${baseline.schemaVersion}.`);
  }

  const comparison = compareInventories(current, baseline);
  if (comparison.increases.length > 0) {
    console.error('\nDesign-system adoption debt increased:');
    for (const item of comparison.increases.slice(0, 80)) {
      console.error(`  ${item.key}: ${item.actual} (baseline ${item.allowed})`);
    }
    if (comparison.increases.length > 80) {
      console.error(`  ... ${comparison.increases.length - 80} more`);
    }
    console.error('\nUse @bitfun/ui or an owning product composition. Do not raise the baseline.');
    process.exitCode = 1;
    return;
  }

  if (updateBaseline) {
    await writeFile(baselinePath, `${JSON.stringify(current, null, 2)}\n`);
    console.log(`Ratchet baseline lowered at ${path.relative(repositoryRoot, baselinePath)}.`);
    return;
  }

  if (comparison.reductions.length > 0) {
    console.log(`\nDebt is below baseline in ${comparison.reductions.length} dimensions.`);
    console.log('Run pnpm run design-system:adoption:update after the migration is verified.');
  } else {
    console.log('\nDesign-system adoption debt did not increase.');
  }
}

const isMainModule = process.argv[1]
  && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url;

if (isMainModule) {
  await main();
}
