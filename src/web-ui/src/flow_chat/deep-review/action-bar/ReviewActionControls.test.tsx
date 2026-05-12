import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { ReviewActionControls } from './ReviewActionControls';

vi.mock('../../tool-cards/CodeReviewReportExportActions', () => ({
  CodeReviewReportExportActions: ({ actions, variant }: { actions?: string[]; variant?: string }) => (
    <span data-actions={actions?.join(',')} data-variant={variant}>Open as Markdown</span>
  ),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => {
      const template = options?.defaultValue ?? _key;
      return template.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? _match));
    },
  }),
}));

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
  isFixDisabled: false,
  isResumeRunning: false,
  remainingFixIds: [],
  modelRecoveryAction: null,
  onRetryIncompleteSlices: vi.fn(),
  onStartFixing: vi.fn(),
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

    expect(html).toContain('Retry incomplete slices (2)');
    expect(html).toContain('Start fixing');
    expect(html).toContain('Fix and re-review');
    expect(html).toContain('Fill to input');
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
    expect(html).toContain('Copy diagnostics');
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

    expect(html).toContain('Fix was interrupted. 2 items remain.');
    expect(html).toContain('Continue fixing 2 items');
    expect(html).toContain('Skip remaining');
  });
});
