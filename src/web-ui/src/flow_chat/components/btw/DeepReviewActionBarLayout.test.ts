import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readActionBarStylesheet(): string {
  const stylesheet = readFileSync(
    fileURLToPath(new URL('./DeepReviewActionBar.scss', import.meta.url)),
    'utf8',
  );
  return stylesheet.replace(/\r\n/g, '\n');
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stylesheet.match(new RegExp(`${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`));
  return match?.groups?.body ?? '';
}

describe('DeepReviewActionBar layout styles', () => {
  it('keeps the sticky actions visually merged with the floating panel', () => {
    const stylesheet = readActionBarStylesheet();
    const root = extractBlock(stylesheet, '.deep-review-action-bar');
    const success = extractBlock(stylesheet, '&--success');
    const loading = extractBlock(stylesheet, '&--loading');
    const error = extractBlock(stylesheet, '&--error');
    const warning = extractBlock(stylesheet, '&--warning');
    const actions = extractBlock(stylesheet, '&__actions');

    expect(root).toContain('--deep-review-action-bar-surface:');
    expect(success).toContain('--deep-review-action-bar-surface:');
    expect(loading).toContain('--deep-review-action-bar-surface:');
    expect(error).toContain('--deep-review-action-bar-surface:');
    expect(warning).toContain('--deep-review-action-bar-surface:');
    expect(actions).toContain('background: var(--deep-review-action-bar-surface);');
    expect(actions).toContain('border-top: 1px solid color-mix(in srgb, var(--border-base) 56%, transparent);');
    expect(stylesheet).not.toContain('--deep-review-action-bar-actions-surface');
    expect(actions).not.toContain('var(--color-bg-secondary) 94%');
    expect(actions).not.toContain('backdrop-filter');
  });

  it('keeps sticky actions from creating a horizontal scrollbar', () => {
    const stylesheet = readActionBarStylesheet();
    const root = extractBlock(stylesheet, '.deep-review-action-bar');
    const actions = extractBlock(stylesheet, '&__actions');

    expect(root).toContain('overflow-x: hidden;');
    expect(root).toContain('overflow-y: auto;');
    expect(root).toContain('scrollbar-gutter: stable;');
    expect(actions).toContain('margin: 2px 0 -6px;');
    expect(actions).toContain('padding: 12px 0 6px;');
    expect(stylesheet).not.toContain('--deep-review-action-bar-scrollbar-gutter');
    expect(actions).not.toContain('calc(');
  });
});
