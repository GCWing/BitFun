/**
 * Session driver resolution.
 *
 * A session's driver decides which transport family owns its lifecycle,
 * submission, permissions, and persistence. The id is always derived — never
 * stored on the session — because a projection can exist before its dispatch
 * config is bound (startup metadata races `DispatchJobObserver.ensureProjection`)
 * and a stored id would go stale in that window.
 */

import { isNonLocalDispatchTarget } from '@/features/dispatch/types';
import type { DispatchTarget, DispatchTargetRequest } from '@/features/dispatch/types';
import { dispatchJobStore } from '@/features/dispatch/dispatchJobStore';

export type SessionDriverId = 'local' | 'dispatch';

/**
 * Structural view of the session fields the resolver reads. Kept minimal so
 * store modules and tests can pass lightweight objects.
 */
export interface DriverResolvableSession {
  config?: {
    dispatchTarget?: DispatchTarget;
    dispatchJobId?: string;
  };
}

/**
 * Three-signal core. All three are required because each covers a window the
 * others miss:
 * - `dispatchTarget` is unbound while `ensureProjection` races startup
 *   workspace metadata;
 * - `dispatchJobId` survives on config after the observer store was pruned;
 * - the job-store membership catches sessions created by the observer before
 *   any config binding happened.
 *
 * A weaker predicate here persisted observer projections to disk or created
 * controller-side backend sessions for target-owned jobs.
 */
export function resolveSessionDriverIdWith(
  sessionId: string,
  session: DriverResolvableSession | undefined,
  isDispatchObservedSession: (sessionId: string) => boolean,
): SessionDriverId {
  if (isNonLocalDispatchTarget(session?.config?.dispatchTarget)) {
    return 'dispatch';
  }
  if (session?.config?.dispatchJobId?.trim()) {
    return 'dispatch';
  }
  if (isDispatchObservedSession(sessionId)) {
    return 'dispatch';
  }
  return 'local';
}

export function dispatchJobStoreObservesSession(sessionId: string): boolean {
  return Object.values(dispatchJobStore.getState().jobs)
    .some(job => job.sessionId === sessionId);
}

/** The single source of truth for "which driver owns this session". */
export function resolveSessionDriverId(
  sessionId: string,
  session: DriverResolvableSession | undefined,
): SessionDriverId {
  return resolveSessionDriverIdWith(sessionId, session, dispatchJobStoreObservesSession);
}

/** Creation has no session yet; the requested target decides the driver. */
export function resolveSessionDriverIdForCreation(config: {
  dispatchTargetRequest?: DispatchTargetRequest;
}): SessionDriverId {
  return isNonLocalDispatchTarget(config.dispatchTargetRequest) ? 'dispatch' : 'local';
}
