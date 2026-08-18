import { create } from 'zustand';

export type SessionMutationKind = 'rollback' | 'edit-rerun';
export type SessionMutationPhase = 'active' | 'reconciliation_required';

export interface SessionMutationLease {
  sessionId: string;
  leaseId: string;
  kind: SessionMutationKind;
  targetTurnId: string;
}

export interface SessionMutation extends SessionMutationLease {
  startedAt: number;
  phase: SessionMutationPhase;
  reconciliationReason?: string;
}

interface SessionMutationState {
  mutations: ReadonlyMap<string, SessionMutation>;
  retiredTurnIds: ReadonlyMap<string, ReadonlySet<string>>;
  tryBegin: (
    sessionId: string,
    mutation: Pick<SessionMutation, 'kind' | 'targetTurnId'>,
  ) => SessionMutationLease | null;
  end: (lease: SessionMutationLease) => void;
  requireReconciliation: (lease: SessionMutationLease, reason: string) => void;
  completeReconciliation: (sessionId: string) => void;
  markRetired: (sessionId: string, turnIds: Iterable<string>) => void;
}

let nextLeaseId = 0;

export const useSessionMutationStore = create<SessionMutationState>((set, get) => ({
  mutations: new Map(),
  retiredTurnIds: new Map(),
  tryBegin: (sessionId, mutation) => {
    if (get().mutations.has(sessionId)) {
      return null;
    }
    const lease: SessionMutationLease = {
      sessionId,
      leaseId: `session-mutation-${Date.now()}-${nextLeaseId++}`,
      ...mutation,
    };
    const mutations = new Map(get().mutations);
    mutations.set(sessionId, { ...lease, startedAt: Date.now(), phase: 'active' });
    set({ mutations });
    return lease;
  },
  end: lease => {
    const current = get().mutations.get(lease.sessionId);
    if (
      !current
      || current.leaseId !== lease.leaseId
      || current.phase === 'reconciliation_required'
    ) {
      return;
    }
    const mutations = new Map(get().mutations);
    mutations.delete(lease.sessionId);
    set({ mutations });
  },
  requireReconciliation: (lease, reason) => {
    const current = get().mutations.get(lease.sessionId);
    if (!current || current.leaseId !== lease.leaseId) {
      return;
    }
    const mutations = new Map(get().mutations);
    mutations.set(lease.sessionId, {
      ...current,
      phase: 'reconciliation_required',
      reconciliationReason: reason,
    });
    set({ mutations });
  },
  completeReconciliation: sessionId => {
    const current = get().mutations.get(sessionId);
    if (!current || current.phase !== 'reconciliation_required') {
      return;
    }
    const mutations = new Map(get().mutations);
    mutations.delete(sessionId);
    set({ mutations });
  },
  markRetired: (sessionId, turnIds) => {
    const nextSessionIds = new Set(get().retiredTurnIds.get(sessionId));
    for (const turnId of turnIds) {
      if (turnId) {
        nextSessionIds.add(turnId);
      }
    }
    if (nextSessionIds.size === 0) {
      return;
    }
    const retiredTurnIds = new Map(get().retiredTurnIds);
    retiredTurnIds.set(sessionId, nextSessionIds);
    set({ retiredTurnIds });
  },
}));

export function tryBeginSessionMutation(
  sessionId: string,
  kind: SessionMutationKind,
  targetTurnId: string,
): SessionMutationLease | null {
  return useSessionMutationStore.getState().tryBegin(sessionId, { kind, targetTurnId });
}

export function assertSessionMutationLease(lease: SessionMutationLease): void {
  const current = useSessionMutationStore.getState().mutations.get(lease.sessionId);
  if (
    !current
    || current.leaseId !== lease.leaseId
    || current.kind !== lease.kind
    || current.targetTurnId !== lease.targetTurnId
    || current.phase !== 'active'
  ) {
    throw new Error('Session history mutation lease is no longer active');
  }
}

export function endSessionMutation(lease: SessionMutationLease): void {
  useSessionMutationStore.getState().end(lease);
}

export function requireSessionMutationReconciliation(
  lease: SessionMutationLease,
  reason: string,
): void {
  useSessionMutationStore.getState().requireReconciliation(lease, reason);
}

export function completeSessionMutationReconciliation(sessionId: string): void {
  useSessionMutationStore.getState().completeReconciliation(sessionId);
}

export function assertSessionSubmissionAllowed(
  sessionId: string,
  leaseId?: string,
): void {
  const current = useSessionMutationStore.getState().mutations.get(sessionId);
  if (!current) {
    return;
  }
  if (current.phase === 'reconciliation_required') {
    throw new Error('Session history must be reloaded before submitting another message');
  }
  if (current.kind !== 'edit-rerun' || !leaseId || current.leaseId !== leaseId) {
    throw new Error('Wait for the current Session history mutation to finish');
  }
}

export function markSessionTurnsRetired(sessionId: string, turnIds: Iterable<string>): void {
  useSessionMutationStore.getState().markRetired(sessionId, turnIds);
}

export function isSessionTurnRetired(sessionId: string, turnId: string): boolean {
  return useSessionMutationStore.getState().retiredTurnIds.get(sessionId)?.has(turnId) === true;
}
