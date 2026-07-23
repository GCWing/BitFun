/**
 * Hook to detect when the current turn's user message header has scrolled
 * out of view above the viewport, with hysteresis to avoid flickering.
 *
 * - Shows button only when user has scrolled up past the current turn's header
 *   by at least UPPER_HYSTERESIS_PX, and the viewport top is in an earlier turn.
 * - Hides button when the header comes back within LOWER_HYSTERESIS_PX of the
 *   viewport top, or the user has scrolled back to the current/newer turn.
 */

import { useRef, useCallback, useState, useEffect } from 'react';
import type { VisibleTurnInfo } from '../store/modernFlowChatStore';

const UPPER_HYSTERESIS_PX = 150;
const LOWER_HYSTERESIS_PX = 50;

interface UseScrollToTurnHeaderOptions {
  scrollerRef: React.RefObject<HTMLElement | null>;
  currentTurnId: string | null;
  currentTurnIndex: number;
  visibleTurnInfo: VisibleTurnInfo | null;
  onJumpToCurrentTurn: () => void;
}

interface UseScrollToTurnHeaderReturn {
  shouldShowButton: boolean;
  handleClick: () => void;
}

export function useScrollToTurnHeader(
  options: UseScrollToTurnHeaderOptions
): UseScrollToTurnHeaderReturn {
  const {
    scrollerRef,
    currentTurnId,
    currentTurnIndex,
    visibleTurnInfo,
    onJumpToCurrentTurn,
  } = options;

  const [shouldShow, setShouldShow] = useState(false);
  const lastVisibleStateRef = useRef(false);

  const checkShouldShow = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller || !currentTurnId || currentTurnIndex <= 0) {
      if (lastVisibleStateRef.current) {
        lastVisibleStateRef.current = false;
        setShouldShow(false);
      }
      return;
    }

    // Condition 1: viewport top must be in an earlier turn.
    const viewportTopIsEarlier = visibleTurnInfo != null && visibleTurnInfo.turnIndex < currentTurnIndex;
    if (!viewportTopIsEarlier) {
      if (lastVisibleStateRef.current) {
        lastVisibleStateRef.current = false;
        setShouldShow(false);
      }
      return;
    }

    // Find the current turn's user message element.
    const targetElement = scroller.querySelector<HTMLElement>(
      `.virtual-item-wrapper[data-item-type="user-message"][data-turn-id="${currentTurnId}"]`,
    );

    if (!targetElement) {
      // Element not rendered (outside overscan). Treat as "scrolled out above".
      if (!lastVisibleStateRef.current) {
        lastVisibleStateRef.current = true;
        setShouldShow(true);
      }
      return;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const targetRect = targetElement.getBoundingClientRect();
    const viewportTop = scrollerRect.top + 57; // Keep in sync with PINNED_TURN_VIEWPORT_OFFSET_PX.

    // Distance from target top to viewport top (positive = target is above viewport).
    const distanceAboveViewport = viewportTop - targetRect.top;

    if (lastVisibleStateRef.current) {
      // Currently showing: hide only when header comes back close to viewport.
      if (distanceAboveViewport < LOWER_HYSTERESIS_PX) {
        lastVisibleStateRef.current = false;
        setShouldShow(false);
      }
    } else {
      // Currently hidden: show only when scrolled up far enough.
      if (distanceAboveViewport > UPPER_HYSTERESIS_PX) {
        lastVisibleStateRef.current = true;
        setShouldShow(true);
      }
    }
  }, [scrollerRef, currentTurnId, currentTurnIndex, visibleTurnInfo]);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;

    let rafId: number | null = null;
    const throttledCheck = () => {
      if (rafId) return;
      rafId = requestAnimationFrame(() => {
        checkShouldShow();
        rafId = null;
      });
    };

    scroller.addEventListener('scroll', throttledCheck, { passive: true });
    // Initial check.
    checkShouldShow();

    return () => {
      scroller.removeEventListener('scroll', throttledCheck);
      if (rafId) cancelAnimationFrame(rafId);
    };
  }, [checkShouldShow, scrollerRef]);

  // Reset when turn/session changes.
  useEffect(() => {
    lastVisibleStateRef.current = false;
    setShouldShow(false);
  }, [currentTurnId]);

  const handleClick = useCallback(() => {
    onJumpToCurrentTurn();
    lastVisibleStateRef.current = false;
    setShouldShow(false);
  }, [onJumpToCurrentTurn]);

  return {
    shouldShowButton: shouldShow,
    handleClick,
  };
}
