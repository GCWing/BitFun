interface SessionNavRowActiveInput {
  rowSessionId: string;
  activeTabId?: string | null;
  activeSessionId?: string | null;
  activeChildSessionId?: string | null;
  activeChildParentSessionId?: string | null;
  activeChildHasVisibleRow: boolean;
}

const SESSION_TAB_ID = 'session';

export function isSessionNavRowActive({
  rowSessionId,
  activeTabId,
  activeSessionId,
  activeChildSessionId,
  activeChildParentSessionId,
  activeChildHasVisibleRow,
}: SessionNavRowActiveInput): boolean {
  if (activeTabId !== SESSION_TAB_ID || !activeSessionId) {
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
