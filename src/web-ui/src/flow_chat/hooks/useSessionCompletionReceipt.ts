import { useEffect, useMemo, useSyncExternalStore, type RefObject } from 'react';
import { getActiveSurfaceId, onSurfaceActivated } from '@/infrastructure/peer-device/deviceSurface';
import { flowChatStore } from '../store/FlowChatStore';
import { sessionToVirtualItems } from '../store/modernFlowChatStore';
import { getVirtualItemStableKey } from '../components/modern/virtualItemIdentity';
import { completionResultItem, isCompletionResultVisible, sessionCompletionReceipt } from '../utils/sessionCompletionReceipt';
import type { Session } from '../types/flow-chat';
import { lastUserDialogTurn } from '../utils/flowChatTurnIdentity';

/** Confirm the displayed result, never just the selection or a mounted overscan row. */
export function useSessionCompletionReceipt(
  sessionId: string | null,
  scrollerRef: RefObject<HTMLElement | null>,
  isActive = true,
): void {
  const source = useMemo(() => {
    let cachedSession: Session | undefined;
    let cachedSurface = '';
    let cachedSnapshot = '';
    const getSnapshot = () => {
      const session = sessionId ? flowChatStore.getState().sessions.get(sessionId) : undefined;
      const surface = getActiveSurfaceId();
      if (session === cachedSession && surface === cachedSurface) return cachedSnapshot;
      cachedSession = session;
      cachedSurface = surface;
      cachedSnapshot = '';
      const receipt = sessionCompletionReceipt(session);
      if (!receipt || !session) return '';
      const turnId = lastUserDialogTurn(session)!.id;
      const result = completionResultItem(sessionToVirtualItems(session), turnId);
      cachedSnapshot = result ? JSON.stringify([surface, receipt, getVirtualItemStableKey(result)]) : '';
      return cachedSnapshot;
    };
    const subscribe = (notify: () => void) => {
      const disposeStore = flowChatStore.subscribeSelector(getSnapshot, notify);
      const disposeSurface = onSurfaceActivated(notify);
      return () => { disposeStore(); disposeSurface(); };
    };
    return { subscribe, getSnapshot };
  }, [sessionId]);
  const snapshot = useSyncExternalStore(source.subscribe, source.getSnapshot, source.getSnapshot);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!snapshot || !sessionId || !scroller || !isActive) return;
    const activeSessionId = sessionId;
    const activeScroller = scroller;
    const [surfaceId, receipt, resultKey] = JSON.parse(snapshot) as [string, string, string];
    let frame: number | undefined;
    let observedResult: HTMLElement | undefined;
    function schedule() {
      if (frame === undefined) frame = requestAnimationFrame(check);
    }
    const resizeObserver = new ResizeObserver(schedule);
    function check() {
      frame = undefined;
      if (getActiveSurfaceId() !== surfaceId || document.visibilityState !== 'visible'
        || !document.hasFocus() || !activeScroller.isConnected
        || activeScroller.closest('[hidden], [inert], [aria-hidden="true"]')) return;
      const style = getComputedStyle(activeScroller);
      if (style.visibility !== 'visible' || style.display === 'none') return;
      const result = Array.from(activeScroller.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-virtual-item-key]'))
        .find(element => element.dataset.virtualItemKey === resultKey);
      if (!result) return;
      if (observedResult !== result) {
        if (observedResult) resizeObserver.unobserve(observedResult);
        resizeObserver.observe(result);
        observedResult = result;
      }
      const rect = activeScroller.getBoundingClientRect();
      if (isCompletionResultVisible(result.getBoundingClientRect(), {
        top: Math.max(rect.top, 0), bottom: Math.min(rect.bottom, window.innerHeight),
        left: Math.max(rect.left, 0), right: Math.min(rect.right, window.innerWidth),
      })) {
        flowChatStore.clearSessionUnreadCompletion(activeSessionId, { surfaceId, receipt });
      }
    }
    // Only attached while a settled result is unread. Child-list observation
    // handles virtual-window mounts without watching every streamed text byte.
    const observer = new MutationObserver(schedule);
    observer.observe(activeScroller, { childList: true, subtree: true });
    for (let ancestor: HTMLElement | null = activeScroller; ancestor; ancestor = ancestor.parentElement) {
      observer.observe(ancestor, { attributes: true, attributeFilter: ['hidden', 'inert', 'aria-hidden', 'style', 'class'] });
    }
    resizeObserver.observe(activeScroller);
    activeScroller.addEventListener('scroll', schedule, { passive: true });
    window.addEventListener('focus', schedule);
    document.addEventListener('visibilitychange', schedule);
    schedule();
    return () => {
      if (frame !== undefined) cancelAnimationFrame(frame);
      observer.disconnect();
      resizeObserver.disconnect();
      activeScroller.removeEventListener('scroll', schedule);
      window.removeEventListener('focus', schedule);
      document.removeEventListener('visibilitychange', schedule);
    };
  }, [snapshot, sessionId, scrollerRef, isActive]);
}
