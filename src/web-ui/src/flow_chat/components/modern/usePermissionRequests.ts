import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  agentAPI,
  type PermissionReplyKind,
  type PermissionRequestEvent,
  type PermissionRequest,
} from '@/infrastructure/api/service-api/AgentAPI';
import {
  applyPermissionRequestEvent,
  reconcilePermissionRequestSnapshot,
  selectActivePermissionBatch,
  selectPermissionRequestsForSession,
} from './permissionRequestRouting';
import { dispatchApi } from '@/features/dispatch/dispatchApi';
import { useDispatchJobStore } from '@/features/dispatch/dispatchJobStore';
import { requestDispatchJobRefresh } from '@/features/dispatch/DispatchJobObserver';

const EMPTY_DISPATCH_PERMISSIONS: Array<Record<string, unknown>> = [];

export function usePermissionRequests(sessionId?: string, dispatchJobId?: string) {
  const [requests, setRequests] = useState<PermissionRequest[]>([]);
  const resolvedIds = useRef(new Set<string>());
  const dispatchRequests = useDispatchJobStore(state => (
    dispatchJobId
      ? state.jobs[dispatchJobId]?.pendingPermissions ?? EMPTY_DISPATCH_PERMISSIONS
      : EMPTY_DISPATCH_PERMISSIONS
  )) as unknown as PermissionRequest[];

  useEffect(() => {
    if (dispatchJobId) {
      setRequests([]);
      return undefined;
    }
    let disposed = false;
    const unlisten = agentAPI.onPermissionRequestEvent((event: PermissionRequestEvent) => {
      if (disposed) return;
      setRequests((current) => {
        if (event.event === 'asked') {
          resolvedIds.current.delete(event.request.requestId);
        } else {
          resolvedIds.current.add(event.requestId);
        }
        return applyPermissionRequestEvent(current, event);
      });
    });

    void (async () => {
      try {
        await agentAPI.subscribePermissionRequests();
        const pending = await agentAPI.listPendingPermissionRequests();
        if (!disposed) {
          setRequests((current) =>
            reconcilePermissionRequestSnapshot(current, pending, resolvedIds.current),
          );
        }
      } catch {
        if (!disposed) setRequests([]);
      }
    })();

    return () => {
      disposed = true;
      unlisten();
    };
  }, [dispatchJobId]);

  const respond = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      if (dispatchJobId) {
        await dispatchApi.answerPermission(dispatchJobId, requestId, reply, feedback);
        requestDispatchJobRefresh(dispatchJobId);
        return;
      }
      await agentAPI.respondPermission(requestId, reply, feedback);
      resolvedIds.current.add(requestId);
      setRequests((current) => current.filter((request) => request.requestId !== requestId));
    },
    [dispatchJobId],
  );

  const respondBatch = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      if (dispatchJobId) {
        const batch = selectActivePermissionBatch(dispatchRequests as PermissionRequest[], sessionId);
        const requestIds = batch?.requests.map(request => request.requestId) ?? [requestId];
        for (const pendingRequestId of requestIds) {
          await dispatchApi.answerPermission(
            dispatchJobId,
            pendingRequestId,
            reply,
            feedback,
          );
        }
        requestDispatchJobRefresh(dispatchJobId);
        return;
      }
      const resolvedRequestIds = await agentAPI.respondPermissionBatch(requestId, reply, feedback);
      const resolved = new Set(resolvedRequestIds);
      resolvedRequestIds.forEach((id) => resolvedIds.current.add(id));
      setRequests((current) => current.filter((request) => !resolved.has(request.requestId)));
    },
    [dispatchJobId, dispatchRequests, sessionId],
  );

  const effectiveRequests = dispatchJobId ? dispatchRequests : requests;
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
