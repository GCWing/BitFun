import { describe, expect, it } from 'vitest';
import type { PullRequestContext } from '@/shared/types/context';
import { formatContextForPrompt } from './contextPrompt';

describe('formatContextForPrompt', () => {
  it('includes remote id for pull request contexts', () => {
    const context: PullRequestContext = {
      id: 'pr-1',
      type: 'pull-request',
      label: 'PR #42 overview',
      section: 'overview',
      content: 'Review this change.',
      remoteId: 'origin-github',
      repository: 'owner/repo',
      pullRequestNumber: 42,
      pullRequestTitle: 'Fix bug',
      sourceUrl: 'https://example.com/owner/repo/pull/42',
      timestamp: 123,
    };

    const rendered = formatContextForPrompt(context);

    expect(rendered).toContain('Remote ID: origin-github');
    expect(rendered).toContain('Repository: owner/repo');
    expect(rendered).toContain('Pull Request: #42 Fix bug');
    expect(rendered).toContain('URL: https://example.com/owner/repo/pull/42');
  });

  it('formats pull request CI contexts', () => {
    const context: PullRequestContext = {
      id: 'pr-ci-1',
      type: 'pull-request',
      label: 'PR #42 CI',
      section: 'ci',
      content: 'Checks: 2/3 passed, 1 failed, 0 pending',
      remoteId: 'origin-github',
      repository: 'owner/repo',
      pullRequestNumber: 42,
      pullRequestTitle: 'Fix bug',
      timestamp: 123,
    };

    const rendered = formatContextForPrompt(context);

    expect(rendered).toContain('[Pull Request Context: PR #42 CI]');
    expect(rendered).toContain('Section: ci');
    expect(rendered).toContain('Checks: 2/3 passed, 1 failed, 0 pending');
  });
});
