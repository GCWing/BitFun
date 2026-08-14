import { useCallback, useMemo, useSyncExternalStore } from 'react';
import {
  type PermissionReplyKind,
  type PermissionRequest,
} from '@/infrastructure/api/service-api/AgentAPI';
import {
  selectActivePermissionBatch,
  selectPermissionRequestsForSession,
} from './permissionRequestRouting';
import {
  getLivePermissionRequests,
  markLivePermissionRequestsResolved,
  subscribeLivePermissionRequests,
} from './permissionRequestLiveStore';
import { FlowChatStore } from '../../store/FlowChatStore';
import { driverForSession } from '../../session-drivers/registry';

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

  const effectiveRequests = useSyncExternalStore(
    isLiveSource ? subscribeLivePermissionRequests : source.subscribe,
    isLiveSource ? getLivePermissionRequests : source.getSnapshot,
  ) as unknown as PermissionRequest[];

  const respond = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      await driver.respondPermission(sessionId ?? '', requestId, reply, feedback);
      if (isLiveSource) {
        markLivePermissionRequestsResolved([requestId]);
      }
    },
    [driver, sessionId, isLiveSource],
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
        markLivePermissionRequestsResolved(resolvedRequestIds);
      }
    },
    [driver, sessionId, isLiveSource],
  );

  const sessionRequests = useMemo(
    () => selectPermissionRequestsForSession(effectiveRequests, sessionId),
    [effectiveRequests, sessionId],
  );

  const activeBatch = useMemo(
    () => selectActivePermissionBatch(effectiveRequests, sessionId),
    [effectiveRequests, sessionId],
  );

  return { requests: sessionRequests, activeBatch, respond, respondBatch };
}
