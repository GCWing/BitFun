import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { CodeReviewReportExportActions } from './CodeReviewReportExportActions';
import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';

const formatCodeReviewReportMarkdownMock = vi.hoisted(() => vi.fn(() => '# Review'));

function Icon({ name }: { name: string }) {
  return <svg data-icon={name} />;
}

vi.mock('lucide-react', () => ({
  Loader2: () => <Icon name="loader" />,
}));

vi.mock('@openbitfun/ui', () => ({
  Button: ({
    children,
    leadingIcon,
    onClick,
  }: {
    children: React.ReactNode;
    leadingIcon?: React.ReactNode;
    onClick?: React.MouseEventHandler<HTMLButtonElement>;
  }) => (
    <button type="button" onClick={onClick}>
      {leadingIcon}
      {children}
    </button>
  ),
  Icon: ({ name }: { name: string }) => <Icon name={name} />,
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const labels: Record<string, string> = {
        'toolCards.codeReview.export.copyMarkdown': 'Copy Markdown',
        'toolCards.codeReview.export.openMarkdown': 'Open as Markdown',
        'toolCards.codeReview.export.saveMarkdown': 'Save Markdown',
        'toolCards.codeReview.coverageSources.focusedCheck': '补充检查',
      };
      return labels[key] ?? key;
    },
  }),
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

vi.mock('@/shared/utils/tabUtils', () => ({
  createMarkdownEditorTab: vi.fn(),
}));

vi.mock('../utils/codeReviewReport', () => ({
  formatCodeReviewReportMarkdown: (...args: unknown[]) => formatCodeReviewReportMarkdownMock(...args),
}));

describe('CodeReviewReportExportActions', () => {
  it('uses the same copy icon as other copy buttons', () => {
    const html = renderToStaticMarkup(
      <CodeReviewReportExportActions reviewData={{ summary: { recommended_action: 'approve' } }} />,
    );

    expect(html).toContain('aria-label="Copy Markdown"');
    expect(html).toContain('data-icon="duplicate"');
    expect(html).not.toContain('data-icon="clipboard-copy"');
  });

  it('uses a download icon for saving Markdown', () => {
    const html = renderToStaticMarkup(
      <CodeReviewReportExportActions reviewData={{ summary: { recommended_action: 'approve' } }} />,
    );

    expect(html).toContain('aria-label="Save Markdown"');
    expect(html).toContain('data-icon="arrow-down"');
  });

  it('can limit the visible export actions for compact surfaces', () => {
    const html = renderToStaticMarkup(
      <CodeReviewReportExportActions
        reviewData={{ summary: { recommended_action: 'approve' } }}
        actions={['copy', 'save']}
      />,
    );

    expect(html).toContain('aria-label="Copy Markdown"');
    expect(html).toContain('aria-label="Save Markdown"');
    expect(html).not.toContain('aria-label="Open as Markdown"');
  });

  it('passes the review run manifest into Markdown formatting', () => {
    const runManifest = {
      strategyLevel: 'quick',
      skippedReviewers: [],
    };

    renderToStaticMarkup(
      <CodeReviewReportExportActions
        reviewData={{
          review_mode: 'deep',
          summary: { recommended_action: 'approve' },
        }}
        runManifest={runManifest as unknown as ReviewTeamRunManifest}
      />,
    );

    expect(formatCodeReviewReportMarkdownMock).toHaveBeenCalledWith(
      {
        review_mode: 'deep',
        summary: { recommended_action: 'approve' },
      },
      expect.any(Object),
      { runManifest },
    );
  });

  it('does not project a Deep Review manifest into a standard Review export', () => {
    const runManifest = {
      strategyLevel: 'quick',
      skippedReviewers: [],
    };

    renderToStaticMarkup(
      <CodeReviewReportExportActions
        reviewData={{
          review_mode: 'standard',
          summary: { recommended_action: 'approve' },
        }}
        runManifest={runManifest as unknown as ReviewTeamRunManifest}
      />,
    );

    expect(formatCodeReviewReportMarkdownMock).toHaveBeenLastCalledWith(
      {
        review_mode: 'standard',
        summary: { recommended_action: 'approve' },
      },
      expect.any(Object),
      { runManifest: undefined },
    );
  });

  it('passes the localized additional-check label into Markdown formatting', () => {
    renderToStaticMarkup(
      <CodeReviewReportExportActions reviewData={{ summary: { recommended_action: 'approve' } }} />,
    );

    expect(formatCodeReviewReportMarkdownMock).toHaveBeenLastCalledWith(
      { summary: { recommended_action: 'approve' } },
      expect.objectContaining({
        coverageSourceLabels: expect.objectContaining({ focusedCheck: '补充检查' }),
      }),
      { runManifest: undefined },
    );
  });
});
