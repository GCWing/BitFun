import { describe, expect, it } from 'vitest';
import { resolveVisibleFlowChatTurnIds } from './flowChatVisibleTurns';

describe('resolveVisibleFlowChatTurnIds', () => {
  it('excludes mounted overscan items that are outside the physical viewport', () => {
    expect(resolveVisibleFlowChatTurnIds([
      { turnId: 'turn-1', top: -180, bottom: -20 },
      { turnId: 'turn-2', top: 40, bottom: 260 },
    ], 0, 800)).toEqual(['turn-2']);
  });

  it('keeps a Turn visible through its content after its user-message header is gone', () => {
    expect(resolveVisibleFlowChatTurnIds([
      { turnId: 'turn-2', top: -120, bottom: 420 },
      { turnId: 'turn-2', top: 420, bottom: 900 },
    ], 0, 800)).toEqual(['turn-2']);
  });

  it('deduplicates visible Turn ids while preserving transcript order', () => {
    expect(resolveVisibleFlowChatTurnIds([
      { turnId: 'turn-1', top: -30, bottom: 100 },
      { turnId: 'turn-1', top: 100, bottom: 300 },
      { turnId: 'turn-2', top: 300, bottom: 700 },
    ], 0, 600)).toEqual(['turn-1', 'turn-2']);
  });
});
