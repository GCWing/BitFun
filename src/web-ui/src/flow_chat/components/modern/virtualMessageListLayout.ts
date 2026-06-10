import type { AnyFlowItem } from '../../types/flow-chat';
import type { VirtualItem } from '../../store/modernFlowChatStore';

export const LIVE_SESSION_DEFAULT_ITEM_HEIGHT_PX = 200;
export const HISTORICAL_SESSION_DEFAULT_ITEM_HEIGHT_PX = 72;
export const HISTORICAL_SESSION_MODEL_ROUND_DEFAULT_ITEM_HEIGHT_PX = 960;
export const INITIAL_HISTORY_RENDER_MIN_TURN_COUNT = 2;
const INITIAL_HISTORY_RENDER_USER_ONLY_LATEST_MIN_TURN_COUNT = 3;
export const INITIAL_HISTORY_RENDER_MIN_ESTIMATED_HEIGHT_PX = 1400;
const USER_MESSAGE_BASE_HEIGHT_PX = 96;
const USER_MESSAGE_LINE_HEIGHT_PX = 22;
const MODEL_ROUND_BASE_HEIGHT_PX = 80;
const MODEL_ROUND_TEXT_BASE_HEIGHT_PX = 72;
const MODEL_ROUND_TEXT_LINE_HEIGHT_PX = 30;
const TOOL_CARD_ESTIMATE_HEIGHT_PX = 88;
const EXPLORE_GROUP_BASE_HEIGHT_PX = 96;
const ESTIMATED_TEXT_CHARS_PER_LINE = 60;

export function getVirtualMessageDefaultItemHeight(params: {
  isHistorical: boolean;
  hasCompactHistoricalProjection: boolean;
  hasInitialHistoryModelRoundProjection: boolean;
}): number {
  if (params.hasCompactHistoricalProjection) {
    return HISTORICAL_SESSION_DEFAULT_ITEM_HEIGHT_PX;
  }

  if (params.hasInitialHistoryModelRoundProjection) {
    return HISTORICAL_SESSION_MODEL_ROUND_DEFAULT_ITEM_HEIGHT_PX;
  }

  if (params.isHistorical) {
    return HISTORICAL_SESSION_DEFAULT_ITEM_HEIGHT_PX;
  }

  return LIVE_SESSION_DEFAULT_ITEM_HEIGHT_PX;
}

export function estimateTextHeightFromLength(textLength: number, basePx: number, lineHeightPx: number): number {
  const lineCount = Math.max(1, Math.ceil(textLength / ESTIMATED_TEXT_CHARS_PER_LINE));
  return basePx + lineCount * lineHeightPx;
}

function estimateTextHeight(content: string, basePx: number, lineHeightPx: number): number {
  return estimateTextHeightFromLength(content.length, basePx, lineHeightPx);
}

function getFlowItemTextLength(item: AnyFlowItem): number {
  if (item.type === 'text' || item.type === 'thinking' || item.type === 'user-steering') {
    return item.content.length;
  }
  return 0;
}

function estimateFlowItemHeight(item: AnyFlowItem): number {
  const textLength = getFlowItemTextLength(item);
  if (textLength > 0) {
    return Math.min(
      3200,
      estimateTextHeightFromLength(
        textLength,
        MODEL_ROUND_TEXT_BASE_HEIGHT_PX,
        MODEL_ROUND_TEXT_LINE_HEIGHT_PX,
      ),
    );
  }

  if (item.type === 'tool') {
    return TOOL_CARD_ESTIMATE_HEIGHT_PX;
  }

  if (item.type === 'image-analysis') {
    return 320;
  }

  return HISTORICAL_SESSION_DEFAULT_ITEM_HEIGHT_PX;
}

function estimateModelRoundHeight(item: Extract<VirtualItem, { type: 'model-round' }>): number {
  const flowItems = item.data.items ?? [];
  if (flowItems.length === 0) {
    return LIVE_SESSION_DEFAULT_ITEM_HEIGHT_PX;
  }

  const contentHeight = flowItems.reduce(
    (total, flowItem) => total + estimateFlowItemHeight(flowItem),
    0,
  );
  return Math.min(3600, Math.max(LIVE_SESSION_DEFAULT_ITEM_HEIGHT_PX, MODEL_ROUND_BASE_HEIGHT_PX + contentHeight));
}

function estimateUserMessageHeight(content: string | undefined): number {
  return Math.min(
    320,
    estimateTextHeight(content ?? '', USER_MESSAGE_BASE_HEIGHT_PX, USER_MESSAGE_LINE_HEIGHT_PX),
  );
}

function estimateExploreGroupHeight(item: Extract<VirtualItem, { type: 'explore-group' }>): number {
  const visibleRowCount = Math.min(10, item.data.allItems.length);
  return Math.min(420, EXPLORE_GROUP_BASE_HEIGHT_PX + visibleRowCount * 24);
}

export function estimateVirtualMessageItemHeight(item: VirtualItem): number {
  switch (item.type) {
    case 'user-message':
    case 'user-steering-message':
      return estimateUserMessageHeight(item.data.content);
    case 'model-round':
      return estimateModelRoundHeight(item);
    case 'explore-group':
      return estimateExploreGroupHeight(item);
    case 'image-analyzing':
      return LIVE_SESSION_DEFAULT_ITEM_HEIGHT_PX;
  }
}

export interface InitialHistoryRenderWindow {
  items: VirtualItem[];
  startIndex: number;
  omittedEstimatedHeightPx: number;
  renderedEstimatedHeightPx: number;
  totalEstimatedHeightPx: number;
  isWindowed: boolean;
}

export function mapInitialHistoryExpansionScrollTop(params: {
  previousScrollTop: number;
  previousScrollHeight: number;
  nextScrollHeight: number;
  omittedEstimatedHeightPx: number;
  wasAtBottom: boolean;
  clientHeight: number;
}): number {
  const nextMaxScrollTop = Math.max(0, params.nextScrollHeight - params.clientHeight);
  if (params.wasAtBottom) {
    return nextMaxScrollTop;
  }

  const heightDelta = params.nextScrollHeight - params.previousScrollHeight;
  const omittedEstimatedHeightPx = Math.max(0, params.omittedEstimatedHeightPx);
  if (
    omittedEstimatedHeightPx > 0 &&
    params.previousScrollTop <= omittedEstimatedHeightPx
  ) {
    const actualOmittedHeightPx = Math.max(0, omittedEstimatedHeightPx + heightDelta);
    const omittedRatio = Math.max(
      0,
      Math.min(1, params.previousScrollTop / omittedEstimatedHeightPx),
    );
    return Math.min(nextMaxScrollTop, actualOmittedHeightPx * omittedRatio);
  }

  return Math.min(nextMaxScrollTop, Math.max(0, params.previousScrollTop + heightDelta));
}

function uniqueTurnCount(items: VirtualItem[]): number {
  const turnIds = new Set<string>();
  items.forEach(item => {
    if (item.turnId) {
      turnIds.add(item.turnId);
    }
  });
  return turnIds.size;
}

function getLatestTurnId(items: VirtualItem[]): string | null {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const turnId = items[index]?.turnId;
    if (turnId) {
      return turnId;
    }
  }
  return null;
}

function latestTurnHasModelRound(items: VirtualItem[]): boolean {
  const latestTurnId = getLatestTurnId(items);
  if (!latestTurnId) {
    return true;
  }

  return items.some(item =>
    item.turnId === latestTurnId &&
    item.type === 'model-round'
  );
}

function getInitialHistoryRenderMinTurnCount(items: VirtualItem[]): number {
  return latestTurnHasModelRound(items)
    ? INITIAL_HISTORY_RENDER_MIN_TURN_COUNT
    : INITIAL_HISTORY_RENDER_USER_ONLY_LATEST_MIN_TURN_COUNT;
}

export function selectInitialHistoryRenderWindow(
  items: VirtualItem[],
  options: {
    minTurnCount?: number;
    minEstimatedHeightPx?: number;
  } = {},
): InitialHistoryRenderWindow {
  const minTurnCount = Math.max(1, Math.floor(options.minTurnCount ?? getInitialHistoryRenderMinTurnCount(items)));
  const minEstimatedHeightPx = Math.max(0, options.minEstimatedHeightPx ?? INITIAL_HISTORY_RENDER_MIN_ESTIMATED_HEIGHT_PX);
  const totalEstimatedHeightPx = items.reduce(
    (total, item) => total + estimateVirtualMessageItemHeight(item),
    0,
  );

  if (items.length === 0 || uniqueTurnCount(items) <= minTurnCount) {
    return {
      items,
      startIndex: 0,
      omittedEstimatedHeightPx: 0,
      renderedEstimatedHeightPx: totalEstimatedHeightPx,
      totalEstimatedHeightPx,
      isWindowed: false,
    };
  }

  let startIndex = items.length;
  let renderedEstimatedHeightPx = 0;
  const includedTurnIds = new Set<string>();

  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index];
    startIndex = index;
    renderedEstimatedHeightPx += estimateVirtualMessageItemHeight(item);
    if (item.turnId) {
      includedTurnIds.add(item.turnId);
    }

    const previousItem = items[index - 1];
    const stillInsideSameTurn =
      Boolean(item.turnId) &&
      previousItem?.turnId === item.turnId;
    if (
      !stillInsideSameTurn &&
      includedTurnIds.size >= minTurnCount &&
      renderedEstimatedHeightPx >= minEstimatedHeightPx
    ) {
      break;
    }
  }

  const omittedEstimatedHeightPx = items
    .slice(0, startIndex)
    .reduce((total, item) => total + estimateVirtualMessageItemHeight(item), 0);

  return {
    items: items.slice(startIndex),
    startIndex,
    omittedEstimatedHeightPx,
    renderedEstimatedHeightPx,
    totalEstimatedHeightPx,
    isWindowed: startIndex > 0,
  };
}
