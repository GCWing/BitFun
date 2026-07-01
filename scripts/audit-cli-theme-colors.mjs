#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DEFAULT_ROOT = 'src/apps/cli';
const DEFAULT_BASELINE = 'scripts/theme-color-governance-baseline.cli.json';
const DEFAULT_NEAR_THRESHOLD = 10;

function blendAlphaChannel(channel, alpha, base) {
  return Math.round((channel * alpha + base * (255 - alpha)) / 255);
}

export function normalizeHexColor(value, options = {}) {
  if (typeof value !== 'string') {
    return null;
  }
  const match = /^#([0-9a-f]{6}|[0-9a-f]{8})$/i.exec(value.trim());
  if (!match) {
    return null;
  }
  const raw = match[1].toLowerCase();
  if (raw.length === 6) {
    return `#${raw}`;
  }

  const base = options.mode === 'light' ? 255 : 0;
  const alpha = Number.parseInt(raw.slice(6, 8), 16);
  return rgbToHex(
    blendAlphaChannel(Number.parseInt(raw.slice(0, 2), 16), alpha, base),
    blendAlphaChannel(Number.parseInt(raw.slice(2, 4), 16), alpha, base),
    blendAlphaChannel(Number.parseInt(raw.slice(4, 6), 16), alpha, base),
  );
}

function hexToRgb(hex) {
  const normalized = normalizeHexColor(hex);
  if (!normalized) {
    return null;
  }
  const raw = normalized.slice(1);
  return [
    Number.parseInt(raw.slice(0, 2), 16),
    Number.parseInt(raw.slice(2, 4), 16),
    Number.parseInt(raw.slice(4, 6), 16),
  ];
}

function rgbToHex(r, g, b) {
  return `#${[r, g, b].map(value => value.toString(16).padStart(2, '0')).join('')}`;
}

function colorDistance(a, b) {
  const rgbA = hexToRgb(a);
  const rgbB = hexToRgb(b);
  if (!rgbA || !rgbB) {
    return Number.POSITIVE_INFINITY;
  }
  return Math.hypot(
    rgbA[0] - rgbB[0],
    rgbA[1] - rgbB[1],
    rgbA[2] - rgbB[2],
  );
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
    const color = normalizeHexColor(trimmed, { mode });
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
      color: rgbToHex(
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
      const distance = colorDistance(colors[i], colors[j]);
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
  const rustFallbackEntries = readRustFallbackEntries(root);
  const allEntries = [...presetEntries, ...rustFallbackEntries];
  const presetNearPairs = findNearPairs(presetEntries, threshold);
  const rustFallbackNearPairs = findNearPairs(rustFallbackEntries, threshold);

  return {
    root,
    presetFiles: presetFiles.length,
    presetColorOccurrences: presetEntries.length,
    presetUniqueColors: new Set(presetEntries.map(entry => entry.color)).size,
    rustFallbackColorOccurrences: rustFallbackEntries.length,
    rustFallbackUniqueColors: new Set(rustFallbackEntries.map(entry => entry.color)).size,
    totalUniqueColors: new Set(allEntries.map(entry => entry.color)).size,
    nearThreshold: threshold,
    presetNearPairs: {
      nearTotal: presetNearPairs.length,
      near: presetNearPairs,
    },
    rustFallbackNearPairs: {
      nearTotal: rustFallbackNearPairs.length,
      near: rustFallbackNearPairs,
    },
    topPresetColors: countByColor(presetEntries),
    topRustFallbackColors: countByColor(rustFallbackEntries),
  };
}

function parseArgs(argv) {
  const options = {
    root: DEFAULT_ROOT,
    baseline: DEFAULT_BASELINE,
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

function getMetric(report, pathExpression) {
  return pathExpression.split('.').reduce((value, key) => {
    if (value && Object.prototype.hasOwnProperty.call(value, key)) {
      return value[key];
    }
    return undefined;
  }, report);
}

export function checkBaseline(report, baselinePath) {
  if (!baselinePath) {
    return [];
  }
  const baseline = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
  const baselineLabel = baselinePath;
  const failures = [];
  if (baseline.version !== 1) {
    failures.push(`${baselineLabel} must use version 1`);
  }
  if (!baseline.budgets || typeof baseline.budgets !== 'object' || Array.isArray(baseline.budgets)) {
    failures.push(`${baselineLabel} must define a budgets object`);
    return failures;
  }
  for (const [metric, budget] of Object.entries(baseline.budgets ?? {})) {
    if (!budget || typeof budget !== 'object' || Array.isArray(budget)) {
      failures.push(`${baselineLabel} ${metric} budget must be an object`);
      continue;
    }
    if (typeof budget.max !== 'number') {
      failures.push(`${baselineLabel} ${metric}.max must be a number`);
      continue;
    }
    const actual = getMetric(report, metric);
    if (typeof actual !== 'number') {
      failures.push(`${baselineLabel} references unknown numeric metric ${metric}`);
      continue;
    }
    if (actual > budget.max) {
      failures.push(`${metric} has ${actual} candidate(s), baseline is ${budget.max}`);
    } else if (actual < budget.max) {
      failures.push(`${metric} has ${actual} candidate(s), below baseline ${budget.max}; lower ${baselineLabel}.`);
    }
  }
  return failures;
}

export function writeReportJson(report, reportJsonPath) {
  const outputPath = path.resolve(reportJsonPath);
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
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
  console.log(`Rust fallback Color::Rgb occurrences: ${report.rustFallbackColorOccurrences}`);
  console.log(`Rust fallback unique colors: ${report.rustFallbackUniqueColors}`);
  console.log(`Total CLI unique colors: ${report.totalUniqueColors}`);
  console.log(`Preset near pairs: ${report.presetNearPairs.nearTotal}`);
  console.log(`Rust fallback near pairs: ${report.rustFallbackNearPairs.nearTotal}`);
  console.log('');
  printRows('Top preset colors:', report.topPresetColors, top);
  console.log('');
  printRows('Top Rust fallback colors:', report.topRustFallbackColors, top);
  console.log('');
  printRows('Preset near pairs:', report.presetNearPairs.near, Math.min(top, 20));
  console.log('');
  printRows('Rust fallback near pairs:', report.rustFallbackNearPairs.near, Math.min(top, 20));
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const report = createCliThemeColorReport({ root: options.root });
  printReport(report, options.top);

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
