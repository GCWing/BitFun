import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { PartialResultsPanel } from './PartialResultsPanel';

const messages: Record<string, string> = {
  'deepReviewActionBar.hidePartialResults': 'Hide partial results',
  'deepReviewActionBar.partialIssues': '{{count}} issues found',
  'deepReviewActionBar.partialRemediationItems': '{{count}} remediation items',
  'deepReviewActionBar.partialResultsDescription': '{{completed}}/{{total}} reviewers completed',
  'deepReviewActionBar.partialReviewerSummaries': '{{count}} reviewer summaries',
  'deepReviewActionBar.viewPartialResults': 'View partial results',
};

function t(key: string, options?: Record<string, unknown> & { defaultValue?: string }): string {
  const template = messages[key] ?? options?.defaultValue ?? key;
  return template.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? _match));
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t,
  }),
}));

describe('PartialResultsPanel', () => {
  const progressSummary = {
    completed: 2,
    failed: 1,
    timedOut: 0,
    running: 1,
    skipped: 0,
    unknown: 1,
    handled: 3,
    total: 4,
    text: '2/4 completed',
  };

  const partialResults = {
    hasPartialResults: true,
    completedReviewerCount: 2,
    totalReviewerCount: 4,
    completedIssues: [{ title: 'Race condition' }],
    completedRemediationItems: ['Guard async state'],
    completedReviewerSummaries: ['Security completed'],
  };

  it('renders interruption summary in collapsed state', () => {
    const html = renderToStaticMarkup(
      <PartialResultsPanel
        progressSummary={progressSummary}
        partialResults={partialResults}
        showPartialResults={false}
        onTogglePartialResults={vi.fn()}
      />,
    );

    expect(html).toContain('2/4 reviewers completed');
    expect(html).toContain('View partial results');
    expect(html).not.toContain('1 issues found');
  });

  it('renders partial result details when expanded', () => {
    const html = renderToStaticMarkup(
      <PartialResultsPanel
        progressSummary={progressSummary}
        partialResults={partialResults}
        showPartialResults
        onTogglePartialResults={vi.fn()}
      />,
    );

    expect(html).toContain('Hide partial results');
    expect(html).toContain('1 issues found');
    expect(html).toContain('1 remediation items');
    expect(html).toContain('Security completed');
  });
});
