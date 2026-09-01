import assert from 'node:assert/strict';
import test from 'node:test';

import {
  auditTypographyFileContractText,
  auditTypographyText,
} from './audit-typography-tokens.mjs';

function codesFor(source, relativePath) {
  return auditTypographyText(source, relativePath).map(issue => issue.code);
}

test('accepts canonical CSS typography, inheritance, structural zeros, and documented geometry', () => {
  const issues = auditTypographyText(`
    .body {
      font-family: var(--bf-type-body-md-font-family);
      font-size: var(--bf-type-body-md-font-size);
      font-weight: var(--bf-font-weight-regular);
      line-height: var(--bf-line-height-base);
      letter-spacing: var(--bf-letter-spacing-normal);
    }
    .inherit { font: inherit; }
    .structural { font-size: 0; line-height: 0; }
    .avatar {
      /* typography-audit: allow -- emoji is graphic content scaled from its avatar box */
      font-size: calc(var(--avatar-size) * 0.5);
    }
  `, 'fixture.scss');

  assert.deepEqual(issues, []);
});

test('rejects raw CSS sizes, weights, line heights, tracking, and font stacks', () => {
  const codes = codesFor(`
    .bad {
      font-family: Inter, sans-serif;
      font-size: 14px;
      font-weight: 650;
      line-height: 1.5;
      letter-spacing: -0.01em;
    }
  `, 'fixture.css');

  assert.equal(codes.filter(code => code === 'raw-css-typography').length, 5);
});

test('accepts only build-owned font faces and stacks in Web font profiles', () => {
  const issues = auditTypographyText(`
    @font-face {
      font-family: "BitFun HarmonyOS Sans";
      font-weight: 500;
    }
    @layer bf.tokens.system {
      :where([data-bf-design-system-root]) {
        --bf-font-family-sans: "BitFun HarmonyOS Sans", system-ui, sans-serif;
        --bf-font-family-control: "BitFun HarmonyOS Sans", system-ui, sans-serif;
        --bf-font-family-mono: "Fira Code", monospace;
      }
    }
  `, 'src/web-ui/src/font-profiles/harmony-bundled.css');

  assert.deepEqual(issues, []);
});

test('rejects unrelated raw typography inside Web font profiles', () => {
  const codes = codesFor(`
    .bad {
      font-family: Inter, sans-serif;
      font-size: 14px;
      font-weight: 650;
      --feature-font-family: Inter, sans-serif;
    }
  `, 'src/web-ui/src/font-profiles/harmony-bundled.css');

  assert.equal(codes.filter(code => code === 'raw-css-typography').length, 3);
  assert.equal(codes.filter(code => code === 'raw-private-typography-token').length, 1);
});

test('requires semantic roles in the complete component library and every React product frontend', () => {
  const primitiveCodes = codesFor(`
    .title {
      font-family: var(--bf-font-family-control);
      font-size: var(--bf-font-size-sm);
      font-weight: var(--bf-font-weight-semibold);
      line-height: var(--bf-line-height-tight);
      letter-spacing: var(--bf-letter-spacing-normal);
    }
  `, 'design-system/packages/ui/src/components/Card/Card.module.css');

  assert.equal(
    primitiveCodes.filter(code => code === 'foundation-typography-in-semantic-consumer').length,
    5,
  );

  const semanticIssues = auditTypographyText(`
    .title {
      font-family: var(--bf-type-heading-card-font-family);
      font-size: var(--bf-type-heading-card-font-size);
      font-weight: var(--bf-type-heading-card-font-weight);
      line-height: var(--bf-type-heading-card-line-height);
      letter-spacing: var(--bf-type-heading-card-letter-spacing);
    }
  `, 'design-system/packages/ui/src/components/Card/Card.module.css');

  assert.deepEqual(semanticIssues, []);

  const geometryIssues = auditTypographyText(`
    .icon { inline-size: var(--bf-font-size-sm); block-size: var(--bf-font-size-sm); }
  `, 'design-system/packages/ui/src/components/ActionItem/ActionItem.module.css');

  assert.deepEqual(geometryIssues, []);

  const flowChatCodes = codesFor(`
    .message { font-size: var(--bf-font-size-sm); }
  `, 'design-system/packages/ui/src/flow-chat/Message.module.css');

  assert.equal(
    flowChatCodes.filter(code => code === 'foundation-typography-in-semantic-consumer').length,
    1,
  );

  const webUiCodes = codesFor(`
    .label {
      font-size: var(--bf-font-size-xs);
      line-height: var(--bf-line-height-tight);
    }
  `, 'src/web-ui/src/app/components/LegacyLabel.scss');

  assert.equal(
    webUiCodes.filter(code => code === 'foundation-typography-in-semantic-consumer').length,
    2,
  );

  const webUiIndirectCodes = codesFor(`
    .title {
      --local-title-font: var(--bf-font-family-sans);
      min-height: calc(var(--bf-font-size-sm) * var(--bf-line-height-base));
    }
  `, 'src/web-ui/src/app/components/LegacyTitle.scss');

  assert.equal(
    webUiIndirectCodes.filter(code => code === 'foundation-typography-in-semantic-consumer').length,
    3,
  );

  for (const productPath of [
    'src/mobile-web/src/styles/LegacyLabel.scss',
    'BitFun-Installer/src/styles/LegacyLabel.css',
    'design-system/apps/design-lab/src/LegacyLabel.css',
  ]) {
    const productCodes = codesFor(`
      .label { font-size: var(--bf-font-size-xs); }
    `, productPath);

    assert.equal(
      productCodes.filter(code => code === 'foundation-typography-in-semantic-consumer').length,
      1,
      productPath,
    );
  }
});

test('rejects static TypeScript typography but accepts tokens, adapters, and dynamic previews', () => {
  const codes = codesFor(`
    const RAW_FONT_SIZE = 12;
    const terminalOptions = { fontSize: 13, lineHeight: 1.4 };
    const compilerKeys = { fontSize: 'font-size', lineHeight: 'line-height' };
    const tokenOptions = { fontSize: getTypographyTokenPx('font.size.base') };
    const dynamicPreview = { fontSize: \`\${previewPx}px\` };
    const tokenStyle = { fontWeight: 'var(--bf-font-weight-semibold)' };
    const Component = ({ fontSize = 16 }) => (
      <svg><text fontSize={10} fontWeight="var(--bf-font-weight-medium)" /></svg>
    );
  `, 'fixture.tsx');

  assert.equal(codes.filter(code => code === 'raw-script-typography').length, 5);
});

test('requires semantic roles in product frontend inline styles and embedded CSS', () => {
  const codes = codesFor(`
    const style = { fontSize: 'var(--bf-font-size-sm)' };
    const shell = \`<style>.title { font-weight: var(--bf-font-weight-semibold); }</style>\`;
  `, 'src/web-ui/src/fixture.tsx');

  assert.equal(
    codes.filter(code => code === 'foundation-typography-in-semantic-consumer').length,
    2,
  );

  const semanticIssues = auditTypographyText(`
    const style = { fontSize: 'var(--bf-type-label-md-font-size)' };
    const shell = \`<style>.title { font-weight: var(--bf-type-heading-card-font-weight); }</style>\`;
  `, 'src/web-ui/src/fixture.tsx');

  assert.deepEqual(semanticIssues, []);

  for (const productPath of [
    'src/mobile-web/src/fixture.tsx',
    'BitFun-Installer/src/fixture.tsx',
  ]) {
    const productCodes = codesFor(`
      const style = { fontSize: 'var(--bf-font-size-sm)' };
    `, productPath);
    assert.equal(
      productCodes.filter(code => code === 'foundation-typography-in-semantic-consumer').length,
      1,
      productPath,
    );
  }
});

test('rejects retired aliases and FlowChat font APIs', () => {
  const codes = codesFor(`
    const oldToken = 'font.size.body';
    const oldVar = '--bf-appearance-token-flowchat-font-size-base';
    setFlowChatFont('independent');
  `, 'fixture.ts');

  assert.deepEqual(codes.sort(), [
    'retired-flowchat-font-api',
    'retired-flowchat-font-variable',
    'retired-font-token-name',
  ]);
});

test('rejects retired component typography tokens and variables', () => {
  const codes = codesFor(`
    const oldToken = 'control.button.xsFontSize';
    const oldVariable = '--bf-overlay-modal-title-font-weight';
  `, 'fixture.ts');

  assert.deepEqual(codes.sort(), [
    'retired-component-typography-token',
    'retired-component-typography-variable',
  ]);
});

test('allows retired names only inside explicit negative-test blocks', () => {
  const issues = auditTypographyText(`
    // typography-audit: negative-test-start -- verifies the retired alias stays absent
    expect(css).not.toContain('--bf-appearance-token-font-size-base');
    // typography-audit: negative-test-end
  `, 'fixture.test.ts');

  assert.deepEqual(issues, []);
});

test('does not mistake token-editor labels or schema property maps for typography values', () => {
  const issues = auditTypographyText(`
    const categories = { lineHeight: 'Line height' };
    const messages = { fontSize: 'tokens.category.fontSize' };
    const cssProperties = {
      fontFamily: 'font-family',
      fontSize: 'font-size',
      fontWeight: 'font-weight',
      lineHeight: 'line-height',
      letterSpacing: 'letter-spacing',
    };
  `, 'fixture.ts');

  assert.deepEqual(issues, []);
});

test('limits the AppearanceStyle typography ban to that interface block', () => {
  const path = 'src/web-ui/src/infrastructure/appearance/types/index.ts';
  const valid = auditTypographyFileContractText(`
    export interface AppearanceStyle {
      backgroundColor?: string;
    }

    export interface MonacoAppearanceTokenRule {
      fontStyle?: string;
    }
  `, path);
  assert.deepEqual(valid, []);

  const invalid = auditTypographyFileContractText(`
    export interface AppearanceStyle {
      fontStyle?: string;
    }
  `, path);
  assert.deepEqual(invalid.map(issue => issue.code), [
    'appearance-style-typography-reintroduced',
  ]);
});

test('rejects a backend FlowChat font snapshot', () => {
  const issues = auditTypographyFileContractText(`
    pub struct FontPreferenceSnapshot {
        pub ui_size: UiFontSizeSnapshot,
        pub flow_chat: FlowChatFontSnapshot,
    }
  `, 'src/crates/assembly/core/src/service/config/types.rs');

  assert.deepEqual(issues.map(issue => issue.code), [
    'backend-flowchat-font-snapshot-reintroduced',
  ]);
});

test('rejects retired FlowChat typography in the distributed Appearance registry', () => {
  const issues = auditTypographyFileContractText(`{
    "components": [{ "id": "font-preference", "parts": [{ "id": "flowChatControls" }] }]
  }`, 'src/crates/assembly/core/builtin_skills/create-bitfun-skin/references/appearance-registry.json');

  assert.deepEqual(issues.map(issue => issue.code), [
    'appearance-registry-flowchat-font-contract-reintroduced',
  ]);
});

test('rejects typography in the standalone Appearance validator but allows Monaco fontStyle', () => {
  const path = 'src/crates/assembly/core/builtin_skills/create-bitfun-skin/scripts/bitfun_appearance.py';
  const valid = auditTypographyFileContractText(`
    font_style = rule.get("fontStyle")
  `, path);
  assert.deepEqual(valid, []);

  const invalid = auditTypographyFileContractText(`
    STYLE_PROPERTIES = {"backgroundColor", "fontSize"}
  `, path);
  assert.deepEqual(invalid.map(issue => issue.code), [
    'standalone-appearance-typography-reintroduced',
  ]);
});
