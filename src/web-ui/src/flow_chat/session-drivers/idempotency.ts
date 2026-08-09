/**
 * Per-session idempotent submission ids.
 *
 * A transport call whose response was lost must be retryable without running
 * the operation twice on the other side. The id is minted once per
 * (session, scope, content) and reused verbatim by retries with the same
 * content; the durable target dedupes on it.
 */

export interface PendingSubmissionRetry {
  content: string;
  displayContent?: string;
  id: string;
}

const pendingByScope = new Map<string, Map<string, PendingSubmissionRetry>>();

function scopeMap(scope: string): Map<string, PendingSubmissionRetry> {
  let map = pendingByScope.get(scope);
  if (!map) {
    map = new Map();
    pendingByScope.set(scope, map);
  }
  return map;
}

/**
 * Return the pending retry for this content, or mint and remember a new id.
 * Different content for the same session replaces the pending entry — only
 * the latest attempt is retryable.
 */
export function claimSubmissionRetry(
  scope: string,
  sessionId: string,
  content: string,
  displayContent: string | undefined,
  mintId: () => string,
): PendingSubmissionRetry {
  const map = scopeMap(scope);
  const current = map.get(sessionId);
  const retry =
    current?.content === content && current.displayContent === displayContent
      ? current
      : { content, displayContent, id: mintId() };
  map.set(sessionId, retry);
  return retry;
}

/** Forget the retry, but only if it is still the current one for the session. */
export function releaseSubmissionRetry(
  scope: string,
  sessionId: string,
  id: string,
): void {
  const map = scopeMap(scope);
  if (map.get(sessionId)?.id === id) {
    map.delete(sessionId);
  }
}
