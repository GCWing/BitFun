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
  /** Child sessions (subagents, /btw) inherit the driver of their parent. */
  parentSessionId?: string;
  config?: {
    dispatchTarget?: DispatchTarget;
    dispatchJobId?: string;
  };
}

type SessionLookup = (sessionId: string) => DriverResolvableSession | undefined;

let parentSessionLookup: SessionLookup | null = null;

/**
 * Register how the resolver walks a session's parent chain. Installed once by
 * FlowChatManager; kept as an injected function because the store module
 * itself depends on this resolver.
 */
export function registerDriverSessionLookup(lookup: SessionLookup): void {
  parentSessionLookup = lookup;
}

const MAX_PARENT_CHAIN_DEPTH = 4;

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
  lookupSession: SessionLookup | null = parentSessionLookup,
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
  // Fourth signal: a child session (subagent projection) inherits the driver
  // of its parent chain — it has no dispatch config of its own, but its
  // events come from the target and it must never be persisted or driven as
  // a local backend session.
  let parentId = session?.parentSessionId?.trim();
  for (let depth = 0; parentId && lookupSession && depth < MAX_PARENT_CHAIN_DEPTH; depth += 1) {
    if (isDispatchObservedSession(parentId)) {
      return 'dispatch';
    }
    const parent = lookupSession(parentId);
    if (!parent) {
      break;
    }
    if (
      isNonLocalDispatchTarget(parent.config?.dispatchTarget)
      || parent.config?.dispatchJobId?.trim()
    ) {
      return 'dispatch';
    }
    parentId = parent.parentSessionId?.trim();
  }
  return 'local';
}

export function dispatchJobStoreObservesSession(sessionId: string): boolean {
  // `jobs` always exists on the real store; the fallback tolerates partial
  // test doubles of dispatchJobStore so an unrelated suite cannot turn a
  // resolver lookup into a thrown error.
  return Object.values(dispatchJobStore.getState().jobs ?? {})
    .some(job => job.sessionId === sessionId);
}

/** The single source of truth for "which driver owns this session". */
export function resolveSessionDriverId(
  sessionId: string,
  session: DriverResolvableSession | undefined,
): SessionDriverId {
  return resolveSessionDriverIdWith(sessionId, session, dispatchJobStoreObservesSession);
}

/** Resolve a child projection to the durable job that owns its files. */
export function resolveDispatchJobId(
  sessionId: string,
  session: DriverResolvableSession | undefined,
  lookupSession: SessionLookup | null = parentSessionLookup,
): string | undefined {
  let currentId: string | undefined = sessionId;
  let current = session;
  for (let depth = 0; currentId && depth <= MAX_PARENT_CHAIN_DEPTH; depth += 1) {
    const configured = current?.config?.dispatchJobId?.trim();
    if (configured) return configured;
    const observed = Object.values(dispatchJobStore.getState().jobs ?? {})
      .find(job => job.sessionId === currentId);
    if (observed) return observed.jobId;
    currentId = current?.parentSessionId?.trim();
    current = currentId ? lookupSession?.(currentId) : undefined;
  }
  return undefined;
}

/** Creation has no session yet; the requested target decides the driver. */
export function resolveSessionDriverIdForCreation(config: {
  dispatchTargetRequest?: DispatchTargetRequest;
}): SessionDriverId {
  return isNonLocalDispatchTarget(config.dispatchTargetRequest) ? 'dispatch' : 'local';
}
