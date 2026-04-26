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
  it('blocks /DeepReview while the parent session already has a blocking review activity', () => {
    expect(shouldBlockDeepReviewCommand('/DeepReview', activity())).toBe(true);
    expect(shouldBlockDeepReviewCommand('/DeepReview focus on auth', activity())).toBe(true);
  });

  it('does not block non-DeepReview input, lowercase aliases, or completed review activity', () => {
    expect(shouldBlockDeepReviewCommand('please review this', activity())).toBe(false);
    expect(shouldBlockDeepReviewCommand('/deepreview', activity())).toBe(false);
    expect(
      shouldBlockDeepReviewCommand(
        '/DeepReview',
        activity({
          lifecycle: 'completed',
          isBlocking: false,
        }),
      ),
    ).toBe(false);
  });
});
