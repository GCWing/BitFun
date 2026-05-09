import { describe, expect, it } from 'vitest';
import { buildRecoveryPlan } from './deepReviewExperience';

describe('deepReviewExperience', () => {
  it('keeps skipped reviewers out of rerun work and shows them in the recovery plan', () => {
    const plan = buildRecoveryPlan({
      phase: 'review_interrupted',
      childSessionId: 'deep-review-session',
      originalTarget: '/DeepReview review latest commit',
      errorDetail: { category: 'model_error' },
      canResume: true,
      recommendedActions: [],
      reviewers: [
        { reviewer: 'ReviewPerformance', status: 'completed' },
        { reviewer: 'ReviewSecurity', status: 'timed_out' },
        { reviewer: 'ReviewFrontend', status: 'skipped' },
      ],
    });

    expect(plan.willPreserve).toEqual(['ReviewPerformance']);
    expect(plan.willRerun).toEqual(['ReviewSecurity']);
    expect(plan.willSkip).toEqual(['ReviewFrontend']);
  });
});
