import { describe, expect, it } from 'vitest';
import {
  formatFileList,
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
