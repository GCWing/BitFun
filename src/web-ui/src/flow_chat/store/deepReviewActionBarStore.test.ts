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

  it('does not expose a hard dismiss state or action', () => {
    const s = bar() as unknown as Record<string, unknown>;

    expect('dismissed' in s).toBe(false);
    expect('dismiss' in s).toBe(false);
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
      bar().setActiveAction('fix', { baselineTurnId: 'review-turn-1' });

      expect(bar().fixingRemediationIds.has('remediation-0')).toBe(true);
      expect(bar().fixingBaselineTurnId).toBe('review-turn-1');
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
      expect(s.fixingBaselineTurnId).toBeNull();
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
      expect(s.fixingBaselineTurnId).toBeNull();
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

  describe('capacity queue controls', () => {
    it('can bind a visible queue state before the review report is available', () => {
      bar().showCapacityQueueBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        capacityQueueState: {
          status: 'queued_for_capacity',
          queuedReviewerCount: 2,
          activeReviewerCount: 1,
        },
      });

      expect(bar().childSessionId).toBe('child-1');
      expect(bar().reviewMode).toBe('deep');
      expect(bar().phase).toBe('review_waiting_capacity');
      expect(bar().reviewData).toBeNull();
      expect(bar().capacityQueueState?.queuedReviewerCount).toBe(2);
    });

    it('pauses and resumes capacity queue state without clearing completed remediation', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1', 'Fix issue 2'],
        },
        completedRemediationIds: new Set(['remediation-0']),
      });

      const queueActions = bar() as unknown as {
        setCapacityQueueState: (state: {
          status: string;
          queuedReviewerCount: number;
          optionalReviewerCount: number;
        }) => void;
        pauseCapacityQueue: () => void;
        continueCapacityQueue: () => void;
      };

      queueActions.setCapacityQueueState({
        status: 'queued_for_capacity',
        queuedReviewerCount: 2,
        optionalReviewerCount: 1,
      });
      queueActions.pauseCapacityQueue();

      expect((bar() as unknown as { capacityQueueState: { status: string } }).capacityQueueState.status).toBe('paused_by_user');
      expect(bar().completedRemediationIds.has('remediation-0')).toBe(true);

      queueActions.continueCapacityQueue();

      expect((bar() as unknown as { capacityQueueState: { status: string } }).capacityQueueState.status).toBe('queued_for_capacity');
      expect(bar().completedRemediationIds.has('remediation-0')).toBe(true);
    });

    it('can skip optional queued reviewers without cancelling required queued work', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1'],
        },
      });

      const queueActions = bar() as unknown as {
        setCapacityQueueState: (state: {
          status: string;
          queuedReviewerCount: number;
          optionalReviewerCount: number;
        }) => void;
        skipOptionalQueuedReviewers: () => void;
      };

      queueActions.setCapacityQueueState({
        status: 'queued_for_capacity',
        queuedReviewerCount: 3,
        optionalReviewerCount: 2,
      });
      queueActions.skipOptionalQueuedReviewers();

      const state = (bar() as unknown as {
        capacityQueueState: { status: string; queuedReviewerCount: number; optionalReviewerCount: number };
      }).capacityQueueState;
      expect(state.status).toBe('queued_for_capacity');
      expect(state.queuedReviewerCount).toBe(1);
      expect(state.optionalReviewerCount).toBe(0);
    });

    it('merges reviewer queue events and closes the queue when the last reviewer leaves', () => {
      bar().showCapacityQueueBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        capacityQueueState: {
          toolId: 'task-security',
          subagentType: 'ReviewSecurity',
          status: 'queued_for_capacity',
          reason: 'local_concurrency_cap',
          queuedReviewerCount: 1,
          waitingReviewers: [{
            toolId: 'task-security',
            subagentType: 'ReviewSecurity',
            displayName: 'Security reviewer',
            status: 'queued_for_capacity',
            reason: 'local_concurrency_cap',
          }],
        },
      });

      bar().applyCapacityQueueState({
        toolId: 'task-frontend',
        subagentType: 'ReviewFrontend',
        status: 'queued_for_capacity',
        reason: 'launch_batch_blocked',
        queuedReviewerCount: 1,
        waitingReviewers: [{
          toolId: 'task-frontend',
          subagentType: 'ReviewFrontend',
          displayName: 'Frontend reviewer',
          status: 'queued_for_capacity',
          reason: 'launch_batch_blocked',
        }],
      });

      expect(bar().capacityQueueState?.queuedReviewerCount).toBe(2);
      expect(bar().capacityQueueState?.waitingReviewers?.map((reviewer) => reviewer.displayName)).toEqual([
        'Security reviewer',
        'Frontend reviewer',
      ]);

      bar().applyCapacityQueueState({
        toolId: 'task-security',
        subagentType: 'ReviewSecurity',
        status: 'running',
        queuedReviewerCount: 0,
        waitingReviewers: [],
      });

      expect(bar().capacityQueueState?.queuedReviewerCount).toBe(1);
      expect(bar().capacityQueueState?.waitingReviewers?.map((reviewer) => reviewer.displayName)).toEqual([
        'Frontend reviewer',
      ]);
      expect(bar().phase).toBe('review_waiting_capacity');

      bar().applyCapacityQueueState({
        toolId: 'task-frontend',
        subagentType: 'ReviewFrontend',
        status: 'capacity_skipped',
        queuedReviewerCount: 0,
        waitingReviewers: [],
      });

      expect(bar().capacityQueueState).toBeNull();
      expect(bar().phase).toBe('idle');
    });

    it('keeps the capacity queue visible when a terminal reviewer event reports more queued reviewers', () => {
      bar().showCapacityQueueBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        capacityQueueState: {
          toolId: 'task-security',
          subagentType: 'ReviewSecurity',
          status: 'queued_for_capacity',
          reason: 'local_concurrency_cap',
          queuedReviewerCount: 1,
          waitingReviewers: [{
            toolId: 'task-security',
            subagentType: 'ReviewSecurity',
            displayName: 'Security reviewer',
            status: 'queued_for_capacity',
            reason: 'local_concurrency_cap',
          }],
        },
      });

      bar().applyCapacityQueueState({
        toolId: 'task-security',
        subagentType: 'ReviewSecurity',
        status: 'running',
        reason: 'launch_batch_blocked',
        queuedReviewerCount: 1,
        activeReviewerCount: 1,
        waitingReviewers: [],
      });

      expect(bar().capacityQueueState).toMatchObject({
        status: 'queued_for_capacity',
        queuedReviewerCount: 1,
        activeReviewerCount: 1,
      });
      expect(bar().capacityQueueState?.waitingReviewers).toEqual([]);
      expect(bar().phase).toBe('review_waiting_capacity');
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

      bar().toggleRemediation('remediation-0');
      expect(bar().selectedRemediationIds.has('remediation-0')).toBe(false);
    });

    it('does not select completed items through select-all', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          summary: { recommended_action: 'request_changes' },
          remediation_plan: ['Fix issue 1', 'Fix issue 2'],
        },
        completedRemediationIds: new Set(['remediation-0']),
      });

      bar().setSelectedRemediationIds(new Set());
      bar().toggleAllRemediation();

      expect([...bar().selectedRemediationIds].sort()).toEqual(['remediation-1']);
    });

    it('does not select completed items through a remediation group root toggle', () => {
      bar().showActionBar({
        childSessionId: 'child-1',
        parentSessionId: 'parent-1',
        reviewData: {
          review_mode: 'deep',
          report_sections: {
            remediation_groups: {
              should_improve: [
                'Improve error copy',
                'Improve retry state',
              ],
            },
          },
        },
        completedRemediationIds: new Set(['remediation-should_improve-0']),
      });

      bar().setSelectedRemediationIds(new Set());
      bar().toggleGroupRemediation('should_improve');

      expect([...bar().selectedRemediationIds].sort()).toEqual(['remediation-should_improve-1']);

      bar().toggleGroupRemediation('should_improve');
      expect(bar().selectedRemediationIds.size).toBe(0);
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
