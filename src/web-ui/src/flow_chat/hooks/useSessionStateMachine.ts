/**
 * Session state machine hook.
 */

import { useState, useEffect, useMemo, useSyncExternalStore } from 'react';
import {
  getActiveSurfaceScope,
  onSurfaceActivated,
} from '@/infrastructure/peer-device/deviceSurface';
import { stateMachineManager } from '../state-machine';
import {
  SessionStateMachine,
  SessionDerivedState,
  SessionExecutionEvent,
} from '../state-machine/types';
import { deriveSessionState } from '../state-machine/derivedState';

const subscribeToSurfaceActivation = (listener: () => void): (() => void) =>
  onSurfaceActivated(() => listener());

const currentSurfaceEpoch = (): number => getActiveSurfaceScope().epoch;

/**
 * Access the session state machine.
 */
export function useSessionStateMachine(sessionId: string | null) {
  const surfaceEpoch = useSyncExternalStore(
    subscribeToSurfaceActivation,
    currentSurfaceEpoch,
    currentSurfaceEpoch,
  );
  const identity = `${surfaceEpoch}:${sessionId ?? ''}`;
  const [snapshotState, setSnapshotState] = useState<{
    identity: string;
    snapshot: SessionStateMachine | null;
  }>({ identity, snapshot: null });

  useEffect(() => {
    if (!sessionId) {
      setSnapshotState({ identity, snapshot: null });
      return;
    }

    const machine = stateMachineManager.getOrCreate(sessionId);

    setSnapshotState({ identity, snapshot: machine.getSnapshot() });

    const unsubscribe = machine.subscribe((newSnapshot) => {
      setSnapshotState({ identity, snapshot: newSnapshot });
    });

    return () => {
      unsubscribe();
    };
  }, [identity, sessionId]);

  // React retains hook state for one render when either the Session or Surface
  // identity changes. Never pair that previous machine snapshot with the new
  // Session id while the effect re-subscribes.
  return snapshotState.identity === identity ? snapshotState.snapshot : null;
}

/**
 * Derived session state.
 * @param processingInputDraftTrimmed - trimmed chat input while generating; keeps send UI in `split` when user has typed a follow-up (see derivedState).
 */
export function useSessionDerivedState(
  sessionId: string | null,
  processingInputDraftTrimmed?: string
): SessionDerivedState | null {
  const snapshot = useSessionStateMachine(sessionId);

  const derivedState = useMemo(() => {
    if (!snapshot) return null;
    const opts =
      processingInputDraftTrimmed !== undefined
        ? { processingInputDraftTrimmed }
        : undefined;
    return deriveSessionState(snapshot, opts);
  }, [snapshot, processingInputDraftTrimmed]);

  return derivedState;
}

/**
 * State machine actions.
 */
export function useSessionStateMachineActions(sessionId: string | null) {
  const transition = async (event: SessionExecutionEvent, payload?: any) => {
    if (!sessionId) return false;
    return stateMachineManager.transition(sessionId, event, payload);
  };

  const setQueuedInput = (input: string | null) => {
    if (!sessionId) return;
    const machine = stateMachineManager.get(sessionId);
    if (machine) {
      machine.setQueuedInput(input);
    }
  };

  const updatePlanner = (todos: any[], isActive: boolean) => {
    if (!sessionId) return;
    const machine = stateMachineManager.get(sessionId);
    if (machine) {
      machine.updatePlanner(todos, isActive);
    }
  };

  return {
    transition,
    setQueuedInput,
    updatePlanner,
  };
}
