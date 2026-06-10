import { describe, expect, it } from 'vitest';
import {
  estimateTextHeightFromLength,
  estimateVirtualMessageItemHeight,
  getVirtualMessageDefaultItemHeight,
  mapInitialHistoryExpansionScrollTop,
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

  function exploreItem(turnIndex: number): VirtualItem {
    const id = `turn-${turnIndex}`;
    return {
      type: 'explore-group',
      turnId: id,
      data: {
        allItems: [{ id: `explore-${id}`, label: `Explore ${turnIndex}` }],
        timestamp: turnIndex,
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

  it('keeps an extra previous turn when the latest turn is user-only', () => {
    const items = [
      ...Array.from({ length: 7 }, (_, index) => [
        userItem(index),
        exploreItem(index),
        modelItem(index),
      ]).flat(),
      userItem(7),
    ];

    const window = selectInitialHistoryRenderWindow(items);
    const renderedTurnIds = Array.from(new Set(window.items.map(item => item.turnId)));

    expect(renderedTurnIds).toEqual(['turn-5', 'turn-6', 'turn-7']);
    expect(window.items[0]?.turnId).toBe('turn-5');
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

describe('mapInitialHistoryExpansionScrollTop', () => {
  const base = {
    previousScrollHeight: 5000,
    nextScrollHeight: 5600,
    omittedEstimatedHeightPx: 3000,
    clientHeight: 1000,
  };

  it('keeps a direct jump to the omitted history top at the real top', () => {
    expect(mapInitialHistoryExpansionScrollTop({
      ...base,
      previousScrollTop: 0,
      wasAtBottom: false,
    })).toBe(0);
  });

  it('maps positions inside the omitted history spacer by ratio', () => {
    expect(mapInitialHistoryExpansionScrollTop({
      ...base,
      previousScrollTop: 1500,
      wasAtBottom: false,
    })).toBe(1800);
  });

  it('keeps visible tail content stable after the omitted spacer boundary', () => {
    expect(mapInitialHistoryExpansionScrollTop({
      ...base,
      previousScrollTop: 3400,
      wasAtBottom: false,
    })).toBe(4000);
  });

  it('keeps bottom-pinned sessions at the new physical bottom', () => {
    expect(mapInitialHistoryExpansionScrollTop({
      ...base,
      previousScrollTop: 4000,
      wasAtBottom: true,
    })).toBe(4600);
  });

  it('falls back to physical height delta when the omitted estimate is zero', () => {
    expect(mapInitialHistoryExpansionScrollTop({
      ...base,
      previousScrollTop: 700,
      omittedEstimatedHeightPx: 0,
      wasAtBottom: false,
    })).toBe(1300);
  });

  it('keeps omitted-history ratio stable when the expanded content is shorter than estimated', () => {
    expect(mapInitialHistoryExpansionScrollTop({
      previousScrollTop: 1500,
      previousScrollHeight: 5000,
      nextScrollHeight: 2600,
      omittedEstimatedHeightPx: 3000,
      clientHeight: 1000,
      wasAtBottom: false,
    })).toBe(300);
  });

  it('clamps stale visible-tail scroll positions to the expanded scroll range', () => {
    expect(mapInitialHistoryExpansionScrollTop({
      previousScrollTop: 7000,
      previousScrollHeight: 5000,
      nextScrollHeight: 5200,
      omittedEstimatedHeightPx: 3000,
      clientHeight: 1000,
      wasAtBottom: false,
    })).toBe(4200);
  });

  it('keeps bottom-pinned sessions at zero when content is shorter than the viewport', () => {
    expect(mapInitialHistoryExpansionScrollTop({
      previousScrollTop: 4000,
      previousScrollHeight: 5000,
      nextScrollHeight: 800,
      omittedEstimatedHeightPx: 3000,
      clientHeight: 1000,
      wasAtBottom: true,
    })).toBe(0);
  });
});
