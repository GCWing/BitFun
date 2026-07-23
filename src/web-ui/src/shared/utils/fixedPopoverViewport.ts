/**
 * Helpers for positioning `position: fixed` popovers anchored to another element,
 * staying within the visual viewport with optional flip above the anchor.
 */

export const DEFAULT_POPOVER_VIEWPORT_PADDING = 8;

export function clampFixedPopoverLeft(
  preferredLeft: number,
  menuWidth: number,
  padding = DEFAULT_POPOVER_VIEWPORT_PADDING,
): number {
  const vw = window.innerWidth;
  const maxLeft = vw - menuWidth - padding;
  return Math.max(padding, Math.min(preferredLeft, maxLeft));
}

/**
 * Prefer opening below the anchor; if that overflows the bottom, flip above when possible;
 * otherwise pin within the viewport (menu taller than viewport should use max-height + scroll).
 */
export function clampFixedPopoverTop(
  anchorRect: DOMRectReadOnly,
  menuHeight: number,
  gap = 6,
  padding = DEFAULT_POPOVER_VIEWPORT_PADDING,
): number {
  const vh = window.innerHeight;
  const maxTop = vh - padding - menuHeight;
  const top = anchorRect.bottom + gap;

  if (top <= maxTop) {
    return Math.max(padding, top);
  }

  const aboveTop = anchorRect.top - gap - menuHeight;
  if (aboveTop >= padding) {
    return aboveTop;
  }

  return Math.max(padding, Math.min(top, maxTop));
}

export function computeFixedPopoverPosition(
  anchorRect: DOMRectReadOnly,
  menuWidth: number,
  menuHeight: number,
  gap = 6,
  padding = DEFAULT_POPOVER_VIEWPORT_PADDING,
): { top: number; left: number } {
  return {
    top: clampFixedPopoverTop(anchorRect, menuHeight, gap, padding),
    left: clampFixedPopoverLeft(anchorRect.left, menuWidth, padding),
  };
}
