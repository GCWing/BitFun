import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./AgentCompanionDesktopPet.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stylesheet.match(
    new RegExp(`^\\s*${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`, 'm'),
  );
  return match?.groups?.body ?? '';
}

describe('AgentCompanionDesktopPet styles', () => {
  it('keeps context-menu foregrounds and interaction states theme-aware', () => {
    const stylesheet = readStylesheet();
    const overlay = extractBlock(stylesheet, '&__overlay');
    const menuItem = extractBlock(stylesheet, '&__menu-item');

    expect(overlay).toContain('color: var(--bf-appearance-token-color-text-primary);');
    expect(overlay).not.toContain('color: var(--bitfun-agent-companion-bubble-fg);');
    expect(menuItem).toContain('color: var(--bf-appearance-token-color-text-secondary);');
    expect(menuItem).toContain('color: var(--bf-appearance-token-color-text-primary);');
    expect(menuItem).toContain('background: var(--bf-appearance-token-element-bg-soft);');
    expect(stylesheet).toContain(
      '&:active {\n      background: var(--bf-appearance-token-element-bg-medium);\n    }',
    );
  });
});
