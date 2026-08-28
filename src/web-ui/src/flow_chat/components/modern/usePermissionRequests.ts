import { useCallback, useEffect, useMemo, useSyncExternalStore } from 'react';
import {
  type PermissionReplyKind,
  type PermissionRequest,
} from '@/infrastructure/api/service-api/AgentAPI';
import {
  selectActivePermissionBatch,
  selectActivePermissionBatchOwnedBySession,
  selectPermissionRequestsForSession,
  selectPermissionRequestsOwnedBySession,
} from './permissionRequestRouting';
import { FlowChatStore } from '../../store/FlowChatStore';
import { driverForSession } from '../../session-drivers/registry';
import {
  ensureActivePermissionMailbox,
  liveSessionInteractionStore,
} from '../../services/liveSessionInteractionStore';

const EMPTY_EXTERNAL_REQUESTS: PermissionRequest[] = [];
const noopSubscribe = () => () => {};
const emptyExternalSnapshot = () => EMPTY_EXTERNAL_REQUESTS;

export function usePermissionRequests(sessionId?: string) {
  // Driver resolution is per-render: the caller re-renders on any session
  // change, so a projection whose dispatch config binds late is picked up.
  const session = sessionId
    ? FlowChatStore.getInstance().getState().sessions.get(sessionId)
    : undefined;
  const driver = driverForSession(sessionId ?? '', session);
  const source = useMemo(
    () => driver.permissionRequestSource(sessionId ?? ''),
    [driver, sessionId],
  );
  const isLiveSource = source === 'live';

  const externalRequests = useSyncExternalStore(
    isLiveSource ? noopSubscribe : source.subscribe,
    isLiveSource ? emptyExternalSnapshot : source.getSnapshot,
  ) as unknown as PermissionRequest[];

  const liveMailbox = useSyncExternalStore(
    liveSessionInteractionStore.subscribe,
    liveSessionInteractionStore.getActiveSnapshot,
    liveSessionInteractionStore.getActiveSnapshot,
  );

  useEffect(() => {
    if (isLiveSource) void ensureActivePermissionMailbox();
  }, [isLiveSource, liveMailbox.surfaceId]);

  const respond = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      await driver.respondPermission(sessionId ?? '', requestId, reply, feedback);
      if (isLiveSource) {
        liveSessionInteractionStore.markPermissionResolved(liveMailbox.surfaceId, requestId);
      }
    },
    [driver, sessionId, isLiveSource, liveMailbox.surfaceId],
  );

  const respondBatch = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      const resolvedRequestIds = await driver.respondPermissionBatch(
        sessionId ?? '',
        requestId,
        reply,
        feedback,
      );
      if (isLiveSource) {
        resolvedRequestIds.forEach((requestId) => {
          liveSessionInteractionStore.markPermissionResolved(
            liveMailbox.surfaceId,
            requestId,
          );
        });
      }
    },
    [driver, sessionId, isLiveSource, liveMailbox.surfaceId],
  );

  const effectiveRequests = isLiveSource ? liveMailbox.requests : externalRequests;
  const sessionRequests = useMemo(
    () => selectPermissionRequestsForSession(effectiveRequests, sessionId),
    [effectiveRequests, sessionId],
  );

  const activeBatch = useMemo(
    () => selectActivePermissionBatch(effectiveRequests, sessionId),
    [effectiveRequests, sessionId],
  );
  const ownedRequests = useMemo(
    () => selectPermissionRequestsOwnedBySession(effectiveRequests, sessionId),
    [effectiveRequests, sessionId],
  );
  const ownedActiveBatch = useMemo(
    () => selectActivePermissionBatchOwnedBySession(effectiveRequests, sessionId),
    [effectiveRequests, sessionId],
  );

  // Keep the broad projection for transcript/task-card state, while exposing
  // the owner-only projection for permission UI so one request has one
  // actionable surface.
  return {
    requests: sessionRequests,
    activeBatch,
    ownedRequests,
    ownedActiveBatch,
    respond,
    respondBatch,
  };
}
