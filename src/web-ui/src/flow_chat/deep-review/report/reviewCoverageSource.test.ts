import { describe, expect, it } from 'vitest';
import { formatReviewCoverageSource } from './reviewCoverageSource';

describe('formatReviewCoverageSource', () => {
  it('maps known read-only review roles to user-facing labels', () => {
    expect(formatReviewCoverageSource('ReviewSecurity')).toBe('Security coverage');
    expect(formatReviewCoverageSource('ReviewJudge')).toBe('Quality check');
    expect(formatReviewCoverageSource('Review Quality Check')).toBe('Quality check');
  });

  it('does not hide Review-prefixed remediation sources', () => {
    expect(formatReviewCoverageSource('ReviewFixer')).toBe('ReviewFixer');
  });

  it('projects the structured worker identity as a subordinate product label', () => {
    expect(formatReviewCoverageSource('ReviewWorker')).toBe('Additional check');
  });

  it('does not mislabel primary-review prose or leak capability identities', () => {
    expect(formatReviewCoverageSource('Primary review')).toBeNull();
    expect(formatReviewCoverageSource('主审核')).toBeNull();
    expect(formatReviewCoverageSource('code-review-testing')).toBeNull();
    expect(formatReviewCoverageSource('My custom review agent')).toBeNull();
  });
});
