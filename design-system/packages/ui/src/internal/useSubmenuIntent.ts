import { useCallback, useEffect, useRef, type RefObject } from 'react';

export interface SubmenuIntentPoint {
  x: number;
  y: number;
}

export interface SubmenuIntentRect {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

interface TimedPointerPoint extends SubmenuIntentPoint {
  time: number;
}

interface PointerLikeEvent {
  clientX: number;
  clientY: number;
  pointerType?: string;
  timeStamp: number;
}

interface PendingTransition<T> {
  targetId: T | null;
  kind: 'open' | 'switch';
  timer: number;
}

export interface UseSubmenuIntentOptions<T> {
  activeId: T | null;
  onActiveIdChange: (id: T | null) => void;
  parentRef: RefObject<HTMLElement | null>;
  submenuRef: RefObject<HTMLElement | null>;
  enabled?: boolean;
  openDelayMs?: number;
  closeDelayMs?: number;
  switchDelayMs?: number;
  tolerance?: number;
}

export interface SubmenuIntentControls<T> {
  requestChange: (targetId: T | null, event: PointerLikeEvent) => void;
  requestClose: (event?: PointerLikeEvent) => void;
  keepOpen: () => void;
  openNow: (id: T) => void;
  closeNow: () => void;
  cancelPending: () => void;
}

const DEFAULT_OPEN_DELAY_MS = 120;
const DEFAULT_CLOSE_DELAY_MS = 300;
const DEFAULT_SWITCH_DELAY_MS = 300;
const DEFAULT_TOLERANCE = 48;
const POINTER_HISTORY_MAX_AGE_MS = 300;
const POINTER_HISTORY_MIN_DISTANCE_PX = 2;
// Include the parent's inner edge padding, not a wide band of sibling rows.
const BRIDGE_EDGE_PADDING_PX = 4;

const pointInRect = (point: SubmenuIntentPoint, rect: SubmenuIntentRect): boolean => (
  rect.right > rect.left
  && rect.bottom > rect.top
  && point.x >= rect.left
  && point.x <= rect.right
  && point.y >= rect.top
  && point.y <= rect.bottom
);

/** A stationary, bidirectional bridge across the gap; independent of menu aim. */
export const isPointInSubmenuBridge = (
  point: SubmenuIntentPoint,
  parentRect: SubmenuIntentRect,
  submenuRect: SubmenuIntentRect,
): boolean => {
  if (parentRect.right <= parentRect.left || parentRect.bottom <= parentRect.top
    || submenuRect.right <= submenuRect.left || submenuRect.bottom <= submenuRect.top) {
    return false;
  }

  const opensRight = submenuRect.left + submenuRect.right > parentRect.left + parentRect.right;
  return pointInRect(point, {
    left: (opensRight ? parentRect.right : submenuRect.right) - BRIDGE_EDGE_PADDING_PX,
    right: (opensRight ? submenuRect.left : parentRect.left) + BRIDGE_EDGE_PADDING_PX,
    top: submenuRect.top - BRIDGE_EDGE_PADDING_PX,
    bottom: submenuRect.bottom + BRIDGE_EDGE_PADDING_PX,
  });
};

const distanceToRect = (point: SubmenuIntentPoint, rect: SubmenuIntentRect): number => {
  const dx = Math.max(rect.left - point.x, 0, point.x - rect.right);
  const dy = Math.max(rect.top - point.y, 0, point.y - rect.bottom);
  return Math.hypot(dx, dy);
};

const signedArea = (
  point: SubmenuIntentPoint,
  start: SubmenuIntentPoint,
  end: SubmenuIntentPoint,
): number => (
  (point.x - end.x) * (start.y - end.y)
  - (start.x - end.x) * (point.y - end.y)
);

const pointInTriangle = (
  point: SubmenuIntentPoint,
  first: SubmenuIntentPoint,
  second: SubmenuIntentPoint,
  third: SubmenuIntentPoint,
): boolean => {
  const firstSign = signedArea(point, first, second);
  const secondSign = signedArea(point, second, third);
  const thirdSign = signedArea(point, third, first);
  const hasNegative = firstSign < 0 || secondSign < 0 || thirdSign < 0;
  const hasPositive = firstSign > 0 || secondSign > 0 || thirdSign > 0;
  return !(hasNegative && hasPositive);
};

/**
 * Returns true when the pointer is entering the corridor between its previous
 * position and the nearest vertical edge of the open submenu. The nearest edge
 * makes the same calculation work for menus that open to either side.
 */
export const isPointerMovingTowardSubmenu = (
  previous: SubmenuIntentPoint,
  current: SubmenuIntentPoint,
  submenuRect: SubmenuIntentRect,
  tolerance = DEFAULT_TOLERANCE,
): boolean => {
  if (pointInRect(current, submenuRect)) return true;

  const submenuOpensRight = previous.x <= submenuRect.left;
  const submenuOpensLeft = previous.x >= submenuRect.right;
  if (!submenuOpensRight && !submenuOpensLeft) return false;

  const edgeX = submenuOpensRight ? submenuRect.left : submenuRect.right;
  const horizontalDelta = current.x - previous.x;
  if ((submenuOpensRight && horizontalDelta <= 0) || (submenuOpensLeft && horizontalDelta >= 0)) {
    return false;
  }

  if (distanceToRect(current, submenuRect) >= distanceToRect(previous, submenuRect)) {
    return false;
  }

  return pointInTriangle(
    current,
    previous,
    { x: edgeX, y: submenuRect.top - tolerance },
    { x: edgeX, y: submenuRect.bottom + tolerance },
  );
};

const isMousePointer = (event: PointerLikeEvent): boolean => (
  !event.pointerType || event.pointerType === 'mouse'
);

export const useSubmenuIntent = <T,>({
  activeId,
  onActiveIdChange,
  parentRef,
  submenuRef,
  enabled = true,
  openDelayMs = DEFAULT_OPEN_DELAY_MS,
  closeDelayMs = DEFAULT_CLOSE_DELAY_MS,
  switchDelayMs = DEFAULT_SWITCH_DELAY_MS,
  tolerance = DEFAULT_TOLERANCE,
}: UseSubmenuIntentOptions<T>): SubmenuIntentControls<T> => {
  const activeIdRef = useRef(activeId);
  const onActiveIdChangeRef = useRef(onActiveIdChange);
  const pointerHistoryRef = useRef<TimedPointerPoint[]>([]);
  const pendingTransitionRef = useRef<PendingTransition<T> | null>(null);
  const closeTimerRef = useRef<number | null>(null);
  // Preserve the leave intent when the bridge cancels its timer, so moving out
  // of the bridge can arm closing again without another DOM pointerleave.
  const closeRequestedRef = useRef(false);

  activeIdRef.current = activeId;
  onActiveIdChangeRef.current = onActiveIdChange;

  const clearPendingTransition = useCallback(() => {
    const pending = pendingTransitionRef.current;
    if (pending) {
      window.clearTimeout(pending.timer);
      pendingTransitionRef.current = null;
    }
  }, []);

  const clearCloseTimer = useCallback(() => {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
  }, []);

  const cancelPending = useCallback(() => {
    clearPendingTransition();
    clearCloseTimer();
    closeRequestedRef.current = false;
  }, [clearCloseTimer, clearPendingTransition]);

  const commitChange = useCallback((targetId: T | null) => {
    cancelPending();
    activeIdRef.current = targetId;
    onActiveIdChangeRef.current(targetId);
  }, [cancelPending]);

  const scheduleTransition = useCallback((
    targetId: T | null,
    kind: PendingTransition<T>['kind'],
    delayMs: number,
  ) => {
    clearPendingTransition();
    const timer = window.setTimeout(() => {
      pendingTransitionRef.current = null;
      commitChange(targetId);
    }, delayMs);
    pendingTransitionRef.current = { targetId, kind, timer };
  }, [clearPendingTransition, commitChange]);

  const scheduleClose = useCallback(() => {
    clearCloseTimer();
    closeTimerRef.current = window.setTimeout(() => {
      closeTimerRef.current = null;
      commitChange(null);
    }, closeDelayMs);
  }, [clearCloseTimer, closeDelayMs, commitChange]);

  const findPreviousPoint = useCallback((current: TimedPointerPoint): TimedPointerPoint | null => {
    for (let index = pointerHistoryRef.current.length - 1; index >= 0; index -= 1) {
      const candidate = pointerHistoryRef.current[index];
      if (!candidate) continue;
      const age = current.time - candidate.time;
      const distance = Math.hypot(current.x - candidate.x, current.y - candidate.y);
      if (age >= 0 && age <= POINTER_HISTORY_MAX_AGE_MS && distance >= POINTER_HISTORY_MIN_DISTANCE_PX) {
        return candidate;
      }
    }
    return null;
  }, []);

  const pointerHeadsToOpenSubmenu = useCallback((
    previous: TimedPointerPoint | null,
    current: TimedPointerPoint,
  ): boolean => {
    const submenuRect = submenuRef.current?.getBoundingClientRect();
    return Boolean(
      previous
      && submenuRect
      && isPointerMovingTowardSubmenu(previous, current, submenuRect, tolerance),
    );
  }, [submenuRef, tolerance]);

  const getProtectedRegion = useCallback((point: SubmenuIntentPoint): 'submenu' | 'bridge' | null => {
    if (activeIdRef.current === null) return null;
    const submenuRect = submenuRef.current?.getBoundingClientRect();
    if (!submenuRect) return null;
    if (pointInRect(point, submenuRect)) return 'submenu';
    const parentRect = parentRef.current?.getBoundingClientRect();
    return parentRect && isPointInSubmenuBridge(point, parentRect, submenuRect) ? 'bridge' : null;
  }, [parentRef, submenuRef]);

  const requestChange = useCallback((targetId: T | null, event: PointerLikeEvent) => {
    if (!enabled || !isMousePointer(event)) return;

    clearCloseTimer();
    closeRequestedRef.current = false;
    const currentActiveId = activeIdRef.current;
    if (Object.is(currentActiveId, targetId)) {
      clearPendingTransition();
      return;
    }

    const currentPoint: TimedPointerPoint = {
      x: event.clientX,
      y: event.clientY,
      time: event.timeStamp,
    };
    const previousPoint = findPreviousPoint(currentPoint);

    if (
      currentActiveId !== null
      && pointerHeadsToOpenSubmenu(previousPoint, currentPoint)
    ) {
      scheduleTransition(targetId, 'switch', switchDelayMs);
      return;
    }

    if (currentActiveId === null && targetId !== null && openDelayMs > 0) {
      scheduleTransition(targetId, 'open', openDelayMs);
      return;
    }

    commitChange(targetId);
  }, [
    clearCloseTimer,
    clearPendingTransition,
    commitChange,
    enabled,
    findPreviousPoint,
    openDelayMs,
    pointerHeadsToOpenSubmenu,
    scheduleTransition,
    switchDelayMs,
  ]);

  const requestClose = useCallback((event?: PointerLikeEvent) => {
    if (!enabled || (event && !isMousePointer(event))) return;
    clearPendingTransition();
    if (activeIdRef.current === null) return;
    closeRequestedRef.current = true;
    // pointerleave can be the last event before the mouse stops in the gap.
    // Do not depend on receiving a later pointermove to cancel closing.
    if (event && getProtectedRegion({ x: event.clientX, y: event.clientY })) {
      clearCloseTimer();
      return;
    }
    scheduleClose();
  }, [clearCloseTimer, clearPendingTransition, enabled, getProtectedRegion, scheduleClose]);

  const keepOpen = useCallback(() => {
    cancelPending();
  }, [cancelPending]);

  const openNow = useCallback((id: T) => {
    commitChange(id);
  }, [commitChange]);

  const closeNow = useCallback(() => {
    commitChange(null);
  }, [commitChange]);

  useEffect(() => {
    if (!enabled) {
      cancelPending();
      pointerHistoryRef.current = [];
      return;
    }

    const handlePointerMove = (event: PointerEvent) => {
      if (!isMousePointer(event)) return;

      const currentPoint: TimedPointerPoint = {
        x: event.clientX,
        y: event.clientY,
        time: event.timeStamp,
      };
      const previousPoint = findPreviousPoint(currentPoint);
      const protectedRegion = getProtectedRegion(currentPoint);
      const headingToSubmenu = !protectedRegion && activeIdRef.current !== null
        && pointerHeadsToOpenSubmenu(previousPoint, currentPoint);
      const pending = pendingTransitionRef.current;

      if (protectedRegion) {
        cancelPending();
        closeRequestedRef.current = protectedRegion === 'bridge';
      } else if (pending?.kind === 'switch') {
        if (headingToSubmenu) {
          scheduleTransition(pending.targetId, 'switch', switchDelayMs);
        } else {
          commitChange(pending.targetId);
        }
      } else if (closeRequestedRef.current && (closeTimerRef.current === null || headingToSubmenu)) {
        scheduleClose();
      }

      pointerHistoryRef.current.push(currentPoint);
      if (pointerHistoryRef.current.length > 4) {
        pointerHistoryRef.current.shift();
      }
    };

    const handlePointerExit = () => requestClose();
    document.addEventListener('pointermove', handlePointerMove, { capture: true, passive: true });
    document.addEventListener('pointerleave', handlePointerExit);
    window.addEventListener('blur', handlePointerExit);
    return () => {
      document.removeEventListener('pointermove', handlePointerMove, true);
      document.removeEventListener('pointerleave', handlePointerExit);
      window.removeEventListener('blur', handlePointerExit);
    };
  }, [
    cancelPending,
    commitChange,
    enabled,
    findPreviousPoint,
    getProtectedRegion,
    pointerHeadsToOpenSubmenu,
    requestClose,
    scheduleClose,
    scheduleTransition,
    switchDelayMs,
  ]);

  useEffect(() => cancelPending, [cancelPending]);

  return {
    requestChange,
    requestClose,
    keepOpen,
    openNow,
    closeNow,
    cancelPending,
  };
};
