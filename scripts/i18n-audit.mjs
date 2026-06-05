import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const require = createRequire(import.meta.url);
const root = process.cwd();
const contractPath = path.join(root, 'src', 'shared', 'i18n', 'contract', 'locales.json');
const hardcodedBaselinePath = path.join(root, 'scripts', 'i18n-hardcoded-baseline.json');
const literalFallbackBaselinePath = path.join(root, 'scripts', 'i18n-literal-fallback-baseline.json');
const localeFormatBaselinePath = path.join(root, 'scripts', 'i18n-locale-format-baseline.json');
const dynamicKeyAllowlistPath = path.join(root, 'scripts', 'i18n-dynamic-key-allowlist.json');
const l10nIdenticalAllowlistPath = path.join(root, 'scripts', 'i18n-l10n-identical-allowlist.json');
const governanceBaselinePath = path.join(root, 'scripts', 'i18n-governance-baseline.json');
const sharedTermsDir = path.join(root, 'src', 'shared', 'i18n', 'resources', 'shared');
const webLocalesDir = path.join(root, 'src', 'web-ui', 'src', 'locales');
const namespaceRegistryPath = path.join(
  root,
  'src',
  'web-ui',
  'src',
  'infrastructure',
  'i18n',
  'presets',
  'namespaceRegistry.ts',
);
const webSourceDir = path.join(root, 'src', 'web-ui', 'src');
const mobileWebSourceDir = path.join(root, 'src', 'mobile-web', 'src');
const mobileWebMessagesPath = path.join(mobileWebSourceDir, 'i18n', 'messages.ts');
const installerSourceDir = path.join(root, 'BitFun-Installer', 'src');
const installerLocalesDir = path.join(installerSourceDir, 'i18n', 'locales');
const coreLocalesDir = path.join(root, 'src', 'crates', 'assembly', 'core', 'locales');
const relayHomepageDir = path.join(root, 'src', 'apps', 'relay-server', 'static', 'homepage');
const relayHomepageI18nPath = path.join(relayHomepageDir, 'i18n.json');
const supportedLocales = fs
  .readdirSync(webLocalesDir, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();
const baselineLocale = supportedLocales.includes('en-US') ? 'en-US' : supportedLocales[0];
const localeContract = readJsonFile(contractPath);

let errorCount = 0;
let warningCount = 0;
let auditTypeScript = null;
let cliOptions = { reportJsonPath: null };
let governanceBaselineCache;
const reportCategories = [
  'confirmedUnusedKeys',
  'dynamicKeyCandidates',
  'sharedTermDuplicates',
  'sameTextLocaleInventory',
  'l10nQualityCandidates',
  'literalDefaultValueFallbacks',
  'localeFormatCandidates',
];
const governanceReport = {
  version: 1,
  generatedBy: 'scripts/i18n-audit.mjs',
  summary: {
    baseline: {
      path: 'scripts/i18n-governance-baseline.json',
      enforced: false,
    },
    byCategory: {},
    counts: {},
  },
  confirmedUnusedKeys: [],
  dynamicKeyCandidates: [],
  sharedTermDuplicates: [],
  sameTextLocaleInventory: [],
  l10nQualityCandidates: [],
  literalDefaultValueFallbacks: [],
  localeFormatCandidates: [],
};

function reportError(message) {
  errorCount += 1;
  console.error(`[i18n:audit] ERROR ${message}`);
}

function reportWarning(message) {
  warningCount += 1;
  console.warn(`[i18n:audit] WARN ${message}`);
}

function parseCliOptions(args) {
  const options = { reportJsonPath: null };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--report-json') {
      const value = args[index + 1];
      if (!value || value.startsWith('--')) {
        reportError('--report-json requires an output path');
      } else {
        options.reportJsonPath = value;
        index += 1;
      }
      continue;
    }

    if (arg.startsWith('--report-json=')) {
      const value = arg.slice('--report-json='.length);
      if (!value) {
        reportError('--report-json requires an output path');
      } else {
        options.reportJsonPath = value;
      }
      continue;
    }

    reportError(`Unknown i18n audit option "${arg}"`);
  }

  return options;
}

cliOptions = parseCliOptions(process.argv.slice(2));

function loadTypeScriptForAudit() {
  try {
    return require('typescript');
  } catch (error) {
    reportError(
      `Failed to load TypeScript for i18n audit: ${error.message}. Run pnpm install before running pnpm run i18n:audit.`,
    );
    return null;
  }
}

function toPosixPath(value) {
  return value.split(path.sep).join('/');
}

function listFiles(dir, predicate) {
  const output = [];
  if (!fs.existsSync(dir)) return output;

  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      output.push(...listFiles(fullPath, predicate));
    } else if (!predicate || predicate(fullPath)) {
      output.push(fullPath);
    }
  }

  return output;
}

function readJsonFile(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function listLocaleNamespaces(locale) {
  const localeDir = path.join(webLocalesDir, locale);
  const namespaces = listFiles(localeDir, (file) => file.endsWith('.json'))
    .map((file) => toPosixPath(path.relative(localeDir, file)).replace(/\.json$/, ''))
    .sort();
  if (fs.existsSync(path.join(sharedTermsDir, locale, 'terms.json'))) {
    namespaces.push('shared');
  }
  return namespaces.sort();
}

function readRegistryNamespaces() {
  const source = fs.readFileSync(namespaceRegistryPath, 'utf8');
  const match = source.match(/ALL_NAMESPACES\s*=\s*\[([\s\S]*?)\]\s*as const/);
  if (!match) {
    reportError(`Could not parse ALL_NAMESPACES from ${namespaceRegistryPath}`);
    return [];
  }

  return Array.from(match[1].matchAll(/['"]([^'"]+)['"]/g))
    .map((item) => item[1])
    .sort();
}

function readRegistryLocales() {
  return [...localeContract.surfaceOrders['web-ui']].sort();
}

function flattenKeys(value, prefix = '') {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    return prefix ? [prefix] : [];
  }

  const keys = [];
  for (const [key, child] of Object.entries(value)) {
    const nextPrefix = prefix ? `${prefix}.${key}` : key;
    if (child != null && typeof child === 'object' && !Array.isArray(child)) {
      keys.push(...flattenKeys(child, nextPrefix));
    } else {
      keys.push(nextPrefix);
    }
  }
  return keys.sort();
}

function flattenStringEntries(value, prefix = '') {
  if (typeof value === 'string') {
    return prefix ? [[prefix, value]] : [];
  }
  if (Array.isArray(value)) {
    const text = value.filter((item) => typeof item === 'string').join('\n');
    return prefix ? [[prefix, text]] : [];
  }
  if (value == null || typeof value !== 'object') {
    return prefix ? [[prefix, '']] : [];
  }

  return Object.entries(value)
    .flatMap(([key, child]) => flattenStringEntries(child, prefix ? `${prefix}.${key}` : key))
    .sort(([left], [right]) => left.localeCompare(right));
}

function sortedUnique(values) {
  return Array.from(new Set(values)).sort();
}

function isPlainObject(value) {
  return value != null && typeof value === 'object' && !Array.isArray(value);
}

function extractI18nextPlaceholders(value) {
  const matches = String(value).matchAll(/\{\{\s*-?\s*([A-Za-z_][\w]*)\s*\}\}/g);
  return sortedUnique(Array.from(matches, (match) => match[1]));
}

function extractMobilePlaceholders(value) {
  const matches = String(value).matchAll(/\{\s*([A-Za-z_][\w]*)\s*\}/g);
  return sortedUnique(Array.from(matches, (match) => match[1]));
}

function extractFluentPlaceholders(value) {
  const matches = String(value).matchAll(/\$\s*([A-Za-z_][\w-]*)/g);
  return sortedUnique(Array.from(matches, (match) => match[1]));
}

function sameSet(left, right) {
  if (left.length !== right.length) return false;
  return left.every((item, index) => item === right[index]);
}

function hasHanText(value) {
  return /\p{Script=Han}/u.test(String(value));
}

const zhTwSameTextTerminologySignals = ['了解'];
// Keep this as a review signal, not a converter. The goal is to catch obvious
// Simplified-copy residue without flagging every valid same-writing Han term.
const zhTwSameTextScriptSignals = new Set([
  '这',
  '个',
  '们',
  '为',
  '与',
  '开',
  '关',
  '发',
  '复',
  '后',
  '过',
  '还',
  '进',
  '选',
  '连',
  '远',
  '时',
  '设',
  '数',
  '据',
  '页',
  '项',
  '态',
  '错',
  '误',
  '删',
  '编',
  '导',
  '启',
  '闭',
  '间',
  '类',
  '显',
  '隐',
  '径',
  '档',
  '检',
  '测',
  '权',
  '务',
  '动',
  '应',
  '统',
  '议',
  '读',
  '写',
  '话',
  '请',
  '询',
  '详',
  '语',
  '认',
  '让',
  '讲',
  '试',
  '论',
  '证',
  '评',
  '识',
  '访',
  '调',
  '变',
  '图',
  '标',
  '链',
  '节',
  '览',
  '获',
]);

function getZhTwSameTextSignal(value) {
  const text = String(value);
  for (const phrase of zhTwSameTextTerminologySignals) {
    if (text.includes(phrase)) {
      return { type: 'terminology', match: phrase };
    }
  }

  for (const character of text) {
    if (zhTwSameTextScriptSignals.has(character)) {
      return { type: 'script-variant', match: character };
    }
  }

  return null;
}

function sortByReportIdentity(left, right) {
  return JSON.stringify(left).localeCompare(JSON.stringify(right));
}

function countEntriesBy(entries, field, options = {}) {
  const emptyLabel = options.emptyLabel ?? '';
  const includeKeys = options.includeKeys ?? [];
  const counts = entries.reduce((acc, entry) => {
    const value = entry[field] ?? emptyLabel;
    const key = value === '' ? emptyLabel : value;
    acc.set(key, (acc.get(key) ?? 0) + 1);
    return acc;
  }, new Map(includeKeys.map((key) => [key, 0])));

  return Object.fromEntries(
    Array.from(counts.entries())
      .sort(([left], [right]) => String(left).localeCompare(String(right))),
  );
}

function finalizeGovernanceReport() {
  for (const category of reportCategories) {
    governanceReport[category].sort(sortByReportIdentity);
    governanceReport.summary.counts[category] = governanceReport[category].length;
  }

  const governanceSurfaceIds = collectGovernanceBudgetSurfaceIds();
  const localeFormatSurfaceIds = collectLocaleFormatSurfaceIds();

  governanceReport.summary.byCategory = {
    confirmedUnusedKeys: {
      bySurface: countEntriesBy(governanceReport.confirmedUnusedKeys, 'surface'),
    },
    dynamicKeyCandidates: {
      byAllowlistId: countEntriesBy(governanceReport.dynamicKeyCandidates, 'allowlistId'),
      byProofState: countEntriesBy(governanceReport.dynamicKeyCandidates, 'proofState'),
      bySurface: countEntriesBy(governanceReport.dynamicKeyCandidates, 'surface'),
    },
    sharedTermDuplicates: {
      byNamespace: countEntriesBy(governanceReport.sharedTermDuplicates, 'namespace', { emptyLabel: '<none>' }),
      bySharedKey: countEntriesBy(governanceReport.sharedTermDuplicates, 'sharedKey'),
      bySurface: countEntriesBy(governanceReport.sharedTermDuplicates, 'surface', { includeKeys: governanceSurfaceIds }),
    },
    sameTextLocaleInventory: {
      byAllowlistState: countEntriesBy(governanceReport.sameTextLocaleInventory, 'allowlistState'),
      byNamespace: countEntriesBy(governanceReport.sameTextLocaleInventory, 'namespace', { emptyLabel: '<none>' }),
      bySignalType: countEntriesBy(governanceReport.sameTextLocaleInventory, 'signalType'),
      bySurface: countEntriesBy(governanceReport.sameTextLocaleInventory, 'surface', { includeKeys: governanceSurfaceIds }),
    },
    l10nQualityCandidates: {
      byComparisonLocale: countEntriesBy(governanceReport.l10nQualityCandidates, 'comparisonLocale'),
      byNamespace: countEntriesBy(governanceReport.l10nQualityCandidates, 'namespace', { emptyLabel: '<none>' }),
      bySurface: countEntriesBy(governanceReport.l10nQualityCandidates, 'surface', { includeKeys: governanceSurfaceIds }),
    },
    literalDefaultValueFallbacks: {
      byFile: countEntriesBy(governanceReport.literalDefaultValueFallbacks, 'file'),
      byNamespace: countEntriesBy(governanceReport.literalDefaultValueFallbacks, 'namespace', { emptyLabel: '<none>' }),
    },
    localeFormatCandidates: {
      byFile: countEntriesBy(governanceReport.localeFormatCandidates, 'file'),
      bySurface: countEntriesBy(governanceReport.localeFormatCandidates, 'surface', { includeKeys: localeFormatSurfaceIds }),
    },
  };
}

function writeGovernanceReport() {
  finalizeGovernanceReport();
  if (!cliOptions.reportJsonPath) return;

  const outputPath = path.isAbsolute(cliOptions.reportJsonPath)
    ? cliOptions.reportJsonPath
    : path.join(root, cliOptions.reportJsonPath);
  try {
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.writeFileSync(outputPath, `${JSON.stringify(governanceReport, null, 2)}\n`, 'utf8');
  } catch (error) {
    reportError(`Failed to write i18n governance report to ${toPosixPath(path.relative(root, outputPath))}: ${error.message}`);
  }
}

function reportPlaceholderParity(surface, locale, key, expected, actual) {
  if (sameSet(expected, actual)) return;
  reportError(
    `${surface} ${locale} key "${key}" placeholder mismatch: expected [${expected.join(', ')}], got [${actual.join(', ')}]`,
  );
}

function readJsonKeys(locale, namespace) {
  const file = namespace === 'shared'
    ? path.join(sharedTermsDir, locale, 'terms.json')
    : path.join(webLocalesDir, locale, `${namespace}.json`);
  try {
    return flattenKeys(readJsonFile(file));
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, file))}: ${error.message}`);
    return [];
  }
}

function readJsonEntries(locale, namespace) {
  const file = namespace === 'shared'
    ? path.join(sharedTermsDir, locale, 'terms.json')
    : path.join(webLocalesDir, locale, `${namespace}.json`);
  try {
    return new Map(flattenStringEntries(readJsonFile(file)));
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, file))}: ${error.message}`);
    return new Map();
  }
}

const sharedTermEntryCache = new Map();

function readSharedTermMap(locale) {
  if (sharedTermEntryCache.has(locale)) {
    return sharedTermEntryCache.get(locale);
  }

  const file = path.join(sharedTermsDir, locale, 'terms.json');
  try {
    const entries = new Map(flattenStringEntries(readJsonFile(file)));
    sharedTermEntryCache.set(locale, entries);
    return entries;
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, file))}: ${error.message}`);
    const entries = new Map();
    sharedTermEntryCache.set(locale, entries);
    return entries;
  }
}

function readInstallerJsonKeys(uiLocale) {
  const file = path.join(installerLocalesDir, `${uiLocale}.json`);
  try {
    return flattenKeys(readJsonFile(file));
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, file))}: ${error.message}`);
    return [];
  }
}

function readInstallerJsonEntries(uiLocale) {
  const file = path.join(installerLocalesDir, `${uiLocale}.json`);
  try {
    return new Map(flattenStringEntries(readJsonFile(file)));
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, file))}: ${error.message}`);
    return new Map();
  }
}

function propertyNameToString(ts, name) {
  if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
    return name.text;
  }
  return null;
}

function unwrapTsExpression(ts, expression) {
  let current = expression;
  while (current && (ts.isAsExpression(current) || ts.isSatisfiesExpression(current))) {
    current = current.expression;
  }
  return current;
}

function flattenTsObjectKeys(ts, objectLiteral, prefix = '') {
  const keys = [];
  for (const property of objectLiteral.properties) {
    if (!ts.isPropertyAssignment(property)) continue;

    const key = propertyNameToString(ts, property.name);
    if (!key) continue;
    if (!prefix && key === 'shared') continue;

    const nextPrefix = prefix ? `${prefix}.${key}` : key;
    const initializer = unwrapTsExpression(ts, property.initializer);

    if (ts.isObjectLiteralExpression(initializer)) {
      keys.push(...flattenTsObjectKeys(ts, initializer, nextPrefix));
    } else {
      keys.push(nextPrefix);
    }
  }
  return keys.sort();
}

function flattenTsObjectEntries(ts, objectLiteral, prefix = '') {
  const entries = [];
  for (const property of objectLiteral.properties) {
    if (!ts.isPropertyAssignment(property)) continue;

    const key = propertyNameToString(ts, property.name);
    if (!key) continue;
    if (!prefix && key === 'shared') continue;

    const nextPrefix = prefix ? `${prefix}.${key}` : key;
    const initializer = unwrapTsExpression(ts, property.initializer);

    if (ts.isObjectLiteralExpression(initializer)) {
      entries.push(...flattenTsObjectEntries(ts, initializer, nextPrefix));
    } else if (
      ts.isStringLiteral(initializer) ||
      ts.isNoSubstitutionTemplateLiteral(initializer)
    ) {
      entries.push([nextPrefix, initializer.text]);
    } else {
      entries.push([nextPrefix, '']);
    }
  }
  return entries.sort(([left], [right]) => left.localeCompare(right));
}

function readMobileMessagesByLocale() {
  const ts = auditTypeScript;
  if (!ts) {
    return new Map();
  }

  const source = fs.readFileSync(mobileWebMessagesPath, 'utf8');
  const sourceFile = ts.createSourceFile(mobileWebMessagesPath, source, ts.ScriptTarget.Latest, true);
  const output = new Map();

  function visit(node) {
    if (
      ts.isVariableDeclaration(node) &&
      ts.isIdentifier(node.name) &&
      node.name.text === 'messages'
    ) {
      const initializer = unwrapTsExpression(ts, node.initializer);
      if (!initializer || !ts.isObjectLiteralExpression(initializer)) {
        reportError('mobile-web messages export is not an object literal');
        return;
      }

      for (const property of initializer.properties) {
        if (!ts.isPropertyAssignment(property)) continue;

        const locale = propertyNameToString(ts, property.name);
        if (!locale) continue;

        const value = unwrapTsExpression(ts, property.initializer);
        if (!ts.isObjectLiteralExpression(value)) {
          reportError(`mobile-web messages.${locale} is not an object literal`);
          continue;
        }

        output.set(locale, new Map(flattenTsObjectEntries(ts, value)));
      }
    }
    ts.forEachChild(node, visit);
  }

  visit(sourceFile);
  return output;
}

function readMobileMessageKeysByLocale() {
  return new Map(
    Array.from(readMobileMessagesByLocale().entries())
      .map(([locale, entries]) => [locale, Array.from(entries.keys()).sort()]),
  );
}

function diffSets(left, right) {
  const rightSet = new Set(right);
  return left.filter((item) => !rightSet.has(item));
}

function auditNamespaceCoverage() {
  const registryLocales = readRegistryLocales();
  for (const locale of supportedLocales.filter((item) => !registryLocales.includes(item))) {
    reportError(`${locale} locale directory exists but is not in the web-ui i18n contract surface order`);
  }
  for (const locale of registryLocales.filter((item) => !supportedLocales.includes(item))) {
    reportError(`web-ui i18n contract surface order includes ${locale} but no matching locale directory exists`);
  }

  const registryNamespaces = readRegistryNamespaces();
  const registrySet = new Set(registryNamespaces);

  for (const locale of supportedLocales) {
    const localeNamespaces = listLocaleNamespaces(locale);
    const missingFromRegistry = localeNamespaces.filter((item) => !registrySet.has(item));
    const missingFromLocale = registryNamespaces.filter((item) => !localeNamespaces.includes(item));

    for (const namespace of missingFromRegistry) {
      reportError(`${locale} namespace "${namespace}" exists on disk but is not in ALL_NAMESPACES`);
    }
    for (const namespace of missingFromLocale) {
      reportError(`ALL_NAMESPACES includes "${namespace}" but ${locale} has no matching JSON file`);
    }
  }

  const baselineNamespaces = listLocaleNamespaces(baselineLocale);
  for (const locale of supportedLocales.filter((item) => item !== baselineLocale)) {
    const localeNamespaces = listLocaleNamespaces(locale);
    for (const namespace of diffSets(baselineNamespaces, localeNamespaces)) {
      reportError(`${locale} is missing namespace "${namespace}"`);
    }
    for (const namespace of diffSets(localeNamespaces, baselineNamespaces)) {
      reportError(`${locale} has extra namespace "${namespace}"`);
    }
  }

  return registryNamespaces;
}

function auditSurfaceResourceRoots() {
  const localeById = new Map(localeContract.locales.map((locale) => [locale.id, locale]));
  for (const [surface, config] of Object.entries(localeContract.surfaces ?? {})) {
    const resourceRoot = path.join(root, config.resourceRoot);
    if (!fs.existsSync(resourceRoot)) {
      reportError(`${surface} resourceRoot does not exist: ${config.resourceRoot}`);
      continue;
    }

    for (const localeId of localeContract.surfaceOrders?.[surface] ?? []) {
      if (surface === 'web-ui') {
        const localeDir = path.join(resourceRoot, localeId);
        if (!fs.existsSync(localeDir)) {
          reportError(`${surface} is missing ${localeId} locale directory`);
        }
      } else if (surface === 'installer') {
        const installerLocale = localeById.get(localeId)?.installer?.uiCode;
        if (!installerLocale || !fs.existsSync(path.join(resourceRoot, `${installerLocale}.json`))) {
          reportError(`${surface} is missing ${localeId} resource JSON`);
        }
      } else if (surface === 'core') {
        if (!fs.existsSync(path.join(resourceRoot, `${localeId}.ftl`))) {
          reportError(`${surface} is missing ${localeId} Fluent resource`);
        }
      } else if (surface === 'mobile-web') {
        if (!fs.existsSync(path.join(resourceRoot, 'messages.ts'))) {
          reportError(`${surface} is missing messages.ts`);
        }
      }
    }
  }
}

function auditGeneratedContract() {
  try {
    execFileSync(process.execPath, ['scripts/generate-i18n-contract.mjs', '--check'], {
      cwd: root,
      stdio: 'pipe',
    });
  } catch (error) {
    const stderr = error.stderr?.toString?.().trim();
    reportError(`Generated i18n contract files are out of date. Run pnpm run i18n:generate.${stderr ? ` ${stderr}` : ''}`);
  }
}

function auditSharedTermsCoverage() {
  const expectedLocaleIds = localeContract.locales.map((locale) => locale.id);
  if (!fs.existsSync(sharedTermsDir)) {
    reportError(`Missing shared i18n terms directory: ${toPosixPath(path.relative(root, sharedTermsDir))}`);
    return;
  }

  const baselineTermsPath = path.join(sharedTermsDir, expectedLocaleIds[0], 'terms.json');
  let baselineKeys = [];
  try {
    baselineKeys = flattenKeys(readJsonFile(baselineTermsPath));
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, baselineTermsPath))}: ${error.message}`);
    return;
  }

  for (const localeId of expectedLocaleIds) {
    const termsPath = path.join(sharedTermsDir, localeId, 'terms.json');
    if (!fs.existsSync(termsPath)) {
      reportError(`${localeId} is missing shared terms.json`);
      continue;
    }

    let keys = [];
    try {
      keys = flattenKeys(readJsonFile(termsPath));
    } catch (error) {
      reportError(`Failed to parse ${toPosixPath(path.relative(root, termsPath))}: ${error.message}`);
      continue;
    }

    for (const key of diffSets(baselineKeys, keys)) {
      reportError(`${localeId} shared terms.json is missing key "${key}"`);
    }
    for (const key of diffSets(keys, baselineKeys)) {
      reportError(`${localeId} shared terms.json has extra key "${key}"`);
    }
  }
}

function auditMobileWebBoundary() {
  const sourceFiles = listFiles(
    mobileWebSourceDir,
    (file) => file.endsWith('.ts') || file.endsWith('.tsx'),
  );
  const forbiddenPatterns = [
    /src[/\\]web-ui[/\\]src[/\\]locales/,
    /src[/\\]web-ui[/\\]src[/\\]infrastructure[/\\]i18n/,
    /\.\.[/\\]\.\.[/\\]web-ui[/\\]/,
  ];

  for (const file of sourceFiles) {
    const text = fs.readFileSync(file, 'utf8');
    if (forbiddenPatterns.some((pattern) => pattern.test(text))) {
      reportError(`${toPosixPath(path.relative(root, file))} imports or references web-ui i18n resources`);
    }
  }
}

function auditKeyParity(namespaces) {
  for (const namespace of namespaces) {
    const baselineKeys = readJsonKeys(baselineLocale, namespace);
    for (const locale of supportedLocales.filter((item) => item !== baselineLocale)) {
      const localeKeys = readJsonKeys(locale, namespace);
      const missing = diffSets(baselineKeys, localeKeys);
      const extra = diffSets(localeKeys, baselineKeys);

      if (missing.length > 0) {
        reportError(`${locale}/${namespace}.json is missing ${missing.length} key(s): ${missing.slice(0, 8).join(', ')}`);
      }
      if (extra.length > 0) {
        reportError(`${locale}/${namespace}.json has ${extra.length} extra key(s): ${extra.slice(0, 8).join(', ')}`);
      }
    }
  }
}

function auditWebI18nextPlaceholderParity(namespaces) {
  for (const namespace of namespaces) {
    const baselineEntries = readJsonEntries(baselineLocale, namespace);
    const baselinePlaceholders = new Map(
      Array.from(baselineEntries.entries()).map(([key, value]) => [
        key,
        extractI18nextPlaceholders(value),
      ]),
    );

    for (const locale of supportedLocales.filter((item) => item !== baselineLocale)) {
      const localeEntries = readJsonEntries(locale, namespace);
      for (const [key, expected] of baselinePlaceholders.entries()) {
        if (!localeEntries.has(key)) continue;
        const actual = extractI18nextPlaceholders(localeEntries.get(key));
        reportPlaceholderParity(`web-ui ${namespace}`, locale, key, expected, actual);
      }
    }
  }
}

function auditMobileWebMessageParity() {
  const messagesByLocale = readMobileMessageKeysByLocale();
  const baselineKeys = messagesByLocale.get('en-US');
  if (!baselineKeys) {
    reportError('mobile-web messages are missing the en-US baseline locale');
    return;
  }

  for (const [locale, keys] of messagesByLocale.entries()) {
    if (locale === 'en-US') continue;

    const missing = diffSets(baselineKeys, keys);
    const extra = diffSets(keys, baselineKeys);
    if (missing.length > 0) {
      reportError(`mobile-web ${locale} messages are missing ${missing.length} key(s): ${missing.slice(0, 8).join(', ')}`);
    }
    if (extra.length > 0) {
      reportError(`mobile-web ${locale} messages have ${extra.length} extra key(s): ${extra.slice(0, 8).join(', ')}`);
    }
  }
}

function auditMobileWebPlaceholderParity() {
  const messagesByLocale = readMobileMessagesByLocale();
  const baselineEntries = messagesByLocale.get('en-US');
  if (!baselineEntries) {
    reportError('mobile-web messages are missing the en-US baseline locale');
    return;
  }

  const baselinePlaceholders = new Map(
    Array.from(baselineEntries.entries()).map(([key, value]) => [
      key,
      extractMobilePlaceholders(value),
    ]),
  );

  for (const [locale, entries] of messagesByLocale.entries()) {
    if (locale === 'en-US') continue;
    for (const [key, expected] of baselinePlaceholders.entries()) {
      if (!entries.has(key)) continue;
      const actual = extractMobilePlaceholders(entries.get(key));
      reportPlaceholderParity('mobile-web', locale, key, expected, actual);
    }
  }
}

function auditInstallerKeyParity() {
  const baselineKeys = readInstallerJsonKeys('en');
  for (const uiLocale of ['zh', 'zh-TW']) {
    const keys = readInstallerJsonKeys(uiLocale);
    const missing = diffSets(baselineKeys, keys);
    const extra = diffSets(keys, baselineKeys);

    if (missing.length > 0) {
      reportError(`installer ${uiLocale}.json is missing ${missing.length} key(s): ${missing.slice(0, 8).join(', ')}`);
    }
    if (extra.length > 0) {
      reportError(`installer ${uiLocale}.json has ${extra.length} extra key(s): ${extra.slice(0, 8).join(', ')}`);
    }
  }
}

function auditInstallerPlaceholderParity() {
  const baselineEntries = readInstallerJsonEntries('en');
  const baselinePlaceholders = new Map(
    Array.from(baselineEntries.entries()).map(([key, value]) => [
      key,
      extractI18nextPlaceholders(value),
    ]),
  );

  for (const uiLocale of ['zh', 'zh-TW']) {
    const entries = readInstallerJsonEntries(uiLocale);
    for (const [key, expected] of baselinePlaceholders.entries()) {
      if (!entries.has(key)) continue;
      const actual = extractI18nextPlaceholders(entries.get(key));
      reportPlaceholderParity('installer', uiLocale, key, expected, actual);
    }
  }
}

function readFluentMessages(localeId) {
  const file = path.join(coreLocalesDir, `${localeId}.ftl`);
  const messages = new Map();
  let currentKey = null;
  let currentLines = [];

  function flushCurrent() {
    if (currentKey) {
      messages.set(currentKey, currentLines.join('\n'));
    }
    currentKey = null;
    currentLines = [];
  }

  try {
    const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
    for (const line of lines) {
      const match = line.match(/^([A-Za-z][\w-]*)\s*=\s*(.*)$/);
      if (match) {
        flushCurrent();
        currentKey = match[1];
        currentLines = [match[2]];
        continue;
      }
      if (currentKey && (/^\s+/.test(line) || line.trim().startsWith('*[') || line.trim().startsWith('['))) {
        currentLines.push(line);
      }
    }
    flushCurrent();
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, file))}: ${error.message}`);
  }

  return messages;
}

function auditCoreFluentParity() {
  const coreLocales = localeContract.surfaceOrders?.core ?? [];
  const baselineCoreLocale = coreLocales.includes('en-US') ? 'en-US' : coreLocales[0];
  const baselineEntries = readFluentMessages(baselineCoreLocale);
  const baselineKeys = Array.from(baselineEntries.keys()).sort();
  const baselinePlaceholders = new Map(
    Array.from(baselineEntries.entries()).map(([key, value]) => [
      key,
      extractFluentPlaceholders(value),
    ]),
  );

  for (const locale of coreLocales.filter((item) => item !== baselineCoreLocale)) {
    const entries = readFluentMessages(locale);
    const keys = Array.from(entries.keys()).sort();
    for (const key of diffSets(baselineKeys, keys)) {
      reportError(`core ${locale}.ftl is missing key "${key}"`);
    }
    for (const key of diffSets(keys, baselineKeys)) {
      reportError(`core ${locale}.ftl has extra key "${key}"`);
    }
    for (const [key, expected] of baselinePlaceholders.entries()) {
      if (!entries.has(key)) continue;
      const actual = extractFluentPlaceholders(entries.get(key));
      reportPlaceholderParity('core Fluent', locale, key, expected, actual);
    }
  }
}

function readRelayHomepageMessages() {
  let resource;
  try {
    resource = readJsonFile(relayHomepageI18nPath);
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, relayHomepageI18nPath))}: ${error.message}`);
    return { localeIds: [], entriesByLocale: new Map() };
  }

  const entriesByLocale = new Map();
  for (const [locale, messages] of Object.entries(resource)) {
    entriesByLocale.set(locale, new Map(flattenRelayHomepageEntries(messages, locale)));
  }

  return {
    localeIds: Object.keys(resource).sort(),
    entriesByLocale,
  };
}

function flattenRelayHomepageEntries(value, locale, prefix = '') {
  if (isPlainObject(value) && Object.hasOwn(value, '$shared')) {
    const keys = Object.keys(value);
    if (keys.length !== 1) {
      reportError(`relay static homepage ${locale} key "${prefix}" mixes $shared with local fields`);
    }
    const sharedKey = value.$shared;
    if (!isNonEmptyString(sharedKey)) {
      reportError(`relay static homepage ${locale} key "${prefix}" has an invalid $shared reference`);
      return prefix ? [[prefix, '']] : [];
    }
    if (!readSharedTermMap(locale).has(sharedKey)) {
      reportError(`relay static homepage ${locale} key "${prefix}" references missing shared term "${sharedKey}"`);
    }
    return prefix ? [[prefix, `shared:${sharedKey}`]] : [];
  }

  if (typeof value === 'string') {
    return prefix ? [[prefix, value]] : [];
  }
  if (Array.isArray(value)) {
    const text = value.filter((item) => typeof item === 'string').join('\n');
    return prefix ? [[prefix, text]] : [];
  }
  if (value == null || typeof value !== 'object') {
    return prefix ? [[prefix, '']] : [];
  }

  return Object.entries(value)
    .flatMap(([key, child]) => flattenRelayHomepageEntries(child, locale, prefix ? `${prefix}.${key}` : key))
    .sort(([left], [right]) => left.localeCompare(right));
}

function collectRelayHomepageDataKeys() {
  const htmlPath = path.join(relayHomepageDir, 'index.html');
  const html = fs.readFileSync(htmlPath, 'utf8');
  return sortedUnique(Array.from(html.matchAll(/\bdata-i18n="([^"]+)"/g), (match) => match[1]));
}

function auditRelayStaticHomepageResources() {
  const expectedLocaleIds = (localeContract.locales ?? []).map((locale) => locale.id).sort();
  const { localeIds, entriesByLocale } = readRelayHomepageMessages();
  const baselineLocaleId = expectedLocaleIds.includes('en-US') ? 'en-US' : expectedLocaleIds[0];
  const baselineEntries = entriesByLocale.get(baselineLocaleId) ?? new Map();
  const baselineKeys = Array.from(baselineEntries.keys()).sort();
  const dataKeys = collectRelayHomepageDataKeys();

  for (const locale of diffSets(expectedLocaleIds, localeIds)) {
    reportError(`relay static homepage i18n.json is missing locale "${locale}"`);
  }
  for (const locale of diffSets(localeIds, expectedLocaleIds)) {
    reportError(`relay static homepage i18n.json has non-canonical locale "${locale}"`);
  }
  for (const key of diffSets(dataKeys, baselineKeys)) {
    reportError(`relay static homepage index.html references missing i18n key "${key}"`);
  }
  for (const key of diffSets(baselineKeys, dataKeys)) {
    reportError(`relay static homepage i18n.json has unused baseline key "${key}"`);
  }

  const baselinePlaceholders = new Map(
    Array.from(baselineEntries.entries()).map(([key, value]) => [
      key,
      extractI18nextPlaceholders(value),
    ]),
  );

  for (const locale of expectedLocaleIds.filter((item) => item !== baselineLocaleId)) {
    const entries = entriesByLocale.get(locale);
    if (!entries) continue;
    const keys = Array.from(entries.keys()).sort();
    for (const key of diffSets(baselineKeys, keys)) {
      reportError(`relay static homepage ${locale} messages are missing key "${key}"`);
    }
    for (const key of diffSets(keys, baselineKeys)) {
      reportError(`relay static homepage ${locale} messages have extra key "${key}"`);
    }
    for (const [key, expected] of baselinePlaceholders.entries()) {
      if (!entries.has(key)) continue;
      const actual = extractI18nextPlaceholders(entries.get(key));
      reportPlaceholderParity('relay static homepage', locale, key, expected, actual);
    }
  }
}

function maybeNamespaceResourceKey(namespace, key) {
  return namespace ? `${namespace}:${key}` : key;
}

function pushResourceEntry(entries, { surface, locale, namespace = null, key, value, file }) {
  entries.push({
    surface,
    locale,
    namespace,
    key,
    resourceKey: maybeNamespaceResourceKey(namespace, key),
    value: String(value ?? ''),
    file,
  });
}

function collectI18nResourceEntries(namespaces) {
  const entries = [];
  const localeById = new Map(localeContract.locales.map((locale) => [locale.id, locale]));

  for (const locale of supportedLocales) {
    for (const namespace of namespaces) {
      const surface = namespace === 'shared' ? 'shared' : 'web-ui';
      const file = namespace === 'shared'
        ? `src/shared/i18n/resources/shared/${locale}/terms.json`
        : `src/web-ui/src/locales/${locale}/${namespace}.json`;
      for (const [key, value] of readJsonEntries(locale, namespace)) {
        pushResourceEntry(entries, {
          surface,
          locale,
          namespace: namespace === 'shared' ? 'shared' : namespace,
          key,
          value,
          file,
        });
      }
    }
  }

  if (auditTypeScript) {
    for (const [locale, messageEntries] of readMobileMessagesByLocale().entries()) {
      for (const [key, value] of messageEntries.entries()) {
        pushResourceEntry(entries, {
          surface: 'mobile-web',
          locale,
          key,
          value,
          file: 'src/mobile-web/src/i18n/messages.ts',
        });
      }
    }
  }

  for (const localeId of localeContract.surfaceOrders?.installer ?? []) {
    const uiLocale = localeById.get(localeId)?.installer?.uiCode;
    if (!uiLocale) continue;
    for (const [key, value] of readInstallerJsonEntries(uiLocale).entries()) {
      pushResourceEntry(entries, {
        surface: 'installer',
        locale: localeId,
        key,
        value,
        file: `BitFun-Installer/src/i18n/locales/${uiLocale}.json`,
      });
    }
  }

  for (const locale of localeContract.surfaceOrders?.core ?? []) {
    for (const [key, value] of readFluentMessages(locale).entries()) {
      pushResourceEntry(entries, {
        surface: 'core',
        locale,
        key,
        value,
        file: `src/crates/assembly/core/locales/${locale}.ftl`,
      });
    }
  }

  const relayMessages = readRelayHomepageMessages();
  for (const [locale, relayEntries] of relayMessages.entriesByLocale.entries()) {
    for (const [key, value] of relayEntries.entries()) {
      pushResourceEntry(entries, {
        surface: 'relay-static-homepage',
        locale,
        key,
        value,
        file: 'src/apps/relay-server/static/homepage/i18n.json',
      });
    }
  }

  return entries;
}

function resourceGroupId(entry) {
  return [entry.surface, entry.namespace ?? '', entry.key].join('\u0000');
}

function buildResourceGroups(entries) {
  const groups = new Map();

  for (const entry of entries) {
    const id = resourceGroupId(entry);
    const group = groups.get(id) ?? {
      id,
      surface: entry.surface,
      namespace: entry.namespace,
      key: entry.key,
      resourceKey: entry.resourceKey,
      locales: [],
      files: [],
      valueByLocale: new Map(),
    };
    group.locales.push(entry.locale);
    group.files.push(entry.file);
    group.valueByLocale.set(entry.locale, entry.value);
    groups.set(id, group);
  }

  return Array.from(groups.values()).map((group) => ({
    ...group,
    locales: sortedUnique(group.locales),
    files: sortedUnique(group.files),
  }));
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function optionalStringArray(entry, field) {
  return Array.isArray(entry[field]) ? entry[field] : [];
}

function validateOptionalStringArray(entry, field, label) {
  if (Object.hasOwn(entry, field) && !Array.isArray(entry[field])) {
    reportError(`${label} "${entry.id}" ${field} must be an array`);
    return [];
  }
  return optionalStringArray(entry, field);
}

function readDynamicKeyOwnerSource(entry) {
  if (!isNonEmptyString(entry.owner)) {
    return '';
  }

  const ownerPath = path.join(root, entry.owner);
  if (!fs.existsSync(ownerPath)) {
    reportError(`Dynamic key allowlist "${entry.id}" owner path does not exist: ${entry.owner}`);
    return '';
  }

  const stat = fs.statSync(ownerPath);
  if (stat.isFile()) {
    return fs.readFileSync(ownerPath, 'utf8');
  }

  if (!stat.isDirectory()) {
    reportError(`Dynamic key allowlist "${entry.id}" owner path is not a file or directory: ${entry.owner}`);
    return '';
  }

  return listFiles(ownerPath, (file) => /\.(?:cjs|js|jsx|mjs|rs|ts|tsx)$/.test(file))
    .map((file) => fs.readFileSync(file, 'utf8'))
    .join('\n');
}

function readDynamicKeyAllowlist() {
  if (!fs.existsSync(dynamicKeyAllowlistPath)) {
    reportError('Missing scripts/i18n-dynamic-key-allowlist.json');
    return { entries: [] };
  }

  let allowlist;
  try {
    allowlist = readJsonFile(dynamicKeyAllowlistPath);
  } catch (error) {
    reportError(`Failed to parse scripts/i18n-dynamic-key-allowlist.json: ${error.message}`);
    return { entries: [] };
  }

  if (allowlist.version !== 1) {
    reportError('scripts/i18n-dynamic-key-allowlist.json must use version 1');
  }
  if (!Array.isArray(allowlist.entries)) {
    reportError('scripts/i18n-dynamic-key-allowlist.json must define an entries array');
    return { entries: [] };
  }

  const seenIds = new Set();
  for (const entry of allowlist.entries) {
    if (!isNonEmptyString(entry.id)) {
      reportError('Dynamic key allowlist entries require a non-empty id');
      continue;
    }
    if (seenIds.has(entry.id)) {
      reportError(`Dynamic key allowlist id "${entry.id}" is duplicated`);
    }
    seenIds.add(entry.id);

    for (const field of ['surface', 'owner', 'description']) {
      if (!isNonEmptyString(entry[field])) {
        reportError(`Dynamic key allowlist "${entry.id}" requires a non-empty ${field}`);
      }
    }

    const keys = validateOptionalStringArray(entry, 'keys', 'Dynamic key allowlist');
    const prefixes = validateOptionalStringArray(entry, 'keyPrefixes', 'Dynamic key allowlist');
    const sourceReferences = validateOptionalStringArray(entry, 'sourceReferences', 'Dynamic key allowlist');
    if (keys.length === 0 && prefixes.length === 0) {
      reportError(`Dynamic key allowlist "${entry.id}" must define keys or keyPrefixes`);
    }
    if (sourceReferences.length === 0) {
      reportError(`Dynamic key allowlist "${entry.id}" must define sourceReferences`);
    }
    for (const key of keys) {
      if (!isNonEmptyString(key)) {
        reportError(`Dynamic key allowlist "${entry.id}" has an invalid key entry`);
      }
    }
    for (const prefix of prefixes) {
      if (!isNonEmptyString(prefix)) {
        reportError(`Dynamic key allowlist "${entry.id}" has an invalid keyPrefixes entry`);
      }
    }
    for (const sourceReference of sourceReferences) {
      if (!isNonEmptyString(sourceReference)) {
        reportError(`Dynamic key allowlist "${entry.id}" has an invalid sourceReferences entry`);
      }
    }

    const ownerSource = sourceReferences.length > 0 ? readDynamicKeyOwnerSource(entry) : '';
    let proofState = sourceReferences.length > 0 ? 'source-proven' : 'source-unproven';
    for (const sourceReference of sourceReferences.filter(isNonEmptyString)) {
      if (!ownerSource.includes(sourceReference)) {
        reportError(`Dynamic key allowlist "${entry.id}" source reference "${sourceReference}" was not found under ${entry.owner}`);
        proofState = 'source-unproven';
      }
    }
    entry.proofState = proofState;
  }

  return allowlist;
}

function readL10nIdenticalAllowlist() {
  if (!fs.existsSync(l10nIdenticalAllowlistPath)) {
    reportError('Missing scripts/i18n-l10n-identical-allowlist.json');
    return { entries: [] };
  }

  let allowlist;
  try {
    allowlist = readJsonFile(l10nIdenticalAllowlistPath);
  } catch (error) {
    reportError(`Failed to parse scripts/i18n-l10n-identical-allowlist.json: ${error.message}`);
    return { entries: [] };
  }

  if (allowlist.version !== 1) {
    reportError('scripts/i18n-l10n-identical-allowlist.json must use version 1');
  }
  if (!Array.isArray(allowlist.entries)) {
    reportError('scripts/i18n-l10n-identical-allowlist.json must define an entries array');
    return { entries: [] };
  }

  const seenIds = new Set();
  for (const entry of allowlist.entries) {
    if (!isNonEmptyString(entry.id)) {
      reportError('L10n identical allowlist entries require a non-empty id');
      continue;
    }
    if (seenIds.has(entry.id)) {
      reportError(`L10n identical allowlist id "${entry.id}" is duplicated`);
    }
    seenIds.add(entry.id);

    for (const field of ['surface', 'locale', 'comparisonLocale', 'owner', 'reason']) {
      if (!isNonEmptyString(entry[field])) {
        reportError(`L10n identical allowlist "${entry.id}" requires a non-empty ${field}`);
      }
    }

    const keys = validateOptionalStringArray(entry, 'keys', 'L10n identical allowlist');
    const prefixes = validateOptionalStringArray(entry, 'keyPrefixes', 'L10n identical allowlist');
    if (keys.length === 0 && prefixes.length === 0) {
      reportError(`L10n identical allowlist "${entry.id}" must define keys or keyPrefixes`);
    }
    for (const key of keys) {
      if (!isNonEmptyString(key)) {
        reportError(`L10n identical allowlist "${entry.id}" has an invalid key entry`);
      }
    }
    for (const prefix of prefixes) {
      if (!isNonEmptyString(prefix)) {
        reportError(`L10n identical allowlist "${entry.id}" has an invalid keyPrefixes entry`);
      }
    }
  }

  return allowlist;
}

function readGovernanceBaseline() {
  if (governanceBaselineCache !== undefined) {
    return governanceBaselineCache;
  }

  if (!fs.existsSync(governanceBaselinePath)) {
    reportError('Missing scripts/i18n-governance-baseline.json');
    governanceBaselineCache = null;
    return null;
  }

  let baseline;
  try {
    baseline = readJsonFile(governanceBaselinePath);
  } catch (error) {
    reportError(`Failed to parse scripts/i18n-governance-baseline.json: ${error.message}`);
    governanceBaselineCache = null;
    return null;
  }

  if (baseline.version !== 1) {
    reportError('scripts/i18n-governance-baseline.json must use version 1');
  }
  if (!isPlainObject(baseline.budgets)) {
    reportError('scripts/i18n-governance-baseline.json must define a budgets object');
    governanceBaselineCache = null;
    return null;
  }

  governanceBaselineCache = baseline;
  return baseline;
}

function collectGovernanceBudgetSurfaceIds() {
  const baseline = readGovernanceBaseline();
  if (!baseline) return [];

  const surfaces = new Set();
  for (const budget of Object.values(baseline.budgets)) {
    if (!isPlainObject(budget?.bySurface)) continue;
    for (const surface of Object.keys(budget.bySurface)) {
      surfaces.add(surface);
    }
  }

  return Array.from(surfaces).sort();
}

function allowlistTargetForGroup(entry, group) {
  if (entry.surface !== group.surface) return null;
  if (entry.namespace && entry.namespace !== group.namespace) return null;
  if (entry.locale && !group.locales.includes(entry.locale)) return null;
  return entry.namespace ? group.key : group.resourceKey;
}

function collectDynamicKeyCandidates(resourceGroups) {
  const allowlist = readDynamicKeyAllowlist();
  const seen = new Set();

  function addCandidate(entry, group) {
    const id = [entry.id, group.id].join('\u0000');
    if (seen.has(id)) return;
    seen.add(id);

    governanceReport.dynamicKeyCandidates.push({
      allowlistId: entry.id,
      surface: group.surface,
      namespace: group.namespace,
      key: entry.namespace ? group.key : group.resourceKey,
      resourceKey: group.resourceKey,
      owner: entry.owner,
      reason: entry.description,
      proofState: entry.proofState ?? 'source-unproven',
      sourceReferences: entry.sourceReferences ?? [],
      locales: group.locales,
      files: group.files,
    });
  }

  for (const entry of allowlist.entries ?? []) {
    const eligibleGroups = resourceGroups.filter((group) => allowlistTargetForGroup(entry, group) != null);

    for (const key of optionalStringArray(entry, 'keys')) {
      const matches = eligibleGroups.filter((group) => allowlistTargetForGroup(entry, group) === key);
      if (matches.length === 0) {
        reportError(`Dynamic key allowlist "${entry.id}" references "${key}" but no ${entry.surface} resource matches`);
      }
      for (const group of matches) {
        addCandidate(entry, group);
      }
    }

    for (const prefix of optionalStringArray(entry, 'keyPrefixes')) {
      const matches = eligibleGroups.filter((group) => allowlistTargetForGroup(entry, group).startsWith(prefix));
      if (matches.length === 0) {
        reportError(`Dynamic key allowlist "${entry.id}" references prefix "${prefix}" but no ${entry.surface} resource matches`);
      }
      for (const group of matches) {
        addCandidate(entry, group);
      }
    }
  }
}

function l10nAllowlistTargetForGroup(entry, group) {
  if (entry.surface !== group.surface) return null;
  if (entry.namespace && entry.namespace !== group.namespace) return null;
  if (!group.valueByLocale.has(entry.locale) || !group.valueByLocale.has(entry.comparisonLocale)) return null;
  return entry.namespace ? group.key : group.resourceKey;
}

function l10nIdenticalMatchId(group, locale, comparisonLocale) {
  return [group.id, locale, comparisonLocale].join('\u0000');
}

function collectAllowedL10nIdenticalMatches(resourceGroups) {
  const allowlist = readL10nIdenticalAllowlist();
  const allowed = new Set();

  function isIdenticalCandidate(group, entry) {
    const value = group.valueByLocale.get(entry.locale);
    const comparisonValue = group.valueByLocale.get(entry.comparisonLocale);
    return Boolean(value && comparisonValue && value === comparisonValue && hasHanText(value));
  }

  for (const entry of allowlist.entries ?? []) {
    const eligibleGroups = resourceGroups.filter((group) => (
      l10nAllowlistTargetForGroup(entry, group) != null && isIdenticalCandidate(group, entry)
    ));

    for (const key of optionalStringArray(entry, 'keys')) {
      const matches = eligibleGroups.filter((group) => l10nAllowlistTargetForGroup(entry, group) === key);
      if (matches.length === 0) {
        reportError(`L10n identical allowlist "${entry.id}" references "${key}" but no ${entry.surface} identical resource matches`);
      }
      for (const group of matches) {
        allowed.add(l10nIdenticalMatchId(group, entry.locale, entry.comparisonLocale));
      }
    }

    for (const prefix of optionalStringArray(entry, 'keyPrefixes')) {
      const matches = eligibleGroups.filter((group) => l10nAllowlistTargetForGroup(entry, group).startsWith(prefix));
      if (matches.length === 0) {
        reportError(`L10n identical allowlist "${entry.id}" references prefix "${prefix}" but no ${entry.surface} identical resource matches`);
      }
      for (const group of matches) {
        allowed.add(l10nIdenticalMatchId(group, entry.locale, entry.comparisonLocale));
      }
    }
  }

  return allowed;
}

function collectSharedTermDuplicates(resourceEntries) {
  const sharedByLocaleAndValue = new Map();

  for (const entry of resourceEntries.filter((item) => item.surface === 'shared')) {
    if (!entry.value) continue;
    const localeMap = sharedByLocaleAndValue.get(entry.locale) ?? new Map();
    localeMap.set(entry.value, [...(localeMap.get(entry.value) ?? []), entry]);
    sharedByLocaleAndValue.set(entry.locale, localeMap);
  }

  for (const entry of resourceEntries.filter((item) => item.surface !== 'shared')) {
    if (!entry.value) continue;
    const matches = sharedByLocaleAndValue.get(entry.locale)?.get(entry.value) ?? [];
    for (const shared of matches) {
      governanceReport.sharedTermDuplicates.push({
        surface: entry.surface,
        locale: entry.locale,
        namespace: entry.namespace,
        key: entry.key,
        resourceKey: entry.resourceKey,
        sharedKey: shared.key,
        sharedResourceKey: shared.resourceKey,
        value: entry.value,
        file: entry.file,
        reason: 'matches-shared-term-value',
      });
    }
  }
}

function collectSameTextLocaleInventory(resourceGroups, allowedIdenticalMatches) {
  for (const group of resourceGroups) {
    const simplified = group.valueByLocale.get('zh-CN');
    const traditional = group.valueByLocale.get('zh-TW');
    if (!simplified || !traditional || simplified !== traditional || !hasHanText(traditional)) {
      continue;
    }

    const signal = getZhTwSameTextSignal(traditional);
    const allowlisted = allowedIdenticalMatches.has(l10nIdenticalMatchId(group, 'zh-TW', 'zh-CN'));

    governanceReport.sameTextLocaleInventory.push({
      surface: group.surface,
      namespace: group.namespace,
      key: group.key,
      resourceKey: group.resourceKey,
      locale: 'zh-TW',
      comparisonLocale: 'zh-CN',
      value: traditional,
      files: group.files,
      reason: 'same-text-locale-pair',
      signal,
      signalType: signal?.type ?? 'none',
      allowlisted,
      allowlistState: allowlisted ? 'allowlisted' : 'unreviewed',
    });
  }
}

function collectL10nQualityCandidates(resourceGroups, allowedIdenticalMatches) {
  for (const group of resourceGroups) {
    const simplified = group.valueByLocale.get('zh-CN');
    const traditional = group.valueByLocale.get('zh-TW');
    if (!simplified || !traditional || simplified !== traditional || !hasHanText(traditional)) {
      continue;
    }
    const signal = getZhTwSameTextSignal(traditional);
    if (!signal) {
      continue;
    }
    if (allowedIdenticalMatches.has(l10nIdenticalMatchId(group, 'zh-TW', 'zh-CN'))) {
      continue;
    }

    governanceReport.l10nQualityCandidates.push({
      surface: group.surface,
      namespace: group.namespace,
      key: group.key,
      resourceKey: group.resourceKey,
      locale: 'zh-TW',
      comparisonLocale: 'zh-CN',
      value: traditional,
      files: group.files,
      reason: 'matches-comparison-locale',
      signal,
    });
  }
}

function collectConfirmedUnusedKeys() {
  const expectedLocaleIds = (localeContract.locales ?? []).map((locale) => locale.id).sort();
  const baselineLocaleId = expectedLocaleIds.includes('en-US') ? 'en-US' : expectedLocaleIds[0];
  const { entriesByLocale } = readRelayHomepageMessages();
  const baselineEntries = entriesByLocale.get(baselineLocaleId) ?? new Map();
  const dataKeys = collectRelayHomepageDataKeys();

  for (const key of diffSets(Array.from(baselineEntries.keys()).sort(), dataKeys)) {
    governanceReport.confirmedUnusedKeys.push({
      surface: 'relay-static-homepage',
      key,
      resourceKey: key,
      file: 'src/apps/relay-server/static/homepage/i18n.json',
      reason: 'not-referenced-by-static-data-i18n-attribute',
    });
  }
}

function auditGovernanceCategoryBudget(category, budget) {
  if (!isPlainObject(budget)) {
    reportError(`scripts/i18n-governance-baseline.json ${category} budget must be an object`);
    return;
  }

  const entries = governanceReport[category] ?? [];
  if (typeof budget.maxTotal !== 'number') {
    reportError(`scripts/i18n-governance-baseline.json ${category}.maxTotal must be a number`);
  } else if (entries.length > budget.maxTotal) {
    reportError(`${category} has ${entries.length} candidate(s), baseline is ${budget.maxTotal}`);
  } else if (entries.length < budget.maxTotal) {
    reportError(`${category} has ${entries.length} candidate(s), below baseline ${budget.maxTotal}; lower scripts/i18n-governance-baseline.json.`);
  }

  if (budget.bySurface) {
    if (!isPlainObject(budget.bySurface)) {
      reportError(`scripts/i18n-governance-baseline.json ${category}.bySurface must be an object`);
      return;
    }

    const actualBySurface = countEntriesBy(entries, 'surface');
    const surfaces = sortedUnique([
      ...Object.keys(actualBySurface),
      ...Object.keys(budget.bySurface),
    ]);

    for (const surface of surfaces) {
      const actual = actualBySurface[surface] ?? 0;
      const expected = budget.bySurface[surface];
      if (typeof expected !== 'number') {
        reportError(`scripts/i18n-governance-baseline.json ${category}.bySurface.${surface} must be a number`);
      } else if (actual > expected) {
        reportError(`${category} ${surface} has ${actual} candidate(s), baseline is ${expected}`);
      } else if (actual < expected) {
        reportError(`${category} ${surface} has ${actual} candidate(s), below baseline ${expected}; lower scripts/i18n-governance-baseline.json.`);
      }
    }
  }

  auditGovernanceDimensionBudget(category, budget, 'byNamespace', 'namespace', 'namespace');
  auditGovernanceDimensionBudget(category, budget, 'bySharedKey', 'sharedKey', 'sharedKey');
  auditGovernanceDimensionBudget(category, budget, 'byComparisonLocale', 'comparisonLocale', 'comparisonLocale');
}

function auditGovernanceDimensionBudget(category, budget, budgetField, entryField, label) {
  if (!Object.hasOwn(budget, budgetField)) return;
  if (!isPlainObject(budget[budgetField])) {
    reportError(`scripts/i18n-governance-baseline.json ${category}.${budgetField} must be an object`);
    return;
  }

  const entries = governanceReport[category] ?? [];
  const actualByValue = countEntriesBy(entries, entryField, { emptyLabel: '<none>' });
  const values = sortedUnique([
    ...Object.keys(actualByValue),
    ...Object.keys(budget[budgetField]),
  ]);

  for (const value of values) {
    const actual = actualByValue[value] ?? 0;
    const expected = budget[budgetField][value];
    if (typeof expected !== 'number') {
      reportError(`scripts/i18n-governance-baseline.json ${category}.${budgetField}.${value} must be a number`);
    } else if (actual > expected) {
      reportError(`${category} ${label} ${value} has ${actual} candidate(s), baseline is ${expected}`);
    } else if (actual < expected) {
      reportError(`${category} ${label} ${value} has ${actual} candidate(s), below baseline ${expected}; lower scripts/i18n-governance-baseline.json.`);
    }
  }
}

function auditGovernanceBaseline() {
  const baseline = readGovernanceBaseline();
  if (!baseline) return;

  governanceReport.summary.baseline.enforced = true;

  for (const category of ['confirmedUnusedKeys', 'sharedTermDuplicates', 'l10nQualityCandidates']) {
    auditGovernanceCategoryBudget(category, baseline.budgets[category]);
  }
}

function auditI18nGovernanceReport(namespaces) {
  const resourceEntries = collectI18nResourceEntries(namespaces);
  const resourceGroups = buildResourceGroups(resourceEntries);
  const allowedIdenticalMatches = collectAllowedL10nIdenticalMatches(resourceGroups);

  collectConfirmedUnusedKeys();
  collectDynamicKeyCandidates(resourceGroups);
  collectSharedTermDuplicates(resourceEntries);
  collectSameTextLocaleInventory(resourceGroups, allowedIdenticalMatches);
  collectL10nQualityCandidates(resourceGroups, allowedIdenticalMatches);
  auditGovernanceBaseline();
}

function shouldSkipSourceScan(file) {
  const normalized = toPosixPath(path.relative(root, file));
  return (
    normalized.includes('/locales/') ||
    normalized.endsWith('/generatedLocaleContract.ts') ||
    normalized.endsWith('.test.ts') ||
    normalized.endsWith('.test.tsx') ||
    normalized.endsWith('.spec.ts') ||
    normalized.endsWith('.spec.tsx') ||
    normalized.includes('/component-library/components/registry.tsx')
  );
}

function shouldSkipMobileWebSourceScan(file) {
  const normalized = toPosixPath(path.relative(root, file));
  return (
    normalized.endsWith('/i18n/messages.ts') ||
    normalized.endsWith('/i18n/generatedLocaleContract.ts') ||
    normalized.endsWith('.test.ts') ||
    normalized.endsWith('.test.tsx') ||
    normalized.endsWith('.spec.ts') ||
    normalized.endsWith('.spec.tsx')
  );
}

function shouldSkipInstallerSourceScan(file) {
  const normalized = toPosixPath(path.relative(root, file));
  return (
    normalized.includes('/i18n/locales/') ||
    normalized.endsWith('/i18n/generatedLocaleContract.ts') ||
    normalized.endsWith('.test.ts') ||
    normalized.endsWith('.test.tsx') ||
    normalized.endsWith('.spec.ts') ||
    normalized.endsWith('.spec.tsx')
  );
}

function auditSourceText() {
  const sourceFiles = listFiles(
    webSourceDir,
    (file) => (file.endsWith('.ts') || file.endsWith('.tsx')) && !shouldSkipSourceScan(file),
  );

  const fallbackFindings = [];
  const fallbackPattern = /\bt\s*\(\s*(['"`])(?:\\.|(?!\1).)+\1\s*,\s*(['"`])/g;

  for (const file of sourceFiles) {
    const text = fs.readFileSync(file, 'utf8');
    const lines = text.split(/\r?\n/);
    lines.forEach((line, index) => {
      if (fallbackPattern.test(line)) {
        fallbackFindings.push(`${toPosixPath(path.relative(root, file))}:${index + 1}`);
      }
      fallbackPattern.lastIndex = 0;
    });
  }

  if (fallbackFindings.length > 0) {
    reportError(`Found ${fallbackFindings.length} t(key, "literal fallback") candidate(s). First entries: ${fallbackFindings.slice(0, 12).join(', ')}`);
  }
}

function lineNumberAt(text, index) {
  return text.slice(0, index).split(/\r?\n/).length;
}

function createAuditSourceFile(file, text) {
  const ts = auditTypeScript;
  return ts.createSourceFile(
    file,
    text,
    ts.ScriptTarget.Latest,
    true,
    file.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
}

function staticStringLiteral(ts, expression) {
  const value = unwrapTsExpression(ts, expression);
  if (!value) return null;
  if (ts.isStringLiteral(value) || ts.isNoSubstitutionTemplateLiteral(value)) {
    return value.text;
  }
  return null;
}

function staticStringArrayLiteral(ts, expression) {
  const value = unwrapTsExpression(ts, expression);
  if (!value || !ts.isArrayLiteralExpression(value)) return null;

  const values = [];
  for (const element of value.elements) {
    const literal = staticStringLiteral(ts, element);
    if (!literal) return null;
    values.push(literal);
  }
  return values.length > 0 ? values : null;
}

function isI18nServiceGetTCall(ts, expression) {
  return (
    ts.isCallExpression(expression) &&
    ts.isPropertyAccessExpression(expression.expression) &&
    expression.expression.name.text === 'getT' &&
    ts.isIdentifier(expression.expression.expression) &&
    expression.expression.expression.text === 'i18nService'
  );
}

function isI18nServiceTCall(ts, expression) {
  return (
    ts.isPropertyAccessExpression(expression) &&
    expression.name.text === 't' &&
    ts.isIdentifier(expression.expression) &&
    expression.expression.text === 'i18nService'
  );
}

function isI18nHookCall(ts, expression) {
  return (
    ts.isCallExpression(expression) &&
    ts.isIdentifier(expression.expression) &&
    (expression.expression.text === 'useI18n' || expression.expression.text === 'useTranslation')
  );
}

function collectWebUiTranslationCallFacts() {
  const ts = auditTypeScript;
  const sourceFiles = listFiles(
    webSourceDir,
    (file) => (file.endsWith('.ts') || file.endsWith('.tsx')) && !shouldSkipSourceScan(file),
  );
  const output = [];

  for (const file of sourceFiles) {
    const text = fs.readFileSync(file, 'utf8');
    const sourceFile = createAuditSourceFile(file, text);
    const hookNamespaceByTranslator = new Map();
    const hookNamespaceListByTranslator = new Map();
    const fullKeyTranslatorNames = new Set();

    function rememberHookTranslator(node) {
      if (!ts.isVariableDeclaration(node) || !node.initializer || !isI18nHookCall(ts, node.initializer)) {
        return;
      }

      const namespace = staticStringLiteral(ts, node.initializer.arguments[0]);
      const namespaces = namespace ? null : staticStringArrayLiteral(ts, node.initializer.arguments[0]);
      if (!namespace && !namespaces) return;

      function rememberLocalName(localName) {
        if (!localName) return;
        if (namespace) {
          hookNamespaceByTranslator.set(localName, namespace);
        } else {
          hookNamespaceListByTranslator.set(localName, namespaces);
        }
      }

      if (ts.isObjectBindingPattern(node.name)) {
        for (const element of node.name.elements) {
          const propertyName = element.propertyName ? propertyNameToString(ts, element.propertyName) : propertyNameToString(ts, element.name);
          const localName = propertyNameToString(ts, element.name);
          if (propertyName === 't' && localName) {
            rememberLocalName(localName);
          }
        }
      } else if (ts.isArrayBindingPattern(node.name)) {
        const first = node.name.elements[0];
        if (first && ts.isBindingElement(first) && ts.isIdentifier(first.name)) {
          rememberLocalName(first.name.text);
        }
      }
    }

    function rememberGetTTranslator(node) {
      if (
        ts.isVariableDeclaration(node) &&
        ts.isIdentifier(node.name) &&
        node.initializer &&
        isI18nServiceGetTCall(ts, node.initializer)
      ) {
        fullKeyTranslatorNames.add(node.name.text);
      }
    }

    function recordTranslationCall(node) {
      if (!ts.isCallExpression(node)) return;

      const key = staticStringLiteral(ts, node.arguments[0]);
      if (!key) return;

      let fullKey = null;
      let kind = 'full';
      if (isI18nServiceTCall(ts, node.expression) || isI18nServiceGetTCall(ts, node.expression)) {
        if (!key.includes(':')) return;
        fullKey = key;
      } else if (ts.isIdentifier(node.expression) && fullKeyTranslatorNames.has(node.expression.text)) {
        if (!key.includes(':')) return;
        fullKey = key;
      } else if (ts.isIdentifier(node.expression) && hookNamespaceByTranslator.has(node.expression.text)) {
        const namespace = hookNamespaceByTranslator.get(node.expression.text);
        fullKey = key.includes(':') ? key : `${namespace}:${key}`;
        kind = key.includes(':') ? 'hook-full' : 'hook-relative';
      } else if (ts.isIdentifier(node.expression) && hookNamespaceListByTranslator.has(node.expression.text)) {
        const namespaces = hookNamespaceListByTranslator.get(node.expression.text);
        fullKey = key.includes(':') ? key : `${namespaces[0]}:${key}`;
        kind = key.includes(':') ? 'hook-full-array' : 'hook-relative-array';
      }

      if (!fullKey) return;

      output.push({
        key: fullKey,
        kind,
        options: node.arguments[1],
        location: `${toPosixPath(path.relative(root, file))}:${lineNumberAt(text, node.getStart(sourceFile))}`,
        file: toPosixPath(path.relative(root, file)),
        sourceFile,
      });
    }

    function visit(node) {
      rememberHookTranslator(node);
      rememberGetTTranslator(node);
      recordTranslationCall(node);
      ts.forEachChild(node, visit);
    }

    visit(sourceFile);
  }

  return output;
}

function collectWebUiStaticTranslationKeys() {
  return collectWebUiTranslationCallFacts().map(({ key, kind, location }) => ({ key, kind, location }));
}

function buildWebUiKeySet(namespaces) {
  const keys = new Set();
  for (const namespace of namespaces) {
    for (const key of readJsonKeys(baselineLocale, namespace)) {
      keys.add(`${namespace}:${key}`);
    }
  }
  return keys;
}

function auditWebUiStaticTranslationKeys(namespaces) {
  const knownKeys = buildWebUiKeySet(namespaces);
  const missing = collectWebUiStaticTranslationKeys()
    .filter(({ key }) => !knownKeys.has(key));

  if (missing.length > 0) {
    const relativeCount = missing.filter(({ kind }) => kind === 'hook-relative').length;
    reportError(
      `Found ${missing.length} unknown static Web UI i18n key(s), including ${relativeCount} relative static Web UI i18n key(s). First entries: ${
        missing.slice(0, 12).map(({ key, location }) => `${location} ${key}`).join(', ')
      }`,
    );
  }
}

function isLiteralFallbackInitializer(ts, initializer) {
  const value = unwrapTsExpression(ts, initializer);
  if (!value) return false;
  if (ts.isStringLiteral(value) || ts.isNoSubstitutionTemplateLiteral(value) || ts.isTemplateExpression(value)) {
    return true;
  }
  if (ts.isArrayLiteralExpression(value)) {
    return value.elements.some((element) => (
      ts.isStringLiteral(element) ||
      ts.isNoSubstitutionTemplateLiteral(element) ||
      ts.isTemplateExpression(element)
    ));
  }
  if (ts.isBinaryExpression(value)) {
    return (
      isLiteralFallbackInitializer(ts, value.left) ||
      isLiteralFallbackInitializer(ts, value.right)
    );
  }
  return false;
}

function collectWebUiLiteralFallbackFindings() {
  const ts = auditTypeScript;
  const findings = [];

  for (const call of collectWebUiTranslationCallFacts()) {
    const options = unwrapTsExpression(ts, call.options);
    if (!options || !ts.isObjectLiteralExpression(options)) continue;

    for (const property of options.properties) {
      if (!ts.isPropertyAssignment(property)) continue;
      if (propertyNameToString(ts, property.name) !== 'defaultValue') continue;
      if (!isLiteralFallbackInitializer(ts, property.initializer)) continue;

      const [namespace, ...keyParts] = call.key.split(':');
      findings.push({
        file: call.file,
        location: call.location,
        namespace: keyParts.length > 0 ? namespace : null,
        key: call.key,
      });
    }
  }

  return findings;
}

function auditWebUiLiteralFallbackBudget() {
  if (!fs.existsSync(literalFallbackBaselinePath)) {
    reportError('Missing scripts/i18n-literal-fallback-baseline.json');
    return;
  }

  const baseline = readJsonFile(literalFallbackBaselinePath);
  const budgetByFile = new Map(
    (baseline.budgets ?? []).map((entry) => [entry.path, entry]),
  );
  const findingsByFile = new Map();

  for (const finding of collectWebUiLiteralFallbackFindings()) {
    governanceReport.literalDefaultValueFallbacks.push({
      surface: 'web-ui',
      namespace: finding.namespace,
      key: finding.key,
      file: finding.file,
      location: finding.location,
      reason: 'literal-i18next-defaultValue',
    });
    findingsByFile.set(finding.file, [...(findingsByFile.get(finding.file) ?? []), finding]);
  }

  for (const [file, findings] of findingsByFile.entries()) {
    const budget = budgetByFile.get(file);
    if (!budget) {
      reportError(
        `${file} has ${findings.length} literal i18next defaultValue fallback(s) but is missing from scripts/i18n-literal-fallback-baseline.json. First entries: ${
          findings.slice(0, 8).map((finding) => `${finding.location} ${finding.key}`).join(', ')
        }`,
      );
      continue;
    }

    const actualCountByKey = new Map();
    for (const finding of findings) {
      actualCountByKey.set(finding.key, (actualCountByKey.get(finding.key) ?? 0) + 1);
    }

    if (Array.isArray(budget.literalDefaultValues) || isPlainObject(budget.literalDefaultValues)) {
      const allowedCountByKey = new Map(
        Array.isArray(budget.literalDefaultValues)
          ? budget.literalDefaultValues.map((entry) => [entry.key, entry.count])
          : Object.entries(budget.literalDefaultValues),
      );

      for (const [key, count] of actualCountByKey.entries()) {
        const allowed = allowedCountByKey.get(key);
        if (typeof allowed !== 'number') {
          reportError(`${file} has unbudgeted literal i18next defaultValue fallback for "${key}"`);
        } else if (count > allowed) {
          reportError(`${file} has ${count} literal i18next defaultValue fallback(s) for "${key}", budget is ${allowed}`);
        } else if (count < allowed) {
          reportError(`${file} has ${count} literal i18next defaultValue fallback(s) for "${key}", below baseline ${allowed}; lower scripts/i18n-literal-fallback-baseline.json.`);
        }
      }

      for (const [key, allowed] of allowedCountByKey.entries()) {
        if (allowed > 0 && !actualCountByKey.has(key)) {
          reportError(`${file} no longer has a literal i18next defaultValue fallback for "${key}"; lower scripts/i18n-literal-fallback-baseline.json.`);
        }
      }
    } else if (typeof budget.maxLiteralDefaultValues === 'number') {
      if (findings.length > budget.maxLiteralDefaultValues) {
        reportError(
          `${file} has ${findings.length} literal i18next defaultValue fallback(s), budget is ${budget.maxLiteralDefaultValues}. First entries: ${
            findings.slice(0, 8).map((finding) => `${finding.location} ${finding.key}`).join(', ')
          }`,
        );
      } else if (findings.length < budget.maxLiteralDefaultValues) {
        reportError(
          `${file} has ${findings.length} literal i18next defaultValue fallback(s), below baseline ${budget.maxLiteralDefaultValues}; lower scripts/i18n-literal-fallback-baseline.json.`,
        );
      }
    } else {
      reportError(`${file} has an invalid literal fallback baseline entry`);
    }
  }

  for (const [file, budget] of budgetByFile.entries()) {
    const hasBudgetedFindings = Array.isArray(budget.literalDefaultValues) || isPlainObject(budget.literalDefaultValues)
      ? Object.keys(budget.literalDefaultValues).length > 0
      : budget.maxLiteralDefaultValues > 0;
    if (hasBudgetedFindings && !findingsByFile.has(file)) {
      reportError(
        `${file} no longer has literal i18next defaultValue fallback(s); remove it from scripts/i18n-literal-fallback-baseline.json.`,
      );
    }
  }
}

function countCjkSourceLines(scanRoot, predicate) {
  const cjkPattern = /\p{Script=Han}/u;
  const findings = [];
  const sourceFiles = listFiles(scanRoot, predicate);

  for (const file of sourceFiles) {
    const text = fs.readFileSync(file, 'utf8');
    const lines = text.split(/\r?\n/);
    lines.forEach((line, index) => {
      if (cjkPattern.test(line)) {
        findings.push(`${toPosixPath(path.relative(root, file))}:${index + 1}`);
      }
    });
  }

  return findings;
}

function shouldSkipLocaleFormatSourceScan(file) {
  const normalized = toPosixPath(path.relative(root, file));
  return (
    // Surface i18n owners are the only approved locations for direct Intl usage;
    // product code must call their exported formatting helpers instead.
    normalized === 'src/web-ui/src/infrastructure/i18n/core/I18nService.ts' ||
    normalized === 'src/mobile-web/src/i18n/I18nProvider.tsx' ||
    normalized.endsWith('/generatedLocaleContract.ts') ||
    normalized.endsWith('.test.ts') ||
    normalized.endsWith('.test.tsx') ||
    normalized.endsWith('.spec.ts') ||
    normalized.endsWith('.spec.tsx')
  );
}

function createLocaleFormatScanSpecs() {
  return [
    {
      surface: 'web-ui',
      root: webSourceDir,
      predicate: (file) => (
        (file.endsWith('.ts') || file.endsWith('.tsx')) &&
        !shouldSkipSourceScan(file) &&
        !shouldSkipLocaleFormatSourceScan(file)
      ),
    },
    {
      surface: 'mobile-web',
      root: mobileWebSourceDir,
      predicate: (file) => (
        (file.endsWith('.ts') || file.endsWith('.tsx')) &&
        !shouldSkipMobileWebSourceScan(file) &&
        !shouldSkipLocaleFormatSourceScan(file)
      ),
    },
    {
      surface: 'installer',
      root: installerSourceDir,
      predicate: (file) => (
        (file.endsWith('.ts') || file.endsWith('.tsx')) &&
        !shouldSkipInstallerSourceScan(file) &&
        !shouldSkipLocaleFormatSourceScan(file)
      ),
    },
    {
      surface: 'core-miniapp',
      root: path.join(root, 'src', 'crates', 'contracts', 'product-domains', 'src', 'miniapp', 'builtin', 'assets'),
      predicate: (file) => file.endsWith('.js'),
    },
  ];
}

function collectLocaleFormatSurfaceIds() {
  return sortedUnique(createLocaleFormatScanSpecs().map((spec) => spec.surface));
}

function collectLocaleFormatUsageFindings() {
  const formatPattern = /\b(?:new\s+)?Intl\.(?:DateTimeFormat|NumberFormat|RelativeTimeFormat)\s*\(|\.\s*toLocale(?:String|DateString|TimeString)\s*\(/g;
  const specs = createLocaleFormatScanSpecs();
  const findings = [];

  for (const spec of specs) {
    for (const file of listFiles(spec.root, spec.predicate)) {
      const relativeFile = toPosixPath(path.relative(root, file));
      const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
      lines.forEach((line, index) => {
        formatPattern.lastIndex = 0;
        let match;
        while ((match = formatPattern.exec(line)) != null) {
          findings.push({
            surface: spec.surface,
            file: relativeFile,
            location: `${relativeFile}:${index + 1}`,
            expression: match[0].trim(),
            snippet: line.trim().slice(0, 200),
            reason: 'direct-locale-format-call',
          });
        }
      });
    }
  }

  return findings.sort(sortByReportIdentity);
}

function auditLocaleFormatUsageBudget() {
  if (!fs.existsSync(localeFormatBaselinePath)) {
    reportError('Missing scripts/i18n-locale-format-baseline.json');
    return;
  }

  const baseline = readJsonFile(localeFormatBaselinePath);
  if (baseline.version !== 1) {
    reportError('scripts/i18n-locale-format-baseline.json must use version 1');
  }
  if (!Array.isArray(baseline.budgets)) {
    reportError('scripts/i18n-locale-format-baseline.json must define a budgets array');
    return;
  }

  const budgetByFile = new Map((baseline.budgets ?? []).map((budget) => [budget.path, budget]));
  const findingsByFile = new Map();

  for (const finding of collectLocaleFormatUsageFindings()) {
    governanceReport.localeFormatCandidates.push(finding);
    findingsByFile.set(finding.file, [...(findingsByFile.get(finding.file) ?? []), finding]);
  }

  for (const [file, findings] of findingsByFile.entries()) {
    const budget = budgetByFile.get(file);
    if (!budget) {
      reportError(
        `${file} has ${findings.length} direct locale formatting call(s) but is missing from scripts/i18n-locale-format-baseline.json. First entries: ${
          findings.slice(0, 8).map((finding) => `${finding.location} ${finding.expression}`).join(', ')
        }`,
      );
      continue;
    }

    if (typeof budget.maxLocaleFormatCalls !== 'number') {
      reportError(`${file} has an invalid locale format baseline entry`);
    } else if (findings.length > budget.maxLocaleFormatCalls) {
      reportError(`${file} has ${findings.length} direct locale formatting call(s), budget is ${budget.maxLocaleFormatCalls}`);
    } else if (findings.length < budget.maxLocaleFormatCalls) {
      reportError(`${file} has ${findings.length} direct locale formatting call(s), below baseline ${budget.maxLocaleFormatCalls}; lower scripts/i18n-locale-format-baseline.json.`);
    }
  }

  for (const [file, budget] of budgetByFile.entries()) {
    if (budget.maxLocaleFormatCalls > 0 && !findingsByFile.has(file)) {
      reportError(`${file} no longer has direct locale formatting call(s); remove it from scripts/i18n-locale-format-baseline.json.`);
    }
  }
}

function auditHardcodedSourceBudgets() {
  const baseline = readJsonFile(hardcodedBaselinePath);
  const budgetById = new Map((baseline.budgets ?? []).map((budget) => [budget.id, budget.maxCjkLines]));
  // Baselines are a no-new-hardcoded-copy gate. Lower them as strings move to
  // owned locale resources; do not raise them for new user-facing text.
  const specs = [
    {
      id: 'web-ui-source',
      root: webSourceDir,
      predicate: (file) => (file.endsWith('.ts') || file.endsWith('.tsx')) && !shouldSkipSourceScan(file),
    },
    {
      id: 'mobile-web-source',
      root: mobileWebSourceDir,
      predicate: (file) => (file.endsWith('.ts') || file.endsWith('.tsx')) && !shouldSkipMobileWebSourceScan(file),
    },
    {
      id: 'installer-source',
      root: installerSourceDir,
      predicate: (file) => (file.endsWith('.ts') || file.endsWith('.tsx')) && !shouldSkipInstallerSourceScan(file),
    },
    {
      id: 'relay-static-homepage',
      root: relayHomepageDir,
      predicate: (file) => file.endsWith('.html') || file.endsWith('.js') || file.endsWith('.css'),
    },
  ];

  for (const spec of specs) {
    const maxCjkLines = budgetById.get(spec.id);
    if (typeof maxCjkLines !== 'number') {
      reportError(`Missing hardcoded CJK budget for ${spec.id}`);
      continue;
    }

    const findings = countCjkSourceLines(spec.root, spec.predicate);
    if (findings.length > maxCjkLines) {
      reportError(`${spec.id} has ${findings.length} CJK source candidate line(s), budget is ${maxCjkLines}. First entries: ${findings.slice(0, 12).join(', ')}`);
    } else if (findings.length > 0) {
      reportWarning(`${spec.id} has ${findings.length} grandfathered CJK source candidate line(s). First entries: ${findings.slice(0, 12).join(', ')}`);
    }
  }
}

auditGeneratedContract();
auditSharedTermsCoverage();
auditSurfaceResourceRoots();
auditMobileWebBoundary();

const namespaces = auditNamespaceCoverage();
auditKeyParity(namespaces);
auditWebI18nextPlaceholderParity(namespaces);
auditTypeScript = loadTypeScriptForAudit();
if (auditTypeScript) {
  auditWebUiStaticTranslationKeys(namespaces);
  auditWebUiLiteralFallbackBudget();
  auditMobileWebMessageParity();
  auditMobileWebPlaceholderParity();
}
auditInstallerKeyParity();
auditInstallerPlaceholderParity();
auditCoreFluentParity();
auditRelayStaticHomepageResources();
auditSourceText();
auditLocaleFormatUsageBudget();
auditHardcodedSourceBudgets();
auditI18nGovernanceReport(namespaces);
writeGovernanceReport();

if (errorCount > 0) {
  console.error(`[i18n:audit] Failed with ${errorCount} error(s) and ${warningCount} warning(s).`);
  process.exit(1);
}

console.log(`[i18n:audit] Passed with ${warningCount} warning(s).`);
