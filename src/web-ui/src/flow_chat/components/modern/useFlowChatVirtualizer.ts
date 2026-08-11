/**
 * FlowChat's virtualizer.
 *
 * Everything that knows which library places the items lives here. The rest of
 * FlowChat asks for offsets in scroller coordinates and gets them back; it never
 * sees an index space of the virtualizer's own, because there isn't one —
 * measurements are cached against item keys, so a history prepend leaves every
 * measured item exactly where it was.
 *
 * The reason this is TanStack Virtual and not react-virtuoso is one line of its
 * measurement pass:
 *
 *     const size = measured ?? this.options.estimateSize(i)
 *
 * A per-item estimate for everything unmeasured. react-virtuoso reserves a
 * single scalar for all of them, and this transcript alternates 38px user
 * messages with model rounds up to 5012px, so the scroll range was wrong by an
 * order of magnitude until an item was actually measured. Scrolling into a
 * freshly paged block then forced a burst of measurement — stalls of 119ms and
 * 295ms with no animation frame at all — and a correction of up to 11705px.
 *
 * Items are laid out in normal flow inside a padded window, not absolutely
 * positioned. When an item inside the window changes height the browser reflows
 * the ones below it in the same layout pass, so there is no frame where the
 * scroll has been corrected but the items have not moved yet.
 */

import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import type { FlowChatViewportOwner } from './flowChatViewportOwnership';

/** Item-count overscan. Roughly two Turns either side of the viewport. */
const FLOW_CHAT_OVERSCAN_ITEMS = 6;

/**
 * The virtualizer does not compensate for its own late measurements.
 *
 * Its rule is the right shape — adjust by *this item's* delta, and only for an
 * item above the viewport — but it applies that delta to `scrollOffset`, which
 * is the library's own copy of the scroll position and is refreshed only from
 * scroll events. Every continuous writer here assigns `scrollTop` directly, and
 * the scroll event for that assignment does not land until the following frame,
 * so a measurement arriving in between is compensated from a position the
 * viewport has already left. Measured on session open: nine corrections across
 * two frames walked the viewport from 7440 back to 3556 before the follow loop
 * wrote 7440 again.
 *
 * The viewport anchor is the compensator instead. It restores a relationship
 * rather than replaying a delta, so it has no base to go stale.
 */
const neverAdjustScrollPositionOnItemResize = () => false;

export interface FlowChatVirtualRow {
  index: number;
  key: string;
  startPx: number;
  endPx: number;
}

export interface FlowChatItemBounds {
  startPx: number;
  endPx: number;
}

/**
 * The measurement pass, which the published types keep to themselves.
 *
 * `measurementsCache` is a public field, but it is only the place the memo
 * behind `getMeasurements` leaves its answer — reading it returns whatever the
 * last render put there, and a measurement arriving since then has invalidated
 * the memo without re-running it. `getMeasurements` is `private` in the `.d.ts`
 * although it is a plain instance field at runtime, so it is reached through
 * this shape rather than through some other public call whose side effect
 * happens to refresh the field.
 */
interface MeasuringVirtualizer {
  getMeasurements: () => ReadonlyArray<{ start: number; end: number }>;
}

export interface UseFlowChatVirtualizerOptions<T> {
  items: readonly T[];
  scrollerRef: RefObject<HTMLElement | null>;
  /**
   * Content rendered above the items, inside the same scroller.
   *
   * Its height is the offset every item start is expressed against, so it is
   * measured rather than assumed: the history paging sentinel appears and
   * disappears inside it while the reader is scrolling.
   */
  headerRef: RefObject<HTMLElement | null>;
  getItemKey: (item: T) => string;
  estimateItemHeightPx: (item: T) => number;
  /**
   * Gap kept above a Turn that has been scrolled to the top of the viewport.
   * Applied by the virtualizer itself so that its re-aim, which runs while
   * items below the target are still measuring, keeps the same gap.
   */
  scrollPaddingStartPx: number;
  /**
   * Every write the library makes goes through this.
   *
   * `scrollToFn` is a first-class option, so `scrollToIndex`, `scrollToOffset`
   * and the re-aim that follows them all arrive here — which is what makes
   * "nothing moves the viewport unregistered" true rather than aspirational.
   * The library's writes are a continuation of whoever asked for the aim, so
   * they are attributed to that owner and not to the library.
   */
  writeViewport: (request: {
    owner: FlowChatViewportOwner;
    topPx: number;
    behavior?: ScrollBehavior;
    holdForMs?: number;
  }) => boolean;
}

export interface FlowChatVirtualizer {
  /** The window of items to render, in order. */
  rows: FlowChatVirtualRow[];
  /** Space standing in for the items above the window. */
  paddingTopPx: number;
  /** Space standing in for the items below the window. */
  paddingBottomPx: number;
  /** Ref callback for a rendered row's outermost element. */
  measureRowElement: (element: HTMLElement | null) => void;
  /** Where an item sits in scroller coordinates, or null if it has no place yet. */
  getItemBounds: (index: number) => FlowChatItemBounds | null;
  /**
   * Bring the measurement cache level with the DOM for every rendered row.
   *
   * For callers that have to read a position in the same commit that changed
   * the items, before the library's own measurement has caught up.
   */
  measureRenderedItems: () => void;
  /**
   * The items intersecting the viewport right now, read from live geometry.
   *
   * A callback rather than a value because it has to answer during a scroll,
   * which moves the viewport across the rendered window without changing it.
   */
  getVisibleItemRange: () => FlowChatVisibleItemRange | null;
  /**
   * Scroll so that an item lands at the viewport's start or centre.
   *
   * Preferred over an offset wherever it fits: the virtualizer re-aims at the
   * item for as long as the measurements under it keep moving, which an offset
   * cannot do because the offset is already stale by then.
   */
  scrollItemIntoView: (
    index: number,
    options: {
      align: 'start' | 'center';
      behavior?: 'auto' | 'smooth';
      /** Who the aim, and every re-aim it produces, belongs to. */
      owner: FlowChatViewportOwner;
      holdForMs?: number;
    },
  ) => void;
  /** Scroll to an offset in scroller coordinates. */
  scrollToOffset: (
    offsetPx: number,
    options: {
      behavior?: 'auto' | 'smooth';
      owner: FlowChatViewportOwner;
      holdForMs?: number;
    },
  ) => void;
}

export interface FlowChatVisibleItemRange {
  startIndex: number;
  endIndex: number;
}

/**
 * The items the reader can actually see.
 *
 * Deliberately not the rendered window: that carries overscan, and a transcript
 * short enough to render whole reports the first and last item as rendered no
 * matter where the viewport is. Anything asking "has the reader reached the end
 * of what is loaded" needs this instead, or it is asking whether the item
 * exists.
 *
 * Null when no item intersects the viewport at all, which is a real state — the
 * reserved tail blank is below every item — and the honest answer is that the
 * reader is at neither boundary.
 */
export function visibleRowRange(
  rows: readonly FlowChatVirtualRow[],
  scrollTopPx: number,
  clientHeightPx: number,
): FlowChatVisibleItemRange | null {
  const viewportEndPx = scrollTopPx + clientHeightPx;
  let startIndex = -1;
  let endIndex = -1;
  for (const row of rows) {
    if (row.endPx <= scrollTopPx || row.startPx >= viewportEndPx) continue;
    if (startIndex === -1) startIndex = row.index;
    endIndex = row.index;
  }
  return startIndex === -1 ? null : { startIndex, endIndex };
}

export interface FlowChatVirtualWindowPadding {
  paddingTopPx: number;
  paddingBottomPx: number;
}

/**
 * The space that stands in for the transcript outside the rendered window.
 *
 * The scroll range has to be the whole transcript's even though only a window
 * of it exists, and this is what makes up the difference. Offsets arrive in
 * scroller coordinates — measured from the top of the scroller, past whatever
 * is rendered above the items — so `contentStartPx` comes back off both ends:
 * padding is measured inside the element that holds the items.
 *
 * An empty window reserves nothing. There is no window to be outside of, and a
 * padding derived from one row that is not there would be the whole transcript
 * on one side and nothing on the other.
 */
export function virtualWindowPaddingPx(
  rows: readonly FlowChatVirtualRow[],
  totalSizePx: number,
  contentStartPx: number,
): FlowChatVirtualWindowPadding {
  const first = rows[0];
  const last = rows[rows.length - 1];
  if (!first || !last) return { paddingTopPx: 0, paddingBottomPx: 0 };
  return {
    paddingTopPx: Math.max(0, first.startPx - contentStartPx),
    paddingBottomPx: Math.max(0, totalSizePx - (last.endPx - contentStartPx)),
  };
}

export function useFlowChatVirtualizer<T>({
  items,
  scrollerRef,
  headerRef,
  getItemKey,
  estimateItemHeightPx,
  scrollPaddingStartPx,
  writeViewport,
}: UseFlowChatVirtualizerOptions<T>): FlowChatVirtualizer {
  const itemsRef = useRef(items);
  itemsRef.current = items;
  const writeViewportRef = useRef(writeViewport);
  writeViewportRef.current = writeViewport;
  /**
   * Who the aim currently in flight belongs to.
   *
   * The library re-aims for as long as the measurements under its target keep
   * moving, and those later writes arrive with no caller of ours on the stack.
   * They are still the same request, so they are attributed to whoever made
   * it — which is also what lets a gesture preempt a navigation that is still
   * chasing its Turn.
   *
   * Until anyone has asked for an aim, the library still writes: it places the
   * scroller against its own state on mount. That write belongs to no request
   * of ours, so it is attributed to the lowest authority that describes it —
   * the library putting content where it thinks it goes. Naming a navigation
   * here instead let a 0px write on session open outrank follow-output for the
   * whole of the opening reveal.
   */
  const aimOwnerRef = useRef<FlowChatViewportOwner>('layout-correction');
  const aimHoldForMsRef = useRef<number | undefined>(undefined);
  const getItemKeyRef = useRef(getItemKey);
  getItemKeyRef.current = getItemKey;
  const estimateItemHeightRef = useRef(estimateItemHeightPx);
  estimateItemHeightRef.current = estimateItemHeightPx;

  const [contentStartPx, setContentStartPx] = useState(0);
  useEffect(() => {
    const header = headerRef.current;
    if (!header) return;
    const observer = new ResizeObserver(() => {
      setContentStartPx(header.offsetHeight);
    });
    observer.observe(header, { box: 'border-box' });
    setContentStartPx(header.offsetHeight);
    return () => observer.disconnect();
  }, [headerRef]);

  /*
   * Stable across renders on purpose. The virtualizer keys its measurement pass
   * on the identity of these, so a fresh closure per render would re-measure
   * the whole transcript every time a single streaming item changed. They read
   * the current items through a ref instead, which is sound because a change
   * that matters — an item added or removed — moves `count` as well.
   */
  const estimateSize = useCallback((index: number) => {
    const item = itemsRef.current[index];
    return item === undefined ? 0 : estimateItemHeightRef.current(item);
  }, []);

  const resolveItemKey = useCallback((index: number) => {
    const item = itemsRef.current[index];
    return item === undefined ? index : getItemKeyRef.current(item);
  }, []);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollerRef.current,
    estimateSize,
    getItemKey: resolveItemKey,
    // Items carry their own index attribute already; measuring reads it back.
    indexAttribute: 'data-virtual-index',
    overscan: FLOW_CHAT_OVERSCAN_ITEMS,
    scrollMargin: contentStartPx,
    scrollPaddingStart: scrollPaddingStartPx,
    /*
     * The library's only way out to the DOM. Routing it through the register
     * is what closes the gap the contract used to describe as unclosable: a
     * write from inside the library is now a write like any other, refusable
     * by whatever outranks the aim it came from.
     */
    scrollToFn: (offsetPx, { behavior }) => {
      writeViewportRef.current({
        owner: aimOwnerRef.current,
        topPx: offsetPx,
        behavior,
        holdForMs: aimHoldForMsRef.current,
      });
    },
  });
  // An instance field rather than an option, so it is assigned here — before
  // any measurement callback can reach `resizeItem`.
  virtualizer.shouldAdjustScrollPositionOnItemSizeChange = neverAdjustScrollPositionOnItemResize;

  const virtualRows = virtualizer.getVirtualItems();
  const totalSizePx = virtualizer.getTotalSize();

  const rows = useMemo<FlowChatVirtualRow[]>(() => virtualRows.map(row => ({
    index: row.index,
    key: String(row.key),
    startPx: row.start,
    endPx: row.end,
  })), [virtualRows]);

  const { paddingTopPx, paddingBottomPx } = virtualWindowPaddingPx(
    rows,
    totalSizePx,
    contentStartPx,
  );

  const measureRowElement = virtualizer.measureElement;

  const rowsRef = useRef(rows);
  rowsRef.current = rows;

  const getVisibleItemRange = useCallback((): FlowChatVisibleItemRange | null => {
    const scroller = scrollerRef.current;
    if (!scroller) return null;
    return visibleRowRange(rowsRef.current, scroller.scrollTop, scroller.clientHeight);
  }, [scrollerRef]);

  /**
   * Everything rendered, measured now rather than whenever the library gets to it.
   *
   * The library measures a row inline as its ref attaches, but only when the
   * reader is holding still:
   *
   *     if ((!this.isScrolling || this.scrollState) && ...) this.resizeItem(...)
   *
   * History pages in exactly when they are not: the reader scrolls up into the
   * boundary, the rows arrive in the DOM at their real heights, and the cache
   * keeps the estimates it reserved for them. The ResizeObserver does catch
   * them — after the layout effects of the commit that added them, which is a
   * frame later than the one reader of these positions that cannot wait. The
   * prepend compensation has to move the reader in the same paint as the rows
   * that displaced them. Measured across four junctions, height reserved
   * against scroll range actually gained: 2174/670, 2494/949, 2346/609,
   * 2580/1917.
   *
   * Rendered rows only, because only they have a height to read; the rest stay
   * estimates, which is what a virtualizer is. A row whose height has not
   * changed costs nothing — `resizeItem` returns on a zero delta — so this is
   * the same work the ResizeObserver was going to do, done a frame earlier.
   */
  const measureRenderedItems = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const elements = scroller.querySelectorAll<HTMLElement>('[data-virtual-index]');
    for (const element of elements) {
      const index = Number(element.getAttribute('data-virtual-index'));
      if (!Number.isInteger(index)) continue;
      virtualizer.resizeItem(
        index,
        virtualizer.options.measureElement(element, undefined, virtualizer),
      );
    }
  }, [scrollerRef, virtualizer]);

  const getItemBounds = useCallback((index: number): FlowChatItemBounds | null => {
    const measurement = (virtualizer as unknown as MeasuringVirtualizer)
      .getMeasurements()[index];
    return measurement ? { startPx: measurement.start, endPx: measurement.end } : null;
  }, [virtualizer]);

  const scrollItemIntoView = useCallback((
    index: number,
    options: {
      align: 'start' | 'center';
      behavior?: 'auto' | 'smooth';
      owner: FlowChatViewportOwner;
      holdForMs?: number;
    },
  ) => {
    aimOwnerRef.current = options.owner;
    aimHoldForMsRef.current = options.holdForMs;
    virtualizer.scrollToIndex(index, {
      align: options.align,
      behavior: options.behavior ?? 'auto',
    });
  }, [virtualizer]);

  const scrollToOffset = useCallback((
    offsetPx: number,
    options: {
      behavior?: 'auto' | 'smooth';
      owner: FlowChatViewportOwner;
      holdForMs?: number;
    },
  ) => {
    aimOwnerRef.current = options.owner;
    aimHoldForMsRef.current = options.holdForMs;
    virtualizer.scrollToOffset(offsetPx, {
      align: 'start',
      behavior: options.behavior ?? 'auto',
    });
  }, [virtualizer]);

  return {
    rows,
    paddingTopPx,
    paddingBottomPx,
    measureRowElement,
    getItemBounds,
    measureRenderedItems,
    getVisibleItemRange,
    scrollItemIntoView,
    scrollToOffset,
  };
}
