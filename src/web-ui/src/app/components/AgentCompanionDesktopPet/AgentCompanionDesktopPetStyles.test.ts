import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./AgentCompanionDesktopPet.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function readSource(): string {
  return readFileSync(
    fileURLToPath(new URL('./AgentCompanionDesktopPet.tsx', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function readPublicActionItemStyles(): string {
  return readFileSync(
    fileURLToPath(new URL(
      '../../../../../../design-system/packages/ui/src/components/ActionItem/ActionItem.module.css',
      import.meta.url,
    )),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const matches = [...stylesheet.matchAll(
    new RegExp(`^\\s*${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`, 'gm'),
  )];
  return matches.at(-1)?.groups?.body ?? '';
}

describe('AgentCompanionDesktopPet styles', () => {
  it('pairs theme-aware bubble surfaces with readable theme content colors', () => {
    const stylesheet = readStylesheet();
    const bubble = extractBlock(stylesheet, '&__bubble');
    const status = extractBlock(stylesheet, '&__bubble-status');
    const output = extractBlock(stylesheet, '&__bubble-output');
    const composerInput = extractBlock(stylesheet, '&__bubble-composer-input');

    expect(bubble).toContain('background-color: var(--bf-color-surface-raised);');
    expect(bubble).toContain('color: var(--bf-color-content-primary);');
    expect(status).toContain('color: var(--bf-color-content-secondary);');
    expect(output).toContain('color: var(--bf-color-content-secondary);');
    expect(composerInput).toContain('background: var(--bf-color-field-background);');
    expect(composerInput).toContain('color: var(--bf-color-content-primary);');
    expect(stylesheet).toContain('background: var(--bf-color-surface-raised);');

    expect(stylesheet).not.toContain('--bf-color-content-on-light');
    expect(stylesheet).not.toContain('--bf-color-content-on-dark');
    expect(stylesheet).not.toMatch(
      /&__bubble--(?:attention|waiting|completed|error|interrupted)\s+&__bubble-status/,
    );
  });

  it('layers translucent task-state colors over the opaque themed bubble surface', () => {
    const stylesheet = readStylesheet();

    for (const [selector, surfaceToken] of [
      ['&__bubble--attention', '--bf-color-status-warning-surface'],
      ['&__bubble--waiting', '--bf-color-status-warning-surface'],
      ['&__bubble--completed', '--bf-color-status-success-surface'],
      ['&__bubble--error,\n  &__bubble--interrupted', '--bf-color-status-danger-surface'],
    ] as const) {
      const state = extractBlock(stylesheet, selector);
      expect(state).toContain('background-image: linear-gradient(');
      expect(state).toContain(`var(${surfaceToken})`);
      expect(state).not.toContain(`background: var(${surfaceToken});`);
    }
  });

  it('delegates context-menu foregrounds and interaction states to the public menu item', () => {
    const source = readSource();
    const stylesheet = readStylesheet();
    const actionItemStyles = readPublicActionItemStyles();
    const overlay = extractBlock(stylesheet, '&__overlay');
    const menuItem = extractBlock(stylesheet, '&__menu-item');

    expect(source).toContain('import { Menu, MenuItem, ScrollArea } from \'@bitfun/ui\'');
    expect(source).not.toContain('triggerClassName');
    expect(overlay).not.toMatch(/\b(?:color|background|border|box-shadow|backdrop-filter)\s*:/);
    expect(menuItem).toBe('');
    expect(actionItemStyles).toContain('color: var(--bf-color-action-neutral-content);');
    expect(actionItemStyles).toContain('background: var(--bf-color-action-neutral-surface);');
    expect(actionItemStyles).toContain('background: var(--bf-color-action-neutral-surface-pressed);');
  });
});
