import { describe, expect, it } from 'vitest';
import {
  buildReviewRemediationItems,
  buildSelectedRemediationPrompt,
  buildSelectedReviewRemediationPrompt,
  getDefaultSelectedRemediationIds,
} from './codeReviewRemediation';
import type { CodeReviewRemediationData } from './codeReviewRemediation';

function createReviewData(
  overrides: Partial<CodeReviewRemediationData> = {},
): CodeReviewRemediationData {
  return {
    summary: {},
    issues: [],
    remediation_plan: [],
    ...overrides,
  };
}

describe('buildReviewRemediationItems', () => {
  it('returns empty array for empty plan', () => {
    const items = buildReviewRemediationItems(createReviewData());
    expect(items).toEqual([]);
  });

  it('skips empty plan items', () => {
    const items = buildReviewRemediationItems(
      createReviewData({ remediation_plan: ['', '  ', 'valid plan'] }),
    );
    expect(items).toHaveLength(1);
    expect(items[0].plan).toBe('valid plan');
  });

  it('selects critical severity by default', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Fix critical bug'],
        issues: [{ severity: 'critical' }],
      }),
    );
    expect(items[0].defaultSelected).toBe(true);
  });

  it('selects high severity by default', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Fix high bug'],
        issues: [{ severity: 'high' }],
      }),
    );
    expect(items[0].defaultSelected).toBe(true);
  });

  it('selects medium severity by default', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Fix medium bug'],
        issues: [{ severity: 'medium' }],
      }),
    );
    expect(items[0].defaultSelected).toBe(true);
  });

  it('does not select low severity by default', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Fix low bug'],
        issues: [{ severity: 'low' }],
      }),
    );
    expect(items[0].defaultSelected).toBe(false);
  });

  it('does not select info severity by default', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Fix info bug'],
        issues: [{ severity: 'info' }],
      }),
    );
    expect(items[0].defaultSelected).toBe(false);
  });

  it('selects by confirmed certainty + suggestion even for low severity', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Fix with suggestion'],
        issues: [{ severity: 'low', certainty: 'confirmed', suggestion: 'do this' }],
      }),
    );
    expect(items[0].defaultSelected).toBe(true);
  });

  it('does not select by suggestion alone without confirmed certainty', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Fix with suggestion'],
        issues: [{ severity: 'low', certainty: 'likely', suggestion: 'do this' }],
      }),
    );
    expect(items[0].defaultSelected).toBe(false);
  });

  it('selects plan-only items when recommended_action is request_changes', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Plan without issue'],
        summary: { recommended_action: 'request_changes' },
      }),
    );
    expect(items[0].defaultSelected).toBe(true);
  });

  it('selects plan-only items when recommended_action is block', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Plan without issue'],
        summary: { recommended_action: 'block' },
      }),
    );
    expect(items[0].defaultSelected).toBe(true);
  });

  it('does not select plan-only items when recommended_action is approve', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Plan without issue'],
        summary: { recommended_action: 'approve' },
      }),
    );
    expect(items[0].defaultSelected).toBe(false);
  });

  it('links issues to plans by index', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Plan 0', 'Plan 1'],
        issues: [{ title: 'Issue 0' }, { title: 'Issue 1' }],
      }),
    );
    expect(items).toHaveLength(2);
    expect(items[0].issue?.title).toBe('Issue 0');
    expect(items[1].issue?.title).toBe('Issue 1');
  });

  it('builds structured remediation items from report sections', () => {
    const items = buildReviewRemediationItems(
      createReviewData({
        remediation_plan: ['Legacy fallback should not duplicate'],
        report_sections: {
          remediation_groups: {
            must_fix: ['Fix the blocking bug'],
            should_improve: ['Clean up the helper'],
            needs_decision: ['Confirm the desired product behavior'],
            verification: ['Run the focused regression test'],
          },
        },
      }),
    );

    expect(items.map((item) => item.plan)).toEqual([
      'Fix the blocking bug',
      'Clean up the helper',
      'Confirm the desired product behavior',
      'Run the focused regression test',
    ]);
    expect(items.find((item) => item.groupId === 'needs_decision')?.defaultSelected).toBe(false);
    expect(items.find((item) => item.groupId === 'needs_decision')?.requiresDecision).toBe(true);
    expect(items.find((item) => item.groupId === 'verification')?.defaultSelected).toBe(false);
  });
});

describe('getDefaultSelectedRemediationIds', () => {
  it('returns ids of default selected items', () => {
    const items = [
      { id: 'a', defaultSelected: true },
      { id: 'b', defaultSelected: false },
      { id: 'c', defaultSelected: true },
    ] as any;
    expect(getDefaultSelectedRemediationIds(items)).toEqual(['a', 'c']);
  });

  it('returns empty array when no items selected', () => {
    const items = [
      { id: 'a', defaultSelected: false },
      { id: 'b', defaultSelected: false },
    ] as any;
    expect(getDefaultSelectedRemediationIds(items)).toEqual([]);
  });
});

describe('buildSelectedRemediationPrompt', () => {
  it('returns empty string when no ids selected', () => {
    const prompt = buildSelectedRemediationPrompt({
      reviewData: createReviewData({ remediation_plan: ['Plan'] }),
      selectedIds: new Set(),
      rerunReview: false,
    });
    expect(prompt).toBe('');
  });

  it('returns empty string when selected ids do not match any items', () => {
    const prompt = buildSelectedRemediationPrompt({
      reviewData: createReviewData({ remediation_plan: ['Plan'] }),
      selectedIds: new Set(['non-existent']),
      rerunReview: false,
    });
    expect(prompt).toBe('');
  });

  it('includes selected plan items in prompt', () => {
    const prompt = buildSelectedRemediationPrompt({
      reviewData: createReviewData({
        remediation_plan: ['Plan A', 'Plan B'],
      }),
      selectedIds: new Set(['remediation-0']),
      rerunReview: false,
    });
    expect(prompt).toContain('Plan A');
    expect(prompt).not.toContain('Plan B');
    expect(prompt).toContain('Selected Remediation Plan');
  });

  it('includes rerun review instruction when rerunReview is true', () => {
    const prompt = buildSelectedRemediationPrompt({
      reviewData: createReviewData({ remediation_plan: ['Plan'] }),
      selectedIds: new Set(['remediation-0']),
      rerunReview: true,
    });
    expect(prompt).toContain('follow-up review');
  });

  it('includes summary instruction when rerunReview is false', () => {
    const prompt = buildSelectedRemediationPrompt({
      reviewData: createReviewData({ remediation_plan: ['Plan'] }),
      selectedIds: new Set(['remediation-0']),
      rerunReview: false,
    });
    expect(prompt).toContain('summarize what changed');
  });

  it('includes issue details when issue is linked', () => {
    const prompt = buildSelectedRemediationPrompt({
      reviewData: createReviewData({
        remediation_plan: ['Fix bug'],
        issues: [{
          severity: 'critical',
          certainty: 'confirmed',
          title: 'Critical Bug',
          file: 'src/main.ts',
          line: 42,
          description: 'Memory leak',
          suggestion: 'Use WeakRef',
        }],
      }),
      selectedIds: new Set(['remediation-0']),
      rerunReview: false,
    });
    expect(prompt).toContain('Critical Bug');
    expect(prompt).toContain('src/main.ts:42');
    expect(prompt).toContain('Memory leak');
    expect(prompt).toContain('Use WeakRef');
  });

  it('uses Code Review wording for standard review remediation prompts', () => {
    const prompt = buildSelectedReviewRemediationPrompt({
      reviewData: createReviewData({ remediation_plan: ['Fix standard issue'] }),
      selectedIds: new Set(['remediation-0']),
      rerunReview: true,
      reviewMode: 'standard',
    });

    expect(prompt).toContain('selected Code Review findings only');
    expect(prompt).toContain('follow-up standard code review');
    expect(prompt).not.toContain('Deep Review findings only');
    expect(prompt).not.toContain('parallel, followed by ReviewJudge');
  });
});
