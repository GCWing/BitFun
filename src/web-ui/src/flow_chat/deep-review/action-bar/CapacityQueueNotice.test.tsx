import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { CapacityQueueNotice } from './CapacityQueueNotice';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, options?: Record<string, unknown> & { defaultValue?: string }) => {
      if (_key === 'deepReviewActionBar.capacityQueue.reasons.launchBatchBlocked') {
        return 'previous launch batch still running';
      }
      const template = options?.defaultValue ?? _key;
      return template.replace(/{{(\w+)}}/g, (_match, token: string) => String(options?.[token] ?? _match));
    },
  }),
}));

vi.mock('@/component-library', () => ({
  Button: ({
    children,
  }: {
    children: React.ReactNode;
  }) => <button type="button">{children}</button>,
}));

describe('CapacityQueueNotice', () => {
  it('renders queue reason, elapsed time, and compact controls', () => {
    const html = renderToStaticMarkup(
      <CapacityQueueNotice
        capacityQueueState={{
          status: 'queued_for_capacity',
          reason: 'provider_concurrency_limit',
          queuedReviewerCount: 2,
          optionalReviewerCount: 1,
          queueElapsedMs: 12_000,
          maxQueueWaitSeconds: 60,
          sessionConcurrencyHigh: true,
        }}
        supportsInlineQueueControls
        onPauseQueue={vi.fn()}
        onContinueQueue={vi.fn()}
        onSkipOptionalQueuedReviewers={vi.fn()}
        onCancelQueuedReviewers={vi.fn()}
        onRunSlowerNextTime={vi.fn()}
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Reviewers waiting for capacity');
    expect(html).toContain('Queue wait does not count against reviewer runtime.');
    expect(html).toContain('Reason: provider concurrency limit');
    expect(html).toContain('The model provider rejected another concurrent reviewer.');
    expect(html).toContain('Waited 12s of 1m 0s');
    expect(html).toContain('Pause queue');
    expect(html).toContain('Skip optional extras');
    expect(html).toContain('Run slower next time');
  });

  it('renders launch-batch waiting as a concrete queue reason', () => {
    const html = renderToStaticMarkup(
      <CapacityQueueNotice
        capacityQueueState={{
          status: 'queued_for_capacity',
          reason: 'launch_batch_blocked',
          queuedReviewerCount: 1,
          activeReviewerCount: 2,
          queueElapsedMs: 4_000,
          maxQueueWaitSeconds: 60,
        }}
        supportsInlineQueueControls
        onPauseQueue={vi.fn()}
        onContinueQueue={vi.fn()}
        onSkipOptionalQueuedReviewers={vi.fn()}
        onCancelQueuedReviewers={vi.fn()}
        onRunSlowerNextTime={vi.fn()}
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Reason: previous launch batch still running');
    expect(html).toContain('Waiting preserves the planned review order');
    expect(html).toContain('Waited 4s of 1m 0s');
  });

  it('renders the specific reviewers currently waiting', () => {
    const html = renderToStaticMarkup(
      <CapacityQueueNotice
        capacityQueueState={{
          status: 'queued_for_capacity',
          reason: 'local_concurrency_cap',
          queuedReviewerCount: 2,
          waitingReviewers: [
            {
              toolId: 'task-security',
              subagentType: 'ReviewSecurity',
              displayName: 'Security reviewer',
              status: 'queued_for_capacity',
              reason: 'local_concurrency_cap',
              queueElapsedMs: 9_000,
            },
            {
              toolId: 'task-frontend',
              subagentType: 'ReviewFrontend',
              displayName: 'Frontend reviewer',
              status: 'paused_by_user',
              reason: 'launch_batch_blocked',
            },
          ],
        }}
        supportsInlineQueueControls
        onPauseQueue={vi.fn()}
        onContinueQueue={vi.fn()}
        onSkipOptionalQueuedReviewers={vi.fn()}
        onCancelQueuedReviewers={vi.fn()}
        onRunSlowerNextTime={vi.fn()}
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Waiting reviewers');
    expect(html).toContain('Security reviewer');
    expect(html).toContain('Frontend reviewer');
    expect(html).toContain('Paused');
    expect(html).toContain('Waited 9s');
  });

  it('renders the stop hint when inline queue controls are unavailable', () => {
    const html = renderToStaticMarkup(
      <CapacityQueueNotice
        capacityQueueState={{
          status: 'queued_for_capacity',
          queuedReviewerCount: 1,
          controlMode: 'session_stop_only',
        }}
        supportsInlineQueueControls={false}
        onPauseQueue={vi.fn()}
        onContinueQueue={vi.fn()}
        onSkipOptionalQueuedReviewers={vi.fn()}
        onCancelQueuedReviewers={vi.fn()}
        onRunSlowerNextTime={vi.fn()}
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Use Stop to interrupt this review queue.');
    expect(html).not.toContain('Pause queue');
  });
});
