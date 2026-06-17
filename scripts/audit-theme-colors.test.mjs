import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  COLOR_DOMAIN_KEYS,
  COLOR_DOMAIN_RULES,
  DYNAMIC_VAR_FAMILY_CONTRACTS,
} from './theme-css-var-contract.mjs';

const root = process.cwd();

function writeText(filePath, content) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, 'utf8');
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
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
    assert.ok(contract.owner.includes('src/web-ui/src/'), `${contract.prefix} must name a source owner`);
    assert.ok(contract.reason.trim().length >= 20, `${contract.prefix} must explain why it is dynamic`);
  }
});

test('repository dynamic CSS var families match the registered contract', () => {
  const result = runAudit(['--json', '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = JSON.parse(result.stdout);
  const registeredPrefixes = DYNAMIC_VAR_FAMILY_CONTRACTS
    .map(contract => contract.prefix)
    .sort();
  assert.deepEqual(report.cssVarDefinitions.dynamicFamilyPrefixes, registeredPrefixes);
  assert.equal(report.cssVarDefinitions.unregisteredDynamicFamilyUnique, 0);
  assert.equal(report.cssVarDefinitions.staleRegisteredDynamicFamilyUnique, 0);
});

test('theme color audit emits scoped machine-readable reports', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': [
      ':root {',
      '  --color-text-primary: #111111;',
      '  --static-only: #222222;',
      '}',
      '',
    ].join('\n'),
    'infrastructure/theme/core/ThemeService.ts': [
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
  assert.equal(report.colorScopes.token.uniqueColors, 3);
  assert.equal(report.colorScopes.exception.uniqueColors, 1);
  assert.equal(report.tokenAliasLiterals.occurrences, 0);
  assert.equal(report.tokenAliasLiterals.uniqueColors, 0);
  assert.equal(report.cssVarDefinitions.runtimeOnlyRequiredContractUnique, 1);
  assert.equal(report.cssVarDefinitions.unregisteredDynamicFamilyUnique, 0);
  assert.equal(report.cssVarDefinitions.staleRegisteredDynamicFamilyUnique, 0);
  assert.equal(report.summary.baseline.enforced, false);
});

test('theme color audit reports specialized color domains separately from app UI', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': ':root { --color-text-primary: #111111; }\n',
    'infrastructure/theme/presets/dark-theme.ts': "export const bg = '#222222';\n",
    'tools/mermaid-editor/theme/mermaidTheme.ts': "export const node = '#333333';\n",
    'tools/editor/themes/bitfun-dark.theme.ts': "export const editorBg = '#444444';\n",
    'shared/prism/prismTheme.ts': "export const prism = { keyword: '#555555' };\n",
    'tools/terminal/utils/xtermTheme.ts': "export const cursor = '#c0c0c0';\n",
    'tools/generative-widget/themePayload.ts': "export const fallback = { '--color-text-primary': '#666666' };\n",
    'shared/inspector/inspectorOverlayTheme.ts': "export const overlay = { activeBorder: '#777777' };\n",
    'shared/theme/uiExceptionAccents.ts': "export const accents = { tool: '#dddddd' };\n",
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
  assert.equal(report.colorDomainScopes.uiException.uniqueColors, 1);
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
    'infrastructure/theme/core/ThemeService.ts': `${runtimeDefinitions.join('\n')}\n`,
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

test('theme color audit excludes test files from production color budgets', (t) => {
  const { dir, sourceRoot } = createFixture({
    'component-library/styles/tokens.scss': ':root { --color-error: #ef4444; }\n',
    'app/App.scss': '.app { color: #ef4444; }\n',
    'app/App.test.tsx': "expect(button).toHaveStyle({ color: '#ef4444' });\n",
    'app/__tests__/Fixture.tsx': "export const visualLock = '#ef4444';\n",
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));

  const result = runAudit(['--root', sourceRoot, '--json', '--no-baseline']);
  assert.equal(result.status, 0, result.stderr || result.stdout);

  const report = JSON.parse(result.stdout);
  assert.equal(report.filesScanned, 2);
  assert.equal(report.ignoredTestFiles, 2);
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

test('theme color audit requires intentional fallback tokens to be allowlisted', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/App.scss': [
      '.app {',
      '  color: var(--runtime-accent, var(--color-accent-500));',
      '}',
      '',
    ].join('\n'),
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  const baseline = {
    version: 1,
    budgets: {
      fallbackUniqueTokens: { max: 1 },
    },
    allowlists: {
      intentionalFallbackTokens: [],
    },
  };
  writeText(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`);

  const blocked = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(blocked.status, 0, 'unallowlisted fallback tokens must fail the audit');
  assert.match(
    `${blocked.stdout}\n${blocked.stderr}`,
    /intentionalFallbackTokens is missing allowlist entry for --runtime-accent/,
  );

  baseline.allowlists.intentionalFallbackTokens.push({
    key: '--runtime-accent',
    owner: 'scripts/audit-theme-colors.test.mjs',
    reason: 'fixture runtime color fallback',
  });
  writeText(baselinePath, `${JSON.stringify(baseline, null, 2)}\n`);

  const allowed = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.equal(allowed.status, 0, allowed.stderr || allowed.stdout);
});

test('theme color audit fails stale intentional fallback token allowlist entries', (t) => {
  const { dir, sourceRoot } = createFixture({
    'app/App.scss': '.app { color: var(--color-accent-500); }\n',
  });
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const baselinePath = path.join(dir, 'theme-baseline.json');
  writeText(baselinePath, `${JSON.stringify({
    version: 1,
    budgets: {
      fallbackUniqueTokens: { max: 0 },
    },
    allowlists: {
      intentionalFallbackTokens: [
        {
          key: '--removed-fallback',
          owner: 'scripts/audit-theme-colors.test.mjs',
          reason: 'fixture stale fallback token',
        },
      ],
    },
  }, null, 2)}\n`);

  const result = runAudit(['--root', sourceRoot, '--baseline', baselinePath]);
  assert.notEqual(result.status, 0, 'stale fallback allowlist entries must fail the audit');
  assert.match(
    `${result.stdout}\n${result.stderr}`,
    /intentionalFallbackTokens allowlist entry --removed-fallback is stale/,
  );
});

test('theme color audit requires dynamic CSS var families to be registered', (t) => {
  const { dir, sourceRoot } = createFixture({
    'infrastructure/theme/core/ThemeService.ts': [
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
    'infrastructure/theme/core/ThemeService.ts': [
      "for (const [key, value] of Object.entries(theme.effects.spacing)) {",
      "  document.documentElement.style.setProperty(`--spacing-${key}`, value);",
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
