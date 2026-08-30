import {
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
  type RefObject,
} from 'react';

export type SideAnchoredPopoverPlacement = 'left' | 'right';
export type SideAnchoredPopoverAlignment = 'start' | 'end';

export interface SideAnchoredPopoverLayout {
  top: number;
  left: number;
  placement: SideAnchoredPopoverPlacement;
  alignment: SideAnchoredPopoverAlignment;
}

interface UseSideAnchoredPopoverPositionOptions {
  open: boolean;
  anchorRef: RefObject<HTMLElement | null>;
  popoverRef: RefObject<HTMLElement | null>;
  preferredPlacement?: SideAnchoredPopoverPlacement;
  gap?: number;
  padding?: number;
  layoutRevision?: unknown;
}

const sameLayout = (
  current: SideAnchoredPopoverLayout | null,
  next: SideAnchoredPopoverLayout,
): boolean => current !== null
  && current.top === next.top
  && current.left === next.left
  && current.placement === next.placement
  && current.alignment === next.alignment;

/** Keeps a portalled submenu beside its owning menu item without viewport clipping. */
export function useSideAnchoredPopoverPosition({
  open,
  anchorRef,
  popoverRef,
  preferredPlacement = 'right',
  gap = 6,
  padding = 8,
  layoutRevision,
}: UseSideAnchoredPopoverPositionOptions): SideAnchoredPopoverLayout | null {
  const [layout, setLayout] = useState<SideAnchoredPopoverLayout | null>(null);
  const frameRef = useRef<number | null>(null);

  const updatePosition = useCallback(() => {
    const anchor = anchorRef.current;
    const popover = popoverRef.current;
    if (!anchor || !popover || typeof window === 'undefined') return;

    const anchorBounds = anchor.getBoundingClientRect();
    const popoverBounds = popover.getBoundingClientRect();
    const popoverWidth = popoverBounds.width || popover.offsetWidth || popover.scrollWidth;
    const popoverHeight = popoverBounds.height || popover.offsetHeight || popover.scrollHeight;
    const rightLeft = anchorBounds.right + gap;
    const leftLeft = anchorBounds.left - gap - popoverWidth;
    const fitsRight = rightLeft + popoverWidth <= window.innerWidth - padding;
    const fitsLeft = leftLeft >= padding;
    const placement = preferredPlacement === 'right'
      ? fitsRight || !fitsLeft ? 'right' : 'left'
      : fitsLeft || !fitsRight ? 'left' : 'right';
    const preferredTop = anchorBounds.top;
    const endAlignedTop = anchorBounds.bottom - popoverHeight;
    const alignment: SideAnchoredPopoverAlignment = preferredTop + popoverHeight <= window.innerHeight - padding
      ? 'start'
      : 'end';
    const unclampedTop = alignment === 'start' ? preferredTop : endAlignedTop;
    const unclampedLeft = placement === 'right' ? rightLeft : leftLeft;
    const nextLayout: SideAnchoredPopoverLayout = {
      top: Math.min(
        Math.max(unclampedTop, padding),
        Math.max(padding, window.innerHeight - popoverHeight - padding),
      ),
      left: Math.min(
        Math.max(unclampedLeft, padding),
        Math.max(padding, window.innerWidth - popoverWidth - padding),
      ),
      placement,
      alignment,
    };
    setLayout(current => sameLayout(current, nextLayout) ? current : nextLayout);
  }, [anchorRef, gap, padding, popoverRef, preferredPlacement]);

  const schedulePositionUpdate = useCallback(() => {
    if (frameRef.current !== null) return;
    frameRef.current = window.requestAnimationFrame(() => {
      frameRef.current = null;
      updatePosition();
    });
  }, [updatePosition]);

  useLayoutEffect(() => {
    if (!open) {
      setLayout(current => current === null ? current : null);
      return;
    }

    updatePosition();
    window.addEventListener('resize', schedulePositionUpdate, { passive: true });
    window.addEventListener('scroll', schedulePositionUpdate, { capture: true, passive: true });
    const resizeObserver = typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(schedulePositionUpdate);
    if (anchorRef.current) resizeObserver?.observe(anchorRef.current);
    if (popoverRef.current) resizeObserver?.observe(popoverRef.current);

    return () => {
      window.removeEventListener('resize', schedulePositionUpdate);
      window.removeEventListener('scroll', schedulePositionUpdate, { capture: true });
      resizeObserver?.disconnect();
      if (frameRef.current !== null) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
    };
  }, [anchorRef, open, popoverRef, schedulePositionUpdate, updatePosition]);

  useLayoutEffect(() => {
    if (open) updatePosition();
  }, [layoutRevision, open, updatePosition]);

  return layout;
}
