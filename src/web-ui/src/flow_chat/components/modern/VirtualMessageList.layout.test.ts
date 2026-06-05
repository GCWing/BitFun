import { describe, expect, it } from 'vitest';
import {
  estimateTextHeightFromLength,
  estimateVirtualMessageItemHeight,
  getVirtualMessageDefaultItemHeight,
} from './virtualMessageListLayout';
import type { VirtualItem } from '../../store/modernFlowChatStore';

describe('getVirtualMessageDefaultItemHeight', () => {
  it('keeps compact historical projections on the small row estimate', () => {
    expect(getVirtualMessageDefaultItemHeight({
      isHistorical: false,
      hasCompactHistoricalProjection: true,
      hasInitialHistoryModelRoundProjection: true,
    })).toBe(72);
  });

  it('uses a taller initial estimate for partial historical model rounds', () => {
    expect(getVirtualMessageDefaultItemHeight({
      isHistorical: false,
      hasCompactHistoricalProjection: false,
      hasInitialHistoryModelRoundProjection: true,
    })).toBeGreaterThan(200);
  });

  it('prioritizes the taller estimate when a historical initial projection contains model rounds', () => {
    expect(getVirtualMessageDefaultItemHeight({
      isHistorical: true,
      hasCompactHistoricalProjection: false,
      hasInitialHistoryModelRoundProjection: true,
    })).toBeGreaterThan(200);
  });

  it('keeps live sessions on the legacy estimate', () => {
    expect(getVirtualMessageDefaultItemHeight({
      isHistorical: false,
      hasCompactHistoricalProjection: false,
      hasInitialHistoryModelRoundProjection: false,
    })).toBe(200);
  });
});

describe('estimateVirtualMessageItemHeight', () => {
  it('estimates text height directly from length', () => {
    expect(estimateTextHeightFromLength(0, 72, 30)).toBe(102);
    expect(estimateTextHeightFromLength(60, 72, 30)).toBe(102);
    expect(estimateTextHeightFromLength(61, 72, 30)).toBe(132);
  });

  it('uses content-aware estimates for large historical model rounds', () => {
    const item = {
      type: 'model-round',
      turnId: 'turn-1',
      isLastRound: true,
      isTurnComplete: true,
      data: {
        id: 'round-1',
        status: 'completed',
        isStreaming: false,
        items: [{
          id: 'text-1',
          type: 'text',
          content: 'x'.repeat(3600),
          status: 'completed',
          timestamp: 1,
        }],
      },
    } as VirtualItem;

    expect(estimateVirtualMessageItemHeight(item)).toBeGreaterThan(1000);
  });

  it('keeps compact user-only rows small enough for partial history tails', () => {
    const item = {
      type: 'user-message',
      turnId: 'turn-1',
      data: {
        id: 'user-1',
        content: 'short prompt',
        timestamp: 1,
      },
    } as VirtualItem;

    expect(estimateVirtualMessageItemHeight(item)).toBeLessThanOrEqual(160);
  });
});
