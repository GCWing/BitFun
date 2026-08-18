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

describe('ChatInputWorkspaceStrip layout styles', () => {
  it('keeps the session usage action visible without overpowering the strip', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain('max-width: 100%;');
    expect(stylesheet).toContain('width: 16px;');
    expect(stylesheet).toContain('height: 16px;');
    expect(stylesheet).toContain('min-width: 16px;');
    expect(stylesheet).toContain('width: 14px;');
    expect(stylesheet).toContain('height: 14px;');
    expect(stylesheet).toContain('color: color-mix(in srgb, var(--bf-appearance-token-color-accent-500) 62%, var(--bf-appearance-token-color-text-secondary));');
    expect(stylesheet).toContain('color: color-mix(in srgb, var(--bf-appearance-token-color-accent-500) 86%, var(--bf-appearance-token-color-text-primary));');
  });

  it('keeps the permission control compact and collapses labels on narrow screens', () => {
    const stylesheet = readWorkspaceStripStylesheet();

    expect(stylesheet).toContain('&__permission-trigger');
    expect(stylesheet).toContain('min-width: 18px;');
    expect(stylesheet).toContain('&--ask {');
    expect(stylesheet).toContain('border-color: var(--bf-appearance-token-color-success-border);');
    expect(stylesheet).toContain('background: var(--bf-appearance-token-color-success-bg);');
    expect(stylesheet).toContain('width: min(240px, calc(100vw - 16px));');
    expect(stylesheet).toContain('@media (max-width: 560px)');
    expect(stylesheet).toContain('&__permission-label');
    expect(stylesheet).toContain('display: none;');
  });

  it('mounts the dispatch picker behind the same Git gate as worktree isolation', () => {
    const component = readWorkspaceStripComponent();

    expect(component).toContain(
      "import { DispatchTargetPicker } from '@/features/dispatch/DispatchTargetPicker';",
    );
    expect(component).toContain('<DispatchTargetPicker');
    // One Git probe decides both controls, so they can never disagree about
    // whether the workspace is a repository. A repository Git refuses to read
    // for ownership reasons counts: `isRepository` only turns true after a
    // status call that rejection blocks, and hiding the controls there would
    // hide the very state the user has to act on.
    expect(component).toContain(
      'const isGitWorkspace = isRepository || repositoryTrustRequired || isWorktree || worktreeEnabled;',
    );
    expect(component).toContain('const showWorktreeToggle = !!worktreeControl && isGitWorkspace;');
    expect(component).toContain('const showDispatchPicker = !!dispatchControl && isGitWorkspace;');
    expect(component).not.toContain('0.2.15 release gate');
  });
});
