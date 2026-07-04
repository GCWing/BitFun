import fs from 'node:fs';
import path from 'node:path';

function blendAlphaChannel(channel, alpha, base) {
  return Math.round((channel * alpha + base * (255 - alpha)) / 255);
}

// CLI theme presets are hex-only. These helpers intentionally do not model CSS
// rgba/hsl alpha semantics used by the web-ui color-domain audit.
export function cliRgbToHex(r, g, b) {
  return `#${[r, g, b].map(value => value.toString(16).padStart(2, '0')).join('')}`;
}

export function normalizeCliHexColor(value, options = {}) {
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
  return cliRgbToHex(
    blendAlphaChannel(Number.parseInt(raw.slice(0, 2), 16), alpha, base),
    blendAlphaChannel(Number.parseInt(raw.slice(2, 4), 16), alpha, base),
    blendAlphaChannel(Number.parseInt(raw.slice(4, 6), 16), alpha, base),
  );
}

function cliHexToRgb(hex) {
  const normalized = normalizeCliHexColor(hex);
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

export function cliHexColorDistance(a, b) {
  const rgbA = cliHexToRgb(a);
  const rgbB = cliHexToRgb(b);
  if (!rgbA || !rgbB) {
    return Number.POSITIVE_INFINITY;
  }
  return Math.hypot(
    rgbA[0] - rgbB[0],
    rgbA[1] - rgbB[1],
    rgbA[2] - rgbB[2],
  );
}

function getMetric(report, pathExpression) {
  return pathExpression.split('.').reduce((value, key) => {
    if (value && Object.prototype.hasOwnProperty.call(value, key)) {
      return value[key];
    }
    return undefined;
  }, report);
}

export function checkBudgetBaseline(report, baselinePath, label = baselinePath) {
  if (!baselinePath) {
    return [];
  }
  const baseline = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
  const failures = [];
  if (baseline.version !== 1) {
    failures.push(`${label} must use version 1`);
  }
  if (!baseline.budgets || typeof baseline.budgets !== 'object' || Array.isArray(baseline.budgets)) {
    failures.push(`${label} must define a budgets object`);
    return failures;
  }
  for (const [metric, budget] of Object.entries(baseline.budgets ?? {})) {
    if (!budget || typeof budget !== 'object' || Array.isArray(budget)) {
      failures.push(`${label} ${metric} budget must be an object`);
      continue;
    }
    if (typeof budget.max !== 'number') {
      failures.push(`${label} ${metric}.max must be a number`);
      continue;
    }
    const actual = getMetric(report, metric);
    if (typeof actual !== 'number') {
      failures.push(`${label} references unknown numeric metric ${metric}`);
      continue;
    }
    if (actual > budget.max) {
      failures.push(`${metric} has ${actual} candidate(s), baseline is ${budget.max}`);
    } else if (actual < budget.max) {
      failures.push(`${metric} has ${actual} candidate(s), below baseline ${budget.max}; lower ${label}.`);
    }
  }
  return failures;
}

export function writeReportJson(report, reportJsonPath) {
  const outputPath = path.resolve(reportJsonPath);
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
}
