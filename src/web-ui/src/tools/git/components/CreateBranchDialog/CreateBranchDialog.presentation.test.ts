import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('CreateBranchDialog scroll layout', () => {
  it('keeps the scrollbar on the dialog edge and the content inset', () => {
    const stylesheet = readFileSync(resolve(__dirname, 'CreateBranchDialog.scss'), 'utf8');

    expect(stylesheet).toContain('padding-block: var(--openbitfun-space-5);');
    expect(stylesheet).toContain('padding-inline: var(--openbitfun-space-5) 0;');
    expect(stylesheet).toMatch(
      /&__scroll\s*\{[\s\S]*?padding-inline-end: var\(--openbitfun-space-5\);/,
    );
  });
});
