/**
 * Run async work over items with a bounded number of concurrent tasks.
 * Avoids spawning unbounded Promise.all IPC/network work that can stall the main thread.
 */
export async function runWithConcurrencyLimit<T, R>(
  items: readonly T[],
  concurrency: number,
  worker: (item: T, index: number) => Promise<R>,
): Promise<R[]> {
  if (items.length === 0) {
    return [];
  }
  const limit = Math.max(1, Math.floor(concurrency));
  const results: R[] = new Array(items.length);
  let nextIndex = 0;

  const runWorker = async (): Promise<void> => {
    for (;;) {
      const i = nextIndex++;
      if (i >= items.length) {
        return;
      }
      results[i] = await worker(items[i], i);
    }
  };

  const workers = Array.from(
    { length: Math.min(limit, items.length) },
    () => runWorker(),
  );
  await Promise.all(workers);
  return results;
}
