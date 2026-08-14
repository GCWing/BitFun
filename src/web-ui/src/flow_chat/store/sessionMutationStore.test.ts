import { beforeEach, describe, expect, it } from 'vitest';

import {
  assertSessionSubmissionAllowed,
  endSessionMutation,
  requireSessionMutationReconciliation,
  tryBeginSessionMutation,
  useSessionMutationStore,
} from './sessionMutationStore';

describe('sessionMutationStore', () => {
  beforeEach(() => {
    useSessionMutationStore.setState({ mutations: new Map(), retiredTurnIds: new Map() });
  });

  it('allows only the edit rerun that owns the active lease to submit', () => {
    const lease = tryBeginSessionMutation('session-1', 'edit-rerun', 'turn-7');
    expect(lease).not.toBeNull();

    expect(() => assertSessionSubmissionAllowed('session-1')).toThrow('history mutation');
    expect(() => assertSessionSubmissionAllowed('session-1', 'wrong-lease')).toThrow(
      'history mutation',
    );
    expect(() => assertSessionSubmissionAllowed('session-1', lease!.leaseId)).not.toThrow();

    endSessionMutation(lease!);
    expect(useSessionMutationStore.getState().mutations.has('session-1')).toBe(false);
  });

  it('does not release a reconciliation-required Session through normal lease cleanup', () => {
    const lease = tryBeginSessionMutation('session-1', 'rollback', 'turn-7');
    expect(lease).not.toBeNull();
    requireSessionMutationReconciliation(lease!, 'reload failed');

    endSessionMutation(lease!);

    expect(useSessionMutationStore.getState().mutations.get('session-1')).toMatchObject({
      phase: 'reconciliation_required',
      reconciliationReason: 'reload failed',
    });
    expect(() => assertSessionSubmissionAllowed('session-1')).toThrow('must be reloaded');
  });
});
