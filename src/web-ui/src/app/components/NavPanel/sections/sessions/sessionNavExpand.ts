export const SESSIONS_LEVEL_0 = 5;
export const SESSIONS_LEVEL_1 = 10;

export type SessionExpandLevel = 0 | 1 | 2;
export type SessionExpandToggleAction = 'show-more' | 'show-all' | 'show-less';

export interface SessionExpandToggleState {
  action: SessionExpandToggleAction;
  collapsedRemainingCount: number;
  expandedRemainingCount: number;
  shouldRender: boolean;
}

export function getEffectiveTopLevelSessionCount(
  metadataTotalTopLevelCount: number | null,
  syncedTopLevelCount: number | null,
  liveTopLevelCount: number,
  isMetadataLoading: boolean
): number {
  if (metadataTotalTopLevelCount === null) {
    return liveTopLevelCount;
  }

  if (isMetadataLoading || syncedTopLevelCount === null) {
    return Math.max(metadataTotalTopLevelCount, liveTopLevelCount);
  }

  return Math.max(
    liveTopLevelCount,
    metadataTotalTopLevelCount + (liveTopLevelCount - syncedTopLevelCount)
  );
}

export function getSessionExpandToggleState(
  totalTopLevelSessionCount: number,
  expandLevel: SessionExpandLevel
): SessionExpandToggleState {
  const collapsedRemainingCount = Math.max(totalTopLevelSessionCount - SESSIONS_LEVEL_0, 0);
  const expandedRemainingCount = Math.max(totalTopLevelSessionCount - SESSIONS_LEVEL_1, 0);

  return {
    action:
      expandLevel === 0
        ? 'show-more'
        : expandLevel === 1 && expandedRemainingCount > 0
          ? 'show-all'
          : 'show-less',
    collapsedRemainingCount,
    expandedRemainingCount,
    shouldRender: collapsedRemainingCount > 0,
  };
}
