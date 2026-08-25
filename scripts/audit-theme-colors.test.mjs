import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  COLOR_DOMAIN_CONTRACTS,
  COLOR_DOMAIN_KEYS,
  COLOR_DOMAIN_RULES,
  DYNAMIC_VAR_FAMILY_CONTRACTS,
  FALLBACK_VAR_CONTRACTS,
  PACKAGE_CSS_VAR_DEFINITION_CONTRACTS,
  SURFACE_TOKEN_RENAME_CONTRACTS,
  TOKEN_COMPATIBILITY_ALIAS_CONTRACTS,
  TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS,
} from './theme-css-var-contract.mjs';

const root = process.cwd();
const SOURCE_OWNER_ROOTS = [
  'BitFun-Installer/src',
  'src/mobile-web/src',
  'src/web-ui/src',
];
const NEAR_PAIR_DECISION_AUDIT_ROOTS = [
  { root: 'src/web-ui/src', args: ['--json', '--no-baseline', '--top', '0'] },
  { root: 'src/mobile-web/src', args: ['--root', 'src/mobile-web/src', '--json', '--no-baseline', '--top', '0'] },
  { root: 'BitFun-Installer/src', args: ['--root', 'BitFun-Installer/src', '--json', '--no-baseline', '--top', '0'] },
];

function contractOwnerHasKnownSource(owner) {
  return String(owner ?? '')
    .split(';')
    .map(entry => entry.trim())
    .filter(Boolean)
    .some(entry => SOURCE_OWNER_ROOTS.some(sourceRoot => (
      entry === sourceRoot
      || entry.startsWith(`${sourceRoot}/`)
    )));
}

function contractOwnerMatchesRoot(contract, sourceRoot) {
  return String(contract.owner ?? '')
    .split(';')
    .map(entry => entry.trim())
    .filter(Boolean)
    .some(entry => (
      entry === sourceRoot
      || entry.startsWith(`${sourceRoot}/`)
      || sourceRoot.startsWith(`${entry}/`)
    ));
}

function writeText(filePath, content) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, 'utf8');
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function readText(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

function collectRepositoryNearPairRows(sourceRoot, report) {
  return COLOR_DOMAIN_KEYS.flatMap((domain) => {
    const pairs = report.colorDomainNearPairs?.[domain];
    if (!pairs) {
      return [];
    }
    return [
      ...(pairs.indistinguishable ?? []),
      ...(pairs.near ?? []),
    ].map(pair => ({
      root: sourceRoot,
      domain,
      key: pair.key,
    }));
  }).sort((left, right) => (
    left.root.localeCompare(right.root)
    || left.domain.localeCompare(right.domain)
    || left.key.localeCompare(right.key)
  ));
}

function formatNearPairDecisionKey(row) {
  return `${row.root}:${row.domain}:${row.key}`;
}

function createFixture(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bitfun-theme-audit-'));
  const sourceRoot = path.join(dir, 'src', 'web-ui', 'src');
  for (const [relativePath, content] of Object.entries(files)) {
    writeText(path.join(sourceRoot, relativePath), content);
  }
  return { dir, sourceRoot };
}

function runAudit(args) {
  return spawnSync(process.execPath, ['scripts/audit-theme-colors.mjs', ...args], {
    cwd: root,
    encoding: 'utf8',
  });
}

test('theme CSS var contract registry is explicit and non-overlapping', () => {
  const domainKeys = new Set(COLOR_DOMAIN_RULES.map(rule => rule.key));
  assert.equal(domainKeys.size, COLOR_DOMAIN_RULES.length, 'color domain keys must be unique');
  assert.ok(COLOR_DOMAIN_KEYS.includes('appUi'), 'app UI must remain the fallback color domain');
  for (const rule of COLOR_DOMAIN_RULES) {
    assert.equal(typeof rule.label, 'string');
    assert.ok(rule.label.trim(), `${rule.key} must have a label`);
    assert.ok(Array.isArray(rule.pathParts) && rule.pathParts.length > 0, `${rule.key} must have path parts`);
  }

  const dynamicPrefixes = new Set(DYNAMIC_VAR_FAMILY_CONTRACTS.map(contract => contract.prefix));
  assert.equal(
    dynamicPrefixes.size,
    DYNAMIC_VAR_FAMILY_CONTRACTS.length,
    'dynamic CSS var family prefixes must be unique',
  );
  for (const contract of DYNAMIC_VAR_FAMILY_CONTRACTS) {
    assert.match(contract.prefix, /^--[a-z0-9-]+-$/);
    assert.ok(contractOwnerHasKnownSource(contract.owner), `${contract.prefix} must name a source owner`);
    assert.ok(contract.reason.trim().length >= 20, `${contract.prefix} must explain why it is dynamic`);
    if (contract.canonicalPrefix !== undefined) {
      assert.match(contract.canonicalPrefix, /^--[a-z0-9-]+-$/);
    }
  }

  const packageVariableNames = PACKAGE_CSS_VAR_DEFINITION_CONTRACTS
    .flatMap(contract => contract.variables);
  assert.equal(
    new Set(packageVariableNames).size,
    packageVariableNames.length,
    'independent package CSS variable contracts must not overlap',
  );
  for (const contract of PACKAGE_CSS_VAR_DEFINITION_CONTRACTS) {
    assert.match(contract.packageName, /^@bitfun\/[a-z0-9-]+$/);
    assert.ok(contract.owner.startsWith('design-system/packages/'));
    assert.ok(contract.variables.length > 0, `${contract.packageName} must expose variables`);
    assert.equal(
      contract.variables.every(name => /^--bf-[a-z0-9-]+$/.test(name)),
      true,
      `${contract.packageName} must expose canonical --bf-* variables only`,
    );
  }
  assert.ok(packageVariableNames.includes('--bf-space-1'));
  assert.ok(packageVariableNames.includes('--bf-color-surface-canvas'));

  const domainContractKeys = new Set(COLOR_DOMAIN_CONTRACTS.map(contract => contract.key));
  assert.equal(
    domainContractKeys.size,
    COLOR_DOMAIN_CONTRACTS.length,
    'color domain contracts must be unique',
  );
  assert.deepEqual(
    [...domainContractKeys].sort(),
    COLOR_DOMAIN_RULES.map(rule => rule.key).sort(),
    'every specialized color domain must have an owner contract',
  );
  for (const contract of COLOR_DOMAIN_CONTRACTS) {
    assert.ok(contract.owner.includes('src/web-ui/src/'), `${contract.key} must name a source owner`);
    assert.ok(contract.reason.trim().length >= 30, `${contract.key} must explain why the domain exists`);
    assert.ok(contract.mergePolicy.trim().length >= 30, `${contract.key} must define a merge policy`);
  }

  const compatibilityAliasKeys = new Set(TOKEN_COMPATIBILITY_ALIAS_CONTRACTS.map(contract => contract.key));
  assert.equal(
    compatibilityAliasKeys.size,
    TOKEN_COMPATIBILITY_ALIAS_CONTRACTS.length,
    'compatibility alias keys must be unique',
  );
  for (const contract of TOKEN_COMPATIBILITY_ALIAS_CONTRACTS) {
    assert.match(contract.key, /^--[a-z0-9-]+$/);
    assert.match(contract.canonical, /^--[a-z0-9-]+$/);
    assert.notEqual(contract.key, contract.canonical, `${contract.key} must point to a different canonical token`);
    assert.ok(contract.owner.includes('src/web-ui/src/'), `${contract.key} must name a source owner`);
    assert.ok(contract.reason.trim().length >= 30, `${contract.key} must explain compatibility need`);
    assert.ok(contract.removal.trim().length >= 30, `${contract.key} must define retirement criteria`);
  }

  const compatibilityAliasPrefixes = new Set(TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS.map(contract => contract.prefix));
  assert.equal(
    compatibilityAliasPrefixes.size,
    TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS.length,
    'compatibility alias family prefixes must be unique',
  );
  for (const contract of TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS) {
    assert.match(contract.prefix, /^--[a-z0-9-]+-$/);
    assert.match(contract.canonicalPrefix, /^--[a-z0-9-]+-$/);
    assert.notEqual(contract.prefix, contract.canonicalPrefix, `${contract.prefix} must point to a different family`);
    assert.ok(contract.owner.includes('src/web-ui/src/'), `${contract.prefix} must name a source owner`);
    assert.ok(contract.reason.trim().length >= 30, `${contract.prefix} must explain compatibility need`);
    assert.ok(contract.removal.trim().length >= 30, `${contract.prefix} must define retirement criteria`);
  }

  const fallbackContractKeys = new Set(FALLBACK_VAR_CONTRACTS.map(contract => contract.key));
  assert.equal(fallbackContractKeys.size, FALLBACK_VAR_CONTRACTS.length, 'fallback contracts must be unique');
  for (const contract of FALLBACK_VAR_CONTRACTS) {
    assert.match(contract.key, /^--[a-z0-9-]+$/);
    assert.ok(contract.owner.includes('src/web-ui/src/'), `${contract.key} must name a source owner`);
    assert.ok(contract.reason.trim().length >= 30, `${contract.key} must explain why fallback is intentional`);
    assert.ok(contract.boundary.trim().length >= 10, `${contract.key} must classify the fallback boundary`);
  }

  const surfaceRenameKeys = new Set(SURFACE_TOKEN_RENAME_CONTRACTS.map(contract => contract.key));
  assert.equal(
    surfaceRenameKeys.size,
    SURFACE_TOKEN_RENAME_CONTRACTS.length,
    'surface token rename contracts must be unique',
  );
  for (const contract of SURFACE_TOKEN_RENAME_CONTRACTS) {
    assert.match(contract.key, /^--[a-z0-9-]+$/);
    assert.match(contract.canonical, /^--[a-z0-9-]+$/);
    assert.notEqual(contract.key, contract.canonical, `${contract.key} must point to a different canonical token`);
    assert.ok(contract.owner.includes('src/web-ui/src/'), `${contract.key} must name a source owner`);
    assert.ok(contract.reason.trim().length >= 30, `${contract.key} must explain the rename boundary`);
  }
});

test('FlowChat heading scale derives from runtime typography without static 4xl root aliases', () => {
  const tokens = readText(path.join(root, 'src/web-ui/src/component-library/styles/tokens.scss'));
  const welcomePanel = readText(path.join(root, 'src/web-ui/src/flow_chat/components/WelcomePanel.css'));
  const navScope = readText(path.join(root, 'src/web-ui/src/app/styles/nav-panel-font-scope.scss'));

  assert.doesNotMatch(
    tokens,
    /^\s*--bf-appearance-token-flowchat-font-size-4xl:\s*var\(--bf-appearance-token-font-size-4xl\);$/m,
    'static root --bf-appearance-token-flowchat-font-size-4xl should not be reintroduced',
  );
  assert.doesNotMatch(
    tokens,
    /^\s*--bf-appearance-token-font-size-4xl:/m,
    'static root --bf-appearance-token-font-size-4xl should not be reintroduced',
  );
  assert.match(
    welcomePanel,
    /font-size:\s*calc\(var\(--bf-appearance-token-flowchat-font-size-3xl\) \+ 4px\);/,
    'Welcome heading should preserve the 4xl visual size by deriving from FlowChat 3xl',
  );
  assert.match(
    navScope,
    /--bf-appearance-token-font-size-4xl:\s*max\(25px, calc\(var\(--bf-appearance-token-flowchat-font-size-3xl\) \+ 4px\)\);/,
    'Nav scope should derive product 4xl from FlowChat 3xl without a static root alias',
  );
});

test('repository dynamic CSS var families match the registered contract', () => {
  for (const sourceRoot of SOURCE_OWNER_ROOTS) {
    const result = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
    assert.equal(result.status, 0, result.stderr || result.stdout);

    const report = JSON.parse(result.stdout);
    const registeredPrefixes = DYNAMIC_VAR_FAMILY_CONTRACTS
      .filter(contract => contractOwnerMatchesRoot(contract, sourceRoot))
      .map(contract => contract.prefix)
      .sort();
    assert.deepEqual(report.cssVarDefinitions.dynamicFamilyPrefixes, registeredPrefixes);
    assert.equal(report.cssVarDefinitions.unregisteredDynamicFamilyUnique, 0);
    assert.equal(report.cssVarDefinitions.staleRegisteredDynamicFamilyUnique, 0);
    assert.equal(report.compatibilityAliases.staleRegisteredUnique, 0);
    assert.equal(report.compatibilityAliases.staleRegisteredFamilyUnique, 0);
    assert.equal(report.compatibilityAliases.missingCanonicalUnique, 0);
    assert.equal(report.generatedWidgetPayload.undefinedUnique, 0);
    assert.equal(report.generatedWidgetPayload.missingCompatibilityCanonicalUnique, 0);
    assert.equal(report.generatedWidgetPayload.unexportedCompatibilityCanonicalUnique, 0);
    assert.equal(report.fallbackContracts.uncontractedUnique, 0);
    assert.equal(report.fallbackContracts.staleRegisteredUnique, 0);
    assert.equal(report.colorDomainContracts.missingRegisteredUnique, 0);
    assert.equal(report.colorDomainContracts.staleRegisteredUnique, 0);
    assert.equal(report.colorDomainContracts.activeUncontractedUnique, 0);
    assert.equal(report.surfaceTokenRenames.activeUnique, 0);
    assert.equal(report.surfaceTokenRenames.activeOccurrences, 0);
    assert.equal(report.surfaceTokenRenames.missingCanonicalUnique, 0);
  }
});

test('plugin appearance projection stays compact and isolated from runtime/widget contracts', () => {
  const projectionSource = readText(path.join(root, 'src/web-ui/src/infrastructure/appearance/adapters/PluginAppearanceProjection.ts'));
  const publicIndexSource = readText(path.join(root, 'src/web-ui/src/infrastructure/appearance/index.ts'));
  const keyListMatch = projectionSource.match(/PLUGIN_APPEARANCE_COLOR_KEYS\s*=\s*\[([\s\S]*?)\]\s+as const;/);
  assert.ok(keyListMatch, 'plugin projection must declare PLUGIN_APPEARANCE_COLOR_KEYS');
  const keyMatches = Array.from(
    keyListMatch[1].matchAll(/'([a-z]+)'/g),
    match => match[1],
  );

  assert.deepEqual(keyMatches, [
    'primary',
    'secondary',
    'accent',
    'success',
    'warning',
    'error',
    'info',
  ]);
  assert.equal(keyMatches.length, 7, 'plugin projection must stay within the documented key cap');
  assert.doesNotMatch(
    projectionSource,
    /ThemeService|appearancePayload|WIDGET_APPEARANCE|getComputedStyle|setProperty|--[a-z0-9-]+/i,
    'plugin projection must not become a runtime CSS var or generated widget schema',
  );
  assert.match(publicIndexSource, /PluginAppearanceProjection/);
  assert.doesNotMatch(
    publicIndexSource,
    /appearancePayload|WIDGET_APPEARANCE/i,
    'Appearance public index must not expose generated widget payload as plugin API',
  );
});

test('repository specialized near color pairs have explicit decisions', () => {
  const reportedRows = NEAR_PAIR_DECISION_AUDIT_ROOTS.flatMap(({ root: sourceRoot, args }) => {
    const result = runAudit(args);
    assert.equal(result.status, 0, result.stderr || result.stdout);

    return collectRepositoryNearPairRows(sourceRoot, JSON.parse(result.stdout));
  });
  const decisions = readJson(path.join(root, 'scripts/theme-color-near-pair-decisions.json'));
  assert.equal(decisions.version, 1);
  assert.ok(Array.isArray(decisions.decisions));

  const reportedKeys = new Set(reportedRows.map(formatNearPairDecisionKey));
  const decisionKeys = new Set(decisions.decisions.map(formatNearPairDecisionKey));
  assert.equal(decisionKeys.size, decisions.decisions.length, 'near-pair decisions must be unique by root, domain, and key');
  const missing = reportedRows
    .filter(row => !decisionKeys.has(formatNearPairDecisionKey(row)))
    .map(formatNearPairDecisionKey);
  const stale = decisions.decisions
    .filter(row => !reportedKeys.has(formatNearPairDecisionKey(row)))
    .map(formatNearPairDecisionKey);

  assert.deepEqual(missing, [], 'new specialized near pairs require an explicit merge/keep/defer decision');
  assert.deepEqual(stale, [], 'retired specialized near-pair decisions must be removed with the lowered baseline');
  for (const decision of decisions.decisions) {
    assert.ok(SOURCE_OWNER_ROOTS.includes(decision.root), `${decision.key} must name a scanned root`);
    assert.ok(['merge', 'keep', 'defer'].includes(decision.decision), `${decision.key} has an invalid decision`);
    assert.ok(String(decision.owner).trim().length > 10, `${decision.key} must name an owner`);
    assert.ok(fs.existsSync(path.join(root, decision.owner)), `${decision.key} owner must exist in the repository`);
    assert.ok(String(decision.reason).trim().length >= 60, `${decision.key} must explain the product/design reason`);
    assert.ok(String(decision.reevaluateWhen).trim().length >= 30, `${decision.key} must define reevaluation criteria`);
  }
});

test('generated widget iframe has no compatibility alias module or contracts', () => {
  assert.equal(TOKEN_COMPATIBILITY_ALIAS_CONTRACTS.length, 0);
  assert.equal(TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS.length, 0);
  assert.equal(
    fs.existsSync(path.join(root, 'src/web-ui/src/tools/generative-widget/appearancePayloadCompatibility.ts')),
    false,
  );
});

test('theme color audit emits scoped machine-readable reports', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': [
      ':root {',
      '  --bf-appearance-token-color-text-primary: #111111;',
      '  --static-only: #222222;',
      '}',
      '',
    ].join('\n'),
    'infrastructure/appearance/adapters/CssTokenAppearanceAdapter.ts': [
      "document.documentElement.style.setProperty('--runtime-only', '#333333');",
      '',
    ].join('\n'),
    'app/App.scss': [
      '.app {',
      '  color: #444444;',
      '  background: var(--fallback-only, #ffffff);',
      '  border-color: var(--runtime-only);',
      '}',
      '',
    ].join('\n'),
    'tools/mermaid-editor/theme/mermaidTheme.ts': "export const line = '#555555';\n",
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const reportPath = path.join(dir, 'theme-report.json');

  const result = runAudit(['--root', sourceRoot, '--report-json', reportPath, '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = readJson(reportPath);
  assert.equal(report.colorScopes.appUi.uniqueColors, 2);
  assert.equal(report.colorScopes.token.uniqueColors, 2);
  assert.equal(report.colorScopes.exception.uniqueColors, 1);
  assert.equal(report.tokenAliasLiterals.occurrences, 0);
  assert.equal(report.tokenAliasLiterals.uniqueColors, 0);
  assert.equal(report.cssVarDefinitions.runtimeOnlyRequiredContractUnique, 1);
  assert.equal(report.cssVarDefinitions.unregisteredDynamicFamilyUnique, 0);
  assert.equal(report.cssVarDefinitions.dynamicFamilyUnexportedUnique, 0);
  assert.equal(report.cssVarDefinitions.staleRegisteredDynamicFamilyUnique, 0);
  assert.equal(report.summary.baseline.enforced, false);
});

test('theme color audit reports static contract token external consumption', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': [
      ':root {',
      '  --bf-appearance-token-color-text-primary: #111111;',
      '  --private-helper-rgb: 17, 17, 17;',
      '  --derived-from-helper: rgb(var(--private-helper-rgb));',
      '  --payload-export: #333333;',
      '  --unused-export: #222222;',
      '}',
      '',
    ].join('\n'),
    'app/App.scss': [
      '.app {',
      '  color: var(--bf-appearance-token-color-text-primary);',
      '  background: var(--derived-from-helper);',
      '}',
      '',
    ].join('\n'),
    'infrastructure/appearance/adapters/widgetAppearanceVariables.ts': [
      'export const WIDGET_APPEARANCE_VARIABLE_NAMES = [',
      "  '--payload-export',",
      '] as const;',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  const result = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = JSON.parse(result.stdout);
  assert.equal(report.cssVarDefinitions.staticContractExternalUsageUnique, 3);
  assert.equal(report.cssVarDefinitions.staticContractInternalOnlyUnique, 2);
  assert.deepEqual(
    report.staticContractLowExternalUsageVars
      .filter(row => row.key === '--payload-export')
      .map(row => [row.key, row.count, row.externalUsageFileCount, row.usageFiles]),
    [['--payload-export', 1, 1, ['infrastructure/appearance/adapters/widgetAppearanceVariables.ts']]],
  );
  assert.deepEqual(
    report.staticContractInternalOnlyVars.map(row => [row.key, row.definitionFiles, row.internalUsageCount]),
    [
      ['--private-helper-rgb', ['component-library/styles/tokens.scss'], 1],
      ['--unused-export', ['component-library/styles/tokens.scss'], 0],
    ],
  );
});

test('theme color audit reports deprecated surface-local token names', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': [
      ':root {',
      '  --base-tool-card-accent-color: #60a5fa;',
      '  --snapshot-card-operation-color: #60a5fa;',
      '}',
      '',
    ].join('\n'),
    'component-library/components/FlowChatCards/BaseToolCard/BaseToolCard.scss': [
      '.base-tool-card {',
      '  --primary-color: var(--base-tool-card-accent-color);',
      '  color: var(--primary-color);',
      '}',
      '',
    ].join('\n'),
    'component-library/components/FlowChatCards/SnapshotCard/SnapshotCard.tsx': [
      "export const style = { '--operation-color': 'var(--snapshot-card-operation-color)' };",
      '',
    ].join('\n'),
    'tools/editor/meditor/components/TiptapEditor.scss': [
      '.m-editor-tiptap {',
      '  --m-editor-highlight-rgb: var(--private-markdown-editor-highlight-rgb);',
      '  background: rgba(var(--m-editor-highlight-rgb), 0.15);',
      '}',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  const result = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = JSON.parse(result.stdout);
  assert.equal(report.surfaceTokenRenames.activeUnique, 3);
  assert.equal(report.surfaceTokenRenames.activeOccurrences, 5);
  assert.deepEqual(
    report.surfaceTokenRenames.active.map(row => [row.key, row.canonical, row.definitionCount, row.usageCount]),
    [
      ['--m-editor-highlight-rgb', '--private-markdown-editor-highlight-rgb', 1, 1],
      ['--primary-color', '--base-tool-card-accent-color', 1, 1],
      ['--operation-color', '--snapshot-card-operation-color', 1, 0],
    ],
  );
});

test('theme color audit counts only the generated widget variable contract', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': [
      ':root {',
      '  --bf-appearance-token-color-bg-primary: #121214;',
      '  --bf-appearance-token-color-bg-secondary: #1c1c1f;',
      '  --bf-appearance-token-font-family-mono: monospace;',
      '}',
      '',
    ].join('\n'),
    'infrastructure/appearance/adapters/widgetAppearanceVariables.ts': [
      'const FALLBACK_VAR = {',
      "  bgPrimary: '--bf-appearance-token-color-bg-primary',",
      "  bgSecondary: '--bf-appearance-token-color-bg-secondary',",
      '} as const;',
      '',
      'const WIDGET_APPEARANCE_STATIC_SHELL_VARS = {',
      "  '--bf-appearance-token-btn-primary-bg': 'var(--bf-appearance-token-color-bg-primary)',",
      '} as const;',
      '',
      'export const WIDGET_APPEARANCE_VARIABLE_NAMES = [',
      "  '--bf-appearance-token-color-bg-primary',",
      "  '--bf-appearance-token-color-bg-secondary',",
      "  '--bf-appearance-token-font-family-mono',",
      '] as const;',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  const result = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = JSON.parse(result.stdout);
  assert.equal(report.generatedWidgetPayload.varUnique, 3);
  assert.equal(report.generatedWidgetPayload.undefinedUnique, 0);
  assert.equal(report.generatedWidgetPayload.externalOnlyCompatibilityUnique, 0);
});

test('theme color audit fails closed when the generated widget variable contract is not parseable', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': [
      ':root {',
      '  --bf-appearance-token-color-bg-primary: #121214;',
      '}',
      '',
    ].join('\n'),
    'infrastructure/appearance/adapters/widgetAppearanceVariables.ts': [
      'const FALLBACK_VAR = {',
      "  bgPrimary: '--bf-appearance-token-color-bg-primary',",
      '} as const;',
      '',
      'export const WIDGET_APPEARANCE_VARIABLE_NAMES = Object.freeze([',
      '  FALLBACK_VAR.bgPrimary,',
      ']);',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  const result = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Unable to parse WIDGET_APPEARANCE_VARIABLE_NAMES/);
});

test('theme color audit reports fallback tokens that lack a boundary contract', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/App.scss': [
      '.app {',
      '  color: var(--runtime-accent, var(--bf-appearance-token-color-accent-500));',
      '}',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  const result = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = JSON.parse(result.stdout);
  assert.equal(report.fallbackContracts.uncontractedUnique, 1);
  assert.deepEqual(report.uncontractedFallbackVars.map(row => row.key), ['--runtime-accent']);
});

test('theme color audit reports specialized color domains separately from app UI', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': ':root { --bf-appearance-token-color-text-primary: #111111; }\n',
    'infrastructure/appearance/builtins/dark.ts': "export const bg = '#222222';\n",
    'tools/mermaid-editor/theme/mermaidTheme.ts': "export const node = '#333333';\n",
    'component-library/components/CodeEditor/editorColor.ts': "export const editorBg = '#444444';\n",
    'shared/prism/prismTheme.ts': "export const prism = { keyword: '#555555' };\n",
    'tools/terminal/utils/xtermTheme.ts': "export const cursor = '#c0c0c0';\n",
    'tools/generative-widget/appearancePayload.ts': "export const fallback = { '--bf-appearance-token-color-text-primary': '#666666' };\n",
    'shared/inspector/inspectorOverlayTheme.ts': "export const overlay = { activeBorder: '#777777' };\n",
    'infrastructure/appearance/appearanceDomainTokens.ts': "export const accents = { tool: '#dddddd' };\n",
    'infrastructure/language-detection/core/LanguageRegistry.ts': "export const rust = '#888888';\n",
    'component-library/components/TextStrokeEffect/TextStrokeEffect.tsx': "export const stroke = '#999999';\n",
    'component-library/components/StreamText/StreamText.scss': ".stream { color: #bbbbbb; }\n",
    'app/tools/mermaid-editorish/FakePanel.ts': "export const fake = '#cccccc';\n",
    'app/App.scss': '.app { color: #aaaaaa; }\n',
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const reportPath = path.join(dir, 'theme-report.json');

  const result = runAudit(['--root', sourceRoot, '--report-json', reportPath, '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = readJson(reportPath);
  assert.equal(report.colorDomainScopes.tokenContract.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.themePreset.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.mermaid.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.editor.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.syntax.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.terminal.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.generatedWidget.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.debugOverlay.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.appearanceDomain.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.languageIdentity.uniqueColors, 1);
  assert.equal(report.colorDomainScopes.visualEffect.uniqueColors, 2);
  assert.equal(report.colorDomainScopes.appUi.uniqueColors, 2);
});

test('theme color audit ignores comment-only color-like text', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/App.tsx': [
      'export const real = "#123456";',
      '// issue #1176 should not be counted as a color',
      '// comment mentions `template` before issue #2026',
      'const escaped = real.replace(/["\\\\]/g, "\\\\$&"); // issue #3456 after a regex',
      'const interpolated = `${real /* issue #7890 inside a template expression */}`;',
      'const url = "https://example.com/#keep-strings";',
      '/*',
      ' * retired value: #abcdef',
      ' */',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const reportPath = path.join(dir, 'theme-report.json');

  const result = runAudit(['--root', sourceRoot, '--report-json', reportPath, '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = readJson(reportPath);
  assert.equal(report.colorOccurrences, 1);
  assert.equal(report.uniqueColors, 1);
  assert.equal(report.topColors[0].key, '#123456');
  assert.equal(report.colorDomainScopes.appUi.uniqueColors, 1);
});

test('theme color audit keeps template literal and expression color values', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/App.tsx': [
      'export const literal = `#abcdef`;',
      'export const expression = `${enabled ? "#654321" : "#111111"}`;',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const reportPath = path.join(dir, 'theme-report.json');

  const result = runAudit(['--root', sourceRoot, '--report-json', reportPath, '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = readJson(reportPath);
  assert.equal(report.colorOccurrences, 3);
  assert.equal(report.uniqueColors, 3);
  assert.deepEqual(new Set(report.topColors.map(entry => entry.key)), new Set(['#abcdef', '#654321', '#111111']));
  assert.equal(report.colorDomainScopes.appUi.uniqueColors, 3);
});

test('theme color audit counts full CSS var governance debt before row truncation', (t) => {
  const missingRules = Array.from(
    { length: 101 },
    (_, index) => `.missing-${index} { color: var(--missing-${index}); }`,
  );
  const fallbackRules = Array.from(
    { length: 101 },
    (_, index) => `.fallback-${index} { color: var(--fallback-${index}, #ffffff); }`,
  );
  const runtimeDefinitions = Array.from(
    { length: 101 },
    (_, index) => `document.documentElement.style.setProperty('--runtime-${index}', '#ffffff');`,
  );
  const runtimeRules = Array.from(
    { length: 101 },
    (_, index) => `.runtime-${index} { color: var(--runtime-${index}); }`,
  );
  const looseStyleEntries = Array.from(
    { length: 101 },
    (_, index) => `  '--loose-${index}': 'red',`,
  );
  const looseRules = Array.from(
    { length: 101 },
    (_, index) => `.loose-${index} { color: var(--loose-${index}); }`,
  );
  const { dir, sourceRoot } = createFixture({
    'infrastructure/appearance/adapters/CssTokenAppearanceAdapter.ts': `${runtimeDefinitions.join('\n')}\n`,
    'app/App.scss': `${missingRules.join('\n')}\n${fallbackRules.join('\n')}\n${runtimeRules.join('\n')}\n`,
    'app/LooseVar.tsx': [
      'export function LooseVar() {',
      '  return <div style={{',
      looseStyleEntries.join('\n'),
      '  }} />;',
      '}',
      '',
    ].join('\n'),
    'app/LooseVarA.scss': `${looseRules.join('\n')}\n`,
    'app/LooseVarB.scss': `${looseRules.join('\n')}\n`,
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const reportPath = path.join(dir, 'theme-report.json');

  const result = runAudit(['--root', sourceRoot, '--report-json', reportPath, '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = readJson(reportPath);
  assert.equal(report.cssVarDefinitions.unresolvedUnique, 202);
  assert.equal(report.cssVarDefinitions.fallbackOnlyUnique, 101);
  assert.equal(report.cssVarDefinitions.unresolvedRequiredUnique, 101);
  assert.equal(report.cssVarDefinitions.runtimeOnlyRequiredContractUnique, 101);
  assert.equal(report.cssVarDefinitions.nonContractCrossFileUnique, 101);
  assert.equal(report.cssVarDefinitions.nonContractDynamicInputUnique, 101);
  assert.equal(report.undefinedVars.length, 100);
  assert.equal(report.fallbackOnlyVars.length, 100);
  assert.equal(report.unresolvedRequiredVars.length, 100);
  assert.equal(report.runtimeOnlyRequiredContractVars.length, 100);
  assert.equal(report.nonContractDynamicInputVars.length, 101);
});

test('theme color audit reports app literals that duplicate token values', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': [
      '$color-accent-600: #3b82f6;',
      '$color-warning: #f59e0b;',
      '',
    ].join('\n'),
    'app/App.scss': [
      '.app {',
      '  color: #3b82f6;',
      '  border-color: rgb(245, 158, 11);',
      '}',
      '',
    ].join('\n'),
    'tools/mermaid-editor/theme/mermaidTheme.ts': "export const accent = '#3b82f6';\n",
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const reportPath = path.join(dir, 'theme-report.json');

  const result = runAudit(['--root', sourceRoot, '--report-json', reportPath, '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = readJson(reportPath);
  assert.equal(report.tokenAliasLiterals.occurrences, 2);
  assert.equal(report.tokenAliasLiterals.uniqueColors, 2);
  assert.deepEqual(
    report.tokenAliasLiterals.top.map(row => row.aliases),
    [['$color-accent-600'], ['$color-warning']],
  );
  assert.equal(report.colorScopes.exception.occurrences, 1);
});

test('theme color audit reports near color pair sources and enforces pair budgets', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/One.scss': '.one { color: #111111; }\n',
    'app/Two.scss': '.two { color: #111112; }\n',
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const reportPath = path.join(dir, 'theme-report.json');

  const reportResult = runAudit(['--root', sourceRoot, '--report-json', reportPath, '--no-baseline']);
  assert.equal(reportResult.status, 0, reportResult.stderr || reportResult.stdout);
  assert.match(reportResult.stdout, /Indistinguishable component color pairs \(total=1, sample\):/);

  const report = readJson(reportPath);
  assert.equal(report.nearPairs.indistinguishableTotal, 1);
  assert.equal(report.nearPairs.indistinguishable.length, 1);
  assert.equal(report.nearPairs.indistinguishable[0].key, '#111111 <-> #111112');
  assert.deepEqual(
    report.nearPairs.indistinguishable[0].files.map(file => file.replace(/\\/g, '/').split('/').slice(-2).join('/')),
    ['app/One.scss', 'app/Two.scss'],
  );

  const baselinePath = path.join(dir, 'theme-baseline.json');
  writeText(baselinePath, `${JSON.stringify({
    version: 1,
    budgets: {
      'nearPairs.indistinguishableTotal': { max: 0 },
    },
  }, null, 2)}\n`);

  const blocked = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(blocked.status, 0, 'new indistinguishable color pairs must fail the audit');
  assert.match(
    `${blocked.stdout}\n${blocked.stderr}`,
    /nearPairs\.indistinguishableTotal has 1 candidate\(s\), baseline is 0/,
  );
});

test('theme color audit reports near color pairs inside specialized color domains', (t) => {
  const { dir, sourceRoot } = createFixture({
    'tools/mermaid-editor/theme/mermaidTheme.ts': [
      "export const lightNode = '#111111';",
      "export const darkNode = '#111112';",
      '',
    ].join('\n'),
    'tools/terminal/utils/xtermTheme.ts': [
      "export const normalBlack = '#222222';",
      "export const brightBlack = '#222225';",
      '',
    ].join('\n'),
    'app/App.scss': '.app { color: #333333; }\n',
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const reportPath = path.join(dir, 'theme-report.json');

  const reportResult = runAudit(['--root', sourceRoot, '--report-json', reportPath, '--no-baseline']);
  assert.equal(reportResult.status, 0, reportResult.stderr || reportResult.stdout);
  assert.match(reportResult.stdout, /Specialized color-domain near pairs:/);

  const report = readJson(reportPath);
  assert.equal(report.colorDomainNearPairs.mermaid.indistinguishableTotal, 1);
  assert.equal(report.colorDomainNearPairs.terminal.nearTotal, 1);
  assert.equal(report.colorDomainNearPairs.appUi.indistinguishableTotal, 0);
  assert.equal(report.colorDomainNearPairs.indistinguishableTotal, 1);
  assert.equal(report.colorDomainNearPairs.nearTotal, 1);
  assert.deepEqual(
    report.colorDomainNearPairs.mermaid.indistinguishable[0].files.map(file => (
      file.replace(/\\/g, '/').split('/').slice(-3).join('/')
    )),
    ['mermaid-editor/theme/mermaidTheme.ts'],
  );
});

test('theme color audit excludes test files from production color budgets', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': ':root { --bf-appearance-token-color-error: #ef4444; }\n',
    'app/App.scss': '.app { color: #ef4444; }\n',
    'app/App.test.tsx': "expect(button).toHaveStyle({ color: '#ef4444' });\n",
    'app/__tests__/Fixture.tsx': "export const visualLock = '#ef4444';\n",
    'generated/version.ts': "export const buildAccent = '#22c55e';\n",
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  const result = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = JSON.parse(result.stdout);
  assert.equal(report.filesScanned, 2);
  assert.equal(report.ignoredTestFiles, 2);
  assert.equal(report.ignoredGeneratedFiles, 1);
  assert.equal(report.colorScopes.appUi.occurrences, 1);
  assert.equal(report.tokenAliasLiterals.occurrences, 1);
});

test('theme color audit fails when metrics exceed the checked baseline', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/App.scss': [
      '.app {',
      '  color: var(--missing, #ffffff);',
      '}',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  writeText(baselinePath, `${JSON.stringify({
    version: 1,
    budgets: {
      fallbackOccurrences: { max: 0 },
    },
  }, null, 2)}\n`);

  const result = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(result.status, 0, 'fallback growth over baseline must fail the audit');
  assert.match(
    `${result.stdout}\n${result.stderr}`,
    /fallbackOccurrences has 1 candidate\(s\), baseline is 0/,
  );
});

test('theme color audit fails when fallback tokens lack a boundary contract', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/App.scss': [
      '.app {',
      '  color: var(--runtime-accent, var(--bf-appearance-token-color-accent-500));',
      '}',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  writeText(baselinePath, `${JSON.stringify({
    version: 1,
    budgets: {
      fallbackUniqueTokens: { max: 1 },
      'fallbackContracts.uncontractedUnique': { max: 0 },
    },
  }, null, 2)}\n`);

  const result = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(result.status, 0, 'uncontracted fallback tokens must fail the audit');
  assert.match(
    `${result.stdout}\n${result.stderr}`,
    /fallbackContracts\.uncontractedUnique has 1 candidate\(s\), baseline is 0/,
  );
});

test('theme color audit requires dynamic CSS var families to be registered', (t) => {
  const { dir, sourceRoot } = createFixture({
    'infrastructure/appearance/adapters/CssTokenAppearanceAdapter.ts': [
      "for (const [key, value] of Object.entries(theme.extra)) {",
      "  document.documentElement.style.setProperty(`--unregistered-${key}`, value);",
      '}',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  writeText(baselinePath, `${JSON.stringify({
    version: 1,
    budgets: {
      'cssVarDefinitions.unregisteredDynamicFamilyUnique': { max: 0 },
    },
  }, null, 2)}\n`);

  const result = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(result.status, 0, 'unregistered dynamic CSS var families must fail the audit');
  assert.match(
    `${result.stdout}\n${result.stderr}`,
    /cssVarDefinitions\.unregisteredDynamicFamilyUnique has 1 candidate\(s\), baseline is 0/,
  );
  assert.match(`${result.stdout}\n${result.stderr}`, /--unregistered-/);
});

test('theme color audit accepts registered dynamic CSS var families', (t) => {
  const { dir, sourceRoot } = createFixture({
    'infrastructure/appearance/adapters/CssTokenAppearanceAdapter.ts': [
      "for (const [key, value] of Object.entries(appearance.typography.size)) {",
      "  document.documentElement.style.setProperty(`--bf-appearance-token-font-size-${key}`, value);",
      '}',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  writeText(baselinePath, `${JSON.stringify({
    version: 1,
    budgets: {
      'cssVarDefinitions.unregisteredDynamicFamilyUnique': { max: 0 },
    },
  }, null, 2)}\n`);

  const result = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.equal(result.status, 0, result.stderr || result.stdout);
});

test('theme color audit requires exact exports for dynamic-family CSS var usages', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': [
      ':root {',
      '  --bf-appearance-token-color-purple-200: rgba(139, 92, 246, 0.15);',
      '}',
      '',
    ].join('\n'),
    'infrastructure/appearance/adapters/CssTokenAppearanceAdapter.ts': [
      "for (const [key, value] of Object.entries(theme.colors.purple)) {",
      "  document.documentElement.style.setProperty(`--bf-appearance-token-color-purple-${key}`, value);",
      '}',
      '',
    ].join('\n'),
    'component-library/components/Tabs/Tabs.scss': [
      '.tabs {',
      '  border-color: var(--bf-appearance-token-color-purple-300);',
      '}',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  writeText(baselinePath, `${JSON.stringify({
    version: 1,
    budgets: {
      'cssVarDefinitions.dynamicFamilyUnexportedUnique': { max: 0 },
    },
  }, null, 2)}\n`);

  const reportResult = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
  assert.equal(reportResult.status, 0, reportResult.stderr || reportResult.stdout);
  const report = JSON.parse(reportResult.stdout);
  assert.equal(report.cssVarDefinitions.dynamicFamilyUnexportedUnique, 1);
  assert.deepEqual(
    report.dynamicFamilyUnexportedVars.map(row => [row.key, row.prefix]),
    [['--bf-appearance-token-color-purple-300', '--bf-appearance-token-color-purple-']],
  );

  const blocked = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(blocked.status, 0, 'dynamic-family usages without exact exports must fail the audit');
  assert.match(
    `${blocked.stdout}\n${blocked.stderr}`,
    /cssVarDefinitions\.dynamicFamilyUnexportedUnique has 1 candidate\(s\), baseline is 0/,
  );
  assert.match(`${blocked.stdout}\n${blocked.stderr}`, /--bf-appearance-token-color-purple-300/);
});

test('theme color audit requires non-contract cross-file vars to be explicitly allowlisted', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/LooseVar.tsx': [
      "export function LooseVar() {",
      "  return <div style={{ '--loose-var': 'red' }} />;",
      '}',
      '',
    ].join('\n'),
    'app/LooseVar.scss': '.one { color: var(--loose-var); }\n',
    'app/LooseVarOther.scss': '.two { border-color: var(--loose-var); }\n',
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  const baseline = {
    version: 1,
    budgets: {
      'cssVarDefinitions.nonContractDynamicInputUnique': { max: 1 },
    },
    allowlists: {
      nonContractDynamicInputs: [],
    },
  };
  writeText(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`);

  const blocked = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(blocked.status, 0, 'unallowlisted dynamic input vars must fail the audit');
  assert.match(
    `${blocked.stdout}\n${blocked.stderr}`,
    /nonContractDynamicInputs is missing allowlist entry for --loose-var/,
  );

  baseline.allowlists.nonContractDynamicInputs.push({
    key: '--loose-var',
    owner: 'scripts/audit-theme-colors.test.mjs',
    reason: 'fixture dynamic input token',
  });
  writeText(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`);

  const allowed = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.equal(allowed.status, 0, allowed.stderr || allowed.stdout);
});

test('theme color audit fails stale non-contract var allowlist entries', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/App.scss': '.app { color: #ffffff; }\n',
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  writeText(baselinePath, `${JSON.stringify({
    version: 1,
    budgets: {
      'cssVarDefinitions.nonContractDynamicInputUnique': { max: 0 },
    },
    allowlists: {
      nonContractDynamicInputs: [
        {
          key: '--removed-var',
          owner: 'scripts/audit-theme-colors.test.mjs',
          reason: 'fixture stale allowlist token',
        },
      ],
    },
  }, null, 2)}\n`);

  const result = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(result.status, 0, 'stale dynamic input allowlist entries must fail the audit');
  assert.match(
    `${result.stdout}\n${result.stderr}`,
    /nonContractDynamicInputs allowlist entry --removed-var is stale/,
  );
});
