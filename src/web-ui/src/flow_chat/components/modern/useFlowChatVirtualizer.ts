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

/** Item-count overscan. Roughly two Turns either side of the viewport. */
const FLOW_CHAT_OVERSCAN_ITEMS = 6;

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
   * Scroll so that an item lands at the viewport's start or centre.
   *
   * Preferred over an offset wherever it fits: the virtualizer re-aims at the
   * item for as long as the measurements under it keep moving, which an offset
   * cannot do because the offset is already stale by then.
   */
  scrollItemIntoView: (
    index: number,
    options: { align: 'start' | 'center'; behavior?: 'auto' | 'smooth' },
  ) => void;
  /** Scroll to an offset in scroller coordinates. */
  scrollToOffset: (offsetPx: number, behavior?: 'auto' | 'smooth') => void;
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
}: UseFlowChatVirtualizerOptions<T>): FlowChatVirtualizer {
  const itemsRef = useRef(items);
  itemsRef.current = items;
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
  });

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

  const getItemBounds = useCallback((index: number): FlowChatItemBounds | null => {
    const measurement = virtualizer.measurementsCache[index];
    return measurement ? { startPx: measurement.start, endPx: measurement.end } : null;
  }, [virtualizer]);

  const scrollItemIntoView = useCallback((
    index: number,
    options: { align: 'start' | 'center'; behavior?: 'auto' | 'smooth' },
  ) => {
    virtualizer.scrollToIndex(index, {
      align: options.align,
      behavior: options.behavior ?? 'auto',
    });
  }, [virtualizer]);

  const scrollToOffset = useCallback((offsetPx: number, behavior: 'auto' | 'smooth' = 'auto') => {
    virtualizer.scrollToOffset(offsetPx, { align: 'start', behavior });
  }, [virtualizer]);

  return {
    rows,
    paddingTopPx,
    paddingBottomPx,
    measureRowElement,
    getItemBounds,
    scrollItemIntoView,
    scrollToOffset,
  };
}
