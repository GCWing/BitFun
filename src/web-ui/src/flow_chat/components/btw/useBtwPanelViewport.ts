import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import type { FlowChatVirtualizer } from '../modern/useFlowChatVirtualizer';
import type { FlowChatViewportOwnerApi } from '../modern/useFlowChatViewportOwner';
import { getVirtualItemStableKey } from '../modern/virtualItemIdentity';
import type { BtwPanelViewState } from './btwPanelViewState';

export function useBtwPanelViewport(
  viewState: BtwPanelViewState | undefined,
  items: VirtualItem[],
  scrollerRef: RefObject<HTMLDivElement | null>,
  windowRef: RefObject<HTMLDivElement | null>,
  virtualizer: FlowChatVirtualizer,
  viewportOwner: FlowChatViewportOwnerApi,
) {
  const restoreRef = useRef(viewState && !viewState.followTail ? viewState.anchor : null);

  useLayoutEffect(() => {
    const anchor = restoreRef.current;
    const scroller = scrollerRef.current;
    if (!anchor || !scroller || !scroller.clientHeight || !viewState) return;
    if (!viewState.restoring) {
      restoreRef.current = null;
      return;
    }
    const index = items.findIndex(item => getVirtualItemStableKey(item) === anchor.key);
    if (index < 0) {
      restoreRef.current = null;
      viewState.restoring = false;
      viewState.anchor = null;
      return;
    }
    // First place the estimated row into the window. Once mounted, use its
    // actual position, including the saved offset inside a tall model round.
    const row = [...(windowRef.current?.children ?? [])].find(element =>
      element.getAttribute('data-virtual-item-key') === anchor.key,
    );
    const bounds = virtualizer.getItemBounds(index);
    if (!bounds) return;
    const offset = row
      ? Math.min(anchor.offsetPx, Math.max(0, row.getBoundingClientRect().height - 1))
      : anchor.offsetPx;
    const topPx = row
      ? scroller.scrollTop + row.getBoundingClientRect().top
        - scroller.getBoundingClientRect().top - scroller.clientTop + offset
      : bounds.startPx + offset;
    if (viewportOwner.write({ owner: 'one-shot-navigation', topPx, holdForMs: 0 }) && row) {
      restoreRef.current = null;
      viewState.restoring = false;
    }
  }, [items, scrollerRef, windowRef, virtualizer, viewportOwner, viewState]);

  useEffect(() => {
    const scroller = scrollerRef.current;
    const window = windowRef.current;
    if (!scroller || !window || !viewState) return;
    const capture = () => {
      if (viewState.restoring || !scroller.clientHeight) return;
      if (viewState.followTail) {
        viewState.anchor = null;
        return;
      }
      const top = scroller.getBoundingClientRect().top + scroller.clientTop;
      const row = [...window.children].find(element => element.getBoundingClientRect().bottom > top);
      const key = row?.getAttribute('data-virtual-item-key');
      if (row && key) viewState.anchor = { key, offsetPx: top - row.getBoundingClientRect().top };
    };
    const interrupt = () => {
      restoreRef.current = null;
      viewState.restoring = false;
    };
    scroller.addEventListener('scroll', capture);
    scroller.addEventListener('wheel', interrupt, { passive: true });
    scroller.addEventListener('touchstart', interrupt, { passive: true });
    scroller.addEventListener('pointerdown', interrupt);
    scroller.addEventListener('keydown', interrupt);
    const observer = new ResizeObserver(capture);
    observer.observe(window);
    return () => {
      observer.disconnect();
      scroller.removeEventListener('scroll', capture);
      scroller.removeEventListener('wheel', interrupt);
      scroller.removeEventListener('touchstart', interrupt);
      scroller.removeEventListener('pointerdown', interrupt);
      scroller.removeEventListener('keydown', interrupt);
    };
  }, [scrollerRef, windowRef, viewState]);
}
