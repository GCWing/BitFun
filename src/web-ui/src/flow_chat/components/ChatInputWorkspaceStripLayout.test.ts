import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readWorkspaceStripStylesheet(): string {
  const stylesheet = readFileSync(
    fileURLToPath(new URL('./ChatInputWorkspaceStrip.scss', import.meta.url)),
    'utf8',
  );
  return stylesheet.replace(/\r\n/g, '\n');
}

describe('ChatInputWorkspaceStrip layout styles', () => {
  it('keeps the session usage action visible without overpowering the strip', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain('max-width: 100%;');
    expect(stylesheet).toContain('width: 16px;');
    expect(stylesheet).toContain('height: 16px;');
    expect(stylesheet).toContain('min-width: 16px;');
    expect(stylesheet).toContain('width: 14px;');
    expect(stylesheet).toContain('height: 14px;');
    expect(stylesheet).toContain('color: color-mix(in srgb, var(--color-accent-500) 62%, var(--color-text-secondary));');
    expect(stylesheet).toContain('color: color-mix(in srgb, var(--color-accent-500) 86%, var(--color-text-primary));');
  });

  it('keeps the permission control compact and collapses labels on narrow screens', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain('&__permission-trigger');
    expect(stylesheet).toContain('min-width: 18px;');
    expect(stylesheet).toContain('width: min(286px, calc(100vw - 24px));');
    expect(stylesheet).toContain('@media (max-width: 560px)');
    expect(stylesheet).toContain('&__permission-label');
    expect(stylesheet).toContain('display: none;');
  });
});
