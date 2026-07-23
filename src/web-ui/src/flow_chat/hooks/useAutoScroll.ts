/**
 * Auto-scroll hook for chat messages.
 * - Pause auto-scroll when the user scrolls up
 * - Provide the "scroll to bottom" button state
 */

import { useRef, useCallback, useState, useEffect, RefObject } from 'react';

interface UseAutoScrollOptions {
  enabled?: boolean;
  threshold?: number;
  dependencies?: any[];
}

interface UseAutoScrollReturn {
  messagesEndRef: RefObject<HTMLDivElement | null>;
  messagesContainerRef: RefObject<HTMLDivElement | null>;
  isUserScrolledUp: boolean;
  scrollToBottom: (force?: boolean) => void; // Force ignores user position.
  shouldShowScrollButton: boolean;
}

export const useAutoScroll = (
  options: UseAutoScrollOptions = {}
): UseAutoScrollReturn => {
  const {
    enabled = true,
    threshold = 100,
    dependencies = []
  } = options;

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const [isUserScrolledUp, setIsUserScrolledUp] = useState(false);

  const checkUserScrollPosition = useCallback(() => {
    if (!messagesContainerRef.current) return;

    const container = messagesContainerRef.current;
    const distanceFromBottom = 
      container.scrollHeight - container.scrollTop - container.clientHeight;
    
    const isNearBottom = distanceFromBottom < threshold;
    setIsUserScrolledUp(!isNearBottom);
  }, [threshold]);

  const scrollToBottom = useCallback((force = false) => {
    if (!messagesEndRef.current) return;
    
    // Force scroll, or auto-scroll when user is at the bottom.
    if (force || (enabled && !isUserScrolledUp)) {
      messagesEndRef.current.scrollIntoView({ behavior: 'smooth' });
      
      // Reset the user-scrolled-up state on force.
      if (force) {
        setIsUserScrolledUp(false);
      }
    }
  }, [enabled, isUserScrolledUp]);

  useEffect(() => {
    const container = messagesContainerRef.current;
    if (!container) return;

    // Throttle using rAF to reduce callback frequency.
    let rafId: number | null = null;
    const throttledCheck = () => {
      if (rafId) return;
      rafId = requestAnimationFrame(() => {
        checkUserScrollPosition();
        rafId = null;
      });
    };

    container.addEventListener('scroll', throttledCheck, { passive: true });
    return () => {
      container.removeEventListener('scroll', throttledCheck);
      if (rafId) cancelAnimationFrame(rafId);
    };
  }, [checkUserScrollPosition]);

  useEffect(() => {
    scrollToBottom();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, dependencies);

  const shouldShowScrollButton = isUserScrolledUp;

  return {
    messagesEndRef,
    messagesContainerRef,
    isUserScrolledUp,
    scrollToBottom,
    shouldShowScrollButton
  };
};

