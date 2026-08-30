import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

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

describe('UserMessageItem action visibility', () => {
  it('reveals the copy, edit, and rollback actions as one cluster', () => {
    const stylesheet = readFileSync(
      fileURLToPath(new URL('./UserMessageItem.scss', import.meta.url)),
      'utf8',
    ).replace(/\r\n?/g, '\n');
    const item = extractBlock(stylesheet, '.user-message-item {');
    const actions = extractBlock(stylesheet, '\n.user-message-item__actions {');
    const hover = extractBlock(item, '&:hover {');
    const focusWithin = extractBlock(item, '&:focus-within {');

    expect(actions).toContain('opacity: 0;');
    expect(extractBlock(hover, '.user-message-item__actions {')).toContain('opacity: 1;');
    expect(extractBlock(focusWithin, '.user-message-item__actions {')).toContain('opacity: 1;');

    expect(stylesheet).toContain([
      '.user-message-item__copy-btn,',
      '.user-message-item__edit-btn,',
      '.user-message-item__rollback-btn {',
    ].join('\n'));
    expect(stylesheet).not.toContain('.user-message-item__edit-btn {\n  opacity: 1;');
  });
});
