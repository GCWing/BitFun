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

export function usePermissionRequests(sessionId?: string) {
  const [requests, setRequests] = useState<PermissionRequest[]>([]);
  const resolvedIds = useRef(new Set<string>());

  useEffect(() => {
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
  }, []);

  const respond = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      await agentAPI.respondPermission(requestId, reply, feedback);
      resolvedIds.current.add(requestId);
      setRequests((current) => current.filter((request) => request.requestId !== requestId));
    },
    [],
  );

  const respondBatch = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      const resolvedRequestIds = await agentAPI.respondPermissionBatch(requestId, reply, feedback);
      const resolved = new Set(resolvedRequestIds);
      resolvedRequestIds.forEach((id) => resolvedIds.current.add(id));
      setRequests((current) => current.filter((request) => !resolved.has(request.requestId)));
    },
    [],
  );

  const sessionRequests = useMemo(
    () => selectPermissionRequestsForSession(requests, sessionId),
    [requests, sessionId],
  );

  const activeBatch = useMemo(
    () => selectActivePermissionBatch(requests, sessionId),
    [requests, sessionId],
  );

  return { requests: sessionRequests, activeBatch, respond, respondBatch };
}
