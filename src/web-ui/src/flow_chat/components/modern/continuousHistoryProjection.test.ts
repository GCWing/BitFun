import { describe, expect, it } from 'vitest';
import type {
  DialogTurn,
  SessionHistoryPresentation,
} from '../../types/flow-chat';
import {
  buildContinuousHistoryProjection,
  canRetainContinuousHistoryProjection,
  CONTINUOUS_HISTORY_PROJECTION_MAX_TURN_COUNT,
  CONTINUOUS_HISTORY_PROJECTION_MAX_VIRTUAL_ITEM_COUNT,
} from './continuousHistoryProjection';

function createTurn(index: number, variant: string = 'cached'): DialogTurn {
  return {
    id: `turn-${index}`,
    sessionId: 'session-1',
    userMessage: {
      id: `user-${index}-${variant}`,
      content: `${variant} ${index}`,
      timestamp: index,
    },
    modelRounds: [],
    status: 'completed',
    startTime: index,
  };
}

function createPresentation(turnCount: number): SessionHistoryPresentation {
  return {
    range: {
      startOrdinal: 0,
      endOrdinalExclusive: turnCount,
      targetTurnId: 'turn-4',
      mode: 'history-window',
    },
    turns: Array.from({ length: turnCount }, (_, index) => createTurn(index)),
  };
}

describe('continuousHistoryProjection', () => {
  it('overlays the canonical tail and appends a newly submitted Turn', () => {
    const presentation = createPresentation(11);
    const canonicalTail = Array.from(
      { length: 4 },
      (_, index) => createTurn(index + 8, 'canonical'),
    );

    const result = buildContinuousHistoryProjection({
      dialogTurns: canonicalTail,
      totalTurnCount: 12,
    }, presentation);

    expect(result?.range).toMatchObject({
      startOrdinal: 0,
      endOrdinalExclusive: 12,
      mode: 'history-window',
    });
    expect(result?.turns).toHaveLength(12);
    expect(result?.turns.slice(8)).toEqual(canonicalTail);
  });

  it('rejects a cache-to-tail gap', () => {
    const presentation = createPresentation(5);
    expect(buildContinuousHistoryProjection({
      dialogTurns: [createTurn(8), createTurn(9)],
      totalTurnCount: 10,
    }, presentation)).toBeNull();
  });

  it('rejects an ordinal overlap with mismatched Turn identities', () => {
    const presentation = createPresentation(10);
    expect(buildContinuousHistoryProjection({
      dialogTurns: [createTurn(8), { ...createTurn(9), id: 'other-turn' }],
      totalTurnCount: 10,
    }, presentation)).toBeNull();
  });

  it('retains only projections inside both Turn and virtual-item budgets', () => {
    expect(canRetainContinuousHistoryProjection(
      createPresentation(CONTINUOUS_HISTORY_PROJECTION_MAX_TURN_COUNT),
      CONTINUOUS_HISTORY_PROJECTION_MAX_VIRTUAL_ITEM_COUNT,
    )).toBe(true);
    expect(canRetainContinuousHistoryProjection(
      createPresentation(CONTINUOUS_HISTORY_PROJECTION_MAX_TURN_COUNT + 1),
      1,
    )).toBe(false);
    expect(canRetainContinuousHistoryProjection(
      createPresentation(1),
      CONTINUOUS_HISTORY_PROJECTION_MAX_VIRTUAL_ITEM_COUNT + 1,
    )).toBe(false);
  });
});
