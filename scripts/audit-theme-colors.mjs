#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const DEFAULT_ROOT = 'src/web-ui/src';
const COLOR_EXTENSIONS = new Set(['.css', '.scss', '.sass', '.ts', '.tsx', '.js', '.jsx']);
const TOKEN_PATH_PARTS = [
  'component-library/styles',
  'infrastructure/theme',
  'theme/presets',
];
const EXCEPTION_PATH_PARTS = [
  'monaco',
  'terminal',
  'mermaid',
  'syntax',
  'CodeEditor',
];

const COLOR_PATTERN =
  /#[0-9a-fA-F]{3,8}\b|rgba?\(\s*[-+]?\d*\.?\d+\s*,\s*[-+]?\d*\.?\d+\s*,\s*[-+]?\d*\.?\d+(?:\s*,\s*(?:[-+]?\d*\.?\d+|var\([^)]+\)))?\s*\)|hsla?\(\s*[-+]?\d*\.?\d+(?:deg|rad|turn)?\s*,\s*[-+]?\d*\.?\d+%\s*,\s*[-+]?\d*\.?\d+%(?:\s*,\s*(?:[-+]?\d*\.?\d+|var\([^)]+\)))?\s*\)/g;
const CSS_VAR_USAGE_PATTERN = /var\(\s*(--[a-zA-Z0-9_-]+)/g;
const CSS_VAR_DEFINITION_PATTERN = /(^|[;{\s])(--[a-zA-Z0-9_-]+)\s*:/g;
const VAR_FALLBACK_PATTERN = /var\(\s*(--[a-zA-Z0-9_-]+)\s*,/g;

function parseArgs(argv) {
  const options = {
    root: DEFAULT_ROOT,
    json: false,
    top: 15,
    budget: 120,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--json') {
      options.json = true;
    } else if (arg === '--root') {
      options.root = argv[++index] ?? DEFAULT_ROOT;
    } else if (arg === '--top') {
      options.top = Number(argv[++index] ?? options.top);
    } else if (arg === '--budget') {
      options.budget = Number(argv[++index] ?? options.budget);
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function printHelp() {
  console.log(`Usage: node scripts/audit-theme-colors.mjs [options]

Options:
  --root <path>     Directory to scan. Default: ${DEFAULT_ROOT}
  --top <number>    Number of top rows to print. Default: 15
  --budget <number> Unique app color budget for the summary. Default: 120
  --json            Print machine-readable JSON instead of text.
`);
}

function walkFiles(root) {
  const result = [];
  const stack = [root];

  while (stack.length > 0) {
    const current = stack.pop();
    const entries = fs.readdirSync(current, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === 'node_modules' || entry.name === 'dist' || entry.name === 'build') {
          continue;
        }
        stack.push(fullPath);
        continue;
      }
      if (entry.isFile() && COLOR_EXTENSIONS.has(path.extname(entry.name))) {
        result.push(fullPath);
      }
    }
  }

  return result.sort();
}

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function isTokenFile(relativePath) {
  return TOKEN_PATH_PARTS.some(part => relativePath.includes(part));
}

function isExceptionFile(relativePath) {
  return EXCEPTION_PATH_PARTS.some(part => relativePath.toLowerCase().includes(part.toLowerCase()));
}

function incrementMap(map, key, amount = 1) {
  map.set(key, (map.get(key) ?? 0) + amount);
}

function collectMatches(content, pattern) {
  pattern.lastIndex = 0;
  return Array.from(content.matchAll(pattern));
}

function parseColor(color) {
  const trimmed = color.trim().toLowerCase();
  const hex = /^#([0-9a-f]{3,8})$/.exec(trimmed);
  if (hex) {
    const raw = hex[1];
    const expanded = raw.length === 3 || raw.length === 4
      ? raw.split('').map(char => char + char).join('')
      : raw;
    const rgbHex = expanded.slice(0, 6);
    const alphaHex = expanded.length === 8 ? expanded.slice(6, 8) : null;
    return {
      r: parseInt(rgbHex.slice(0, 2), 16),
      g: parseInt(rgbHex.slice(2, 4), 16),
      b: parseInt(rgbHex.slice(4, 6), 16),
      a: alphaHex ? Math.round((parseInt(alphaHex, 16) / 255) * 1000) / 1000 : 1,
    };
  }

  const rgb = /^rgba?\(\s*([-+]?\d*\.?\d+)\s*,\s*([-+]?\d*\.?\d+)\s*,\s*([-+]?\d*\.?\d+)(?:\s*,\s*([-+]?\d*\.?\d+))?\s*\)$/.exec(trimmed);
  if (rgb) {
    return {
      r: Number(rgb[1]),
      g: Number(rgb[2]),
      b: Number(rgb[3]),
      a: rgb[4] === undefined ? 1 : Number(rgb[4]),
    };
  }

  return null;
}

function colorDistance(a, b) {
  return Math.sqrt(
    (a.r - b.r) ** 2 +
    (a.g - b.g) ** 2 +
    (a.b - b.b) ** 2
  );
}

function buildNearColorPairs(colorCounts) {
  const parsed = Array.from(colorCounts.entries())
    .map(([color, count]) => ({ color, count, parsed: parseColor(color) }))
    .filter(entry => entry.parsed);

  const indistinguishable = [];
  const near = [];

  for (let left = 0; left < parsed.length; left += 1) {
    for (let right = left + 1; right < parsed.length; right += 1) {
      const a = parsed[left];
      const b = parsed[right];
      const alphaDiff = Math.abs(a.parsed.a - b.parsed.a);
      const distance = colorDistance(a.parsed, b.parsed);
      if (distance <= 2 && alphaDiff <= 0.003) {
        indistinguishable.push({ a: a.color, b: b.color, distance, alphaDiff, count: a.count + b.count });
      } else if (distance <= 10 && alphaDiff <= 0.03) {
        near.push({ a: a.color, b: b.color, distance, alphaDiff, count: a.count + b.count });
      }
    }
  }

  const byImpact = (a, b) => b.count - a.count || a.distance - b.distance;
  return {
    indistinguishable: indistinguishable.sort(byImpact).slice(0, 50),
    near: near.sort(byImpact).slice(0, 50),
  };
}

function topEntries(map, limit) {
  return Array.from(map.entries())
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, limit)
    .map(([key, count]) => ({ key, count }));
}

function audit(options) {
  const root = path.resolve(options.root);
  const files = walkFiles(root);
  const cwd = process.cwd();

  const colorCounts = new Map();
  const componentColorCounts = new Map();
  const fallbackTokenCounts = new Map();
  const varUsageCounts = new Map();
  const varDefinitionCounts = new Map();
  const fileColorCounts = new Map();
  const componentFileColorCounts = new Map();
  const exceptionColorCounts = new Map();
  const tokenColorCounts = new Map();

  let colorOccurrences = 0;
  let componentColorOccurrences = 0;
  let fallbackOccurrences = 0;

  for (const file of files) {
    const content = fs.readFileSync(file, 'utf8');
    const relativePath = normalizePath(path.relative(cwd, file));
    const tokenFile = isTokenFile(relativePath);
    const exceptionFile = isExceptionFile(relativePath);
    const colors = collectMatches(content, COLOR_PATTERN).map(match => match[0]);

    if (colors.length > 0) {
      fileColorCounts.set(relativePath, colors.length);
    }

    for (const color of colors) {
      colorOccurrences += 1;
      incrementMap(colorCounts, color);
      if (tokenFile) {
        incrementMap(tokenColorCounts, color);
      } else if (exceptionFile) {
        incrementMap(exceptionColorCounts, color);
      } else {
        componentColorOccurrences += 1;
        incrementMap(componentColorCounts, color);
        incrementMap(componentFileColorCounts, relativePath);
      }
    }

    for (const match of collectMatches(content, CSS_VAR_USAGE_PATTERN)) {
      incrementMap(varUsageCounts, match[1]);
    }

    for (const match of collectMatches(content, CSS_VAR_DEFINITION_PATTERN)) {
      incrementMap(varDefinitionCounts, match[2]);
    }

    for (const match of collectMatches(content, VAR_FALLBACK_PATTERN)) {
      fallbackOccurrences += 1;
      incrementMap(fallbackTokenCounts, match[1]);
    }
  }

  const definedVars = new Set(varDefinitionCounts.keys());
  const undefinedVars = Array.from(varUsageCounts.entries())
    .filter(([name]) => !definedVars.has(name))
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, 100)
    .map(([key, count]) => ({ key, count }));

  const nearPairs = buildNearColorPairs(componentColorCounts);
  const uniqueComponentColors = componentColorCounts.size;

  return {
    root: normalizePath(path.relative(cwd, root)) || '.',
    filesScanned: files.length,
    filesWithColors: fileColorCounts.size,
    colorOccurrences,
    uniqueColors: colorCounts.size,
    componentColorOccurrences,
    componentFilesWithColors: componentFileColorCounts.size,
    uniqueComponentColors,
    tokenUniqueColors: tokenColorCounts.size,
    exceptionUniqueColors: exceptionColorCounts.size,
    fallbackOccurrences,
    budget: {
      uniqueAppColorBudget: options.budget,
      uniqueComponentColors,
      overBudgetBy: Math.max(0, uniqueComponentColors - options.budget),
    },
    topColors: topEntries(colorCounts, options.top),
    topComponentColors: topEntries(componentColorCounts, options.top),
    topFiles: topEntries(fileColorCounts, options.top),
    topFallbackTokens: topEntries(fallbackTokenCounts, options.top),
    undefinedVars,
    nearPairs,
  };
}

function printText(report) {
  const printRows = rows => rows.map(row => `  ${row.count.toString().padStart(5)}  ${row.key}`).join('\n') || '  none';

  console.log(`Theme color audit: ${report.root}`);
  console.log(`Files scanned: ${report.filesScanned}`);
  console.log(`Files with colors: ${report.filesWithColors}`);
  console.log(`Color occurrences: ${report.colorOccurrences}`);
  console.log(`Unique colors: ${report.uniqueColors}`);
  console.log(`Component/non-token color occurrences: ${report.componentColorOccurrences}`);
  console.log(`Files with component/non-token colors: ${report.componentFilesWithColors}`);
  console.log(`Unique component/non-token colors: ${report.uniqueComponentColors}`);
  console.log(`Unique component color budget: ${report.budget.uniqueAppColorBudget}`);
  console.log(`Over budget by: ${report.budget.overBudgetBy}`);
  console.log(`Fallback var occurrences: ${report.fallbackOccurrences}`);

  console.log('\nTop colors:');
  console.log(printRows(report.topColors));

  console.log('\nTop component/non-token colors:');
  console.log(printRows(report.topComponentColors));

  console.log('\nTop files:');
  console.log(printRows(report.topFiles));

  console.log('\nTop fallback tokens:');
  console.log(printRows(report.topFallbackTokens));

  console.log('\nUndefined or dynamically-defined CSS vars (top):');
  console.log(printRows(report.undefinedVars));

  console.log('\nIndistinguishable component color pairs (sample):');
  if (report.nearPairs.indistinguishable.length === 0) {
    console.log('  none');
  } else {
    for (const pair of report.nearPairs.indistinguishable.slice(0, 10)) {
      console.log(`  ${pair.a} <-> ${pair.b}  distance=${pair.distance.toFixed(2)}  alphaDiff=${pair.alphaDiff.toFixed(3)}  combined=${pair.count}`);
    }
  }

  console.log('\nNear component color pairs needing evidence (sample):');
  if (report.nearPairs.near.length === 0) {
    console.log('  none');
  } else {
    for (const pair of report.nearPairs.near.slice(0, 10)) {
      console.log(`  ${pair.a} <-> ${pair.b}  distance=${pair.distance.toFixed(2)}  alphaDiff=${pair.alphaDiff.toFixed(3)}  combined=${pair.count}`);
    }
  }
}

try {
  const options = parseArgs(process.argv.slice(2));
  const report = audit(options);
  if (options.json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printText(report);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
