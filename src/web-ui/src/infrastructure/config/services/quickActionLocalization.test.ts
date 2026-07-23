import { describe, expect, it } from 'vitest';
import type { QuickAction } from './AIExperienceConfigService';
import {
  normalizeQuickActionTextForStorage,
  resolveQuickActionText,
} from './quickActionLocalization';

const labels: Record<string, string> = {
  'quickActions.defaults.commit.label': '提交',
  'quickActions.defaults.commit.prompt': '提交当前所有代码变更',
  'quickActions.defaults.createPr.label': '创建 PR',
  'quickActions.defaults.createPr.prompt': '为当前分支创建 Pull Request',
};

const t = (key: string, options?: Record<string, unknown>) =>
  labels[key] ?? String(options?.defaultValue ?? key);

describe('quick action localization', () => {
  it('localizes stored default built-in quick actions', () => {
    const commit: QuickAction = {
      id: 'commit',
      label: 'Commit',
      prompt: 'Commit all current code changes',
      enabled: true,
    };
    const createPr: QuickAction = {
      id: 'create_pr',
      label: 'Create PR',
      prompt: 'Create a Pull Request for the current branch',
      enabled: true,
    };

    expect(resolveQuickActionText(commit, t)).toEqual({
      label: '提交',
      prompt: '提交当前所有代码变更',
    });
    expect(resolveQuickActionText(createPr, t)).toEqual({
      label: '创建 PR',
      prompt: '为当前分支创建 Pull Request',
    });
  });

  it('preserves customized built-in quick action text', () => {
    const customized: QuickAction = {
      id: 'commit',
      label: 'Ship it',
      prompt: 'Commit only the staged files',
      enabled: true,
    };

    expect(resolveQuickActionText(customized, t)).toEqual({
      label: 'Ship it',
      prompt: 'Commit only the staged files',
    });
  });

  it('stores unchanged localized built-in quick action text as canonical defaults', () => {
    const commit: QuickAction = {
      id: 'commit',
      label: 'Commit',
      prompt: 'Commit all current code changes',
      enabled: true,
    };
    const localized = resolveQuickActionText(commit, t);

    expect(normalizeQuickActionTextForStorage(commit, localized.label, localized.prompt, t)).toEqual({
      label: 'Commit',
      prompt: 'Commit all current code changes',
    });
  });

  it('keeps edited localized built-in quick action text customized', () => {
    const commit: QuickAction = {
      id: 'commit',
      label: 'Commit',
      prompt: 'Commit all current code changes',
      enabled: true,
    };
    const localized = resolveQuickActionText(commit, t);

    expect(normalizeQuickActionTextForStorage(commit, localized.label, 'Commit staged changes only', t)).toEqual({
      label: 'Commit',
      prompt: 'Commit staged changes only',
    });
  });
});
