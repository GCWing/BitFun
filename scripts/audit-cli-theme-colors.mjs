#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  checkBudgetBaseline,
  cliHexColorDistance,
  cliRgbToHex,
  normalizeCliHexColor,
  writeReportJson,
} from './theme-color-audit-utils.mjs';

const DEFAULT_ROOT = 'src/apps/cli';
const DEFAULT_BASELINE = 'scripts/theme-color-governance-baseline.cli.json';
const DEFAULT_NEAR_THRESHOLD = 10;

export { normalizeCliHexColor as normalizeHexColor, writeReportJson } from './theme-color-audit-utils.mjs';

export const CLI_RUNTIME_THEME_KEYS = new Set([
  'primary',
  'success',
  'warning',
  'error',
  'info',
  'textMuted',
  'background',
  'border',
  'backgroundPanel',
  'backgroundElement',
  'inputBackground',
  'diffAdded',
  'diffRemoved',
  'diffAddedBg',
  'diffRemovedBg',
  'borderActive',
  'accent',
  'diffHunkHeader',
  'diffLineNumber',
]);

function baseThemeKey(key) {
  return key.replace(/\.(dark|light)$/, '');
}

export function isRuntimePresetEntry(entry) {
  return CLI_RUNTIME_THEME_KEYS.has(baseThemeKey(entry.key));
}

export function collectPresetColorEntriesFromJson(file, jsonText) {
  const parsed = JSON.parse(jsonText);
  const theme = parsed.theme;
  if (!theme || typeof theme !== 'object' || Array.isArray(theme)) {
    throw new Error(`${file} must contain a theme object`);
  }
  const defs = parsed.defs && typeof parsed.defs === 'object' && !Array.isArray(parsed.defs)
    ? parsed.defs
    : {};
  const defaultMode = /(?:^|[-_/])light(?:[-_.]|$)/i.test(file) ? 'light' : 'dark';

  return Object.entries(theme).flatMap(([key, value]) => {
    return collectColorValueEntries({
      file,
      key,
      value,
      theme,
      defs,
      mode: defaultMode,
      seen: new Set(),
    });
  });
}

function collectColorValueEntries({ file, key, value, theme, defs, mode, seen }) {
  if (typeof value === 'number') {
    return [];
  }
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (trimmed === '' || trimmed.toLowerCase() === 'none' || trimmed.toLowerCase() === 'transparent') {
      return [];
    }
    const color = normalizeCliHexColor(trimmed, { mode });
    if (color) {
      return [{ file, key, color }];
    }

    const referenced = Object.prototype.hasOwnProperty.call(defs, trimmed)
      ? defs[trimmed]
      : Object.prototype.hasOwnProperty.call(theme, trimmed)
        ? theme[trimmed]
        : undefined;
    if (referenced === undefined) {
      return [];
    }
    if (seen.has(trimmed)) {
      throw new Error(`${file} theme color reference cycle detected at "${trimmed}"`);
    }
    const nextSeen = new Set(seen);
    nextSeen.add(trimmed);
    return collectColorValueEntries({
      file,
      key,
      value: referenced,
      theme,
      defs,
      mode,
      seen: nextSeen,
    });
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return [];
  }
  if (Object.prototype.hasOwnProperty.call(value, 'dark') && Object.prototype.hasOwnProperty.call(value, 'light')) {
    return [
      ...collectColorValueEntries({
        file,
        key: `${key}.dark`,
        value: value.dark,
        theme,
        defs,
        mode: 'dark',
        seen: new Set(seen),
      }),
      ...collectColorValueEntries({
        file,
        key: `${key}.light`,
        value: value.light,
        theme,
        defs,
        mode: 'light',
        seen: new Set(seen),
      }),
    ];
  }

  return [];
}

export function collectRustFallbackEntriesFromText(file, sourceText) {
  const entries = [];
  const pattern = /([a-zA-Z_][a-zA-Z0-9_]*):\s*Color::Rgb\(\s*(\d+),\s*(\d+),\s*(\d+)\s*\)/g;
  let match;
  while ((match = pattern.exec(sourceText)) !== null) {
    entries.push({
      file,
      key: match[1],
      color: cliRgbToHex(
        Number.parseInt(match[2], 10),
        Number.parseInt(match[3], 10),
        Number.parseInt(match[4], 10),
      ),
    });
  }
  return entries;
}

export function findNearPairs(entries, threshold = DEFAULT_NEAR_THRESHOLD) {
  const filesByColor = new Map();
  const keysByColor = new Map();
  for (const entry of entries) {
    if (!filesByColor.has(entry.color)) {
      filesByColor.set(entry.color, new Set());
      keysByColor.set(entry.color, new Set());
    }
    filesByColor.get(entry.color).add(entry.file);
    keysByColor.get(entry.color).add(`${entry.file}:${entry.key}`);
  }

  const colors = Array.from(filesByColor.keys()).sort();
  const pairs = [];
  for (let i = 0; i < colors.length; i += 1) {
    for (let j = i + 1; j < colors.length; j += 1) {
      const distance = cliHexColorDistance(colors[i], colors[j]);
      if (distance > 0 && distance <= threshold) {
        pairs.push({
          a: colors[i],
          b: colors[j],
          distance: Number(distance.toFixed(2)),
          files: Array.from(new Set([
            ...filesByColor.get(colors[i]),
            ...filesByColor.get(colors[j]),
          ])).sort(),
          keysByColor: {
            [colors[i]]: Array.from(keysByColor.get(colors[i])).sort(),
            [colors[j]]: Array.from(keysByColor.get(colors[j])).sort(),
          },
        });
      }
    }
  }

  pairs.sort((left, right) => left.distance - right.distance || left.a.localeCompare(right.a));
  return pairs;
}

function countByColor(entries) {
  const counts = new Map();
  for (const entry of entries) {
    counts.set(entry.color, (counts.get(entry.color) ?? 0) + 1);
  }
  return Array.from(counts.entries())
    .map(([color, count]) => ({ color, count }))
    .sort((a, b) => b.count - a.count || a.color.localeCompare(b.color));
}

function listPresetFiles(root) {
  const presetDir = path.join(root, 'themes', 'presets');
  return fs.readdirSync(presetDir)
    .filter(file => file.endsWith('.json'))
    .sort()
    .map(file => path.join(presetDir, file));
}

function readPresetEntries(files) {
  return files.flatMap(file => {
    return collectPresetColorEntriesFromJson(
      path.relative(process.cwd(), file).replaceAll('\\', '/'),
      fs.readFileSync(file, 'utf8'),
    );
  });
}

function readRustFallbackEntries(root) {
  const fullPath = path.join(root, 'src', 'ui', 'theme.rs');
  return collectRustFallbackEntriesFromText(
    path.relative(process.cwd(), fullPath).replaceAll('\\', '/'),
    fs.readFileSync(fullPath, 'utf8'),
  );
}

export function createCliThemeColorReport(options = {}) {
  const root = options.root ?? DEFAULT_ROOT;
  const threshold = options.nearThreshold ?? DEFAULT_NEAR_THRESHOLD;
  const presetFiles = listPresetFiles(root);
  const presetEntries = readPresetEntries(presetFiles);
  const runtimePresetEntries = presetEntries.filter(isRuntimePresetEntry);
  const compatibilityPresetEntries = presetEntries.filter(entry => !isRuntimePresetEntry(entry));
  const rustFallbackEntries = readRustFallbackEntries(root);
  const allEntries = [...presetEntries, ...rustFallbackEntries];
  const runtimeEntries = [...runtimePresetEntries, ...rustFallbackEntries];
  const presetNearPairs = findNearPairs(presetEntries, threshold);
  const runtimePresetNearPairs = findNearPairs(runtimePresetEntries, threshold);
  const compatibilityPresetNearPairs = findNearPairs(compatibilityPresetEntries, threshold);
  const rustFallbackNearPairs = findNearPairs(rustFallbackEntries, threshold);

  return {
    root,
    presetFiles: presetFiles.length,
    presetColorOccurrences: presetEntries.length,
    presetUniqueColors: new Set(presetEntries.map(entry => entry.color)).size,
    runtimePresetColorOccurrences: runtimePresetEntries.length,
    runtimePresetUniqueColors: new Set(runtimePresetEntries.map(entry => entry.color)).size,
    compatibilityPresetColorOccurrences: compatibilityPresetEntries.length,
    compatibilityPresetUniqueColors: new Set(compatibilityPresetEntries.map(entry => entry.color)).size,
    rustFallbackColorOccurrences: rustFallbackEntries.length,
    rustFallbackUniqueColors: new Set(rustFallbackEntries.map(entry => entry.color)).size,
    totalUniqueColors: new Set(allEntries.map(entry => entry.color)).size,
    runtimeTotalUniqueColors: new Set(runtimeEntries.map(entry => entry.color)).size,
    nearThreshold: threshold,
    runtimeThemeKeys: Array.from(CLI_RUNTIME_THEME_KEYS).sort(),
    presetNearPairs: {
      nearTotal: presetNearPairs.length,
      near: presetNearPairs,
    },
    runtimePresetNearPairs: {
      nearTotal: runtimePresetNearPairs.length,
      near: runtimePresetNearPairs,
    },
    compatibilityPresetNearPairs: {
      nearTotal: compatibilityPresetNearPairs.length,
      near: compatibilityPresetNearPairs,
    },
    rustFallbackNearPairs: {
      nearTotal: rustFallbackNearPairs.length,
      near: rustFallbackNearPairs,
    },
    topPresetColors: countByColor(presetEntries),
    topRuntimePresetColors: countByColor(runtimePresetEntries),
    topCompatibilityPresetColors: countByColor(compatibilityPresetEntries),
    topRustFallbackColors: countByColor(rustFallbackEntries),
  };
}

function parseArgs(argv) {
  const options = {
    root: DEFAULT_ROOT,
    baseline: DEFAULT_BASELINE,
    json: false,
    reportJson: null,
    top: 20,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--root') {
      options.root = argv[++i];
      if (!options.root) {
        throw new Error('--root requires a path');
      }
    } else if (arg.startsWith('--root=')) {
      options.root = arg.slice('--root='.length);
      if (!options.root) {
        throw new Error('--root requires a path');
      }
    } else if (arg === '--baseline') {
      options.baseline = argv[++i];
      if (!options.baseline) {
        throw new Error('--baseline requires a baseline path');
      }
    } else if (arg.startsWith('--baseline=')) {
      options.baseline = arg.slice('--baseline='.length);
      if (!options.baseline) {
        throw new Error('--baseline requires a baseline path');
      }
    } else if (arg === '--report-json') {
      options.reportJson = argv[++i];
      if (!options.reportJson) {
        throw new Error('--report-json requires an output path');
      }
    } else if (arg.startsWith('--report-json=')) {
      options.reportJson = arg.slice('--report-json='.length);
      if (!options.reportJson) {
        throw new Error('--report-json requires an output path');
      }
    } else if (arg === '--json') {
      options.json = true;
    } else if (arg === '--top') {
      options.top = Number.parseInt(argv[++i], 10);
    } else if (arg.startsWith('--top=')) {
      options.top = Number.parseInt(arg.slice('--top='.length), 10);
    } else if (arg === '--no-baseline') {
      options.baseline = null;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

export function checkBaseline(report, baselinePath) {
  return checkBudgetBaseline(report, baselinePath);
}

function printRows(title, rows, top) {
  console.log(title);
  if (rows.length === 0) {
    console.log('  none');
    return;
  }
  for (const row of rows.slice(0, top)) {
    if ('count' in row) {
      console.log(`${String(row.count).padStart(7)}  ${row.color}`);
    } else {
      console.log(`  ${row.a} <-> ${row.b} distance=${row.distance} files=${row.files.join(', ')}`);
    }
  }
}

function printReport(report, top) {
  console.log(`CLI theme color audit: ${report.root}`);
  console.log(`Preset files: ${report.presetFiles}`);
  console.log(`Preset color occurrences: ${report.presetColorOccurrences}`);
  console.log(`Preset unique colors: ${report.presetUniqueColors}`);
  console.log(`Runtime-consumed preset color occurrences: ${report.runtimePresetColorOccurrences}`);
  console.log(`Runtime-consumed preset unique colors: ${report.runtimePresetUniqueColors}`);
  console.log(`Compatibility-declared preset color occurrences: ${report.compatibilityPresetColorOccurrences}`);
  console.log(`Compatibility-declared preset unique colors: ${report.compatibilityPresetUniqueColors}`);
  console.log(`Rust fallback Color::Rgb occurrences: ${report.rustFallbackColorOccurrences}`);
  console.log(`Rust fallback unique colors: ${report.rustFallbackUniqueColors}`);
  console.log(`Total CLI unique colors: ${report.totalUniqueColors}`);
  console.log(`Runtime CLI unique colors: ${report.runtimeTotalUniqueColors}`);
  console.log(`Preset near pairs: ${report.presetNearPairs.nearTotal}`);
  console.log(`Runtime-consumed preset near pairs: ${report.runtimePresetNearPairs.nearTotal}`);
  console.log(`Compatibility-declared preset near pairs: ${report.compatibilityPresetNearPairs.nearTotal}`);
  console.log(`Rust fallback near pairs: ${report.rustFallbackNearPairs.nearTotal}`);
  console.log('');
  printRows('Top preset colors:', report.topPresetColors, top);
  console.log('');
  printRows('Top runtime-consumed preset colors:', report.topRuntimePresetColors, top);
  console.log('');
  printRows('Top compatibility-declared preset colors:', report.topCompatibilityPresetColors, top);
  console.log('');
  printRows('Top Rust fallback colors:', report.topRustFallbackColors, top);
  console.log('');
  printRows('Preset near pairs:', report.presetNearPairs.near, Math.min(top, 20));
  console.log('');
  printRows('Runtime-consumed preset near pairs:', report.runtimePresetNearPairs.near, Math.min(top, 20));
  console.log('');
  printRows('Compatibility-declared preset near pairs:', report.compatibilityPresetNearPairs.near, Math.min(top, 20));
  console.log('');
  printRows('Rust fallback near pairs:', report.rustFallbackNearPairs.near, Math.min(top, 20));
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const report = createCliThemeColorReport({ root: options.root });
  if (options.json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printReport(report, options.top);
  }

  if (options.reportJson) {
    writeReportJson(report, options.reportJson);
  }

  const failures = checkBaseline(report, options.baseline);
  if (failures.length > 0) {
    console.error('');
    console.error('CLI theme color audit failed:');
    for (const failure of failures) {
      console.error(`- ${failure}`);
    }
    process.exitCode = 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
