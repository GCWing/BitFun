import { describe, expect, it } from 'vitest';
import type { SessionReviewActivity } from './sessionReviewActivity';
import { shouldBlockDeepReviewCommand } from './deepReviewCommandGuard';

function activity(overrides: Partial<SessionReviewActivity> = {}): SessionReviewActivity {
  return {
    parentSessionId: 'parent',
    childSessionId: 'review-child',
    kind: 'deep_review',
    lifecycle: 'running',
    isBlocking: true,
    startedAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

describe('shouldBlockDeepReviewCommand', () => {
  it('blocks strict Review typed commands while the parent session already has a blocking review activity', () => {
    expect(shouldBlockDeepReviewCommand('/review strict', activity())).toBe(true);
    expect(shouldBlockDeepReviewCommand('/review deep focus on auth', activity())).toBe(false);
    expect(shouldBlockDeepReviewCommand('/DeepReview focus on auth', activity())).toBe(true);
    expect(shouldBlockDeepReviewCommand('/deepreview focus on auth', activity())).toBe(true);
  });

  it('does not block non-strict Review input or completed review activity', () => {
    expect(shouldBlockDeepReviewCommand('please review this', activity())).toBe(false);
    expect(shouldBlockDeepReviewCommand('/review', activity())).toBe(false);
    expect(
      shouldBlockDeepReviewCommand(
        '/review strict',
        activity({
          lifecycle: 'completed',
          isBlocking: false,
        }),
      ),
    ).toBe(false);
  });
});
