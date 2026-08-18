import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  LOCAL_SURFACE_ID,
  activateSurface,
} from '@/infrastructure/peer-device/deviceSurface';
import { stateMachineManager } from './SessionStateMachineManager';
import {
  SessionExecutionEvent,
  SessionExecutionState,
} from './types';

describe('SessionStateMachineManager device surfaces', () => {
  beforeEach(() => {
    stateMachineManager.clear();
    activateSurface(LOCAL_SURFACE_ID);
  });

  it('keeps equal session ids in independent state machines', async () => {
    await stateMachineManager.transition('same-session', SessionExecutionEvent.START);
    await stateMachineManager.transition(
      'same-session',
      SessionExecutionEvent.BACKEND_STREAM_COMPLETED,
    );
    expect(stateMachineManager.getCurrentState('same-session')).toBe(
      SessionExecutionState.FINISHING,
    );

    activateSurface('peer-b');
    expect(stateMachineManager.getCurrentState('same-session')).toBe(SessionExecutionState.IDLE);
    await stateMachineManager.transition('same-session', SessionExecutionEvent.START);
    expect(stateMachineManager.getCurrentState('same-session')).toBe(
      SessionExecutionState.PROCESSING,
    );

    activateSurface(LOCAL_SURFACE_ID);
    expect(stateMachineManager.getCurrentState('same-session')).toBe(
      SessionExecutionState.FINISHING,
    );
  });

  it('does not publish a background surface machine update to active listeners', async () => {
    activateSurface('peer-b');
    const peerMachine = stateMachineManager.getOrCreate('same-session');
    const listener = vi.fn();
    const unsubscribe = stateMachineManager.subscribeGlobal(listener);

    await peerMachine.transition(SessionExecutionEvent.START);
    expect(listener).toHaveBeenCalledTimes(1);
    listener.mockClear();

    activateSurface(LOCAL_SURFACE_ID);
    peerMachine.reset();
    expect(listener).not.toHaveBeenCalled();
    unsubscribe();
  });

  it('discards only the requested peer surface', async () => {
    await stateMachineManager.transition('same-session', SessionExecutionEvent.START);
    activateSurface('peer-b');
    await stateMachineManager.transition('same-session', SessionExecutionEvent.START);

    stateMachineManager.clearSurface('peer-b');
    expect(stateMachineManager.getCurrentState('same-session')).toBe(SessionExecutionState.IDLE);

    activateSurface(LOCAL_SURFACE_ID);
    expect(stateMachineManager.getCurrentState('same-session')).toBe(
      SessionExecutionState.PROCESSING,
    );
  });
});
