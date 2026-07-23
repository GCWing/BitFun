import { describe, expect, it } from 'vitest';
import type { DialogTurn } from '../types/flow-chat';
import {
  isDialogTurnInFlight,
  isThreadGoalContinuationTurn,
  shouldUseLatestTurnFollowOutput,
  shouldUseStickyLatestPin,
} from './flowChatTurnScrollPolicy';

function makeTurn(partial: Partial<DialogTurn>): DialogTurn {
  return {
    id: 'turn-1',
    sessionId: 'session-1',
    userMessage: {
      id: 'user-1',
      content: 'hello',
      timestamp: 1,
    },
    modelRounds: [],
    status: 'pending',
    startTime: 1,
    ...partial,
  };
}

describe('flowChatTurnScrollPolicy', () => {
  it('treats completed turns as not in flight', () => {
    const turn = makeTurn({ status: 'completed' });
    expect(isDialogTurnInFlight(turn)).toBe(false);
  });

  it('skips sticky pin for thread goal continuation turns', () => {
    const turn = makeTurn({
      status: 'processing',
      userMessage: {
        id: 'user-1',
        content: 'check goal',
        timestamp: 1,
        metadata: { threadGoalContinuation: true },
      },
    });
    expect(isThreadGoalContinuationTurn(turn)).toBe(true);
    expect(shouldUseLatestTurnFollowOutput(turn)).toBe(false);
    expect(shouldUseStickyLatestPin(turn)).toBe(false);
  });

  it('allows sticky pin for in-flight user turns', () => {
    const turn = makeTurn({
      status: 'processing',
      modelRounds: [{
        id: 'round-1',
        turnId: 'turn-1',
        items: [],
        isStreaming: true,
        createdAt: 1,
      }],
    } as Partial<DialogTurn>);
    expect(shouldUseLatestTurnFollowOutput(turn)).toBe(true);
    expect(shouldUseStickyLatestPin(turn)).toBe(true);
  });

  it('lets follow-output own a newly submitted user-only turn before sticky pin starts', () => {
    const turn = makeTurn({ status: 'processing', modelRounds: [] });
    expect(isDialogTurnInFlight(turn)).toBe(true);
    expect(shouldUseLatestTurnFollowOutput(turn)).toBe(true);
    expect(shouldUseStickyLatestPin(turn)).toBe(false);
  });
});
