import { describe, expect, it } from 'vitest';
import {
  FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX,
  FLOWCHAT_TURN_RAIL_VERTICAL_PADDING_PX,
  getFlowChatTurnRailScrollTopForOrdinal,
  getFlowChatTurnRailTotalHeight,
  getFlowChatTurnRailWindowRange,
} from './flowChatTurnRailWindow';

describe('flowChatTurnRailWindow', () => {
  it('expresses the full ordinal range with fixed-height rows', () => {
    expect(getFlowChatTurnRailTotalHeight(100)).toBe(
      100 * FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX
      + FLOWCHAT_TURN_RAIL_VERTICAL_PADDING_PX * 2,
    );
  });

  it('bounds a long-session render window to visible rows plus overscan', () => {
    expect(getFlowChatTurnRailWindowRange({
      scrollTop: 50 * FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX,
      clientHeight: 4 * FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX,
      totalOrdinalCount: 100,
    })).toEqual({
      startOrdinal: 43,
      endOrdinalExclusive: 60,
    });
  });

  it('keeps ordinals visible without moving an already visible viewport', () => {
    expect(getFlowChatTurnRailScrollTopForOrdinal({
      ordinal: 1,
      currentScrollTop: 0,
      clientHeight: 40,
      totalOrdinalCount: 4,
    })).toBe(0);
    expect(getFlowChatTurnRailScrollTopForOrdinal({
      ordinal: 3,
      currentScrollTop: 0,
      clientHeight: 40,
      totalOrdinalCount: 4,
    })).toBe(11);
    expect(getFlowChatTurnRailScrollTopForOrdinal({
      ordinal: 0,
      currentScrollTop: 50,
      clientHeight: 40,
      totalOrdinalCount: 4,
    })).toBe(FLOWCHAT_TURN_RAIL_VERTICAL_PADDING_PX);
  });

  it('clamps ordinals and scroll positions to the track bounds', () => {
    expect(getFlowChatTurnRailScrollTopForOrdinal({
      ordinal: 999,
      currentScrollTop: 999,
      clientHeight: 40,
      totalOrdinalCount: 4,
    })).toBe(14);
    expect(getFlowChatTurnRailWindowRange({
      scrollTop: 999,
      clientHeight: 40,
      totalOrdinalCount: 4,
    })).toEqual({
      startOrdinal: 0,
      endOrdinalExclusive: 4,
    });
  });
});
