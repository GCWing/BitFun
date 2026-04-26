import { describe, expect, it } from 'vitest';
import {
  buildCodeReviewReportSections,
  formatCodeReviewReportMarkdown,
  getDefaultExpandedCodeReviewSectionIds,
} from './codeReviewReport';

describe('codeReviewReport', () => {
  it('uses structured report sections when present', () => {
    const report = {
      summary: {
        overall_assessment: 'One blocking security issue remains.',
        risk_level: 'high' as const,
        recommended_action: 'request_changes' as const,
        confidence_note: 'Security reviewer timed out, confidence reduced.',
      },
      issues: [
        {
          severity: 'high' as const,
          certainty: 'confirmed' as const,
          category: 'security',
          file: 'src/auth.ts',
          line: 42,
          title: 'Token is logged',
          description: 'The access token is written to logs.',
          suggestion: 'Remove the token from log payloads.',
          source_reviewer: 'Security Reviewer',
          validation_note: 'Quality gate confirmed the token is sensitive.',
        },
      ],
      positive_points: ['Adapter boundary is clear.'],
      review_mode: 'deep' as const,
      review_scope: 'current workspace diff',
      reviewers: [
        {
          name: 'Security Reviewer',
          specialty: 'security',
          status: 'timed_out',
          summary: 'Partial security pass completed.',
          issue_count: 1,
        },
        {
          name: 'Review Quality Inspector',
          specialty: 'quality gate',
          status: 'completed',
          summary: 'Confirmed one finding.',
          issue_count: 1,
        },
      ],
      remediation_plan: ['Remove token logging.', 'Run auth regression tests.'],
      report_sections: {
        executive_summary: ['Fix token logging before merging.'],
        remediation_groups: {
          must_fix: ['Remove token logging.'],
          verification: ['Run auth regression tests.'],
        },
        strength_groups: {
          architecture: ['Adapter boundary is clear.'],
        },
        coverage_notes: ['Security review completed with reduced confidence.'],
      },
    };

    const sections = buildCodeReviewReportSections(report);

    expect(sections.executiveSummary).toEqual(['Fix token logging before merging.']);
    expect(sections.remediationGroups).toEqual([
      { id: 'must_fix', items: ['Remove token logging.'] },
      { id: 'verification', items: ['Run auth regression tests.'] },
    ]);
    expect(sections.strengthGroups).toEqual([
      { id: 'architecture', items: ['Adapter boundary is clear.'] },
    ]);
    expect(sections.coverageNotes).toEqual(['Security review completed with reduced confidence.']);
    expect(sections.issueStats).toMatchObject({ total: 1, high: 1 });
    expect(sections.reviewerStats).toMatchObject({ total: 2, completed: 1, degraded: 1 });
  });

  it('falls back to legacy remediation and positive point fields', () => {
    const report = {
      summary: {
        overall_assessment: 'Looks safe with one suggestion.',
        risk_level: 'low' as const,
        recommended_action: 'approve_with_suggestions' as const,
      },
      issues: [],
      positive_points: ['Tests cover the changed service.'],
      remediation_plan: ['Add a narrow regression assertion.'],
    };

    const sections = buildCodeReviewReportSections(report);

    expect(sections.executiveSummary).toEqual(['Looks safe with one suggestion.']);
    expect(sections.remediationGroups).toEqual([
      { id: 'should_improve', items: ['Add a narrow regression assertion.'] },
    ]);
    expect(sections.strengthGroups).toEqual([
      { id: 'other', items: ['Tests cover the changed service.'] },
    ]);
  });

  it('keeps team and issue details collapsed by default while leaving remediation visible', () => {
    const report = {
      summary: {
        overall_assessment: 'Needs changes.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      issues: [{ severity: 'medium' as const, title: 'Bug', description: 'Bug' }],
      positive_points: ['Simple fix path.'],
      remediation_plan: ['Fix the bug.'],
      reviewers: [{ name: 'Reviewer', specialty: 'logic', status: 'completed', summary: 'Done.' }],
    };

    expect(getDefaultExpandedCodeReviewSectionIds(report)).toEqual(['summary', 'remediation']);
  });

  it('formats a review report as markdown for document export', () => {
    const markdown = formatCodeReviewReportMarkdown({
      summary: {
        overall_assessment: 'One fix required.',
        risk_level: 'medium' as const,
        recommended_action: 'request_changes' as const,
      },
      review_mode: 'deep' as const,
      review_scope: 'src/auth.ts',
      issues: [
        {
          severity: 'medium' as const,
          certainty: 'confirmed' as const,
          category: 'logic',
          file: 'src/auth.ts',
          line: 12,
          title: 'Missing guard',
          description: 'The null guard is missing.',
          suggestion: 'Add the guard.',
        },
      ],
      positive_points: ['Small surface area.'],
      remediation_plan: ['Add the guard.'],
      reviewers: [{ name: 'Business Logic Reviewer', specialty: 'logic', status: 'completed', summary: 'Found one issue.' }],
    });

    expect(markdown).toContain('# Deep Review Report');
    expect(markdown).toContain('## Executive Summary');
    expect(markdown).toContain('- One fix required.');
    expect(markdown).toContain('## Issues');
    expect(markdown).toContain('src/auth.ts:12');
    expect(markdown).toContain('## Remediation Plan');
    expect(markdown).toContain('## Code Review Team');
  });
});
