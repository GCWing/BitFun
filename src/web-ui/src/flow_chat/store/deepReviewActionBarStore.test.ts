import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { useReviewActionBarStore } from './deepReviewActionBarStore';

vi.mock('../services/ReviewActionBarPersistenceService', () => ({
  persistReviewActionState: vi.fn().mockResolvedValue(undefined),
  clearPersistedReviewState: vi.fn().mockResolvedValue(undefined),
  loadPersistedReviewState: vi.fn().mockResolvedValue(null),
}));

/** Zustand replaces state on set(); always read fresh state after actions. */
const bar = () => useReviewActionBarStore.getState();

describe('deepReviewActionBarStore', () => {
  beforeEach(() => {
    bar().reset();
  });

  afterEach(() => {
    bar().reset();
    vi.clearAllMocks();
  });

  describe('showActionBar', () => {
    it('initializes with default selected remediation IDs', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1', 'Fix issue 2'],
        },
      });

      const s = bar();
      expect(s.childSessionId).toBe('child-1');
      expect(s.phase).toBe('review_completed');
      expect(s.selectedRemediationIds.size).toBeGreaterThan(0);
      expect(s.completedRemediationIds.size).toBe(0);
      expect(s.minimized).toBe(false);
    });

    it('preserves completedRemediationIds when re-showing for same session', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1', 'Fix issue 2'],
        },
        completedRemediationIds: new Set(['remediation-0']),
      });

      const s = bar();
      expect(s.completedRemediationIds.has('remediation-0')).toBe(true);
      // Completed items should not be in selected by default
      expect(s.selectedRemediationIds.has('remediation-0')).toBe(false);
    });

    it('filters out completed IDs that no longer exist in new review data', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 2'],
        },
        // Single plan row is remediation-0; remediation-1 cannot exist in this data
        completedRemediationIds: new Set(['remediation-0', 'remediation-1']),
      });

      const s = bar();
      expect(s.completedRemediationIds.has('remediation-0')).toBe(true);
      expect(s.completedRemediationIds.has('remediation-1')).toBe(false);
    });
  });

  describe('minimize and restore', () => {
    it('minimizes the action bar', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1'],
        },
      });

      bar().minimize();
      const s = bar();
      expect(s.minimized).toBe(true);
      expect(s.phase).toBe('review_completed');
    });

    it('restores the action bar from minimized state', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1'],
        },
      });

      bar().minimize();
      bar().restore();
      expect(bar().minimized).toBe(false);
    });
  });

  describe('fix lifecycle', () => {
    it('snapshots selected IDs when starting fix', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1', 'Fix issue 2'],
        },
      });

      bar().setSelectedRemediationIds(new Set(['remediation-0']));
      bar().setActiveAction('fix');

      expect(bar().fixingRemediationIds.has('remediation-0')).toBe(true);
    });

    it('moves fixing IDs to completed when fix completes', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1', 'Fix issue 2'],
        },
      });

      bar().setSelectedRemediationIds(new Set(['remediation-0']));
      bar().setActiveAction('fix');
      bar().updatePhase('fix_running');
      bar().updatePhase('fix_completed');

      const s = bar();
      expect(s.completedRemediationIds.has('remediation-0')).toBe(true);
      expect(s.fixingRemediationIds.size).toBe(0);
      expect(s.phase).toBe('fix_completed');
    });

    it('does not mark items as completed on fix_failed', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1'],
        },
      });

      bar().setSelectedRemediationIds(new Set(['remediation-0']));
      bar().setActiveAction('fix');
      bar().updatePhase('fix_running');
      bar().updatePhase('fix_failed', 'Something went wrong');

      const s = bar();
      expect(s.completedRemediationIds.has('remediation-0')).toBe(false);
      expect(s.phase).toBe('fix_failed');
      expect(s.errorMessage).toBe('Something went wrong');
    });
  });

  describe('skipRemainingFixes', () => {
    it('returns to review_completed and clears remaining fix IDs', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1'],
        },
        phase: 'fix_interrupted',
      });

      bar().skipRemainingFixes();

      const s = bar();
      expect(s.phase).toBe('review_completed');
      expect(s.remainingFixIds).toEqual([]);
      expect(s.activeAction).toBeNull();
    });
  });

  describe('toggleRemediation with completed items', () => {
    it('does not allow toggling completed items', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1', 'Fix issue 2'],
        },
        completedRemediationIds: new Set(['remediation-0']),
      });

      const afterShow = bar();
      // Completed item should not be selected by default
      expect(afterShow.selectedRemediationIds.has('remediation-0')).toBe(false);

      bar().setSelectedRemediationIds(new Set());
      bar().toggleRemediation('remediation-1');
      expect(bar().selectedRemediationIds.has('remediation-1')).toBe(true);
    });
  });

  describe('reset', () => {
    it('clears all state back to initial', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1'],
        },
      });

      bar().minimize();
      bar().reset();

      const s = bar();
      expect(s.phase).toBe('idle');
      expect(s.childSessionId).toBeNull();
      expect(s.minimized).toBe(false);
      expect(s.completedRemediationIds.size).toBe(0);
    });
  });
});
