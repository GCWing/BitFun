/**
 * Session state machine hook.
 */

import { useState, useEffect, useMemo } from 'react';
import { stateMachineManager } from '../state-machine';
import {
  SessionStateMachine,
  SessionDerivedState,
  SessionExecutionEvent,
} from '../state-machine/types';
import { deriveSessionState } from '../state-machine/derivedState';

/**
 * Access the session state machine.
 */
export function useSessionStateMachine(sessionId: string | null) {
  const [snapshot, setSnapshot] = useState<SessionStateMachine | null>(null);

  useEffect(() => {
    if (!sessionId) {
      setSnapshot(null);
      return;
    }

    const machine = stateMachineManager.getOrCreate(sessionId);
    
    setSnapshot(machine.getSnapshot());

    const unsubscribe = machine.subscribe((newSnapshot) => {
      setSnapshot(newSnapshot);
    });

    return () => {
      unsubscribe();
    };
  }, [sessionId]);

  return snapshot;
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

