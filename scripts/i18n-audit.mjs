import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const contractPath = path.join(root, 'src', 'shared', 'i18n', 'contract', 'locales.json');
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

function auditSourceText() {
  const sourceFiles = listFiles(
    webSourceDir,
    (file) => (file.endsWith('.ts') || file.endsWith('.tsx')) && !shouldSkipSourceScan(file),
  );

  const fallbackFindings = [];
  const cjkFindings = [];
  const fallbackPattern = /\bt\s*\(\s*(['"`])(?:\\.|(?!\1).)+\1\s*,\s*(['"`])/g;
  const cjkPattern = /\p{Script=Han}/u;

  for (const file of sourceFiles) {
    const text = fs.readFileSync(file, 'utf8');
    const lines = text.split(/\r?\n/);
    lines.forEach((line, index) => {
      if (fallbackPattern.test(line)) {
        fallbackFindings.push(`${toPosixPath(path.relative(root, file))}:${index + 1}`);
      }
      fallbackPattern.lastIndex = 0;

      if (cjkPattern.test(line)) {
        cjkFindings.push(`${toPosixPath(path.relative(root, file))}:${index + 1}`);
      }
    });
  }

  if (fallbackFindings.length > 0) {
    reportWarning(`Found ${fallbackFindings.length} t(key, "literal fallback") candidate(s). First entries: ${fallbackFindings.slice(0, 12).join(', ')}`);
  }
  if (cjkFindings.length > 0) {
    reportWarning(`Found ${cjkFindings.length} CJK source line candidate(s) outside locale/test/generated files; review user-facing literals for extraction. First entries: ${cjkFindings.slice(0, 12).join(', ')}`);
  }
}

auditGeneratedContract();
auditSharedTermsCoverage();
auditSurfaceResourceRoots();
auditMobileWebBoundary();

const namespaces = auditNamespaceCoverage();
auditKeyParity(namespaces);
auditSourceText();

if (errorCount > 0) {
  console.error(`[i18n:audit] Failed with ${errorCount} error(s) and ${warningCount} warning(s).`);
  process.exit(1);
}

console.log(`[i18n:audit] Passed with ${warningCount} warning(s).`);
