import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { CodeReviewReportExportActions } from './CodeReviewReportExportActions';

function Icon({ name }: { name: string }) {
  return <svg data-icon={name} />;
}

vi.mock('lucide-react', () => ({
  Check: () => <Icon name="check" />,
  ClipboardCopy: () => <Icon name="clipboard-copy" />,
  Copy: () => <Icon name="copy" />,
  Download: () => <Icon name="download" />,
  FileDown: () => <Icon name="file-down" />,
  FilePenLine: () => <Icon name="file-pen-line" />,
  Loader2: () => <Icon name="loader" />,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? _key,
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    onClick,
  }: {
    children: React.ReactNode;
    onClick?: React.MouseEventHandler<HTMLButtonElement>;
  }) => <button type="button" onClick={onClick}>{children}</button>,
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
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
  formatCodeReviewReportMarkdown: () => '# Review',
}));

describe('CodeReviewReportExportActions', () => {
  it('uses the same copy icon as other copy buttons', () => {
    const html = renderToStaticMarkup(
      <CodeReviewReportExportActions reviewData={{ summary: { recommended_action: 'approve' } }} />,
    );

    expect(html).toContain('aria-label="Copy Markdown"');
    expect(html).toContain('data-icon="copy"');
    expect(html).not.toContain('data-icon="clipboard-copy"');
  });

  it('uses a download icon for saving Markdown', () => {
    const html = renderToStaticMarkup(
      <CodeReviewReportExportActions reviewData={{ summary: { recommended_action: 'approve' } }} />,
    );

    expect(html).toContain('aria-label="Save Markdown"');
    expect(html).toContain('data-icon="download"');
    expect(html).not.toContain('data-icon="file-down"');
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
});
