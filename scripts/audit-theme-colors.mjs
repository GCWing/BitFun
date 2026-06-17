#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

import {
  COLOR_DOMAIN_KEYS,
  COLOR_DOMAIN_LABELS,
  COLOR_DOMAIN_RULES,
  COLOR_EXTENSIONS,
  CONTRACT_VAR_DEFINITION_PATH_PARTS,
  DEFAULT_BASELINE_PATH,
  DEFAULT_ROOT,
  EXCEPTION_PATH_PARTS,
  REGISTERED_DYNAMIC_VAR_PREFIXES,
  RUNTIME_CONTRACT_VAR_DEFINITION_PATH_PARTS,
  STATIC_CONTRACT_VAR_DEFINITION_PATH_PARTS,
  TOKEN_ALIAS_SOURCE_PATH_PARTS,
  TOKEN_PATH_PARTS,
} from './theme-css-var-contract.mjs';

const COLOR_PATTERN =
  /#[0-9a-fA-F]{3,8}\b|rgba?\(\s*[-+]?\d*\.?\d+\s*,\s*[-+]?\d*\.?\d+\s*,\s*[-+]?\d*\.?\d+(?:\s*,\s*(?:[-+]?\d*\.?\d+|var\([^)]+\)))?\s*\)|hsla?\(\s*[-+]?\d*\.?\d+(?:deg|rad|turn)?\s*,\s*[-+]?\d*\.?\d+%\s*,\s*[-+]?\d*\.?\d+%(?:\s*,\s*(?:[-+]?\d*\.?\d+|var\([^)]+\)))?\s*\)/g;
const TOKEN_ALIAS_DEFINITION_PATTERN =
  /(?:^|[;{\s])(\$[a-zA-Z0-9_-]+|--[a-zA-Z0-9_-]+)\s*:\s*(#[0-9a-fA-F]{3,8}\b|rgba?\(\s*[-+]?\d*\.?\d+\s*,\s*[-+]?\d*\.?\d+\s*,\s*[-+]?\d*\.?\d+(?:\s*,\s*(?:[-+]?\d*\.?\d+|var\([^)]+\)))?\s*\)|hsla?\(\s*[-+]?\d*\.?\d+(?:deg|rad|turn)?\s*,\s*[-+]?\d*\.?\d+%\s*,\s*[-+]?\d*\.?\d+%(?:\s*,\s*(?:[-+]?\d*\.?\d+|var\([^)]+\)))?\s*\))/gm;
const CSS_VAR_USAGE_PATTERN = /var\(\s*(--[a-zA-Z0-9_-]+)/g;
const CSS_VAR_DEFINITION_PATTERN = /(^|[;{\s])(--[a-zA-Z0-9_-]+)\s*:/g;
const VAR_FALLBACK_PATTERN = /var\(\s*(--[a-zA-Z0-9_-]+)\s*,/g;
const CSS_VAR_SET_PROPERTY_PATTERN = /\.setProperty\(\s*['"`](--[a-zA-Z0-9_-]+)/g;
const CSS_VAR_INLINE_STYLE_PATTERN = /['"`](--[a-zA-Z0-9_-]+)['"`]\s*:/g;
const CSS_VAR_DYNAMIC_SET_PATTERN = /\.setProperty\(\s*`(--[a-zA-Z0-9_-]*)\$\{/g;
const REPORT_ROW_LIMIT = 100;

function parseArgs(argv) {
  const options = {
    root: DEFAULT_ROOT,
    json: false,
    reportJson: null,
    baselinePath: undefined,
    noBaseline: false,
    top: 15,
    budget: 120,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--json') {
      options.json = true;
    } else if (arg === '--report-json') {
      options.reportJson = argv[++index];
      if (!options.reportJson) {
        throw new Error('--report-json requires an output path');
      }
    } else if (arg.startsWith('--report-json=')) {
      options.reportJson = arg.slice('--report-json='.length);
      if (!options.reportJson) {
        throw new Error('--report-json requires an output path');
      }
    } else if (arg === '--baseline') {
      options.baselinePath = argv[++index];
      if (!options.baselinePath) {
        throw new Error('--baseline requires a baseline path');
      }
    } else if (arg.startsWith('--baseline=')) {
      options.baselinePath = arg.slice('--baseline='.length);
      if (!options.baselinePath) {
        throw new Error('--baseline requires a baseline path');
      }
    } else if (arg === '--no-baseline') {
      options.noBaseline = true;
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
  --root <path>          Directory to scan. Default: ${DEFAULT_ROOT}
  --top <number>         Number of top rows to print. Default: 15
  --budget <number>      Unique app color budget for the summary. Default: 120
  --baseline <path>      Enforce a theme color governance baseline.
  --no-baseline          Disable baseline enforcement.
  --json                 Print machine-readable JSON instead of text.
  --report-json <path>   Write the machine-readable report to a file.
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

function isAuditTestFile(relativePath) {
  return (
    /(^|\/)__tests__\//.test(relativePath)
    || /\.(?:test|spec)\.[a-z0-9]+$/i.test(relativePath)
  );
}

function isTokenFile(relativePath) {
  return TOKEN_PATH_PARTS.some(part => relativePath.includes(part));
}

function isTokenAliasSourceFile(relativePath) {
  return TOKEN_ALIAS_SOURCE_PATH_PARTS.some(part => relativePath.endsWith(part));
}

function isContractVarDefinitionFile(relativePath) {
  return CONTRACT_VAR_DEFINITION_PATH_PARTS.some(part => relativePath.includes(part));
}

function isStaticContractVarDefinitionFile(relativePath) {
  return STATIC_CONTRACT_VAR_DEFINITION_PATH_PARTS.some(part => relativePath.includes(part));
}

function isRuntimeContractVarDefinitionFile(relativePath) {
  return RUNTIME_CONTRACT_VAR_DEFINITION_PATH_PARTS.some(part => relativePath.includes(part));
}

function isExceptionFile(relativePath) {
  return EXCEPTION_PATH_PARTS.some(part => relativePath.toLowerCase().includes(part.toLowerCase()));
}

function pathMatchesPart(relativePath, pathPart) {
  const normalizedPath = relativePath.toLowerCase();
  const normalizedPart = pathPart.toLowerCase();
  return (
    normalizedPath === normalizedPart
    || normalizedPath.startsWith(`${normalizedPart}/`)
    || normalizedPath.startsWith(`${normalizedPart}.`)
    || normalizedPath.includes(`/${normalizedPart}/`)
    || normalizedPath.includes(`/${normalizedPart}.`)
  );
}

function getColorDomain(relativePath) {
  const rule = COLOR_DOMAIN_RULES.find(entry => (
    entry.pathParts.some(part => pathMatchesPart(relativePath, part))
  ));
  return rule?.key ?? 'appUi';
}

function incrementMap(map, key, amount = 1) {
  map.set(key, (map.get(key) ?? 0) + amount);
}

function addToSetMap(map, key, value) {
  const values = map.get(key) ?? new Set();
  values.add(value);
  map.set(key, values);
}

function collectMatches(content, pattern) {
  pattern.lastIndex = 0;
  return Array.from(content.matchAll(pattern));
}

function previousNonWhitespace(chars) {
  for (let index = chars.length - 1; index >= 0; index -= 1) {
    const char = chars[index];
    if (!/\s/.test(char)) {
      return char;
    }
  }
  return null;
}

function isRegexLiteralStart(chars) {
  const previous = previousNonWhitespace(chars);
  return previous == null || '([{=,:;!?&|+-*~^<>'.includes(previous);
}

function stripCommentsForAudit(content, { stripLineComments = true } = {}) {
  const result = [];
  let state = 'code';
  let regexCharClass = false;
  let returnState = 'code';
  let commentReturnState = 'code';
  let templateExpressionDepth = 0;
  const templateReturnStates = [];

  for (let index = 0; index < content.length; index += 1) {
    const char = content[index];
    const next = content[index + 1];

    if (state === 'line-comment') {
      if (char === '\n' || char === '\r') {
        state = commentReturnState;
        result.push(char);
      } else {
        result.push(' ');
      }
      continue;
    }

    if (state === 'block-comment') {
      if (char === '*' && next === '/') {
        result.push(' ', ' ');
        index += 1;
        state = commentReturnState;
      } else {
        result.push(char === '\n' || char === '\r' ? char : ' ');
      }
      continue;
    }

    if (state === 'regex') {
      result.push(char);
      if (char === '\\') {
        index += 1;
        if (index < content.length) {
          result.push(content[index]);
        }
        continue;
      }
      if (char === '[') {
        regexCharClass = true;
      } else if (char === ']') {
        regexCharClass = false;
      } else if (char === '/' && !regexCharClass) {
        state = returnState;
      }
      continue;
    }

    if (state === 'single-quote' || state === 'double-quote') {
      result.push(char);
      if (char === '\\') {
        index += 1;
        if (index < content.length) {
          result.push(content[index]);
        }
        continue;
      }
      if (
        (state === 'single-quote' && char === "'")
        || (state === 'double-quote' && char === '"')
      ) {
        state = returnState;
      }
      continue;
    }

    if (state === 'template') {
      result.push(char);
      if (char === '\\') {
        index += 1;
        if (index < content.length) {
          result.push(content[index]);
        }
        continue;
      }
      if (char === '$' && next === '{') {
        result.push(next);
        index += 1;
        templateExpressionDepth = 1;
        state = 'template-expression';
        continue;
      }
      if (char === '`') {
        state = templateReturnStates.pop() ?? 'code';
      }
      continue;
    }

    if (state === 'template-expression') {
      if (char === '{') {
        templateExpressionDepth += 1;
        result.push(char);
        continue;
      }
      if (char === '}') {
        templateExpressionDepth -= 1;
        result.push(char);
        if (templateExpressionDepth <= 0) {
          templateExpressionDepth = 0;
          state = 'template';
        }
        continue;
      }
    }

    if (char === '/' && next === '*') {
      result.push(' ', ' ');
      index += 1;
      commentReturnState = state;
      state = 'block-comment';
      continue;
    }

    if (stripLineComments && char === '/' && next === '/') {
      result.push(' ', ' ');
      index += 1;
      commentReturnState = state;
      state = 'line-comment';
      continue;
    }

    if (char === '/' && isRegexLiteralStart(result)) {
      regexCharClass = false;
      returnState = state;
      state = 'regex';
      result.push(char);
      continue;
    }

    if (char === "'" || char === '"' || char === '`') {
      if (char === '`') {
        templateReturnStates.push(state);
        state = 'template';
      } else {
        returnState = state;
        state = char === "'" ? 'single-quote' : 'double-quote';
      }
      result.push(char);
      continue;
    }

    result.push(char);
  }

  return result.join('');
}

function createAuditContent(content, relativePath) {
  return stripCommentsForAudit(content, {
    stripLineComments: !relativePath.endsWith('.css'),
  });
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

function canonicalColorKey(color) {
  const parsed = parseColor(color);
  if (!parsed) {
    return null;
  }
  return `${parsed.r},${parsed.g},${parsed.b},${parsed.a}`;
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

function collectTokenAliasDefinitions(files, cwd) {
  const definitionsByColorKey = new Map();

  for (const file of files) {
    const relativePath = normalizePath(path.relative(cwd, file));
    if (!isTokenAliasSourceFile(relativePath)) {
      continue;
    }
    const content = createAuditContent(fs.readFileSync(file, 'utf8'), relativePath);
    for (const match of collectMatches(content, TOKEN_ALIAS_DEFINITION_PATTERN)) {
      const colorKey = canonicalColorKey(match[2]);
      if (!colorKey) {
        continue;
      }
      addToSetMap(definitionsByColorKey, colorKey, match[1]);
    }
  }

  return definitionsByColorKey;
}

function buildTokenAliasLiteralRows({
  tokenAliasLiteralCounts,
  tokenAliasLiteralFiles,
  tokenAliasLiteralExamples,
  tokenAliasDefinitionsByColorKey,
  limit,
}) {
  return Array.from(tokenAliasLiteralCounts.entries())
    .map(([colorKey, count]) => ({
      key: Array.from(tokenAliasLiteralExamples.get(colorKey) ?? []).sort().join(' | '),
      count,
      aliases: Array.from(tokenAliasDefinitionsByColorKey.get(colorKey) ?? []).sort(),
      files: Array.from(tokenAliasLiteralFiles.get(colorKey) ?? []).sort().slice(0, 5),
    }))
    .sort((a, b) => b.count - a.count || a.key.localeCompare(b.key))
    .slice(0, limit);
}

function sumMapValues(map) {
  return Array.from(map.values()).reduce((sum, count) => sum + count, 0);
}

function getValueByPath(value, dottedPath) {
  return dottedPath.split('.').reduce((current, segment) => {
    if (current == null || typeof current !== 'object') {
      return undefined;
    }
    return current[segment];
  }, value);
}

function resolveBaselinePath(options) {
  if (options.noBaseline) {
    return null;
  }
  if (options.baselinePath !== undefined) {
    return path.resolve(options.baselinePath);
  }

  const root = path.resolve(options.root);
  const defaultRoot = path.resolve(DEFAULT_ROOT);
  const defaultBaselinePath = path.resolve(DEFAULT_BASELINE_PATH);
  if (root === defaultRoot && fs.existsSync(defaultBaselinePath)) {
    return defaultBaselinePath;
  }

  return null;
}

function readBaseline(baselinePath) {
  try {
    return JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
  } catch (error) {
    throw new Error(`Failed to parse ${normalizePath(path.relative(process.cwd(), baselinePath))}: ${error.message}`);
  }
}

function validateAllowlistEntry(category, entry, index, baselineLabel) {
  const prefix = `${baselineLabel} allowlists.${category}[${index}]`;
  if (!entry || typeof entry !== 'object' || Array.isArray(entry)) {
    return [`${prefix} must be an object`];
  }
  const failures = [];
  for (const field of ['key', 'owner', 'reason']) {
    if (typeof entry[field] !== 'string' || entry[field].trim() === '') {
      failures.push(`${prefix}.${field} must be a non-empty string`);
    }
  }
  return failures;
}

function evaluateAllowlistCategory({ report, baseline, baselineLabel, category, reportField }) {
  const findings = report[reportField] ?? [];
  if (!Array.isArray(findings)) {
    return [];
  }

  const failures = [];
  const allowlists = baseline.allowlists ?? {};
  const entries = allowlists[category];
  if (!Array.isArray(entries)) {
    if (findings.length === 0) {
      return [];
    }
    return [`${baselineLabel} allowlists.${category} must be an array because ${reportField} has findings`];
  }

  const findingKeys = new Set(findings.map(entry => entry.key));
  const allowlistKeys = new Set();
  entries.forEach((entry, index) => {
    failures.push(...validateAllowlistEntry(category, entry, index, baselineLabel));
    if (typeof entry?.key === 'string') {
      allowlistKeys.add(entry.key);
    }
  });

  for (const key of findingKeys) {
    if (!allowlistKeys.has(key)) {
      failures.push(`${category} is missing allowlist entry for ${key}`);
    }
  }
  for (const key of allowlistKeys) {
    if (!findingKeys.has(key)) {
      failures.push(`${category} allowlist entry ${key} is stale; remove it from ${baselineLabel}.`);
    }
  }

  return failures;
}

function applyBaseline(report, options) {
  const baselinePath = resolveBaselinePath(options);
  const baselineSummary = {
    path: baselinePath ? normalizePath(path.relative(process.cwd(), baselinePath)) : null,
    enforced: false,
    failures: [],
  };
  report.summary.baseline = baselineSummary;

  if (!baselinePath) {
    return baselineSummary;
  }

  if (!fs.existsSync(baselinePath)) {
    baselineSummary.failures.push(`Missing ${baselineSummary.path}`);
    baselineSummary.enforced = true;
    return baselineSummary;
  }

  const baseline = readBaseline(baselinePath);
  baselineSummary.enforced = true;
  const baselineLabel = baselineSummary.path;

  if (baseline.version !== 1) {
    baselineSummary.failures.push(`${baselineLabel} must use version 1`);
  }
  if (!baseline.budgets || typeof baseline.budgets !== 'object' || Array.isArray(baseline.budgets)) {
    baselineSummary.failures.push(`${baselineLabel} must define a budgets object`);
    return baselineSummary;
  }

  for (const [metricPath, budget] of Object.entries(baseline.budgets)) {
    if (!budget || typeof budget !== 'object' || Array.isArray(budget)) {
      baselineSummary.failures.push(`${baselineLabel} ${metricPath} budget must be an object`);
      continue;
    }
    if (typeof budget.max !== 'number') {
      baselineSummary.failures.push(`${baselineLabel} ${metricPath}.max must be a number`);
      continue;
    }
    const actual = getValueByPath(report, metricPath);
    if (typeof actual !== 'number') {
      baselineSummary.failures.push(`${baselineLabel} references unknown numeric metric ${metricPath}`);
      continue;
    }
    if (actual > budget.max) {
      baselineSummary.failures.push(`${metricPath} has ${actual} candidate(s), baseline is ${budget.max}`);
    } else if (actual < budget.max) {
      baselineSummary.failures.push(`${metricPath} has ${actual} candidate(s), below baseline ${budget.max}; lower ${baselineLabel}.`);
    }
  }

  baselineSummary.failures.push(...evaluateAllowlistCategory({
    report,
    baseline,
    baselineLabel,
    category: 'nonContractDynamicInputs',
    reportField: 'nonContractDynamicInputVars',
  }));
  baselineSummary.failures.push(...evaluateAllowlistCategory({
    report,
    baseline,
    baselineLabel,
    category: 'nonContractCssPrivate',
    reportField: 'nonContractCssPrivateVars',
  }));
  baselineSummary.failures.push(...evaluateAllowlistCategory({
    report,
    baseline,
    baselineLabel,
    category: 'intentionalFallbackTokens',
    reportField: 'fallbackVars',
  }));

  return baselineSummary;
}

function audit(options) {
  const root = path.resolve(options.root);
  const checksFullThemeSourceRoot = root === path.resolve(DEFAULT_ROOT);
  const files = walkFiles(root);
  const cwd = process.cwd();
  const auditedFiles = files.filter(file => !isAuditTestFile(normalizePath(path.relative(cwd, file))));
  const tokenAliasDefinitionsByColorKey = collectTokenAliasDefinitions(auditedFiles, cwd);

  const colorCounts = new Map();
  const componentColorCounts = new Map();
  const fallbackTokenCounts = new Map();
  const fallbackTokenFiles = new Map();
  const varUsageCounts = new Map();
  const varDefinitionCounts = new Map();
  const varDefinitionKinds = new Map();
  const varDefinitionFiles = new Map();
  const contractVarDefinitions = new Set();
  const staticContractVarDefinitions = new Set();
  const runtimeContractVarDefinitions = new Set();
  const varUsageFiles = new Map();
  const dynamicDefinitionPrefixes = new Set();
  const dynamicDefinitionFiles = new Map();
  const fileColorCounts = new Map();
  const componentFileColorCounts = new Map();
  const exceptionColorCounts = new Map();
  const tokenColorCounts = new Map();
  const colorDomainCounts = new Map();
  const colorDomainFiles = new Map();
  const tokenAliasLiteralCounts = new Map();
  const tokenAliasLiteralFiles = new Map();
  const tokenAliasLiteralExamples = new Map();

  let colorOccurrences = 0;
  let componentColorOccurrences = 0;
  let fallbackOccurrences = 0;

  for (const file of auditedFiles) {
    const relativePath = normalizePath(path.relative(cwd, file));
    const content = createAuditContent(fs.readFileSync(file, 'utf8'), relativePath);
    const tokenFile = isTokenFile(relativePath);
    const exceptionFile = isExceptionFile(relativePath);
    const colorDomain = getColorDomain(relativePath);
    const colors = collectMatches(content, COLOR_PATTERN).map(match => match[0]);

    if (colors.length > 0) {
      fileColorCounts.set(relativePath, colors.length);
      addToSetMap(colorDomainFiles, colorDomain, relativePath);
    }

    for (const color of colors) {
      colorOccurrences += 1;
      incrementMap(colorCounts, color);
      const domainCounts = colorDomainCounts.get(colorDomain) ?? new Map();
      incrementMap(domainCounts, color);
      colorDomainCounts.set(colorDomain, domainCounts);
      if (tokenFile) {
        incrementMap(tokenColorCounts, color);
      } else if (exceptionFile) {
        incrementMap(exceptionColorCounts, color);
      } else {
        componentColorOccurrences += 1;
        incrementMap(componentColorCounts, color);
        incrementMap(componentFileColorCounts, relativePath);

        const colorKey = canonicalColorKey(color);
        if (colorKey && tokenAliasDefinitionsByColorKey.has(colorKey)) {
          incrementMap(tokenAliasLiteralCounts, colorKey);
          addToSetMap(tokenAliasLiteralFiles, colorKey, relativePath);
          addToSetMap(tokenAliasLiteralExamples, colorKey, color.trim().toLowerCase());
        }
      }
    }

    for (const match of collectMatches(content, CSS_VAR_USAGE_PATTERN)) {
      incrementMap(varUsageCounts, match[1]);
      addToSetMap(varUsageFiles, match[1], relativePath);
    }

    for (const match of collectMatches(content, CSS_VAR_DEFINITION_PATTERN)) {
      incrementMap(varDefinitionCounts, match[2]);
      addToSetMap(varDefinitionKinds, match[2], 'css');
      addToSetMap(varDefinitionFiles, match[2], relativePath);
      if (isContractVarDefinitionFile(relativePath)) {
        contractVarDefinitions.add(match[2]);
      }
      if (isStaticContractVarDefinitionFile(relativePath)) {
        staticContractVarDefinitions.add(match[2]);
      }
    }

    for (const match of collectMatches(content, CSS_VAR_SET_PROPERTY_PATTERN)) {
      incrementMap(varDefinitionCounts, match[1]);
      addToSetMap(varDefinitionKinds, match[1], 'runtime');
      addToSetMap(varDefinitionFiles, match[1], relativePath);
      if (isContractVarDefinitionFile(relativePath)) {
        contractVarDefinitions.add(match[1]);
      }
      if (isRuntimeContractVarDefinitionFile(relativePath)) {
        runtimeContractVarDefinitions.add(match[1]);
      }
    }

    for (const match of collectMatches(content, CSS_VAR_INLINE_STYLE_PATTERN)) {
      incrementMap(varDefinitionCounts, match[1]);
      addToSetMap(varDefinitionKinds, match[1], 'inline-style');
      addToSetMap(varDefinitionFiles, match[1], relativePath);
      if (isContractVarDefinitionFile(relativePath)) {
        contractVarDefinitions.add(match[1]);
      }
    }

    for (const match of collectMatches(content, CSS_VAR_DYNAMIC_SET_PATTERN)) {
      dynamicDefinitionPrefixes.add(match[1]);
      addToSetMap(dynamicDefinitionFiles, match[1], relativePath);
    }

    for (const match of collectMatches(content, VAR_FALLBACK_PATTERN)) {
      fallbackOccurrences += 1;
      incrementMap(fallbackTokenCounts, match[1]);
      addToSetMap(fallbackTokenFiles, match[1], relativePath);
    }
  }

  const definedVars = new Set(varDefinitionCounts.keys());
  const getDefinitionKinds = name => Array.from(varDefinitionKinds.get(name) ?? ['unknown']).sort();
  const getDefinitionKind = name => {
    if (definedVars.has(name)) {
      return getDefinitionKinds(name).join('+');
    }
    const dynamicPrefix = Array.from(dynamicDefinitionPrefixes).find(prefix => name.startsWith(prefix));
    return dynamicPrefix ? `dynamic-family:${dynamicPrefix}*` : null;
  };
  const unresolvedVarEntries = Array.from(varUsageCounts.entries())
    .filter(([name]) => !getDefinitionKind(name))
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  const undefinedVars = unresolvedVarEntries
    .slice(0, REPORT_ROW_LIMIT)
    .map(([key, count]) => ({ key, count }));
  const fallbackOnlyEntries = unresolvedVarEntries
    .filter(([name]) => fallbackTokenCounts.has(name));
  const fallbackOnlyVars = fallbackOnlyEntries
    .slice(0, REPORT_ROW_LIMIT)
    .map(([key, count]) => ({ key, count }));
  const unresolvedRequiredEntries = unresolvedVarEntries
    .filter(([name]) => !fallbackTokenCounts.has(name));
  const unresolvedRequiredVars = unresolvedRequiredEntries
    .slice(0, REPORT_ROW_LIMIT)
    .map(([key, count]) => ({
      key,
      count,
      files: Array.from(varUsageFiles.get(key) ?? []).slice(0, 5),
    }));
  const dynamicDefinedVars = Array.from(varUsageCounts.entries())
    .map(([key, count]) => ({ key, count, kind: getDefinitionKind(key) }))
    .filter(entry => entry.kind && entry.kind !== 'css')
    .sort((a, b) => b.count - a.count || a.key.localeCompare(b.key))
    .slice(0, REPORT_ROW_LIMIT);
  const unregisteredDynamicFamilyEntries = Array.from(dynamicDefinitionPrefixes)
    .filter(prefix => !REGISTERED_DYNAMIC_VAR_PREFIXES.has(prefix))
    .sort((a, b) => a.localeCompare(b))
    .map(prefix => ({
      key: prefix,
      files: Array.from(dynamicDefinitionFiles.get(prefix) ?? []).sort().slice(0, 5),
    }));
  const staleRegisteredDynamicFamilyEntries = checksFullThemeSourceRoot
    ? Array.from(REGISTERED_DYNAMIC_VAR_PREFIXES)
      .filter(prefix => !dynamicDefinitionPrefixes.has(prefix))
      .sort((a, b) => a.localeCompare(b))
      .map(prefix => ({ key: prefix }))
    : [];
  const nonContractDefinedEntries = Array.from(varUsageCounts.entries())
    .filter(([name]) => definedVars.has(name) && !contractVarDefinitions.has(name))
    .map(([key, count]) => ({
      key,
      count,
      definitionKinds: getDefinitionKinds(key),
      definitionFiles: Array.from(varDefinitionFiles.get(key) ?? []).slice(0, 5),
      usageFiles: Array.from(varUsageFiles.get(key) ?? []).slice(0, 5),
      usageFileCount: (varUsageFiles.get(key) ?? new Set()).size,
    }))
    .filter(entry => entry.usageFileCount > 1)
    .sort((a, b) => b.usageFileCount - a.usageFileCount || b.count - a.count || a.key.localeCompare(b.key));
  const nonContractDefinedVars = nonContractDefinedEntries;
  const nonContractDynamicInputEntries = nonContractDefinedEntries
    .filter(entry => entry.definitionKinds.some(kind => kind === 'inline-style' || kind === 'runtime'));
  const nonContractDynamicInputVars = nonContractDynamicInputEntries;
  const nonContractCssPrivateEntries = nonContractDefinedEntries
    .filter(entry => (
      entry.definitionKinds.includes('css')
      && !entry.definitionKinds.some(kind => kind === 'inline-style' || kind === 'runtime')
    ));
  const nonContractCssPrivateVars = nonContractCssPrivateEntries;
  const runtimeOnlyRequiredContractEntries = Array.from(varUsageCounts.entries())
    .filter(([name]) => (
      runtimeContractVarDefinitions.has(name)
      && !staticContractVarDefinitions.has(name)
      && !fallbackTokenCounts.has(name)
    ))
    .map(([key, count]) => ({
      key,
      count,
      definitionFiles: Array.from(varDefinitionFiles.get(key) ?? []).slice(0, 5),
      usageFiles: Array.from(varUsageFiles.get(key) ?? []).slice(0, 5),
      usageFileCount: (varUsageFiles.get(key) ?? new Set()).size,
    }))
    .sort((a, b) => b.count - a.count || a.key.localeCompare(b.key));
  const runtimeOnlyRequiredContractVars = runtimeOnlyRequiredContractEntries
    .slice(0, REPORT_ROW_LIMIT);

  const nearPairs = buildNearColorPairs(componentColorCounts);
  const uniqueComponentColors = componentColorCounts.size;
  const tokenAliasLiteralRows = buildTokenAliasLiteralRows({
    tokenAliasLiteralCounts,
    tokenAliasLiteralFiles,
    tokenAliasLiteralExamples,
    tokenAliasDefinitionsByColorKey,
    limit: options.top,
  });
  const colorDomainScopes = Object.fromEntries(COLOR_DOMAIN_KEYS.map(key => {
    const counts = colorDomainCounts.get(key) ?? new Map();
    const filesWithColors = colorDomainFiles.get(key) ?? new Set();
    return [key, {
      occurrences: sumMapValues(counts),
      filesWithColors: filesWithColors.size,
      uniqueColors: counts.size,
      topColors: topEntries(counts, options.top),
    }];
  }));
  const fallbackVars = Array.from(fallbackTokenCounts.entries())
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([key, count]) => ({
      key,
      count,
      files: Array.from(fallbackTokenFiles.get(key) ?? []).sort().slice(0, 5),
    }));

  return {
    root: normalizePath(path.relative(cwd, root)) || '.',
    filesScanned: auditedFiles.length,
    ignoredTestFiles: files.length - auditedFiles.length,
    filesWithColors: fileColorCounts.size,
    colorOccurrences,
    uniqueColors: colorCounts.size,
    colorScopes: {
      appUi: {
        occurrences: componentColorOccurrences,
        filesWithColors: componentFileColorCounts.size,
        uniqueColors: componentColorCounts.size,
      },
      token: {
        occurrences: sumMapValues(tokenColorCounts),
        uniqueColors: tokenColorCounts.size,
      },
      exception: {
        occurrences: sumMapValues(exceptionColorCounts),
        uniqueColors: exceptionColorCounts.size,
      },
    },
    colorDomainScopes,
    componentColorOccurrences,
    componentFilesWithColors: componentFileColorCounts.size,
    uniqueComponentColors,
    tokenUniqueColors: tokenColorCounts.size,
    exceptionUniqueColors: exceptionColorCounts.size,
    fallbackOccurrences,
    fallbackUniqueTokens: fallbackVars.length,
    budget: {
      uniqueAppColorBudget: options.budget,
      uniqueComponentColors,
      overBudgetBy: Math.max(0, uniqueComponentColors - options.budget),
    },
    topColors: topEntries(colorCounts, options.top),
    topComponentColors: topEntries(componentColorCounts, options.top),
    topFiles: topEntries(fileColorCounts, options.top),
    topFallbackTokens: fallbackVars.slice(0, options.top).map(({ key, count }) => ({ key, count })),
    fallbackVars,
    tokenAliasLiterals: {
      occurrences: sumMapValues(tokenAliasLiteralCounts),
      uniqueColors: tokenAliasLiteralCounts.size,
      top: tokenAliasLiteralRows,
    },
    undefinedVars,
    cssVarDefinitions: {
      definedUnique: definedVars.size,
      contractDefinedUnique: contractVarDefinitions.size,
      staticContractDefinedUnique: staticContractVarDefinitions.size,
      runtimeContractDefinedUnique: runtimeContractVarDefinitions.size,
      dynamicFamilyPrefixes: Array.from(dynamicDefinitionPrefixes).sort(),
      unregisteredDynamicFamilyUnique: unregisteredDynamicFamilyEntries.length,
      staleRegisteredDynamicFamilyUnique: staleRegisteredDynamicFamilyEntries.length,
      unresolvedUnique: unresolvedVarEntries.length,
      fallbackOnlyUnique: fallbackOnlyEntries.length,
      unresolvedRequiredUnique: unresolvedRequiredEntries.length,
      runtimeOnlyRequiredContractUnique: runtimeOnlyRequiredContractEntries.length,
      nonContractCrossFileUnique: nonContractDefinedEntries.length,
      nonContractDynamicInputUnique: nonContractDynamicInputEntries.length,
      nonContractCssPrivateUnique: nonContractCssPrivateEntries.length,
    },
    dynamicDefinedVars,
    unregisteredDynamicFamilies: unregisteredDynamicFamilyEntries,
    staleRegisteredDynamicFamilies: staleRegisteredDynamicFamilyEntries,
    nonContractDefinedVars,
    nonContractDynamicInputVars,
    nonContractCssPrivateVars,
    runtimeOnlyRequiredContractVars,
    fallbackOnlyVars,
    unresolvedRequiredVars,
    nearPairs,
    summary: {
      baseline: {
        path: null,
        enforced: false,
        failures: [],
      },
    },
  };
}

function printText(report) {
  const printRows = rows => rows.map(row => `  ${row.count.toString().padStart(5)}  ${row.key}`).join('\n') || '  none';

  console.log(`Theme color audit: ${report.root}`);
  console.log(`Files scanned: ${report.filesScanned}`);
  console.log(`Ignored test files: ${report.ignoredTestFiles}`);
  console.log(`Files with colors: ${report.filesWithColors}`);
  console.log(`Color occurrences: ${report.colorOccurrences}`);
  console.log(`Unique colors: ${report.uniqueColors}`);
  console.log(`Component/non-token color occurrences: ${report.componentColorOccurrences}`);
  console.log(`Files with component/non-token colors: ${report.componentFilesWithColors}`);
  console.log(`Unique component/non-token colors: ${report.uniqueComponentColors}`);
  console.log(`Unique component color budget: ${report.budget.uniqueAppColorBudget}`);
  console.log(`Over budget by: ${report.budget.overBudgetBy}`);
  console.log(`Fallback var occurrences: ${report.fallbackOccurrences}`);
  console.log(`Fallback var unique tokens: ${report.fallbackUniqueTokens}`);
  console.log(`Token-equivalent app literal occurrences: ${report.tokenAliasLiterals.occurrences}`);
  console.log(`Token-equivalent app literal unique colors: ${report.tokenAliasLiterals.uniqueColors}`);

  console.log('\nTop colors:');
  console.log(printRows(report.topColors));

  console.log('\nTop component/non-token colors:');
  console.log(printRows(report.topComponentColors));

  console.log('\nColor domain scopes:');
  for (const key of COLOR_DOMAIN_KEYS) {
    const scope = report.colorDomainScopes[key];
    if (!scope || scope.occurrences === 0) {
      continue;
    }
    console.log(
      `  ${COLOR_DOMAIN_LABELS[key].padEnd(18)} ` +
      `occurrences=${scope.occurrences.toString().padStart(4)}  ` +
      `unique=${scope.uniqueColors.toString().padStart(4)}  ` +
      `files=${scope.filesWithColors.toString().padStart(3)}`
    );
  }

  console.log('\nTop files:');
  console.log(printRows(report.topFiles));

  console.log('\nTop fallback tokens:');
  console.log(printRows(report.topFallbackTokens));

  console.log('\nTop token-equivalent app literals:');
  if (report.tokenAliasLiterals.top.length === 0) {
    console.log('  none');
  } else {
    for (const row of report.tokenAliasLiterals.top) {
      console.log(
        `  ${row.count.toString().padStart(5)}  ${row.key}  ` +
        `aliases=${row.aliases.join(', ')}  files=${row.files.join(', ')}`
      );
    }
  }

  console.log('\nUnresolved CSS vars before fallback classification (top):');
  console.log(printRows(report.undefinedVars));
  console.log(
    `\nCSS var definition coverage: defined=${report.cssVarDefinitions.definedUnique}, ` +
    `contractDefined=${report.cssVarDefinitions.contractDefinedUnique}, ` +
    `staticContract=${report.cssVarDefinitions.staticContractDefinedUnique}, ` +
    `runtimeContract=${report.cssVarDefinitions.runtimeContractDefinedUnique}, ` +
    `dynamicFamilies=${report.cssVarDefinitions.dynamicFamilyPrefixes.length}, ` +
    `unregisteredDynamicFamilies=${report.cssVarDefinitions.unregisteredDynamicFamilyUnique}, ` +
    `staleRegisteredDynamicFamilies=${report.cssVarDefinitions.staleRegisteredDynamicFamilyUnique}, ` +
    `unresolved=${report.cssVarDefinitions.unresolvedUnique}, ` +
    `fallbackOnly=${report.cssVarDefinitions.fallbackOnlyUnique}, ` +
    `requiredMissing=${report.cssVarDefinitions.unresolvedRequiredUnique}, ` +
    `runtimeOnlyRequired=${report.cssVarDefinitions.runtimeOnlyRequiredContractUnique}, ` +
    `nonContractCrossFile=${report.cssVarDefinitions.nonContractCrossFileUnique}, ` +
    `nonContractDynamicInputs=${report.cssVarDefinitions.nonContractDynamicInputUnique}, ` +
    `nonContractCssPrivate=${report.cssVarDefinitions.nonContractCssPrivateUnique}`
  );

  console.log('\nDynamic/runtime-defined CSS vars (top):');
  console.log(
    report.dynamicDefinedVars
      .slice(0, 10)
      .map(row => `  ${row.count.toString().padStart(5)}  ${row.key}  ${row.kind}`)
      .join('\n') || '  none'
  );

  console.log('\nUnregistered dynamic CSS var families:');
  if (report.unregisteredDynamicFamilies.length === 0) {
    console.log('  none');
  } else {
    for (const row of report.unregisteredDynamicFamilies.slice(0, 10)) {
      console.log(`  ${row.key}  definitions=${row.files.join(', ')}`);
    }
  }

  console.log('\nStale registered dynamic CSS var families:');
  console.log(printRows(report.staleRegisteredDynamicFamilies.slice(0, 10).map(row => ({ ...row, count: 1 }))));

  console.log('\nFallback-only unresolved CSS vars (top):');
  console.log(printRows(report.fallbackOnlyVars.slice(0, 10)));

  console.log('\nNon-contract CSS vars used across files (top):');
  if (report.nonContractDefinedVars.length === 0) {
    console.log('  none');
  } else {
    for (const row of report.nonContractDefinedVars.slice(0, 10)) {
      console.log(
        `  ${row.count.toString().padStart(5)}  ${row.key}  ` +
        `usageFiles=${row.usageFileCount}  kinds=${row.definitionKinds.join('+')}  ` +
        `definitions=${row.definitionFiles.join(', ')}`
      );
    }
  }

  console.log('\nNon-contract dynamic input CSS vars (top):');
  if (report.nonContractDynamicInputVars.length === 0) {
    console.log('  none');
  } else {
    for (const row of report.nonContractDynamicInputVars.slice(0, 10)) {
      console.log(
        `  ${row.count.toString().padStart(5)}  ${row.key}  ` +
        `usageFiles=${row.usageFileCount}  definitions=${row.definitionFiles.join(', ')}`
      );
    }
  }

  console.log('\nNon-contract component-private CSS vars (top):');
  if (report.nonContractCssPrivateVars.length === 0) {
    console.log('  none');
  } else {
    for (const row of report.nonContractCssPrivateVars.slice(0, 10)) {
      console.log(
        `  ${row.count.toString().padStart(5)}  ${row.key}  ` +
        `usageFiles=${row.usageFileCount}  definitions=${row.definitionFiles.join(', ')}`
      );
    }
  }

  console.log('\nRequired unresolved CSS vars (top):');
  if (report.unresolvedRequiredVars.length === 0) {
    console.log('  none');
  } else {
    for (const row of report.unresolvedRequiredVars.slice(0, 10)) {
      console.log(`  ${row.count.toString().padStart(5)}  ${row.key}  files=${row.files.join(', ')}`);
    }
  }

  console.log('\nRuntime-only contract CSS vars without fallback (top):');
  if (report.runtimeOnlyRequiredContractVars.length === 0) {
    console.log('  none');
  } else {
    for (const row of report.runtimeOnlyRequiredContractVars.slice(0, 10)) {
      console.log(
        `  ${row.count.toString().padStart(5)}  ${row.key}  ` +
        `usageFiles=${row.usageFileCount}  definitions=${row.definitionFiles.join(', ')}`
      );
    }
  }

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
  const baselineSummary = applyBaseline(report, options);
  if (options.reportJson) {
    const reportJsonPath = path.resolve(options.reportJson);
    fs.mkdirSync(path.dirname(reportJsonPath), { recursive: true });
    fs.writeFileSync(reportJsonPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  }
  if (options.json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printText(report);
  }
  if (baselineSummary.failures.length > 0) {
    console.error('\nTheme color audit baseline failures:');
    for (const failure of baselineSummary.failures) {
      console.error(`  - ${failure}`);
    }
    process.exit(1);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
