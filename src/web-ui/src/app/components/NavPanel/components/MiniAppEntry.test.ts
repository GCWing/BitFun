import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

describe('MiniAppEntry navigation presentation', () => {
  it('renders the product name without a Beta badge', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./MiniAppEntry.tsx', import.meta.url)),
      'utf8',
    );
    const stylesheet = readFileSync(
      fileURLToPath(new URL('../NavPanel.scss', import.meta.url)),
      'utf8',
    );

    expect(source).not.toContain('Beta');
    expect(source).not.toContain('miniapp-badge');
    expect(stylesheet).not.toContain('&__miniapp-badge');
  });

  it('uses the catalog icon and aligns it with the top-action icon column', () => {
    const source = readFileSync(
      fileURLToPath(new URL('./MiniAppEntry.tsx', import.meta.url)),
      'utf8',
    );
    const stylesheet = readFileSync(
      fileURLToPath(new URL('../NavPanel.scss', import.meta.url)),
      'utf8',
    );

    expect(source).toContain('<Icon name="mini-app" size="md" />');
    expect(source.indexOf('openbitfun-nav-panel__miniapp-entry-icon'))
      .toBeLessThan(source.indexOf('openbitfun-nav-panel__miniapp-entry-title'));
    expect(stylesheet).toContain('&__miniapp-entry-icon');
    expect(stylesheet).toContain('padding: 8px 10px 8px var(--openbitfun-space-2);');
    expect(stylesheet).toContain('gap: var(--openbitfun-space-2);');
    expect(stylesheet).toContain('flex: 0 0 22px;');
  });
});
