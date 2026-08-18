import { describe, expect, it } from 'vitest';
import { getNextState } from './transitions';
import { SessionExecutionEvent, SessionExecutionState } from './types';

describe('recoverable interruption transitions', () => {
  it('keeps the session finishing until backend settlement is observed', () => {
    expect(getNextState(
      SessionExecutionState.PROCESSING,
      SessionExecutionEvent.USER_CANCEL,
    )).toBe(SessionExecutionState.FINISHING);
    expect(getNextState(
      SessionExecutionState.FINISHING,
      SessionExecutionEvent.FINISHING_SETTLED,
    )).toBe(SessionExecutionState.IDLE);
  });

  it('returns to processing if the backend rejects interruption admission', () => {
    expect(getNextState(
      SessionExecutionState.FINISHING,
      SessionExecutionEvent.USER_CANCEL_FAILED,
    )).toBe(SessionExecutionState.PROCESSING);
  });
});
