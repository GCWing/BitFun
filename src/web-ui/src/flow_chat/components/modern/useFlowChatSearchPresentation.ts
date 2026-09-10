import { useLayoutEffect, useState } from 'react';
import { createFlowChatSearchHighlightOwner } from './flowChatSearchDom';
import {
  measureFlowChatSearchLine,
  resolveFlowChatSearchPresentation,
  type FlowChatSearchLineBox,
} from './flowChatSearchPresentation';
import type { SearchMatch } from './useFlowChatSearch';

function sameLine(a: FlowChatSearchLineBox | null, b: FlowChatSearchLineBox | null): boolean {
  return a === b || Boolean(a && b
    && a.top === b.top && a.left === b.left && a.width === b.width && a.height === b.height);
}

/** Passive row presentation. It never scrolls, expands content, or reserves space. */
export function useFlowChatSearchPresentation(
  wrapper: HTMLElement | null,
  query: string | undefined,
  matches: readonly SearchMatch[] | undefined,
  currentMatch: SearchMatch | undefined,
): FlowChatSearchLineBox | null {
  const [line, setLine] = useState<FlowChatSearchLineBox | null>(null);

  useLayoutEffect(() => {
    if (!wrapper || !query?.trim() || !matches?.length) {
      setLine(previous => previous === null ? previous : null);
      return;
    }
    const view = wrapper.ownerDocument.defaultView;
    if (!view) return;
    const owner = createFlowChatSearchHighlightOwner(wrapper.ownerDocument);
    let frame: number | null = null;
    const refresh = () => {
      frame = null;
      const { currentRange, currentRoot, otherRanges } = resolveFlowChatSearchPresentation(
        wrapper, query, matches, currentMatch,
      );
      owner.update(currentRange, otherRanges);
      const next = currentRange && currentRoot
        ? measureFlowChatSearchLine(wrapper, currentRoot, currentRange)
        : null;
      setLine(previous => sameLine(previous, next) ? previous : next);
    };
    const scheduleRefresh = () => {
      if (frame === null) frame = view.requestAnimationFrame(refresh);
    };
    const observer = new MutationObserver(records => {
      // Our marker's geometry must not trigger another presentation update.
      if (records.some(record => !(record.target instanceof Element)
        || !record.target.closest('[data-openbitfun-part="searchLine"]'))) scheduleRefresh();
    });
    observer.observe(wrapper, {
      childList: true,
      characterData: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['class', 'style', 'hidden', 'aria-hidden', 'open', 'data-expanded'],
    });
    const resize = new ResizeObserver(scheduleRefresh);
    resize.observe(wrapper);
    wrapper.addEventListener('scroll', scheduleRefresh, { capture: true, passive: true });
    wrapper.ownerDocument.fonts?.addEventListener('loadingdone', scheduleRefresh);
    refresh();

    return () => {
      if (frame !== null) view.cancelAnimationFrame(frame);
      observer.disconnect();
      resize.disconnect();
      wrapper.removeEventListener('scroll', scheduleRefresh, true);
      wrapper.ownerDocument.fonts?.removeEventListener('loadingdone', scheduleRefresh);
      owner.dispose();
    };
  }, [wrapper, query, matches, currentMatch]);

  return currentMatch ? line : null;
}
