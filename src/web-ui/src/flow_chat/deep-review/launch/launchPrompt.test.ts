import { describe, expect, it } from 'vitest';
import {
  formatSessionFilesLaunchPrompt,
  formatSlashCommandLaunchPrompt,
} from './launchPrompt';

describe('Deep Review launch prompt formatting', () => {
  it('uses the prepared scope and includes focus only once', () => {
    const prompt = formatSessionFilesLaunchPrompt({
      extraContext: 'check regressions',
      reviewTeamPromptBlock: 'Prepared scope: ["src/a.ts"].',
    });

    expect(prompt).toContain('The target and scopes are already resolved.');
    expect(prompt).toContain('User-provided focus:\ncheck regressions');
    expect(prompt.match(/check regressions/g)).toHaveLength(1);
    expect(prompt).toContain('Prepared scope: ["src/a.ts"].');
  });

  it('bounds focus text without duplicating the original slash command', () => {
    const focus = `security ${'x'.repeat(8_100)}`;
    const prompt = formatSlashCommandLaunchPrompt({
      extraContext: focus,
      reviewTeamPromptBlock: 'Prepared Review execution plan.',
    });

    expect(prompt).toContain('The slash-command target is already resolved.');
    expect(prompt).toContain('Omitted 109 characters from the launch prompt.');
    expect(prompt).not.toContain('Original command:');
    expect(prompt).toContain('Prepared Review execution plan.');
  });
});
