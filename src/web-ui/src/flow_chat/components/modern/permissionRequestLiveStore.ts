/**
 * One live permission-request subscription for the whole app.
 *
 * The transcript needs the pending list to decorate tool cards, and the
 * composer needs it to render the approval band. Each of those mounting its own
 * subscription would ask the backend to start streaming twice and then keep two
 * copies of the same list, which drift the moment one of them answers a
 * request. So the stream is owned here and refcounted by its readers.
 */

import {
  agentAPI,
  type PermissionRequest,
  type PermissionRequestEvent,
} from '@/infrastructure/api/service-api/AgentAPI';
import { applyPermissionRequestEvent, reconcilePermissionRequestSnapshot } from './permissionRequestRouting';

const EMPTY: PermissionRequest[] = [];

let requests: PermissionRequest[] = EMPTY;
let listeners = new Set<() => void>();
let readerCount = 0;
let unlisten: (() => void) | null = null;
let generation = 0;
/**
 * Requests this client already answered. A late `asked` snapshot must not
 * resurrect them, and a re-subscribe must not either.
 */
const resolvedIds = new Set<string>();

function publish(next: PermissionRequest[]): void {
  if (next === requests) return;
  requests = next;
  listeners.forEach(listener => listener());
}

function start(): void {
  const startedGeneration = ++generation;
  unlisten = agentAPI.onPermissionRequestEvent((event: PermissionRequestEvent) => {
    if (generation !== startedGeneration) return;
    if (event.event === 'asked') {
      resolvedIds.delete(event.request.requestId);
    } else {
      resolvedIds.add(event.requestId);
    }
    publish(applyPermissionRequestEvent(requests, event));
  });

  void (async () => {
    try {
      await agentAPI.subscribePermissionRequests();
      const pending = await agentAPI.listPendingPermissionRequests();
      if (generation !== startedGeneration) return;
      publish(reconcilePermissionRequestSnapshot(requests, pending, resolvedIds));
    } catch {
      if (generation !== startedGeneration) return;
      publish(EMPTY);
    }
  })();
}

function stop(): void {
  generation += 1;
  unlisten?.();
  unlisten = null;
  publish(EMPTY);
}

export function subscribeLivePermissionRequests(listener: () => void): () => void {
  listeners.add(listener);
  readerCount += 1;
  if (readerCount === 1) {
    start();
  }
  return () => {
    listeners.delete(listener);
    readerCount -= 1;
    if (readerCount === 0) {
      stop();
    }
  };
}

export function getLivePermissionRequests(): PermissionRequest[] {
  return requests;
}

/**
 * Drops a request the local client just answered, so the button it was
 * answered from does not stay live until the backend event arrives.
 */
export function markLivePermissionRequestsResolved(requestIds: readonly string[]): void {
  if (requestIds.length === 0) return;
  const resolved = new Set(requestIds);
  requestIds.forEach(id => resolvedIds.add(id));
  publish(requests.filter(request => !resolved.has(request.requestId)));
}

/** Test seam: forgets every reader, request and answer this module remembers. */
export function resetLivePermissionRequestsForTest(): void {
  generation += 1;
  unlisten?.();
  unlisten = null;
  listeners = new Set();
  readerCount = 0;
  requests = EMPTY;
  resolvedIds.clear();
}
