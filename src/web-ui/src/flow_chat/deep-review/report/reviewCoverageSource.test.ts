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
});
