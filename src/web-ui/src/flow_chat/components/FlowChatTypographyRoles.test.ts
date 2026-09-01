import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  ).replace(/\r\n?/g, '\n');
}

function extractBlock(source: string, selector: string): string {
  const selectorStart = source.indexOf(selector);
  expect(selectorStart, `Missing selector: ${selector}`).toBeGreaterThanOrEqual(0);

  const blockStart = source.indexOf('{', selectorStart);
  expect(blockStart, `Missing block for selector: ${selector}`).toBeGreaterThanOrEqual(0);

  let depth = 0;
  for (let index = blockStart; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(blockStart + 1, index);
    }
  }

  throw new Error(`Unclosed block for selector: ${selector}`);
}

function expectRole(source: string, selector: string, role: string): void {
  const block = extractBlock(source, selector);
  const declaration = `font-size: var(--bf-type-flow-${role}-font-size);`;
  if (block.includes(declaration)) return;

  // A surface may take its role from a shared mixin instead of restating the
  // size. The contract is the role it renders at, not where it is written, and
  // a surface that names its own size is how a track drifts off the ladder.
  const included = block.match(/@include\s+([\w-]+);/)?.[1];
  expect(included, `Missing ${role} role on: ${selector}`).toBeTruthy();
  expect(
    extractBlock(source, `@mixin ${included} {`),
    `Mixin ${included} does not carry the ${role} role for: ${selector}`,
  ).toContain(declaration);
}

describe('FlowChat semantic typography roles', () => {
  it('consumes public semantic roles without a parallel Sass or Appearance ladder', () => {
    const stylesheets = [
      readSource('./ChatInput.scss'),
      readSource('./ChatInputWorkspaceStrip.scss'),
      readSource('./modern/ModelRoundItem.scss'),
      readSource('./modern/UserMessageItem.scss'),
    ].join('\n');

    // typography-audit: negative-test-start -- verifies retired FlowChat typography aliases stay absent
    expect(stylesheets).not.toContain('flow-type.$');
    expect(stylesheets).not.toContain('--bf-appearance-token-flowchat-font');
    // typography-audit: negative-test-end
    for (const role of ['body', 'control', 'support', 'meta', 'micro']) {
      expect(stylesheets).toContain(`--bf-type-flow-${role}-font-size`);
    }
  });

  it('keeps frequent composer and menu actions on the control role', () => {
    const chatInput = readSource('./ChatInput.scss');
    const harness = readSource('./HarnessProfileSelector.scss');
    const model = readSource('./ModelSelector.scss');
    const reasoning = readSource('./ReasoningPresetSelector.scss');

    expectRole(chatInput, '&__target-tab {', 'control');
    expectRole(chatInput, '&__slash-command-name {', 'control');
    expectRole(harness, '.bitfun-harness-selector__trigger {', 'control');
    expectRole(model, '&__trigger {', 'control');
    expectRole(model, '&__option-name {', 'control');
    expectRole(reasoning, '&__title {', 'control');
    expectRole(reasoning, '&__option-label {', 'control');
  });

  it('separates readable content, support text, metadata, and micro badges', () => {
    const chatInput = readSource('./ChatInput.scss');
    const modelRound = readSource('./modern/ModelRoundItem.scss');
    const userMessage = readSource('./modern/UserMessageItem.scss');
    const flowTextBlock = readSource('./FlowTextBlock.scss');
    const workspaceStrip = readSource('./ChatInputWorkspaceStrip.scss');

    expectRole(chatInput, '&__placeholder {', 'control');
    expectRole(chatInput, '&__slash-command-label {', 'support');
    expectRole(chatInput, '&__slash-command-status {', 'meta');
    // The context track is a quiet meta line above the composer surface: one
    // step for every label on it, facts and controls alike.
    expectRole(workspaceStrip, '&__permission-trigger {', 'meta');
    expectRole(modelRound, '.model-round-item__retry-toggle {', 'control');
    expectRole(modelRound, '.model-round-item__attempt-diagnostic-section pre {', 'support');
    expect(extractBlock(modelRound, '.model-round-item__meta {')).toContain(
      'font-size: var(--bf-type-flow-meta-font-size);',
    );
    expectRole(userMessage, '.user-message-item__content {', 'body');
    expectRole(userMessage, '.user-message-item__steering-tag {', 'micro');
    expect(extractBlock(userMessage, '.user-message-item--failed {')).toContain(
      '--_failed-font-size: var(--bf-type-flow-control-font-size);',
    );
    expectRole(flowTextBlock, '.markdown-renderer .inline-code {', 'control');
  });

  it('keeps completion metadata as an unlabeled two-value row on public tokens', () => {
    const component = readSource('./modern/ModelRoundItem.tsx');
    const stylesheet = readSource('./modern/ModelRoundItem.scss');
    const meta = extractBlock(stylesheet, '.model-round-item__meta {');

    expect(component).not.toContain('model-round-item__meta-label');
    expect(component).not.toContain('model-round-item__meta-value');
    expect(component).toContain('aria-label={`${item.label}: ${item.value}`}');
    expect(meta).toContain('gap: var(--bf-space-2);');
    expect(meta).toContain('color: var(--bf-color-content-muted);');
    expect(meta).toContain('font-family: var(--bf-type-flow-meta-font-family);');
    expect(meta).toContain('font-size: var(--bf-type-flow-meta-font-size);');
    expect(meta).toContain('font-weight: var(--bf-type-flow-meta-font-weight);');
    expect(meta).toContain('line-height: var(--bf-type-flow-meta-line-height);');
  });
});
