import { afterEach, describe, expect, it, vi } from 'vitest';
import { interruptedTurnRecoveryGate } from './interruptedTurnRecoveryGate';

const first = {
  sessionId: 'session-1',
  turnId: 'turn-1',
  executionGeneration: 0,
};

afterEach(() => {
  interruptedTurnRecoveryGate.resetForTests();
});

describe('interruptedTurnRecoveryGate', () => {
  it('keeps one session-scoped operation until an authoritative event clears it', () => {
    const listener = vi.fn();
    const unsubscribe = interruptedTurnRecoveryGate.subscribe(listener);

    expect(interruptedTurnRecoveryGate.tryBegin(first)).toBe(true);
    expect(interruptedTurnRecoveryGate.tryBegin(first)).toBe(false);
    expect(interruptedTurnRecoveryGate.isSessionInFlight(first.sessionId)).toBe(true);
    expect(interruptedTurnRecoveryGate.isSessionInFlight('session-2')).toBe(false);

    interruptedTurnRecoveryGate.clearRecovered({
      ...first,
      executionGeneration: 1,
    });
    expect(interruptedTurnRecoveryGate.isSessionInFlight(first.sessionId)).toBe(false);
    expect(listener).toHaveBeenCalledTimes(2);
    unsubscribe();
  });

  it('does not clear for stale recovery or interruption events', () => {
    expect(interruptedTurnRecoveryGate.tryBegin(first)).toBe(true);

    interruptedTurnRecoveryGate.clearRecovered(first);
    interruptedTurnRecoveryGate.clearInterrupted(first);
    expect(interruptedTurnRecoveryGate.isSessionInFlight(first.sessionId)).toBe(true);

    interruptedTurnRecoveryGate.clearInterrupted({
      ...first,
      executionGeneration: 1,
    });
    expect(interruptedTurnRecoveryGate.isSessionInFlight(first.sessionId)).toBe(false);
  });

  it('clears an RPC failure only for the exact request identity', () => {
    expect(interruptedTurnRecoveryGate.tryBegin(first)).toBe(true);

    interruptedTurnRecoveryGate.clearExact({
      ...first,
      turnId: 'another-turn',
    });
    expect(interruptedTurnRecoveryGate.isSessionInFlight(first.sessionId)).toBe(true);

    interruptedTurnRecoveryGate.clearExact(first);
    expect(interruptedTurnRecoveryGate.isSessionInFlight(first.sessionId)).toBe(false);
  });
});
