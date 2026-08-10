import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
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
import { FlowChatStore } from '../../store/FlowChatStore';
import { driverForSession } from '../../session-drivers/registry';

const EMPTY_EXTERNAL_REQUESTS: PermissionRequest[] = [];
const noopSubscribe = () => () => {};
const emptyExternalSnapshot = () => EMPTY_EXTERNAL_REQUESTS;

export function usePermissionRequests(sessionId?: string) {
  const [liveRequests, setLiveRequests] = useState<PermissionRequest[]>([]);
  const resolvedIds = useRef(new Set<string>());

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

  useEffect(() => {
    if (!isLiveSource) {
      setLiveRequests([]);
      return undefined;
    }
    let disposed = false;
    const unlisten = agentAPI.onPermissionRequestEvent((event: PermissionRequestEvent) => {
      if (disposed) return;
      setLiveRequests((current) => {
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
          setLiveRequests((current) =>
            reconcilePermissionRequestSnapshot(current, pending, resolvedIds.current),
          );
        }
      } catch {
        if (!disposed) setLiveRequests([]);
      }
    })();

    return () => {
      disposed = true;
      unlisten();
    };
  }, [isLiveSource]);

  const respond = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      await driver.respondPermission(sessionId ?? '', requestId, reply, feedback);
      if (isLiveSource) {
        resolvedIds.current.add(requestId);
        setLiveRequests((current) => current.filter((request) => request.requestId !== requestId));
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
        const resolved = new Set(resolvedRequestIds);
        resolvedRequestIds.forEach((id) => resolvedIds.current.add(id));
        setLiveRequests((current) => current.filter((request) => !resolved.has(request.requestId)));
      }
    },
    [driver, sessionId, isLiveSource],
  );

  const effectiveRequests = isLiveSource ? liveRequests : externalRequests;
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
