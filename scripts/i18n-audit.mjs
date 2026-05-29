import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const require = createRequire(import.meta.url);
const root = process.cwd();
const contractPath = path.join(root, 'src', 'shared', 'i18n', 'contract', 'locales.json');
const hardcodedBaselinePath = path.join(root, 'scripts', 'i18n-hardcoded-baseline.json');
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
const coreLocalesDir = path.join(root, 'src', 'crates', 'core', 'locales');
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

function reportError(message) {
  errorCount += 1;
  console.error(`[i18n:audit] ERROR ${message}`);
}

function reportWarning(message) {
  warningCount += 1;
  console.warn(`[i18n:audit] WARN ${message}`);
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
  let ts;
  try {
    ts = require('typescript');
  } catch (error) {
    reportError(`Failed to load TypeScript for mobile-web i18n audit: ${error.message}`);
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
    entriesByLocale.set(locale, new Map(flattenStringEntries(messages)));
  }

  return {
    localeIds: Object.keys(resource).sort(),
    entriesByLocale,
  };
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

function collectWebUiStaticTranslationKeys() {
  const sourceFiles = listFiles(
    webSourceDir,
    (file) => (file.endsWith('.ts') || file.endsWith('.tsx')) && !shouldSkipSourceScan(file),
  );
  const output = [];
  const patterns = [
    /i18nService\.t\(\s*(['"`])([^'"`$]+?)\1/g,
    /i18nService\.getT\(\)\(\s*(['"`])([^'"`$]+?)\1/g,
  ];

  for (const file of sourceFiles) {
    const text = fs.readFileSync(file, 'utf8');
    for (const pattern of patterns) {
      for (const match of text.matchAll(pattern)) {
        const key = match[2];
        if (!key.includes(':')) continue;
        output.push({
          key,
          location: `${toPosixPath(path.relative(root, file))}:${lineNumberAt(text, match.index ?? 0)}`,
        });
      }
    }
  }

  return output;
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
    reportError(
      `Found ${missing.length} unknown static Web UI i18n key(s). First entries: ${
        missing.slice(0, 12).map(({ key, location }) => `${location} ${key}`).join(', ')
      }`,
    );
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
auditWebUiStaticTranslationKeys(namespaces);
auditMobileWebMessageParity();
auditMobileWebPlaceholderParity();
auditInstallerKeyParity();
auditInstallerPlaceholderParity();
auditCoreFluentParity();
auditRelayStaticHomepageResources();
auditSourceText();
auditHardcodedSourceBudgets();

if (errorCount > 0) {
  console.error(`[i18n:audit] Failed with ${errorCount} error(s) and ${warningCount} warning(s).`);
  process.exit(1);
}

console.log(`[i18n:audit] Passed with ${warningCount} warning(s).`);
