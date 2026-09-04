import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '..');
const mobileRoot = path.join(repoRoot, 'src/apps/mobile');
const sharedRoot = path.join(mobileRoot, 'shared');

// The complete set of edges the production dependency graph is allowed to
// contain. :core-domain is pure logic, so transport and
// persistence are absent from its row on purpose — they are adapters behind
// ports that :core-feature wires.
const allowedProductionDeps = new Map([
  ['core-protocol', []],
  ['core-crypto', ['core-protocol']],
  ['core-transport', ['core-protocol', 'core-crypto']],
  ['core-persistence', ['core-protocol']],
  ['core-domain', ['core-protocol']],
  ['core-feature', ['core-domain', 'core-transport', 'core-persistence']],
  ['core-testing', []]
]);

// Rule 3: the seam. Platform apps see UiState and Intent from :core-feature and
// nothing else.
const appVisibleModules = ['core-feature'];

// Rule 6: shared/ is platform-agnostic; these trees must stay invisible to it.
const forbiddenTrees = ['harmonyos', 'src/apps/desktop', 'src/web-ui', 'src/mobile-web'];

const modules = [...allowedProductionDeps.keys()];

function relative(file) {
  return path.relative(repoRoot, file).split(path.sep).join('/');
}

function walk(root, extension) {
  if (!fs.existsSync(root)) {
    return [];
  }
  const files = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'build' || entry.name === '.gradle' || entry.name === '.kotlin') {
        continue;
      }
      files.push(...walk(entryPath, extension));
    } else if (entry.isFile() && entry.name.endsWith(extension)) {
      files.push(entryPath);
    }
  }
  return files.sort();
}

/**
 * Blanks out `//` and block comments while preserving line numbering, so the
 * content checks below judge code rather than prose. A comment can document a
 * platform tree or carry a Chinese note without creating a dependency on it.
 */
function stripComments(source) {
  let output = '';
  let inBlock = false;
  for (const line of source.split(/\r?\n/)) {
    let kept = '';
    let index = 0;
    while (index < line.length) {
      if (inBlock) {
        const end = line.indexOf('*/', index);
        if (end === -1) {
          index = line.length;
        } else {
          inBlock = false;
          index = end + 2;
        }
        continue;
      }
      if (line.startsWith('//', index)) break;
      if (line.startsWith('/*', index)) {
        inBlock = true;
        index += 2;
        continue;
      }
      kept += line[index];
      index += 1;
    }
    output += `${kept}\n`;
  }
  return output;
}

/**
 * Extracts the body of every `<sourceSet>.dependencies { ... }` block, keyed by
 * source set name, by matching braces rather than trusting a flat regex.
 */
function dependencyBlocks(source) {
  const blocks = [];
  const opener = /(\w+)\.dependencies\s*\{/g;
  let match;
  while ((match = opener.exec(source)) !== null) {
    let depth = 1;
    let cursor = opener.lastIndex;
    while (cursor < source.length && depth > 0) {
      if (source[cursor] === '{') depth += 1;
      else if (source[cursor] === '}') depth -= 1;
      cursor += 1;
    }
    blocks.push({ sourceSet: match[1], body: source.slice(opener.lastIndex, cursor - 1) });
  }
  return blocks;
}

/**
 * Splits a parameter list on the commas that separate parameters, ignoring the
 * ones inside generics, nested calls and strings, and reports whether each
 * parameter carries a default value.
 */
function parametersOf(list) {
  const parameters = [];
  let current = '';
  let depth = 0;
  let angle = 0;
  let quote = null;
  for (let index = 0; index < list.length; index += 1) {
    const character = list[index];
    if (quote) {
      if (character === '\\') {
        current += character + (list[index + 1] ?? '');
        index += 1;
        continue;
      }
      if (character === quote) quote = null;
      current += character;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      current += character;
      continue;
    }
    if (character === '(' || character === '[') depth += 1;
    else if (character === ')' || character === ']') depth -= 1;
    else if (character === '<') angle += 1;
    else if (character === '>') angle = Math.max(0, angle - 1);
    if (character === ',' && depth === 0 && angle === 0) {
      parameters.push(current);
      current = '';
      continue;
    }
    current += character;
  }
  if (current.trim()) parameters.push(current);
  return parameters;
}

function hasDefault(parameter) {
  let depth = 0;
  let angle = 0;
  let quote = null;
  for (let index = 0; index < parameter.length; index += 1) {
    const character = parameter[index];
    if (quote) {
      if (character === '\\') index += 1;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      continue;
    }
    if (character === '(' || character === '[') depth += 1;
    else if (character === ')' || character === ']') depth -= 1;
    else if (character === '<') angle += 1;
    else if (character === '>') angle = Math.max(0, angle - 1);
    else if (
      character === '=' &&
      depth === 0 &&
      angle === 0 &&
      parameter[index + 1] !== '=' &&
      parameter[index - 1] !== '!' &&
      parameter[index - 1] !== '<' &&
      parameter[index - 1] !== '>'
    ) {
      return true;
    }
  }
  return false;
}

/**
 * Finds public declarations whose parameter list carries a default value.
 *
 * Three shapes qualify: a public function, a public secondary constructor, and
 * the primary constructor of a public class — the last only when it is not
 * explicitly narrowed with `internal constructor` or `private constructor`,
 * since a narrowed one is not part of the public surface.
 */
function publicDefaults(source) {
  const findings = [];
  const declaration =
    /\bpublic\s+(?:(?:inline|infix|operator|suspend|expect|actual|external|tailrec|override)\s+)*(?:(fun)\s+[\w<>.,\s?]*?\(|(constructor)\s*\(|(?:(?:data|value|sealed|abstract|open|annotation|inner)\s+)*class\s+\w+\s*(?:<[^>()]*>)?\s*(?:(internal|private|protected)\s+)?(?:constructor\s*)?\()/g;
  let match;
  while ((match = declaration.exec(source)) !== null) {
    // A primary constructor narrowed below public is not public API. Group 3 —
    // the only other capturing groups are `fun` and `constructor`.
    if (match[3]) continue;
    const open = source.lastIndexOf('(', declaration.lastIndex);
    let depth = 1;
    let cursor = open + 1;
    while (cursor < source.length && depth > 0) {
      if (source[cursor] === '(') depth += 1;
      else if (source[cursor] === ')') depth -= 1;
      cursor += 1;
    }
    const list = source.slice(open + 1, cursor - 1);
    if (!parametersOf(list).some(hasDefault)) continue;
    const line = source.slice(0, match.index).split('\n').length;
    findings.push({ line, text: source.slice(match.index, open).trim() });
  }
  return findings;
}

function projectRefs(body) {
  return [...body.matchAll(/project\(\s*["']:([\w-]+)["']\s*\)/g)].map((match) => match[1]);
}

const buildFiles = new Map(
  modules
    .filter((module) => fs.existsSync(path.join(sharedRoot, module, 'build.gradle.kts')))
    .map((module) => [module, fs.readFileSync(path.join(sharedRoot, module, 'build.gradle.kts'), 'utf8')])
);

const missingModules = modules.filter((module) => !buildFiles.has(module));

// ── Rule 1: commonMain stays free of platform APIs ────────────────────
const platformImportsInCommonMain = [];
for (const module of buildFiles.keys()) {
  const commonMain = path.join(sharedRoot, module, 'src/commonMain');
  for (const file of walk(commonMain, '.kt')) {
    const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
    lines.forEach((line, index) => {
      if (/^\s*import\s+(java|javax|android)\./.test(line)) {
        platformImportsInCommonMain.push(`${relative(file)}:${index + 1}`);
      }
    });
  }
}

// ── Rules 2 and 4: the production dependency graph ────────────────────
const forbiddenModuleDeps = [];
const testOnlyModuleLeaks = [];
const productionGraph = new Map();
for (const [module, source] of buildFiles) {
  const allowed = allowedProductionDeps.get(module);
  const production = new Set();
  for (const block of dependencyBlocks(stripComments(source))) {
    const isTest = block.sourceSet.endsWith('Test');
    for (const dep of projectRefs(block.body)) {
      if (isTest) {
        // Any module may lean on :core-testing from a test source set.
        if (dep !== 'core-testing' && !allowed.includes(dep)) {
          forbiddenModuleDeps.push(`${module}:${block.sourceSet} -> ${dep}`);
        }
        continue;
      }
      production.add(dep);
      if (dep === 'core-testing') {
        testOnlyModuleLeaks.push(`${module}:${block.sourceSet} -> core-testing`);
      } else if (!allowed.includes(dep)) {
        forbiddenModuleDeps.push(`${module}:${block.sourceSet} -> ${dep}`);
      }
    }
  }
  productionGraph.set(module, [...production]);
}

// Cycle detection over the production graph. The allowlist above is already
// acyclic, but it is edited by hand and this is the cheap guard against a
// future edit that is locally reasonable and globally circular.
const dependencyCycles = [];
const visitState = new Map();
function visit(module, stack) {
  if (visitState.get(module) === 'done') return;
  if (visitState.get(module) === 'visiting') {
    dependencyCycles.push([...stack.slice(stack.indexOf(module)), module].join(' -> '));
    return;
  }
  visitState.set(module, 'visiting');
  for (const dep of productionGraph.get(module) ?? []) {
    visit(dep, [...stack, module]);
  }
  visitState.set(module, 'done');
}
for (const module of productionGraph.keys()) {
  visit(module, []);
}

// ── Rule 3: platform apps only reach :core-feature ────────────────────
const appReachesInternalModules = [];
for (const platform of ['android', 'ios']) {
  const platformRoot = path.join(mobileRoot, platform);
  const sources = [...walk(platformRoot, '.kts'), ...walk(platformRoot, '.kt'), ...walk(platformRoot, '.swift')];
  for (const file of sources) {
    const source = fs.readFileSync(file, 'utf8');
    for (const module of modules) {
      if (appVisibleModules.includes(module)) continue;
      const packageSuffix = module.replace(/^core-/, '');
      // Two spellings, because an app includes shared/ as a composite build
      // rather than as subprojects: `project(":core-x")` inside shared/, and the
      // substituted coordinate `"com.openbitfun.mobile:core-x"` (optionally
      // versioned) from an app. Matching only the first would let an app declare
      // a forbidden edge in the one form it is actually able to write.
      const referencesModule =
        new RegExp(`["'][\\w.-]*:${module}(?::[^"']*)?["']`).test(source) ||
        new RegExp(`com\\.openbitfun\\.mobile\\.core\\.${packageSuffix}\\b`).test(source);
      if (referencesModule) {
        appReachesInternalModules.push(`${relative(file)} -> ${module}`);
      }
    }
  }
}

// ── §4.1 rule 2: no default arguments on the Swift-facing surface ─────
// Kotlin default arguments do not survive into Swift — an iOS caller sees the
// full initializer and has to name every parameter, including the ones the
// default existed to hide.
//
// Two modules are in scope, not one: :core-feature because the apps see it, and
// :core-domain because :core-feature re-exports it with api(), which puts its
// public types on the very same Swift surface. The rest sit behind
// implementation() and never reach Swift, so defaults there are ordinary Kotlin.
//
// The scan keys on the `public` keyword, which is only trustworthy if the
// compiler requires it — hence the explicitApi() precondition below. Without
// it a declaration with no visibility modifier is public too, and would slip past.
const swiftFacingModules = ['core-feature', 'core-domain'];
const featureApiNotExplicit = [];
const defaultArgsInFeatureApi = [];
for (const module of swiftFacingModules) {
  const buildFile = path.join(sharedRoot, module, 'build.gradle.kts');
  // Comments stripped first: a commented-out explicitApi() must not satisfy the
  // precondition, or the scan below silently goes back to being unsound.
  if (!/\bexplicitApi\s*\(/.test(stripComments(fs.readFileSync(buildFile, 'utf8')))) {
    featureApiNotExplicit.push(relative(buildFile));
  }
  for (const file of walk(path.join(sharedRoot, module, 'src/commonMain'), '.kt')) {
    const source = stripComments(fs.readFileSync(file, 'utf8'));
    for (const finding of publicDefaults(source)) {
      defaultArgsInFeatureApi.push(`${relative(file)}:${finding.line} ${finding.text}`);
    }
  }
}

// ── Rule 5: the core never returns display strings ────────────────────
// Approximated as "no CJK anywhere in commonMain". Every localized string in
// this repo has a CJK counterpart, so a Chinese character in shared logic means
// a display string escaped into the core instead of a typed state.
const localizedStringsInCore = [];
for (const module of buildFiles.keys()) {
  for (const file of walk(path.join(sharedRoot, module, 'src'), '.kt')) {
    const lines = stripComments(fs.readFileSync(file, 'utf8')).split(/\r?\n/);
    lines.forEach((line, index) => {
      if (/[一-鿿]/.test(line)) {
        localizedStringsInCore.push(`${relative(file)}:${index + 1}`);
      }
    });
  }
}

// ── Rule 6: shared/ never reaches into a platform tree ────────────────
const sharedReachesPlatformTrees = [];
for (const file of [...walk(sharedRoot, '.kt'), ...walk(sharedRoot, '.kts'), ...walk(sharedRoot, '.toml')]) {
  const lines = stripComments(fs.readFileSync(file, 'utf8')).split(/\r?\n/);
  lines.forEach((line, index) => {
    for (const tree of forbiddenTrees) {
      if (line.includes(tree)) {
        sharedReachesPlatformTrees.push(`${relative(file)}:${index + 1} -> ${tree}`);
      }
    }
  });
}

// ── Rule 8: the two Android string tables stay in sync ───────────────
// Rule 5 pushes every user-facing word out of the core and into resources, so
// resources are the only place a missing translation can hide. A key present in
// one table and absent from the other ships as English to a Chinese user — the
// exact regression this rule was added after.
const androidValues = path.join(mobileRoot, 'android/app/src/main/res');
// Brand names are deliberately identical in both locales; duplicating them
// would only invite a "translation" that shouldn't exist.
const untranslatedByDesign = new Set(['app_name']);

function stringKeys(file) {
  if (!fs.existsSync(file)) return null;
  return new Set(
    [...fs.readFileSync(file, 'utf8').matchAll(/<string\s+name="([^"]+)"/g)].map((match) => match[1])
  );
}

const stringTablesOutOfSync = [];
const baseStrings = stringKeys(path.join(androidValues, 'values/strings.xml'));
const zhStrings = stringKeys(path.join(androidValues, 'values-zh/strings.xml'));
if (baseStrings === null || zhStrings === null) {
  stringTablesOutOfSync.push('values/strings.xml or values-zh/strings.xml is missing');
} else {
  for (const key of baseStrings) {
    if (!zhStrings.has(key) && !untranslatedByDesign.has(key)) {
      stringTablesOutOfSync.push(`values-zh/strings.xml is missing ${key}`);
    }
  }
  for (const key of zhStrings) {
    if (!baseStrings.has(key)) {
      stringTablesOutOfSync.push(`values/strings.xml is missing ${key}`);
    }
  }
  stringTablesOutOfSync.sort();
}

const expected = {
  missingModules: [],
  platformImportsInCommonMain: [],
  forbiddenModuleDeps: [],
  testOnlyModuleLeaks: [],
  dependencyCycles: [],
  appReachesInternalModules: [],
  localizedStringsInCore: [],
  sharedReachesPlatformTrees: [],
  featureApiNotExplicit: [],
  defaultArgsInFeatureApi: [],
  stringTablesOutOfSync: []
};

const actual = {
  missingModules,
  platformImportsInCommonMain,
  forbiddenModuleDeps,
  testOnlyModuleLeaks,
  dependencyCycles,
  appReachesInternalModules,
  localizedStringsInCore,
  sharedReachesPlatformTrees,
  featureApiNotExplicit,
  defaultArgsInFeatureApi,
  stringTablesOutOfSync
};

let failed = false;
for (const [name, wanted] of Object.entries(expected)) {
  const found = actual[name];
  if (found.length !== wanted.length || !found.every((item, index) => item === wanted[index])) {
    failed = true;
    console.error(`${name} mismatch`);
    console.error(`expected: ${JSON.stringify(wanted)}`);
    console.error(`actual:   ${JSON.stringify(found)}`);
  }
}

if (failed) {
  process.exitCode = 1;
} else {
  console.log('Mobile shared core architecture contracts are satisfied.');
}
