import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  checkBaseline,
  collectPresetColorEntriesFromJson,
  collectRustFallbackEntriesFromText,
  findNearPairs,
  normalizeHexColor,
  writeReportJson,
} from './audit-cli-theme-colors.mjs';

test('normalizeHexColor accepts supported CLI hex colors only', () => {
  assert.equal(normalizeHexColor('#AABBCC'), '#aabbcc');
  assert.equal(normalizeHexColor('#abc'), null);
  assert.equal(normalizeHexColor('rgba(0, 0, 0, 0.5)'), null);
});

test('normalizeHexColor blends eight-digit hex colors like the CLI parser', () => {
  assert.equal(normalizeHexColor('#ffffff80'), '#808080');
  assert.equal(normalizeHexColor('#00000080', { mode: 'light' }), '#7f7f7f');
});

test('collectPresetColorEntriesFromJson reads opencode theme colors', () => {
  const entries = collectPresetColorEntriesFromJson('theme.json', JSON.stringify({
    theme: {
      background: '#101010',
      primary: '#60A5FA',
      transparent: 'none',
    },
  }));

  assert.deepEqual(entries, [
    { file: 'theme.json', key: 'background', color: '#101010' },
    { file: 'theme.json', key: 'primary', color: '#60a5fa' },
  ]);
});

test('collectPresetColorEntriesFromJson resolves defs and light/dark variants', () => {
  const entries = collectPresetColorEntriesFromJson('bitfun-light.json', JSON.stringify({
    defs: {
      neutral: '#00000080',
    },
    theme: {
      background: 'neutral',
      primary: { dark: '#ffffff80', light: '#00000080' },
    },
  }));

  assert.deepEqual(entries, [
    { file: 'bitfun-light.json', key: 'background', color: '#7f7f7f' },
    { file: 'bitfun-light.json', key: 'primary.dark', color: '#808080' },
    { file: 'bitfun-light.json', key: 'primary.light', color: '#7f7f7f' },
  ]);
});

test('collectRustFallbackEntriesFromText reads Theme struct RGB fields only', () => {
  const entries = collectRustFallbackEntriesFromText('theme.rs', `
    primary: Color::Rgb(59, 130, 246),
    let other = Color::Rgb(1, 2, 3);
    muted: Color::DarkGray,
  `);

  assert.deepEqual(entries, [
    { file: 'theme.rs', key: 'primary', color: '#3b82f6' },
  ]);
});

test('findNearPairs reports nearby but not identical colors', () => {
  const pairs = findNearPairs([
    { file: 'a.json', key: 'background', color: '#0e0e10' },
    { file: 'b.json', key: 'background', color: '#101010' },
    { file: 'c.json', key: 'primary', color: '#60a5fa' },
  ], 10);

  assert.equal(pairs.length, 1);
  assert.equal(pairs[0].a, '#0e0e10');
  assert.equal(pairs[0].b, '#101010');
});

test('checkBaseline requires budgets to be lowered when CLI color debt drops', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-cli-theme-audit-'));
  try {
    const baselinePath = path.join(tempDir, 'baseline.json');
    fs.writeFileSync(baselinePath, JSON.stringify({
      version: 1,
      budgets: {
        presetUniqueColors: { max: 2 },
        'rustFallbackNearPairs.nearTotal': { max: 1 },
      },
    }));

    const failures = checkBaseline({
      presetUniqueColors: 1,
      rustFallbackNearPairs: { nearTotal: 0 },
    }, baselinePath);

    assert.match(failures.join('\n'), /presetUniqueColors has 1 candidate\(s\), below baseline 2/);
    assert.match(failures.join('\n'), /rustFallbackNearPairs\.nearTotal has 0 candidate\(s\), below baseline 1/);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test('checkBaseline validates CLI baseline budget shape', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-cli-theme-audit-'));
  try {
    const baselinePath = path.join(tempDir, 'baseline.json');
    fs.writeFileSync(baselinePath, JSON.stringify({
      version: 1,
      budgets: {
        presetUniqueColors: null,
        totalUniqueColors: {},
      },
    }));

    const failures = checkBaseline({ presetUniqueColors: 1, totalUniqueColors: 1 }, baselinePath);

    assert.match(failures.join('\n'), /presetUniqueColors budget must be an object/);
    assert.match(failures.join('\n'), /totalUniqueColors\.max must be a number/);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test('writeReportJson creates parent directories for report output', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-cli-theme-audit-'));
  try {
    const reportPath = path.join(tempDir, 'nested', 'report.json');
    writeReportJson({ totalUniqueColors: 1 }, reportPath);

    assert.deepEqual(JSON.parse(fs.readFileSync(reportPath, 'utf8')), { totalUniqueColors: 1 });
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});
