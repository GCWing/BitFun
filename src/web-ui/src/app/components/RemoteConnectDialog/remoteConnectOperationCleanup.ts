/**
 * A stop sent before an asynchronous start settles can be overtaken by that
 * start, leaving a hidden connection alive. Always settle start first, then
 * issue the compensating stop.
 */
export async function stopAfterPendingStart(
  pendingStart: Promise<unknown> | null,
  stop: () => Promise<void>,
): Promise<void> {
  if (pendingStart) {
    try {
      await pendingStart;
    } catch {
      // A failed start still proceeds through the idempotent cleanup path.
    }
  }
  await stop();
}

/** Apply an async operation's UI update only while that operation still owns the surface. */
export function updateIfOperationCurrent(
  isCurrent: () => boolean,
  update: () => void,
): boolean {
  if (!isCurrent()) return false;
  update();
  return true;
}
