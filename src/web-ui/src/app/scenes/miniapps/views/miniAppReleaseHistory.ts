/** A release entry as far as the version history list is concerned. */
export interface ReleaseHistoryEntry {
  releaseNumber: number;
}

export interface ReleaseHistoryView<T extends ReleaseHistoryEntry> {
  /** Releases to render, newest first. */
  visible: T[];
  /** Older releases folded away while collapsed. */
  hiddenCount: number;
}

/**
 * Only the newest release matters to someone deciding whether to install, so
 * the history collapses to that single row until it is expanded. The server
 * already returns releases newest-first, but the order is re-established here
 * so the visible row is the newest one regardless of how the list arrived.
 */
export function buildReleaseHistory<T extends ReleaseHistoryEntry>(
  releases: T[],
  expanded: boolean,
): ReleaseHistoryView<T> {
  const ordered = [...releases].sort((a, b) => b.releaseNumber - a.releaseNumber);
  if (expanded || ordered.length <= 1) {
    return { visible: ordered, hiddenCount: 0 };
  }
  return { visible: ordered.slice(0, 1), hiddenCount: ordered.length - 1 };
}
