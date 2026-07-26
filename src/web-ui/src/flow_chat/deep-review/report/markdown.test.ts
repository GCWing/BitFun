import { describe, expect, it } from 'vitest';
import { formatCodeReviewReportMarkdown } from './markdown';

describe('markdown', () => {
  it('formats standard reports without Deep Review manifest sections', () => {
    const markdown = formatCodeReviewReportMarkdown({
      review_mode: 'standard',
      evidence_status: 'complete',
      summary: {
        overall_assessment: 'Looks good.',
        risk_level: 'low',
        recommended_action: 'approve',
      },
      issues: [],
    });

    expect(markdown).toContain('# Review Report');
    expect(markdown).toContain('## Executive Summary');
    expect(markdown).toContain('- Looks good.');
    expect(markdown).toContain('- Evidence Status: complete');
    expect(markdown).not.toContain('## Review Coverage And Cost');
  });

  it('maps internal reviewer source ids to coverage labels', () => {
    const markdown = formatCodeReviewReportMarkdown({
      review_mode: 'deep',
      summary: {
        risk_level: 'medium',
        recommended_action: 'request_changes',
      },
      issues: [{
        severity: 'high',
        certainty: 'likely',
        title: 'Token leak',
        description: 'A token is logged.',
        source_reviewer: 'ReviewSecurity',
      }],
    });

    expect(markdown).toContain('- Source: Security coverage');
    expect(markdown).not.toContain('ReviewSecurity');
  });

  it('does not claim a focused check for an ordinary primary-review source', () => {
    const markdown = formatCodeReviewReportMarkdown({
      review_mode: 'standard',
      summary: {
        risk_level: 'low',
        recommended_action: 'approve',
      },
      issues: [{
        severity: 'medium',
        certainty: 'likely',
        title: 'Primary finding',
        description: 'Found by the primary review.',
        source_reviewer: 'Primary review',
      }],
    });

    expect(markdown).not.toContain('- Source:');
    expect(markdown).not.toContain('Focused check');
    expect(markdown).not.toContain('Primary review');
  });
});
