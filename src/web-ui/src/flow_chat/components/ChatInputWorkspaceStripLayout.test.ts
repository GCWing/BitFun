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

function readWorkspaceStripComponent(): string {
  return readFileSync(
    fileURLToPath(new URL('./ChatInputWorkspaceStrip.tsx', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function readDispatchPickerStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('../../features/dispatch/DispatchTargetPicker.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
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
    expect(stylesheet).toContain('&--ask {');
    expect(stylesheet).toContain('border-color: var(--color-success-border);');
    expect(stylesheet).toContain('background: var(--color-success-bg);');
    expect(stylesheet).toContain('width: min(286px, calc(100vw - 24px));');
    expect(stylesheet).toContain('@media (max-width: 560px)');
    expect(stylesheet).toContain('&__permission-label');
    expect(stylesheet).toContain('display: none;');
  });

  it('places dispatch first in right actions and protects the narrow layout', () => {
    const component = readWorkspaceStripComponent();
    const pickerStylesheet = readDispatchPickerStylesheet();
    const actionsStart = component.indexOf(
      '<div className="bitfun-chat-input-workspace-strip__actions">',
    );
    const dispatchIndex = component.indexOf('<DispatchTargetPicker', actionsStart);
    const permissionIndex = component.indexOf('{showPermission ? (', actionsStart);

    expect(actionsStart).toBeGreaterThan(-1);
    expect(dispatchIndex).toBeGreaterThan(actionsStart);
    expect(permissionIndex).toBeGreaterThan(dispatchIndex);
    expect(pickerStylesheet).toContain('max-width: 142px;');
    expect(pickerStylesheet).toContain('@media (max-width: 560px)');
    expect(pickerStylesheet).toContain('width: 18px;');
    expect(pickerStylesheet).toContain('> .dispatch-target-picker__chevron');
  });
});
