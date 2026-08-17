import { create } from 'zustand';
import type { FlowChatState, Session } from '../types/flow-chat';
import {
  reconcileSubagentIdentityAssignments,
  type SubagentIdentityAssignment,
  type SubagentIdentityAssignments,
  type SubagentIdentitySubject,
} from './allocator';

interface SubagentIdentityState {
  assignments: SubagentIdentityAssignments;
  reconcileRoot: (rootSessionId: string, subjects: readonly SubagentIdentitySubject[]) => void;
  clear: () => void;
}

function assignmentsEqual(
  left: SubagentIdentityAssignments,
  right: SubagentIdentityAssignments,
): boolean {
  const leftKeys = Object.keys(left);
  const rightKeys = Object.keys(right);
  if (leftKeys.length !== rightKeys.length) return false;
  return leftKeys.every(sessionId => {
    const previous = left[sessionId];
    const next = right[sessionId];
    return !!next
      && previous.rootSessionId === next.rootSessionId
      && previous.avatarId === next.avatarId
      && previous.nameId === next.nameId;
  });
}

export const useSubagentIdentityStore = create<SubagentIdentityState>((set) => ({
  assignments: {},
  reconcileRoot: (rootSessionId, subjects) => {
    if (!rootSessionId.trim() || subjects.length === 0) return;
    set(state => {
      const assignments = reconcileSubagentIdentityAssignments(
        rootSessionId,
        subjects,
        state.assignments,
      );
      return assignmentsEqual(state.assignments, assignments) ? state : { assignments };
    });
  },
  clear: () => set({ assignments: {} }),
}));

export function resolveSubagentIdentityRootSessionId(
  sessions: ReadonlyMap<string, Session>,
  anchorSessionId: string,
): string {
  let currentSessionId = anchorSessionId;
  const visited = new Set<string>();

  while (!visited.has(currentSessionId)) {
    visited.add(currentSessionId);
    const session = sessions.get(currentSessionId);
    const parentSessionId = session?.parentSessionId?.trim();
    if (!parentSessionId) return currentSessionId;
    if (!sessions.has(parentSessionId)) return parentSessionId;
    currentSessionId = parentSessionId;
  }
  return currentSessionId;
}

function isSubagentSessionActive(session: Session): boolean {
  if (session.needsUserAttention) return true;
  const latestTurn = session.dialogTurns[session.dialogTurns.length - 1];
  switch (latestTurn?.status) {
    case 'completed':
    case 'cancelled':
    case 'error':
      return false;
    default:
      return session.persistedStatus !== 'completed' && session.status !== 'error';
  }
}

export function collectSubagentIdentitySubjects(
  state: FlowChatState,
  rootSessionId: string,
): SubagentIdentitySubject[] {
  const subjects: SubagentIdentitySubject[] = [];
  for (const session of state.sessions.values()) {
    if (session.sessionKind !== 'subagent') continue;
    if (resolveSubagentIdentityRootSessionId(state.sessions, session.sessionId) !== rootSessionId) {
      continue;
    }
    subjects.push({
      sessionId: session.sessionId,
      createdAt: session.createdAt,
      active: isSubagentSessionActive(session),
    });
  }
  return subjects;
}

export function reconcileSubagentIdentitiesFromFlowState(
  state: FlowChatState,
  anchorSessionId: string,
): void {
  const rootSessionId = resolveSubagentIdentityRootSessionId(state.sessions, anchorSessionId);
  const subjects = collectSubagentIdentitySubjects(state, rootSessionId);
  useSubagentIdentityStore.getState().reconcileRoot(rootSessionId, subjects);
}

export function getSubagentIdentity(sessionId?: string | null): SubagentIdentityAssignment | undefined {
  if (!sessionId) return undefined;
  return useSubagentIdentityStore.getState().assignments[sessionId];
}
