export const SESSIONS_LEVEL_0 = 5;
export const SESSIONS_LEVEL_1 = 10;

/**
 * Rows kept loaded past the visible slice. Deleting a session drops one row out
 * of the list, and without a buffer the row that takes its place only arrives
 * after a metadata round trip — the list would visibly close up and then push a
 * row back in. The buffer lets the replacement render in the same commit.
 */
export const SESSIONS_BUFFER_PREFETCH = 3;

export type SessionExpandLevel = 0 | 1 | 2;
export type SessionExpandToggleAction = 'show-more' | 'show-all' | 'show-less';

export interface SessionExpandToggleState {
  action: SessionExpandToggleAction;
  collapsedRemainingCount: number;
  expandedRemainingCount: number;
  shouldRender: boolean;
}

export function getSessionDisplayLimit(params: {
  loadedTopLevelCount: number;
  expandLevel: SessionExpandLevel;
  level2DisplayCount: number;
  showAllWithoutLimit: boolean;
}): number {
  const {
    loadedTopLevelCount,
    expandLevel,
    level2DisplayCount,
    showAllWithoutLimit,
  } = params;

  if (showAllWithoutLimit) {
    return loadedTopLevelCount;
  }
  if (expandLevel === 2) {
    return Math.min(loadedTopLevelCount, level2DisplayCount);
  }
  if (loadedTopLevelCount <= SESSIONS_LEVEL_0) {
    return loadedTopLevelCount;
  }
  if (expandLevel === 1) {
    return Math.min(loadedTopLevelCount, SESSIONS_LEVEL_1);
  }
  return SESSIONS_LEVEL_0;
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

/**
 * How many extra top-level rows the collapsed/expanded view should keep loaded
 * beyond what it renders. Returns 0 when the buffer is already full, when every
 * session is loaded, or when the list is fully expanded (level 2 pages itself).
 */
export function getSessionBufferPrefetchLimit(params: {
  expandLevel: SessionExpandLevel;
  loadedTopLevelCount: number;
  totalTopLevelCount: number;
  hasMore: boolean;
}): number {
  const { expandLevel, loadedTopLevelCount, totalTopLevelCount, hasMore } = params;
  if (!hasMore || expandLevel === 2) {
    return 0;
  }

  const visibleCount = expandLevel === 0 ? SESSIONS_LEVEL_0 : SESSIONS_LEVEL_1;
  const targetLoadedCount = Math.min(visibleCount + SESSIONS_BUFFER_PREFETCH, totalTopLevelCount);
  return Math.max(targetLoadedCount - loadedTopLevelCount, 0);
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
