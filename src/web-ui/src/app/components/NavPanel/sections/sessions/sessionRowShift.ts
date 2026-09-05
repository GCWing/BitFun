/**
 * Session nav rows are plain flex children, so removing one (delete / archive)
 * snaps every row below it into its new slot within a single commit — and the
 * buffered row that takes the freed slot appears out of nowhere at the same
 * time. Measuring the rows before and after that commit lets the section replay
 * the jump as a short slide, which reads as the list closing up instead of
 * flashing.
 */

import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react';

export const SESSION_ROW_SHIFT_DURATION_MS = 180;
export const SESSION_ROW_SHIFT_EASING = 'cubic-bezier(0.22, 1, 0.36, 1)';

/** Sub-pixel jumps are invisible; animating them only costs a paint. */
const MIN_SHIFT_PX = 0.5;
/** Selector for a top-level or child session row inside the inline list. */
const SESSION_ROW_SELECTOR = '.openbitfun-nav-panel__inline-item[data-session-id]';

export interface SessionRowShift {
  sessionId: string;
  /** Offset (px) the row starts from so it appears not to have moved yet. */
  fromOffsetY: number;
}

export interface SessionRowRemovalTransition {
  /** Whether a row left the list, which is what puts the list in motion. */
  hasRemovedRow: boolean;
  /** Rows that must slide from their old slot into their new one. */
  shifts: SessionRowShift[];
  /** Rows pulled in to backfill the list, which fade in instead of popping. */
  enteringSessionIds: string[];
}

const NO_TRANSITION: SessionRowRemovalTransition = {
  hasRemovedRow: false,
  shifts: [],
  enteringSessionIds: [],
};

/**
 * What moved because a row left the list. Returns nothing when no row was
 * removed, so plain inserts (metadata streaming in during startup, expanding
 * the list) keep landing without motion.
 */
export function computeSessionRowRemovalTransition(
  previousOffsets: ReadonlyMap<string, number>,
  nextOffsets: ReadonlyMap<string, number>,
): SessionRowRemovalTransition {
  let hasRemovedRow = false;
  for (const sessionId of previousOffsets.keys()) {
    if (!nextOffsets.has(sessionId)) {
      hasRemovedRow = true;
      break;
    }
  }
  if (!hasRemovedRow) {
    return NO_TRANSITION;
  }

  const shifts: SessionRowShift[] = [];
  const enteringSessionIds: string[] = [];
  for (const [sessionId, nextOffset] of nextOffsets) {
    const previousOffset = previousOffsets.get(sessionId);
    if (previousOffset === undefined) {
      enteringSessionIds.push(sessionId);
      continue;
    }
    const fromOffsetY = previousOffset - nextOffset;
    if (Math.abs(fromOffsetY) < MIN_SHIFT_PX) {
      continue;
    }
    shifts.push({ sessionId, fromOffsetY });
  }
  return { hasRemovedRow, shifts, enteringSessionIds };
}

const prefersReducedMotion = (): boolean =>
  typeof window !== 'undefined'
  && typeof window.matchMedia === 'function'
  && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

const clearRowTransition = (row: HTMLElement): void => {
  row.style.transition = '';
  row.style.transform = '';
  row.style.opacity = '';
};

const clearListCollapse = (list: HTMLElement): void => {
  list.style.transition = '';
  list.style.height = '';
  list.style.overflow = '';
};

/**
 * Height the list would have without an in-flight collapse, so a second delete
 * measures the layout rather than the frame the animation happens to be on.
 */
const measureNaturalHeight = (list: HTMLElement): number => {
  const inlineHeight = list.style.height;
  if (!inlineHeight) {
    return list.offsetHeight;
  }
  list.style.height = '';
  const naturalHeight = list.offsetHeight;
  list.style.height = inlineHeight;
  return naturalHeight;
};

/**
 * `offsetTop` ignores transforms, so a row measured mid-slide still reports the
 * slot it settles into — a second delete during the animation stays correct.
 */
const readSessionRows = (container: HTMLElement): Map<string, HTMLElement> => {
  const rows = new Map<string, HTMLElement>();
  for (const element of container.querySelectorAll<HTMLElement>(SESSION_ROW_SELECTOR)) {
    const sessionId = element.dataset.sessionId;
    if (sessionId) {
      rows.set(sessionId, element);
    }
  }
  return rows;
};

/**
 * Slides the surviving rows up from where they used to be — and fades in the
 * row that backfills the list — whenever a session row leaves `listRef`.
 * `rowSignature` must change whenever the rendered row set changes, so the hook
 * re-measures on the commit that removed the row.
 */
export function useSessionRowRemovalTransition(
  listRef: RefObject<HTMLElement | null>,
  rowSignature: string,
): void {
  const previousOffsetsRef = useRef<Map<string, number>>(new Map());
  const previousHeightRef = useRef<number | null>(null);
  /** Row → timer that strips its inline styles once the animation settles. */
  const settleTimersRef = useRef<Map<HTMLElement, number>>(new Map());
  const listSettleTimerRef = useRef<number | null>(null);

  useEffect(() => {
    const settleTimers = settleTimersRef.current;
    const list = listRef.current;
    return () => {
      for (const [row, timer] of settleTimers) {
        window.clearTimeout(timer);
        clearRowTransition(row);
      }
      settleTimers.clear();
      if (listSettleTimerRef.current !== null) {
        window.clearTimeout(listSettleTimerRef.current);
        listSettleTimerRef.current = null;
      }
      if (list) {
        clearListCollapse(list);
      }
    };
  }, [listRef]);

  useLayoutEffect(() => {
    const container = listRef.current;
    if (!container) {
      previousOffsetsRef.current = new Map();
      previousHeightRef.current = null;
      return;
    }

    const rows = readSessionRows(container);
    const nextOffsets = new Map<string, number>();
    for (const [sessionId, row] of rows) {
      nextOffsets.set(sessionId, row.offsetTop);
    }

    const previousOffsets = previousOffsetsRef.current;
    previousOffsetsRef.current = nextOffsets;
    const previousHeight = previousHeightRef.current;
    const nextHeight = measureNaturalHeight(container);
    previousHeightRef.current = nextHeight;

    if (prefersReducedMotion()) {
      return;
    }

    const { hasRemovedRow, shifts, enteringSessionIds } = computeSessionRowRemovalTransition(
      previousOffsets,
      nextOffsets,
    );

    const scheduleListSettle = (): void => {
      if (listSettleTimerRef.current !== null) {
        window.clearTimeout(listSettleTimerRef.current);
      }
      listSettleTimerRef.current = window.setTimeout(() => {
        listSettleTimerRef.current = null;
        clearListCollapse(container);
      }, SESSION_ROW_SHIFT_DURATION_MS + 60);
    };

    if (hasRemovedRow && previousHeight !== null && previousHeight > nextHeight + MIN_SHIFT_PX) {
      // Shrink the list in step with the rows. Without this the sections below
      // it would close the gap a frame after the delete, while the rows above
      // are still sliding into it.
      container.style.transition = 'none';
      container.style.overflow = 'hidden';
      container.style.height = `${previousHeight}px`;
      requestAnimationFrame(() => {
        container.style.transition = `height ${SESSION_ROW_SHIFT_DURATION_MS}ms ${SESSION_ROW_SHIFT_EASING}`;
        container.style.height = `${nextHeight}px`;
      });
      scheduleListSettle();
    } else if (listSettleTimerRef.current !== null && container.style.height) {
      // A row landing mid-collapse (a background refill) would sit clipped
      // under the pinned height; retarget the running animation instead.
      container.style.height = `${nextHeight}px`;
      scheduleListSettle();
    }

    if (!hasRemovedRow) {
      return;
    }

    const settleTimers = settleTimersRef.current;
    const takeRow = (sessionId: string): HTMLElement | null => {
      const row = rows.get(sessionId);
      if (!row) {
        return null;
      }
      const pendingTimer = settleTimers.get(row);
      if (pendingTimer !== undefined) {
        window.clearTimeout(pendingTimer);
        settleTimers.delete(row);
      }
      return row;
    };

    const animatedRows: Array<{ row: HTMLElement; property: 'transform' | 'opacity' }> = [];
    for (const shift of shifts) {
      const row = takeRow(shift.sessionId);
      if (!row) {
        continue;
      }
      row.style.transition = 'none';
      row.style.transform = `translateY(${shift.fromOffsetY}px)`;
      animatedRows.push({ row, property: 'transform' });
    }
    for (const sessionId of enteringSessionIds) {
      const row = takeRow(sessionId);
      if (!row) {
        continue;
      }
      row.style.transition = 'none';
      row.style.opacity = '0';
      animatedRows.push({ row, property: 'opacity' });
    }
    if (animatedRows.length === 0) {
      return;
    }

    // Paint the starting values first, then release them so the browser has a
    // start and an end to interpolate between. Rows keep animating across later
    // renders (a background metadata refresh commits while the slide runs), so
    // the styles are cleaned up per row on a timer, not on effect teardown.
    requestAnimationFrame(() => {
      for (const { row, property } of animatedRows) {
        row.style.transition = `${property} ${SESSION_ROW_SHIFT_DURATION_MS}ms ${SESSION_ROW_SHIFT_EASING}`;
        if (property === 'transform') {
          row.style.transform = '';
        } else {
          row.style.opacity = '';
        }
      }
    });
    for (const { row } of animatedRows) {
      settleTimers.set(
        row,
        window.setTimeout(() => {
          settleTimers.delete(row);
          clearRowTransition(row);
        }, SESSION_ROW_SHIFT_DURATION_MS + 60),
      );
    }
  }, [listRef, rowSignature]);
}
