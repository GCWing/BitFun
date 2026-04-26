import { describe, expect, it } from 'vitest';
import {
  buildReviewRemediationItems,
  buildSelectedReviewRemediationPrompt,
  getDefaultSelectedRemediationIds,
} from './codeReviewRemediation';

describe('buildReviewRemediationItems', () => {
  it('builds items from remediation_plan', () => {
    const result = buildReviewRemediationItems({
      summary: { recommended_action: 'request_changes' },
      remediation_plan: ['Fix issue 1', 'Fix issue 2'],
    });

    expect(result).toHaveLength(2);
    expect(result[0].id).toBe('remediation-0');
    expect(result[0].plan).toBe('Fix issue 1');
    expect(result[1].id).toBe('remediation-1');
    expect(result[1].plan).toBe('Fix issue 2');
  });

  it('builds items from structured remediation groups', () => {
    const result = buildReviewRemediationItems({
      summary: { recommended_action: 'request_changes' },
      report_sections: {
        remediation_groups: {
          must_fix: ['Critical fix 1'],
          should_improve: ['Improvement 1'],
          needs_decision: ['Decision needed'],
          verification: ['Verify fix'],
        },
      },
    });

    expect(result.length).toBeGreaterThan(0);
    expect(result.some((item) => item.groupId === 'must_fix')).toBe(true);
    expect(result.some((item) => item.groupId === 'should_improve')).toBe(true);
  });

  it('marks must_fix items as default selected', () => {
    const result = buildReviewRemediationItems({
      summary: { recommended_action: 'request_changes' },
      report_sections: {
        remediation_groups: {
          must_fix: ['Critical fix 1'],
          should_improve: ['Improvement 1'],
        },
      },
    });

    const mustFixItems = result.filter((item) => item.groupId === 'must_fix');
    const shouldImproveItems = result.filter((item) => item.groupId === 'should_improve');

    expect(mustFixItems.every((item) => item.defaultSelected)).toBe(true);
    expect(shouldImproveItems.every((item) => !item.defaultSelected)).toBe(true);
  });
});

describe('getDefaultSelectedRemediationIds', () => {
  it('returns IDs of default selected items', () => {
    const items = buildReviewRemediationItems({
      summary: { recommended_action: 'request_changes' },
      remediation_plan: ['Fix issue 1', 'Fix issue 2'],
      issues: [
        { severity: 'high', title: 'Issue 1' },
        { severity: 'low', title: 'Issue 2' },
      ],
    });

    const selectedIds = getDefaultSelectedRemediationIds(items);
    expect(selectedIds.length).toBeGreaterThan(0);
    expect(selectedIds).toContain('remediation-0');
  });
});

describe('buildSelectedReviewRemediationPrompt', () => {
  it('returns empty string when no items selected', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      selectedIds: new Set(),
      rerunReview: false,
      reviewMode: 'deep',
    });

    expect(prompt).toBe('');
  });

  it('builds prompt with selected items for deep review', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      selectedIds: new Set(['remediation-0']),
      rerunReview: false,
      reviewMode: 'deep',
    });

    expect(prompt).toContain('Deep Review findings only');
    expect(prompt).toContain('Fix issue 1');
    expect(prompt).toContain('Selected Remediation Plan');
  });

  it('builds prompt with rerun instruction when rerunReview is true', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      selectedIds: new Set(['remediation-0']),
      rerunReview: true,
      reviewMode: 'deep',
    });

    expect(prompt).toContain('follow-up deep review');
    expect(prompt).toContain('dispatching all enabled review team reviewers');
  });

  it('builds prompt with standard review mode', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      selectedIds: new Set(['remediation-0']),
      rerunReview: false,
      reviewMode: 'standard',
    });

    expect(prompt).toContain('Code Review findings only');
  });

  it('appends continuation context when completedItems provided', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1', 'Fix issue 2', 'Fix issue 3'],
      },
      selectedIds: new Set(['remediation-1', 'remediation-2']),
      rerunReview: false,
      reviewMode: 'deep',
      completedItems: ['remediation-0'],
    });

    expect(prompt).toContain('Continuation Context');
    expect(prompt).toContain('Already completed items (DO NOT re-fix)');
    expect(prompt).toContain('Fix issue 1');
    expect(prompt).toContain('Please focus only on the remaining items');
  });

  it('does not append continuation context when completedItems is empty', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      selectedIds: new Set(['remediation-0']),
      rerunReview: false,
      reviewMode: 'deep',
      completedItems: [],
    });

    expect(prompt).not.toContain('Continuation Context');
  });

  it('does not append continuation context when completedItems not provided', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1'],
      },
      selectedIds: new Set(['remediation-0']),
      rerunReview: false,
      reviewMode: 'deep',
    });

    expect(prompt).not.toContain('Continuation Context');
  });

  it('only includes completed items that exist in review data', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: {
        summary: { recommended_action: 'request_changes' },
        remediation_plan: ['Fix issue 1', 'Fix issue 2'],
      },
      selectedIds: new Set(['remediation-1']),
      rerunReview: false,
      reviewMode: 'deep',
      completedItems: ['remediation-0', 'non-existent-id'],
    });

    expect(prompt).toContain('Fix issue 1');
    expect(prompt).not.toContain('non-existent-id');
  });
});
