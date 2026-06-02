export const LIVE_SESSION_DEFAULT_ITEM_HEIGHT_PX = 200;
export const HISTORICAL_SESSION_DEFAULT_ITEM_HEIGHT_PX = 72;
export const HISTORICAL_SESSION_MODEL_ROUND_DEFAULT_ITEM_HEIGHT_PX = 960;

export function getVirtualMessageDefaultItemHeight(params: {
  isHistorical: boolean;
  hasCompactHistoricalProjection: boolean;
  hasInitialHistoryModelRoundProjection: boolean;
}): number {
  if (params.isHistorical || params.hasCompactHistoricalProjection) {
    return HISTORICAL_SESSION_DEFAULT_ITEM_HEIGHT_PX;
  }

  if (params.hasInitialHistoryModelRoundProjection) {
    return HISTORICAL_SESSION_MODEL_ROUND_DEFAULT_ITEM_HEIGHT_PX;
  }

  return LIVE_SESSION_DEFAULT_ITEM_HEIGHT_PX;
}
