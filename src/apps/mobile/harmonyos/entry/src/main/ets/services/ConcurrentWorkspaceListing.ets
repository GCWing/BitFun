import { RemoteSession } from '../model/RemoteModels';
import { remoteSessionIdentity } from './RemoteSessionIdentity';

/**
 * How many workspaces are listed at once during cross-workspace discovery.
 *
 * Each listing is one round trip to the desktop over the relay, and a phone
 * that has been used for a while has twenty-odd recent workspaces: done one
 * after another that is five seconds of the session list rearranging itself
 * behind the user. Kept small because these run alongside the polling that
 * drives an open conversation, and a burst wide enough to crowd it out would
 * trade a slow list for a slow chat.
 */
export const CONCURRENT_WORKSPACE_LISTINGS: number = 4;

export async function mapInConcurrentBatches<T, R>(
  items: T[],
  mapper: (item: T) => Promise<R>,
  concurrency: number = CONCURRENT_WORKSPACE_LISTINGS
): Promise<R[]> {
  const all: R[] = [];
  const width = Math.max(1, concurrency);
  for (let start = 0; start < items.length; start += width) {
    const batch: T[] = items.slice(start, start + width);
    const results: R[] = await Promise.all(batch.map(mapper));
    results.forEach((item: R) => {
      all.push(item);
    });
  }
  return all;
}

export function mergeUniqueSessions(groups: RemoteSession[][]): RemoteSession[] {
  const seen: Set<string> = new Set<string>();
  const all: RemoteSession[] = [];
  groups.forEach((sessions: RemoteSession[]) => {
    sessions.forEach((item: RemoteSession) => {
      const key = remoteSessionIdentity(item);
      if (seen.has(key)) {
        return;
      }
      seen.add(key);
      all.push(item);
    });
  });
  return all;
}
