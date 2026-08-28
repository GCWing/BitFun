const DEFAULT_MAX_TRANSCRIPT_CHARS = 1_200;

/**
 * Volcengine ASR delta events contain the latest revised partial transcript,
 * not a suffix to append. Keep the newest snapshot so interim UI matches the
 * final transcript without duplicated phrases.
 */
export function applyRealtimeAsrSnapshot(
  _previous: string,
  snapshot: string,
  maxChars = DEFAULT_MAX_TRANSCRIPT_CHARS,
): string {
  return snapshot.length <= maxChars
    ? snapshot
    : snapshot.slice(snapshot.length - maxChars);
}
