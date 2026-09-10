import { isSessionSceneId } from '@/app/components/SceneBar/types';

interface SessionNavRowActiveInput {
  rowSessionId: string;
  activeTabId?: string | null;
  activeSessionId?: string | null;
  activeChildSessionId?: string | null;
  activeChildParentSessionId?: string | null;
  activeChildHasVisibleRow: boolean;
}

export function isSessionNavRowActive({
  rowSessionId,
  activeTabId,
  activeSessionId,
  activeChildSessionId,
  activeChildParentSessionId,
  activeChildHasVisibleRow,
}: SessionNavRowActiveInput): boolean {
  if (!isSessionSceneId(activeTabId) || !activeSessionId) {
    return false;
  }

  if (
    activeChildHasVisibleRow &&
    activeChildSessionId &&
    activeChildParentSessionId === activeSessionId
  ) {
    return rowSessionId === activeChildSessionId;
  }

  return rowSessionId === activeSessionId;
}
