import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionExecutionEvent, SessionExecutionState } from '../state-machine/types';
import { settleStoppedReviewSessionState } from './reviewSessionStop';

const mocks = vi.hoisted(() => ({
  cancelSessionTask: vi.fn(),
  transition: vi.fn(),
  getCurrentState: vi.fn(),
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    cancelSessionTask: mocks.cancelSessionTask,
  },
}));

vi.mock('../state-machine', () => ({
  stateMachineManager: {
    getCurrentState: mocks.getCurrentState,
    transition: mocks.transition,
  },
}));

describe('settleStoppedReviewSessionState', () => {
  beforeEach(() => {
    mocks.cancelSessionTask.mockClear();
    mocks.transition.mockClear();
    mocks.getCurrentState.mockReset();
  });

  it('marks the review session cancelled and settles a streaming state machine immediately', async () => {
    mocks.getCurrentState.mockReturnValue(SessionExecutionState.PROCESSING);

    await settleStoppedReviewSessionState('review-child');

    expect(mocks.cancelSessionTask).toHaveBeenCalledWith('review-child');
    expect(mocks.transition).toHaveBeenCalledWith(
      'review-child',
      SessionExecutionEvent.FINISHING_SETTLED,
    );
  });

  it('does not transition an already idle review session', async () => {
    mocks.getCurrentState.mockReturnValue(SessionExecutionState.IDLE);

    await settleStoppedReviewSessionState('review-child');

    expect(mocks.cancelSessionTask).toHaveBeenCalledWith('review-child');
    expect(mocks.transition).not.toHaveBeenCalled();
  });
});
