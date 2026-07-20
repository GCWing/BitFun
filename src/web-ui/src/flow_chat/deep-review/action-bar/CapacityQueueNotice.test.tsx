import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { CapacityQueueNotice } from './CapacityQueueNotice';

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
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Waiting for model capacity');
    expect(html).toContain('BitFun is waiting for temporary model capacity.');
    expect(html).toContain('Reason: model concurrency limit');
    expect(html).toContain('The model provider rejected another concurrent review request.');
    expect(html).toContain('Waited 12s of 1m 0s');
    expect(html).toContain('Pause waiting');
    expect(html).toContain('Keep core checks');
    expect(html).not.toContain('Run slower next time');
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
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Reason: earlier review work still running');
    expect(html).toContain('Waiting preserves the planned review order');
    expect(html).toContain('Waiting for review capacity');
    expect(html).not.toContain('Active review work: 2');
    expect(html).not.toContain('Waited 4s of 1m 0s');
  });

  it('explains launch-batch waits that outlast the configured capacity window', () => {
    const html = renderToStaticMarkup(
      <CapacityQueueNotice
        capacityQueueState={{
          status: 'queued_for_capacity',
          reason: 'launch_batch_blocked',
          queuedReviewerCount: 1,
          activeReviewerCount: 1,
          queueElapsedMs: 90_000,
          maxQueueWaitSeconds: 60,
        }}
        supportsInlineQueueControls
        onPauseQueue={vi.fn()}
        onContinueQueue={vi.fn()}
        onSkipOptionalQueuedReviewers={vi.fn()}
        onCancelQueuedReviewers={vi.fn()}
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('waited longer than the configured capacity window');
    expect(html).toContain('Cancel waiting review');
    expect(html).toContain('Open Review settings');
    expect(html).not.toContain('Run slower next time');
    expect(html).not.toContain('Waited 1m 30s of 1m 0s');
  });

  it('does not show the long launch-batch detail before the capacity window is exceeded', () => {
    const html = renderToStaticMarkup(
      <CapacityQueueNotice
        capacityQueueState={{
          status: 'queued_for_capacity',
          reason: 'launch_batch_blocked',
          queuedReviewerCount: 1,
          activeReviewerCount: 1,
          queueElapsedMs: 30_000,
          maxQueueWaitSeconds: 60,
        }}
        supportsInlineQueueControls
        onPauseQueue={vi.fn()}
        onContinueQueue={vi.fn()}
        onSkipOptionalQueuedReviewers={vi.fn()}
        onCancelQueuedReviewers={vi.fn()}
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).not.toContain('waited longer than the configured capacity window');
  });

  it('explains active-reviewer waits without presenting max wait as a hard timeout', () => {
    const html = renderToStaticMarkup(
      <CapacityQueueNotice
        capacityQueueState={{
          status: 'queued_for_capacity',
          reason: 'local_concurrency_cap',
          queuedReviewerCount: 1,
          activeReviewerCount: 2,
          queueElapsedMs: 70_000,
          maxQueueWaitSeconds: 60,
        }}
        supportsInlineQueueControls
        onPauseQueue={vi.fn()}
        onContinueQueue={vi.fn()}
        onSkipOptionalQueuedReviewers={vi.fn()}
        onCancelQueuedReviewers={vi.fn()}
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Waiting for review capacity');
    expect(html).toContain('Waiting review work starts when active review work frees capacity.');
    expect(html).not.toContain('Active review work: 2');
    expect(html).not.toContain('Waited 1m 10s of 1m 0s');
  });

  it('keeps waiting review work summarized instead of listing individual reviewers', () => {
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
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Review waiting for capacity');
    expect(html).not.toContain('Security reviewer');
    expect(html).not.toContain('Frontend reviewer');
    expect(html).not.toContain('Paused');
    expect(html).not.toContain('Waited 9s');
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
        onOpenReviewSettings={vi.fn()}
      />,
    );

    expect(html).toContain('Use Stop to interrupt this review.');
    expect(html).not.toContain('Pause waiting');
  });
});
