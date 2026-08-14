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

function readChatInputStylesheet(): string {
  return readFileSync(
    fileURLToPath(new URL('./ChatInput.scss', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function readStatusControlStylesheet(name: string): string {
  return readFileSync(
    fileURLToPath(new URL(`./${name}.scss`, import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

function readWorkspaceStripComponent(): string {
  return readFileSync(
    fileURLToPath(new URL('./ChatInputWorkspaceStrip.tsx', import.meta.url)),
    'utf8',
  ).replace(/\r\n/g, '\n');
}

describe('ChatInputWorkspaceStrip layout styles', () => {
  it('keeps the status band compact and borderless', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain('min-height: 32px;');
    expect(stylesheet).toContain('border: 0;');
    expect(stylesheet).toContain('background: transparent;');
    expect(stylesheet).toContain('minmax(0, 38fr)');
    expect(stylesheet).toContain('minmax(0, 36fr)');
    expect(stylesheet).toContain('minmax(0, 26fr)');
    expect(stylesheet).toContain('border-left: 1px solid var(--bf-appearance-token-border-subtle);');
    expect(stylesheet).toContain('height: 24px;');
    expect(stylesheet).toContain('width: 16px;');
    expect(stylesheet).toContain('height: 16px;');
    expect(readChatInputStylesheet()).toContain('padding-bottom: 30px;');
    expect(readStatusControlStylesheet('HarnessProfileSelector')).toContain('min-height: 20px;');
    expect(readStatusControlStylesheet('ReasoningPresetSelector')).toContain('height: 24px;');
  });

  it('keeps workspace, harness, and runtime facts in one horizontal track', () => {
    const component = readWorkspaceStripComponent();

    expect(component).toContain('<FolderPen size={16}');
    expect(component).toContain('bitfun-chat-input-workspace-strip__breadcrumb-chevron');
    expect(component).toContain('<HarnessProfileSelector');
    expect(component).toContain('bitfun-chat-input-workspace-strip__permission-host');
    expect(component).toContain('bitfun-chat-input-workspace-strip__runtime');
    expect(component).toContain('bitfun-chat-input-workspace-strip__runtime-divider');
    expect(component).toContain('<span>{usagePercentage}%</span>');
    expect(component).toContain('bitfun-chat-input-workspace-strip__usage-ring');
  });

  it('keeps model-only information in the single-line composer', () => {
    const stylesheet = readChatInputStylesheet();

    expect(stylesheet).toContain('.bitfun-reasoning-preset-selector,');
    expect(stylesheet).toContain('.bitfun-model-selector__ctx-usage {');
    expect(stylesheet).toContain('background: transparent !important;');
    expect(stylesheet).toContain('margin-right: 20px;');
  });

  it('keeps approval as a compact chip in the Harness group', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain('&__permission-trigger');
    expect(stylesheet).toContain('&__permission-overview-icon');
    expect(stylesheet).toContain('&__permission-host');
    expect(stylesheet).toContain('&__actions--portal-only');
    expect(stylesheet).toContain('&__runtime');
    expect(stylesheet).toContain('&__runtime-divider');
    expect(stylesheet).not.toContain('&__permission-meter');
    expect(stylesheet).toContain('width: min(240px, calc(100vw - 16px));');
    expect(stylesheet).toContain('@media (max-width: 560px)');
    expect(stylesheet).toContain('&__permission-label');
    expect(stylesheet).toContain('display: none;');
  });

  it('keeps the local target breadcrumb visible while Git gates mutating controls', () => {
    const component = readWorkspaceStripComponent();

    expect(component).toContain(
      "import { DispatchTargetPicker } from '@/features/dispatch/DispatchTargetPicker';",
    );
    expect(component).toContain('<DispatchTargetPicker');
    expect(component).toContain(
      'const isGitWorkspace = isRepository || isWorktree || worktreeEnabled;',
    );
    expect(component).toContain('const showWorktreeToggle = !!worktreeControl && isGitWorkspace;');
    expect(component).toContain('const showDispatchPicker = !!dispatchControl;');
    expect(component).toContain(
      'const dispatchPickerLocked = !!dispatchControl && (dispatchControl.locked || !isGitWorkspace);',
    );
    expect(component).toContain('locked={dispatchPickerLocked}');
    expect(component).not.toContain('0.2.15 release gate');
  });
});
