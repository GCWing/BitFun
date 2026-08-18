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
  const declaration = `font-size: flow-type.$${role}-size;`;
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
  it('maps the preference-aware ladder onto five stable semantic roles', () => {
    const typography = readSource('../_typography.scss');

    expect(typography).toContain('$body-size: var(--bf-appearance-token-flowchat-font-size-base);');
    expect(typography).toContain('$control-size: var(--bf-appearance-token-flowchat-font-size-sm);');
    expect(typography).toContain('$support-size: var(--bf-appearance-token-flowchat-font-size-xs);');
    expect(typography).toContain('$meta-size: var(--bf-appearance-token-flowchat-font-size-2xs);');
    expect(typography).toContain('$micro-size: var(--bf-appearance-token-flowchat-font-size-xxs);');
  });

  it('keeps frequent composer and menu actions on the control role', () => {
    const chatInput = readSource('./ChatInput.scss');
    const harness = readSource('./HarnessProfileSelector.scss');
    const model = readSource('./ModelSelector.scss');
    const reasoning = readSource('./ReasoningPresetSelector.scss');
    const workspaceStrip = readSource('./ChatInputWorkspaceStrip.scss');

    expectRole(chatInput, '&__target-tab {', 'control');
    expectRole(chatInput, '&__agent-capsule {', 'control');
    expectRole(chatInput, '&__mode-option {', 'control');
    expectRole(chatInput, '&__slash-command-name {', 'control');
    expectRole(harness, '.bitfun-harness-selector__trigger {', 'control');
    expectRole(model, '&__trigger {', 'control');
    expectRole(model, '&__option-name {', 'control');
    expectRole(reasoning, '&__title {', 'control');
    expectRole(reasoning, 'strong {', 'control');
    expectRole(workspaceStrip, '&__permission-trigger {', 'control');
    expectRole(workspaceStrip, '&__permission-option-label {', 'control');
  });

  it('separates readable content, support text, metadata, and micro badges', () => {
    const chatInput = readSource('./ChatInput.scss');
    const modelRound = readSource('./modern/ModelRoundItem.scss');
    const userMessage = readSource('./modern/UserMessageItem.scss');
    const flowTextBlock = readSource('./FlowTextBlock.scss');

    expectRole(chatInput, '&__placeholder {', 'body');
    expectRole(chatInput, '&__slash-command-label {', 'support');
    expectRole(chatInput, '&__slash-command-status {', 'meta');
    expectRole(modelRound, '.model-round-item__retry-toggle {', 'control');
    expectRole(modelRound, '.model-round-item__attempt-diagnostic-section pre {', 'support');
    expectRole(modelRound, '.model-round-item__meta {', 'meta');
    expectRole(userMessage, '.user-message-item__content {', 'body');
    expectRole(userMessage, '.user-message-item__timestamp {', 'meta');
    expectRole(userMessage, '.user-message-item__steering-tag {', 'micro');
    expect(extractBlock(userMessage, '.user-message-item--failed {')).toContain(
      '--user-message-failed-font-size: #{flow-type.$control-size};',
    );
    expectRole(flowTextBlock, '.markdown-renderer .inline-code {', 'control');
  });
});
