import type {
  PermissionRequestEvent,
  PermissionRequest,
} from '@/infrastructure/api/service-api/AgentAPI';

export function permissionRequestBelongsToSession(
  request: PermissionRequest,
  sessionId?: string,
): boolean {
  if (!sessionId) return false;
  return request.sessionId === sessionId || request.delegation?.parentSessionId === sessionId;
}

/**
 * Return the single session surface that owns the user interaction for a
 * permission request. Delegated subagent requests are surfaced by their
 * parent task; direct requests stay with the session that emitted them.
 */
export function permissionRequestOwnerSessionId(request: PermissionRequest): string {
  return request.delegation?.parentSessionId ?? request.sessionId;
}

export function permissionRequestIsOwnedBySession(
  request: PermissionRequest,
  sessionId?: string,
): boolean {
  if (!sessionId) return false;
  return permissionRequestOwnerSessionId(request) === sessionId;
}

export function selectPermissionRequestsForSession(
  requests: readonly PermissionRequest[],
  sessionId?: string,
): PermissionRequest[] {
  return sortPermissionRequests(
    requests.filter((request) => permissionRequestBelongsToSession(request, sessionId)),
  );
}

/**
 * Select requests for the one UI surface that is allowed to present and
 * answer them. This is intentionally separate from
 * `selectPermissionRequestsForSession`, which also projects delegated child
 * requests into the parent session for task-card state and history context.
 */
export function selectPermissionRequestsOwnedBySession(
  requests: readonly PermissionRequest[],
  sessionId?: string,
): PermissionRequest[] {
  return sortPermissionRequests(
    requests.filter((request) => permissionRequestIsOwnedBySession(request, sessionId)),
  );
}

export interface PermissionRequestBatch {
  sessionId: string;
  roundId: string;
  requests: PermissionRequest[];
}

function selectActivePermissionBatchFromRequests(
  routed: readonly PermissionRequest[],
): PermissionRequestBatch | undefined {
  const first = routed[0];
  if (!first) return undefined;

  const batchRequests = routed.filter(
    (request) => request.sessionId === first.sessionId && request.roundId === first.roundId,
  );
  return {
    sessionId: first.sessionId,
    roundId: first.roundId,
    requests: batchRequests,
  };
}

export function selectActivePermissionBatch(
  requests: readonly PermissionRequest[],
  sessionId?: string,
): PermissionRequestBatch | undefined {
  return selectActivePermissionBatchFromRequests(
    selectPermissionRequestsForSession(requests, sessionId),
  );
}

export function selectActivePermissionBatchOwnedBySession(
  requests: readonly PermissionRequest[],
  sessionId?: string,
): PermissionRequestBatch | undefined {
  return selectActivePermissionBatchFromRequests(
    selectPermissionRequestsOwnedBySession(requests, sessionId),
  );
}

/**
 * Keep permission requests in arrival order across rounds, while preserving
 * the model-provided order inside each round. The first-seen batch position is
 * used because round IDs are opaque (usually UUIDs) and are not chronological.
 */
export function sortPermissionRequests(
  requests: readonly PermissionRequest[],
): PermissionRequest[] {
  const firstBatchIndex = new Map<string, number>();
  requests.forEach((request, index) => {
    const batchId = `${request.sessionId}\u0000${request.roundId}`;
    if (!firstBatchIndex.has(batchId)) firstBatchIndex.set(batchId, index);
  });

  return [...requests].sort((left, right) => {
    const leftBatchId = `${left.sessionId}\u0000${left.roundId}`;
    const rightBatchId = `${right.sessionId}\u0000${right.roundId}`;
    const batchOrder =
      (firstBatchIndex.get(leftBatchId) ?? 0) - (firstBatchIndex.get(rightBatchId) ?? 0);
    if (batchOrder !== 0) return batchOrder;

    return left.order - right.order || left.requestId.localeCompare(right.requestId);
  });
}

export function pendingPermissionToolCallIdsForSession(
  requests: readonly PermissionRequest[],
  sessionId?: string,
): ReadonlySet<string> {
  const toolCallIds = new Set<string>();
  if (!sessionId) return toolCallIds;

  for (const request of requests) {
    if (!permissionRequestBelongsToSession(request, sessionId)) continue;

    const toolCallId = request.sessionId === sessionId
      ? request.toolCallId
      : request.delegation?.parentToolCallId;
    if (toolCallId) toolCallIds.add(toolCallId);
  }

  return toolCallIds;
}

export function applyPermissionRequestEvent(
  requests: readonly PermissionRequest[],
  event: PermissionRequestEvent,
): PermissionRequest[] {
  if (event.event !== 'asked') {
    return requests.filter((request) => request.requestId !== event.requestId);
  }

  const existingIndex = requests.findIndex(
    (request) => request.requestId === event.request.requestId,
  );
  if (existingIndex < 0) return [...requests, event.request];

  const next = [...requests];
  next[existingIndex] = event.request;
  return next;
}

export function reconcilePermissionRequestSnapshot(
  current: readonly PermissionRequest[],
  pending: readonly PermissionRequest[],
  resolvedIds: ReadonlySet<string>,
): PermissionRequest[] {
  const currentById = new Map(current.map((request) => [request.requestId, request]));
  const pendingIds = new Set<string>();
  const reconciled: PermissionRequest[] = [];

  for (const request of pending) {
    if (resolvedIds.has(request.requestId)) continue;
    pendingIds.add(request.requestId);
    reconciled.push(currentById.get(request.requestId) ?? request);
  }

  for (const request of current) {
    if (!resolvedIds.has(request.requestId) && !pendingIds.has(request.requestId)) {
      reconciled.push(request);
    }
  }

  return reconciled;
}
