import type { AnyFlowItem } from '../../types/flow-chat';
import type { VirtualItem } from '../../store/modernFlowChatStore';

export const LIVE_SESSION_DEFAULT_ITEM_HEIGHT_PX = 200;
export const HISTORICAL_SESSION_DEFAULT_ITEM_HEIGHT_PX = 72;
export const HISTORICAL_SESSION_MODEL_ROUND_DEFAULT_ITEM_HEIGHT_PX = 960;
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
