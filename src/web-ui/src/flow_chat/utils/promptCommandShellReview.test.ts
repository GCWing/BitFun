import { describe, expect, it, vi } from 'vitest';
import { ConfirmDialogOptions } from '@/infrastructure/confirm-dialog';
import type { PromptCommandShellReviewPlan } from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { reviewPromptCommandShell } from './promptCommandShellReview';

const plan = (canRemember: boolean): PromptCommandShellReviewPlan => ({
  schemaVersion: 1,
  planFingerprint: 'sha256:plan',
  sourceDisplayName: 'OpenCode project commands',
  workingDirectory: 'D:/workspace/project',
  shellDisplayName: 'PowerShell 7',
  shellExecutable: 'C:/Program Files/PowerShell/7/pwsh.exe',
  commands: ['git status --short', 'git diff --stat'],
  canRemember,
  preferenceRevision: 4,
});

const translate = (key: string, values?: Record<string, unknown>) => (
  values ? `${key}:${JSON.stringify(values)}` : key
);

describe('reviewPromptCommandShell', () => {
  it('returns an exact run-once decision and previews every command', async () => {
    let options: ConfirmDialogOptions | undefined;
    const confirm = vi.fn(async (next: ConfirmDialogOptions) => {
      options = next;
      return 'confirm' as const;
    });

    const decision = await reviewPromptCommandShell(plan(true), translate, confirm);

    expect(decision).toEqual({
      planFingerprint: 'sha256:plan',
      mode: 'run_once',
      expectedPreferenceRevision: 4,
    });
    expect(options?.preview).toContain('$ git status --short');
    expect(options?.preview).toContain('$ git diff --stat');
    expect(options?.message).toContain('C:/Program Files/PowerShell/7/pwsh.exe');
    expect(options?.secondaryText).toBe('chatInput.promptCommandShellReview.remember');
  });

  it('does not offer remembered approval for argument-dependent commands', async () => {
    let options: ConfirmDialogOptions | undefined;
    const confirm = vi.fn(async (next: ConfirmDialogOptions) => {
      options = next;
      return 'confirm' as const;
    });

    await reviewPromptCommandShell(plan(false), translate, confirm);

    expect(options?.secondaryText).toBeUndefined();
  });

  it('maps the secondary action to remembered approval and cancel to no decision', async () => {
    const remember = await reviewPromptCommandShell(
      plan(true),
      translate,
      vi.fn(async () => 'secondary' as const),
    );
    const cancel = await reviewPromptCommandShell(
      plan(true),
      translate,
      vi.fn(async () => 'cancel' as const),
    );

    expect(remember?.mode).toBe('remember');
    expect(cancel).toBeNull();
  });
});
