import { describe, expect, it } from 'vitest';
import {
  estimateTextHeightFromLength,
  estimateVirtualMessageItemHeight,
  getVirtualMessageDefaultItemHeight,
  selectInitialHistoryRenderWindow,
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

describe('selectInitialHistoryRenderWindow', () => {
  function userItem(turnIndex: number): VirtualItem {
    const id = `turn-${turnIndex}`;
    return {
      type: 'user-message',
      turnId: id,
      data: {
        id: `user-${id}`,
        content: `prompt ${turnIndex}`,
        timestamp: turnIndex,
      },
    } as VirtualItem;
  }

  function modelItem(turnIndex: number, textLength = 2000): VirtualItem {
    const id = `turn-${turnIndex}`;
    return {
      type: 'model-round',
      turnId: id,
      isLastRound: turnIndex === 7,
      isTurnComplete: true,
      data: {
        id: `round-${id}`,
        status: 'completed',
        isStreaming: false,
        items: [{
          id: `text-${id}`,
          type: 'text',
          content: 'x'.repeat(textLength),
          status: 'completed',
          timestamp: turnIndex,
        }],
      },
    } as VirtualItem;
  }

  it('keeps only the latest render window on large partial history tails', () => {
    const items = Array.from({ length: 8 }, (_, index) => [
      userItem(index),
      modelItem(index),
    ]).flat();

    const window = selectInitialHistoryRenderWindow(items);

    expect(window.startIndex).toBeGreaterThan(0);
    expect(window.items.length).toBeLessThan(items.length);
    expect(window.items[0]?.turnId).toBe('turn-6');
    expect(window.items.at(-1)?.turnId).toBe('turn-7');
    expect(window.omittedEstimatedHeightPx).toBeGreaterThan(0);
  });

  it('keeps all items when the partial history tail is already small', () => {
    const items = [userItem(0), modelItem(0), userItem(1), modelItem(1)];

    const window = selectInitialHistoryRenderWindow(items);

    expect(window.startIndex).toBe(0);
    expect(window.items).toHaveLength(items.length);
    expect(window.omittedEstimatedHeightPx).toBe(0);
  });
});
