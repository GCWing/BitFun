import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
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
const localeRegistryPath = path.join(
  root,
  'src',
  'web-ui',
  'src',
  'infrastructure',
  'i18n',
  'presets',
  'localeRegistry.ts',
);
const webSourceDir = path.join(root, 'src', 'web-ui', 'src');
const supportedLocales = fs
  .readdirSync(webLocalesDir, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();
const baselineLocale = supportedLocales.includes('en-US') ? 'en-US' : supportedLocales[0];

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

function listLocaleNamespaces(locale) {
  const localeDir = path.join(webLocalesDir, locale);
  return listFiles(localeDir, (file) => file.endsWith('.json'))
    .map((file) => toPosixPath(path.relative(localeDir, file)).replace(/\.json$/, ''))
    .sort();
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
  const source = fs.readFileSync(localeRegistryPath, 'utf8');
  return Array.from(source.matchAll(/\bid:\s*['"]([^'"]+)['"]/g))
    .map((item) => item[1])
    .sort();
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
  const file = path.join(webLocalesDir, locale, `${namespace}.json`);
  try {
    return flattenKeys(JSON.parse(fs.readFileSync(file, 'utf8')));
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
    reportError(`${locale} locale directory exists but is not in builtinLocales`);
  }
  for (const locale of registryLocales.filter((item) => !supportedLocales.includes(item))) {
    reportError(`builtinLocales includes ${locale} but no matching locale directory exists`);
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

function auditKeyParity(namespaces) {
  for (const namespace of namespaces) {
    const baselineKeys = readJsonKeys(baselineLocale, namespace);
    for (const locale of supportedLocales.filter((item) => item !== baselineLocale)) {
      const localeKeys = readJsonKeys(locale, namespace);
      const missing = diffSets(baselineKeys, localeKeys);
      const extra = diffSets(localeKeys, baselineKeys);

      if (missing.length > 0) {
        reportWarning(`${locale}/${namespace}.json is missing ${missing.length} key(s): ${missing.slice(0, 8).join(', ')}`);
      }
      if (extra.length > 0) {
        reportWarning(`${locale}/${namespace}.json has ${extra.length} extra key(s): ${extra.slice(0, 8).join(', ')}`);
      }
    }
  }
}

function shouldSkipSourceScan(file) {
  const normalized = toPosixPath(path.relative(root, file));
  return (
    normalized.includes('/locales/') ||
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
    reportWarning(`Found ${cjkFindings.length} CJK source line candidate(s). First entries: ${cjkFindings.slice(0, 12).join(', ')}`);
  }
}

const namespaces = auditNamespaceCoverage();
auditKeyParity(namespaces);
auditSourceText();

if (errorCount > 0) {
  console.error(`[i18n:audit] Failed with ${errorCount} error(s) and ${warningCount} warning(s).`);
  process.exit(1);
}

console.log(`[i18n:audit] Passed with ${warningCount} warning(s).`);
