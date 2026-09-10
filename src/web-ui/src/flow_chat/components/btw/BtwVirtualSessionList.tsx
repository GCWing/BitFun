import { useEffect, useRef, useState, type RefObject } from 'react';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import { VirtualItemRenderer } from '../modern/VirtualItemRenderer';
import { useFlowChatVirtualizer } from '../modern/useFlowChatVirtualizer';
import type { FlowChatViewportOwnerApi } from '../modern/useFlowChatViewportOwner';
import { getVirtualItemStableKey } from '../modern/virtualItemIdentity';
import { estimateVirtualMessageItemHeightWithContext } from '../modern/virtualMessageListLayout';
import type { BtwPanelViewState } from './btwPanelViewState';
import { useBtwPanelViewport } from './useBtwPanelViewport';

interface BtwVirtualSessionListProps {
  items: VirtualItem[];
  scrollerRef: RefObject<HTMLDivElement | null>;
  headerRef: RefObject<HTMLDivElement | null>;
  followRef: RefObject<boolean>;
  viewportOwner: FlowChatViewportOwnerApi;
  exploreGroupStates: Map<string, boolean>;
  isHistorical: boolean;
  viewState?: BtwPanelViewState;
}

/** The embedded transcript shares row placement, not the primary session shell. */
export function BtwVirtualSessionList({
  items, scrollerRef, headerRef, followRef, viewportOwner,
  exploreGroupStates, isHistorical, viewState,
}: BtwVirtualSessionListProps) {
  const windowRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const updateWidth = () => setWidth(scroller.clientWidth);
    const observer = new ResizeObserver(updateWidth);
    observer.observe(scroller);
    updateWidth();
    return () => observer.disconnect();
  }, [scrollerRef]);
  const virtualizer = useFlowChatVirtualizer({
    items,
    scrollerRef,
    headerRef,
    getItemKey: getVirtualItemStableKey,
    estimateItemHeightPx: estimateVirtualMessageItemHeightWithContext,
    estimateContext: {
      availableWidthPx: width || undefined,
      exploreGroupStates,
      isHistorical,
    },
    estimateContextRevision: `${width}|${isHistorical}|${[...exploreGroupStates]
      .map(([id, expanded]) => `${id}:${expanded}`).join(',')}`,
    scrollPaddingStartPx: 0,
    writeViewport: viewportOwner.write,
    shiftViewport: viewportOwner.shift,
  });
  useBtwPanelViewport(viewState, items, scrollerRef, windowRef, virtualizer, viewportOwner);

  // Estimated offscreen rows change the scroll range as they mount. Follow
  // those measurements as well as streamed data, but recheck user intent in
  // the frame itself so a queued update cannot undo an upward gesture.
  useEffect(() => {
    const scroller = scrollerRef.current;
    const window = windowRef.current;
    if (!scroller || !window) return;
    let frame: number | undefined;
    const follow = () => {
      if (frame !== undefined) cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        frame = undefined;
        if (!followRef.current || scroller.clientHeight === 0) return;
        viewportOwner.write({ owner: 'follow-output', topPx: scroller.scrollHeight, holdForMs: 0 });
      });
    };
    const observer = new ResizeObserver(follow);
    observer.observe(window);
    observer.observe(scroller);
    follow();
    return () => {
      observer.disconnect();
      if (frame !== undefined) cancelAnimationFrame(frame);
    };
  }, [items, virtualizer.rows, scrollerRef, followRef, viewportOwner]);

  return (
    <div
      ref={windowRef}
      style={{ paddingTop: virtualizer.paddingTopPx, paddingBottom: virtualizer.paddingBottomPx }}
    >
      {virtualizer.rows.map(row => (
        <VirtualItemRenderer
          key={row.key}
          item={items[row.index]}
          index={row.index}
          measureRef={virtualizer.measureRowElement}
        />
      ))}
    </div>
  );
}
