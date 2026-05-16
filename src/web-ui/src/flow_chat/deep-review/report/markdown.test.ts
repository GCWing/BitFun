import { describe, expect, it } from 'vitest';
import { formatCodeReviewReportMarkdown } from './markdown';

describe('markdown', () => {
  it('formats standard reports without Deep Review manifest sections', () => {
    const markdown = formatCodeReviewReportMarkdown({
      review_mode: 'standard',
      summary: {
        overall_assessment: 'Looks good.',
        risk_level: 'low',
        recommended_action: 'approve',
      },
      issues: [],
    });

    expect(markdown).toContain('# Code Review Report');
    expect(markdown).toContain('## Executive Summary');
    expect(markdown).toContain('- Looks good.');
    expect(markdown).not.toContain('## Run manifest');
  });
});
