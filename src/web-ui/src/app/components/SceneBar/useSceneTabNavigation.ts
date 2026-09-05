import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type WheelEvent,
} from 'react';
import type { InteractionMotion } from '@/shared/utils/motionPreference';
import type { SceneTabId } from './types';

const SCROLL_EDGE_EPSILON = 1;
const MIN_SCROLL_STEP = 160;

interface TabScrollState {
  hasOverflow: boolean;
  canScrollBackward: boolean;
  canScrollForward: boolean;
}

interface UseSceneTabNavigationOptions {
  activeTabId: SceneTabId | null;
  navigationMotion: InteractionMotion;
  openTabIds: readonly SceneTabId[];
}

const INITIAL_TAB_SCROLL_STATE: TabScrollState = {
  hasOverflow: false,
  canScrollBackward: false,
  canScrollForward: false,
};

function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined'
    && typeof window.matchMedia === 'function'
    && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/**
 * Owns tab-strip overflow and keyboard behavior without leaking those layout
 * details into the scene lifecycle store.
 */
export function useSceneTabNavigation({
  activeTabId,
  navigationMotion,
  openTabIds,
}: UseSceneTabNavigationOptions) {
  const tabOrderKey = openTabIds.join('\u0000');
  const tabRegionRef = useRef<HTMLDivElement>(null);
  const tabsRef = useRef<HTMLDivElement>(null);
  const [scrollState, setScrollState] = useState<TabScrollState>(INITIAL_TAB_SCROLL_STATE);

  const getTabElements = useCallback((): HTMLElement[] => {
    const tabs = tabsRef.current;
    if (!tabs) return [];
    return Array.from(tabs.querySelectorAll<HTMLElement>('[role="tab"]'));
  }, []);

  const updateScrollState = useCallback(() => {
    const tabs = tabsRef.current;
    const region = tabRegionRef.current;
    if (!tabs || !region) return;

    const maxScrollLeft = Math.max(0, tabs.scrollWidth - tabs.clientWidth);
    const hasOverflow = region.clientWidth > 0
      && tabs.scrollWidth > region.clientWidth + SCROLL_EDGE_EPSILON;
    const nextState: TabScrollState = {
      hasOverflow,
      canScrollBackward: hasOverflow && tabs.scrollLeft > SCROLL_EDGE_EPSILON,
      canScrollForward: hasOverflow
        && tabs.scrollLeft < maxScrollLeft - SCROLL_EDGE_EPSILON,
    };

    setScrollState((current) => (
      current.hasOverflow === nextState.hasOverflow
        && current.canScrollBackward === nextState.canScrollBackward
        && current.canScrollForward === nextState.canScrollForward
        ? current
        : nextState
    ));
  }, []);

  const scrollTo = useCallback((left: number, behavior: ScrollBehavior) => {
    const tabs = tabsRef.current;
    if (!tabs) return;

    const maxScrollLeft = Math.max(0, tabs.scrollWidth - tabs.clientWidth);
    const nextLeft = Math.max(0, Math.min(left, maxScrollLeft));
    if (Math.abs(nextLeft - tabs.scrollLeft) <= SCROLL_EDGE_EPSILON) return;

    if (typeof tabs.scrollTo === 'function') {
      tabs.scrollTo({ left: nextLeft, behavior });
    } else {
      tabs.scrollLeft = nextLeft;
      updateScrollState();
    }
  }, [updateScrollState]);

  useLayoutEffect(() => {
    updateScrollState();

    if (typeof ResizeObserver === 'undefined') return;
    const region = tabRegionRef.current;
    const tabs = tabsRef.current;
    if (!region || !tabs) return;

    const observer = new ResizeObserver(updateScrollState);
    observer.observe(region);
    observer.observe(tabs);
    getTabElements().forEach((tab) => {
      observer.observe(tab.closest<HTMLElement>('[data-openbitfun-part="item"]') ?? tab);
    });
    return () => observer.disconnect();
  }, [getTabElements, tabOrderKey, updateScrollState]);

  useLayoutEffect(() => {
    const tabs = tabsRef.current;
    const activeTab = getTabElements().find(tab => tab.dataset.openbitfunValue === activeTabId);
    const activeItem = activeTab?.closest<HTMLElement>('[data-openbitfun-part="item"]') ?? activeTab;
    if (!tabs || !activeItem) return;

    const visibleStart = tabs.scrollLeft;
    const visibleEnd = visibleStart + tabs.clientWidth;
    const tabStart = activeItem.offsetLeft;
    const tabEnd = tabStart + activeItem.offsetWidth;
    let nextLeft = visibleStart;

    if (tabStart < visibleStart) {
      nextLeft = tabStart;
    } else if (tabEnd > visibleEnd) {
      nextLeft = tabEnd - tabs.clientWidth;
    }

    const behavior: ScrollBehavior = navigationMotion === 'pointer' && !prefersReducedMotion()
      ? 'smooth'
      : 'auto';
    scrollTo(nextLeft, behavior);
    updateScrollState();
  }, [activeTabId, getTabElements, navigationMotion, scrollTo, tabOrderKey, updateScrollState]);

  const handleScroll = useCallback(() => {
    updateScrollState();
  }, [updateScrollState]);

  const handleWheel = useCallback((event: WheelEvent<HTMLDivElement>) => {
    const tabs = tabsRef.current;
    if (!tabs || !scrollState.hasOverflow) return;

    // Preserve native horizontal trackpad movement. A vertical mouse wheel is
    // translated only while the pointer is over the tab strip.
    if (Math.abs(event.deltaX) >= Math.abs(event.deltaY)) return;
    const maxScrollLeft = Math.max(0, tabs.scrollWidth - tabs.clientWidth);
    const nextLeft = Math.max(0, Math.min(tabs.scrollLeft + event.deltaY, maxScrollLeft));
    if (Math.abs(nextLeft - tabs.scrollLeft) <= SCROLL_EDGE_EPSILON) return;

    event.preventDefault();
    tabs.scrollLeft = nextLeft;
    updateScrollState();
  }, [scrollState.hasOverflow, updateScrollState]);

  const scrollByPage = useCallback((direction: -1 | 1) => {
    const tabs = tabsRef.current;
    if (!tabs) return;
    const step = Math.max(MIN_SCROLL_STEP, tabs.clientWidth * 0.72);
    scrollTo(
      tabs.scrollLeft + direction * step,
      prefersReducedMotion() ? 'auto' : 'smooth',
    );
  }, [scrollTo]);

  return {
    tabRegionRef,
    tabsRef,
    scrollState,
    handleScroll,
    handleWheel,
    scrollByPage,
  };
}
