export interface FlowChatTurnViewportEntry {
  turnId: string | null;
  top: number;
  bottom: number;
}

/**
 * Resolve visible Turns from physical item geometry, not the virtualizer's
 * rendered range. The rendered range includes overscan and a Turn can remain
 * visible after its user-message header has been virtualized away.
 */
export function resolveVisibleFlowChatTurnIds(
  entries: readonly FlowChatTurnViewportEntry[],
  viewportTop: number,
  viewportBottom: number,
): string[] {
  const visibleTurnIds: string[] = [];
  const seenTurnIds = new Set<string>();

  for (const entry of entries) {
    const { turnId } = entry;
    if (
      !turnId
      || seenTurnIds.has(turnId)
      || entry.bottom <= viewportTop
      || entry.top >= viewportBottom
    ) {
      continue;
    }
    seenTurnIds.add(turnId);
    visibleTurnIds.push(turnId);
  }

  return visibleTurnIds;
}
