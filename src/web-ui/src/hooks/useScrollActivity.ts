import { useEffect, useCallback, RefObject } from 'react';

interface UseScrollActivityOptions {
  /** Delay before removing class after scroll stops (ms), default 1000 */
  timeout?: number;
  /** Custom class name, default 'is-scrolling' */
  className?: string;
  enabled?: boolean;
}

/**
 * Adds a class to the element while scrolling, removes it after scroll stops.
 * Works with hover-scrollbar-with-activity mixin.
 */
export function useScrollActivity(
  ref: RefObject<HTMLElement>,
  options: UseScrollActivityOptions = {}
): void {
  const {
    timeout = 1000,
    className = 'is-scrolling',
    enabled = true,
  } = options;

  const handleScroll = useCallback(() => {
    const element = ref.current;
    if (!element) return;

    if (!element.classList.contains(className)) {
      element.classList.add(className);
    }

    const existingTimer = element.dataset.scrollTimer;
    if (existingTimer) {
      clearTimeout(parseInt(existingTimer, 10));
    }

    const timer = setTimeout(() => {
      element.classList.remove(className);
      delete element.dataset.scrollTimer;
    }, timeout);

    element.dataset.scrollTimer = String(timer);
  }, [ref, timeout, className]);

  useEffect(() => {
    const element = ref.current;
    if (!element || !enabled) return;

    element.addEventListener('scroll', handleScroll, { passive: true });

    return () => {
      element.removeEventListener('scroll', handleScroll);
      
      const existingTimer = element.dataset.scrollTimer;
      if (existingTimer) {
        clearTimeout(parseInt(existingTimer, 10));
        delete element.dataset.scrollTimer;
      }
      
      element.classList.remove(className);
    };
  }, [ref, enabled, handleScroll, className]);
}

export default useScrollActivity;
