export const FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX = 12;
export const FLOWCHAT_TURN_RAIL_VERTICAL_PADDING_PX = 3;
export const FLOWCHAT_TURN_RAIL_OVERSCAN_ROWS = 6;

export interface FlowChatTurnRailWindowRange {
  startOrdinal: number;
  endOrdinalExclusive: number;
}

export function getFlowChatTurnRailTotalHeight(totalOrdinalCount: number): number {
  const normalizedCount = Math.max(0, Math.floor(totalOrdinalCount));
  return normalizedCount * FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX
    + FLOWCHAT_TURN_RAIL_VERTICAL_PADDING_PX * 2;
}

export function getFlowChatTurnRailWindowRange(options: {
  scrollTop: number;
  clientHeight: number;
  totalOrdinalCount: number;
  overscanRows?: number;
}): FlowChatTurnRailWindowRange {
  const totalOrdinalCount = Math.max(0, Math.floor(options.totalOrdinalCount));
  if (totalOrdinalCount === 0) {
    return { startOrdinal: 0, endOrdinalExclusive: 0 };
  }

  const clientHeight = Math.max(FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX, options.clientHeight);
  const maxScrollTop = Math.max(
    0,
    getFlowChatTurnRailTotalHeight(totalOrdinalCount) - clientHeight,
  );
  const scrollTop = Math.max(0, Math.min(maxScrollTop, options.scrollTop));
  const overscanRows = Math.max(
    0,
    Math.floor(options.overscanRows ?? FLOWCHAT_TURN_RAIL_OVERSCAN_ROWS),
  );
  const contentScrollTop = Math.max(0, scrollTop - FLOWCHAT_TURN_RAIL_VERTICAL_PADDING_PX);
  const visibleStartOrdinal = Math.floor(contentScrollTop / FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX);
  const visibleEndOrdinalExclusive = Math.ceil(
    (contentScrollTop + clientHeight) / FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX,
  );

  return {
    startOrdinal: Math.max(0, visibleStartOrdinal - overscanRows),
    endOrdinalExclusive: Math.min(
      totalOrdinalCount,
      visibleEndOrdinalExclusive + overscanRows,
    ),
  };
}

export function getFlowChatTurnRailScrollTopForOrdinal(options: {
  ordinal: number;
  currentScrollTop: number;
  clientHeight: number;
  totalOrdinalCount: number;
}): number {
  const totalOrdinalCount = Math.max(0, Math.floor(options.totalOrdinalCount));
  if (totalOrdinalCount === 0 || options.clientHeight <= 0) {
    return Math.max(0, options.currentScrollTop);
  }

  const ordinal = Math.max(0, Math.min(totalOrdinalCount - 1, Math.floor(options.ordinal)));
  const itemTop = FLOWCHAT_TURN_RAIL_VERTICAL_PADDING_PX
    + ordinal * FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX;
  const itemBottom = itemTop + FLOWCHAT_TURN_RAIL_ROW_HEIGHT_PX;
  const currentScrollTop = Math.max(0, options.currentScrollTop);
  const visibleBottom = currentScrollTop + options.clientHeight;
  let nextScrollTop = currentScrollTop;

  if (itemTop < currentScrollTop) {
    nextScrollTop = itemTop;
  } else if (itemBottom > visibleBottom) {
    nextScrollTop = itemBottom - options.clientHeight;
  }

  const maxScrollTop = Math.max(
    0,
    getFlowChatTurnRailTotalHeight(totalOrdinalCount) - options.clientHeight,
  );
  return Math.max(0, Math.min(maxScrollTop, nextScrollTop));
}
