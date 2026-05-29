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
const relayHomepageDir = path.join(root, 'src', 'apps', 'relay-server', 'static', 'homepage');
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

function readInstallerJsonKeys(uiLocale) {
  const file = path.join(installerLocalesDir, `${uiLocale}.json`);
  try {
    return flattenKeys(readJsonFile(file));
  } catch (error) {
    reportError(`Failed to parse ${toPosixPath(path.relative(root, file))}: ${error.message}`);
    return [];
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

function readMobileMessageKeysByLocale() {
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

        output.set(locale, flattenTsObjectKeys(ts, value));
      }
    }
    ts.forEachChild(node, visit);
  }

  visit(sourceFile);
  return output;
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
    reportWarning(`Found ${fallbackFindings.length} t(key, "literal fallback") candidate(s). First entries: ${fallbackFindings.slice(0, 12).join(', ')}`);
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
auditMobileWebMessageParity();
auditInstallerKeyParity();
auditSourceText();
auditHardcodedSourceBudgets();

if (errorCount > 0) {
  console.error(`[i18n:audit] Failed with ${errorCount} error(s) and ${warningCount} warning(s).`);
  process.exit(1);
}

console.log(`[i18n:audit] Passed with ${warningCount} warning(s).`);
