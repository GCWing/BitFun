import { describe, expect, it } from 'vitest';
import {
  formatFileList,
  formatPullRequestLaunchPrompt,
  formatSessionFilesLaunchPrompt,
  formatSlashCommandLaunchPrompt,
} from './launchPrompt';

describe('Deep Review launch prompt formatting', () => {
  it('formats review file lists as markdown bullets', () => {
    expect(formatFileList(['src/a.ts', 'src/b.ts'])).toBe('- src/a.ts\n- src/b.ts');
  });

  it('builds a session-files prompt with explicit scope and optional focus', () => {
    const prompt = formatSessionFilesLaunchPrompt({
      filePaths: ['src/a.ts'],
      extraContext: 'check regressions',
      reviewTeamPromptBlock: 'Review team manifest.',
    });

    expect(prompt).toContain('Review scope: ONLY inspect the following files modified in this session.');
    expect(prompt).toContain('- src/a.ts');
    expect(prompt).toContain('User-provided focus:\ncheck regressions');
    expect(prompt).toContain('Review team manifest.');
  });

  it('builds a pull-request prompt that uses provider diff as source of truth', () => {
    const prompt = formatPullRequestLaunchPrompt({
      filePaths: ['src/a.ts'],
      extraContext: 'PR #42',
      diffContext: 'File: src/a.ts\nPatch:\n+changed',
      reviewTeamPromptBlock: 'Review team manifest.',
    });

    expect(prompt).toContain('Review scope: ONLY inspect the following files changed by this pull request.');
    expect(prompt).toContain('Pull request context:\nPR #42');
    expect(prompt).toContain('Pull request provider diff:\nFile: src/a.ts');
    expect(prompt).toContain('Treat the provider diff as the source of truth');
  });

  it('builds a slash-command prompt with original command and fallback focus', () => {
    const prompt = formatSlashCommandLaunchPrompt({
      commandText: '/DeepReview',
      extraContext: '',
      reviewTeamPromptBlock: 'Review team manifest.',
    });

    expect(prompt).toContain('Original command:\n/DeepReview');
    expect(prompt).toContain(
      'User-provided focus or target:\nNone. If no explicit target is given, review the current workspace changes relative to HEAD.',
    );
    expect(prompt).toContain('Review team manifest.');
  });
});
