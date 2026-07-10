import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { ReviewActionControls } from './ReviewActionControls';

vi.mock('../../tool-cards/CodeReviewReportExportActions', () => ({
  CodeReviewReportExportActions: ({ actions, variant }: { actions?: string[]; variant?: string }) => (
    <span data-actions={actions?.join(',')} data-variant={variant}>Open as Markdown</span>
  ),
}));

vi.mock('react-i18next', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return {
    useTranslation: () => ({
      t: createTestI18nT('flow-chat'),
    }),
  };
});

vi.mock('@/component-library', () => ({
  Button: ({
    children,
    disabled,
  }: {
    children: React.ReactNode;
    disabled?: boolean;
  }) => <button type="button" disabled={disabled}>{children}</button>,
  Checkbox: ({
    checked,
    disabled,
  }: {
    checked?: boolean;
    disabled?: boolean;
  }) => <input type="checkbox" checked={checked} disabled={disabled} readOnly />,
  Tooltip: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

const baseProps = {
  isDeepReview: true,
  retryableSliceCount: 0,
  remediationItemCount: 0,
  hasInterruption: false,
  partialResultsAvailable: false,
  activeAction: null,
  followUpReviewState: 'none' as const,
  canLaunchFollowUpReview: true,
  isFixDisabled: false,
  isResumeRunning: false,
  remainingFixIds: [],
  modelRecoveryAction: null,
  onRetryIncompleteSlices: vi.fn(),
  onStartFixing: vi.fn(),
  onReviewFixes: vi.fn(),
  onOpenFollowUpReview: vi.fn(),
  onFillBackInput: vi.fn(),
  onContinueReview: vi.fn(),
  onOpenModelSettings: vi.fn(),
  onCopyDiagnostics: vi.fn(),
  onViewPartialResults: vi.fn(),
  onContinueFix: vi.fn(),
  onSkipRemainingFixes: vi.fn(),
  onMinimize: vi.fn(),
};

describe('ReviewActionControls', () => {
  it('renders Deep Review retry and remediation actions for completed reviews', () => {
    const html = renderToStaticMarkup(
      <ReviewActionControls
        {...baseProps}
        phase="review_completed"
        retryableSliceCount={2}
        remediationItemCount={1}
      />,
    );

    expect(html).toContain('Retry incomplete review work (2)');
    expect(html).toContain('Start fixing');
    expect(html).not.toContain('Fix &amp; re-review');
    expect(html).toContain('Fill to input');
  });

  it('offers an independent review only after fixing completes', () => {
    const html = renderToStaticMarkup(
      <ReviewActionControls
        {...baseProps}
        phase="fix_completed"
      />,
    );

    expect(html).toContain('Review fixes');
    expect(html).not.toContain('Start fixing');
  });

  it('labels recoverable and completed follow-up reviews without exposing internal state', () => {
    const retryHtml = renderToStaticMarkup(
      <ReviewActionControls {...baseProps} phase="fix_completed" followUpReviewState="retry" />,
    );
    const completedHtml = renderToStaticMarkup(
      <ReviewActionControls {...baseProps} phase="fix_completed" followUpReviewState="completed" />,
    );

    expect(retryHtml).toContain('Retry review');
    expect(completedHtml).toContain('View review');
    expect(completedHtml).not.toContain('Review started');
  });

  it('keeps failed and cancelled attempts viewable before retrying', () => {
    const failedHtml = renderToStaticMarkup(
      <ReviewActionControls {...baseProps} phase="fix_completed" followUpReviewState="failed" />,
    );
    const cancelledHtml = renderToStaticMarkup(
      <ReviewActionControls {...baseProps} phase="fix_completed" followUpReviewState="cancelled" />,
    );

    expect(failedHtml).toContain('View failed review');
    expect(failedHtml).toContain('Retry review');
    expect(cancelledHtml).toContain('View cancelled review');
    expect(cancelledHtml).toContain('Retry review');
  });

  it('hides unsupported launches while preserving access to an existing review', () => {
    const unavailableHtml = renderToStaticMarkup(
      <ReviewActionControls
        {...baseProps}
        phase="fix_completed"
        canLaunchFollowUpReview={false}
      />,
    );
    const existingHtml = renderToStaticMarkup(
      <ReviewActionControls
        {...baseProps}
        phase="fix_completed"
        canLaunchFollowUpReview={false}
        followUpReviewState="completed"
      />,
    );

    expect(unavailableHtml).not.toContain('Review fixes');
    expect(existingHtml).toContain('View review');
  });

  it('opens a metadata-only review instead of offering a duplicate retry', () => {
    const html = renderToStaticMarkup(
      <ReviewActionControls
        {...baseProps}
        phase="fix_completed"
        followUpReviewState="available"
      />,
    );

    expect(html).toContain('Open review');
    expect(html).not.toContain('Retry review');
  });

  it('places Open as Markdown in the completed review action row', () => {
    const html = renderToStaticMarkup(
      <ReviewActionControls
        {...baseProps}
        phase="review_completed"
        remediationItemCount={1}
        reviewData={{ summary: { recommended_action: 'request_changes' } } as any}
      />,
    );

    expect(html).toContain('Open as Markdown');
    expect(html).toContain('data-actions="open"');
    expect(html).toContain('data-variant="footer"');
  });

  it('renders interruption recovery, diagnostics, and partial-results actions', () => {
    const html = renderToStaticMarkup(
      <ReviewActionControls
        {...baseProps}
        phase="review_interrupted"
        hasInterruption
        partialResultsAvailable
        modelRecoveryAction="switch_model"
      />,
    );

    expect(html).toContain('Continue review');
    expect(html).toContain('Switch model');
    expect(html).toContain('Copy troubleshooting summary');
    expect(html).toContain('View partial results');
  });

  it('renders interrupted fix continuation actions', () => {
    const html = renderToStaticMarkup(
      <ReviewActionControls
        {...baseProps}
        phase="fix_interrupted"
        remainingFixIds={['remediation-1', 'remediation-2']}
      />,
    );

    expect(html).toContain('Fix was interrupted. Up to 2 selected items may still need attention.');
    expect(html).toContain('Recheck and continue (2)');
    expect(html).toContain('Skip remaining');
  });
});
