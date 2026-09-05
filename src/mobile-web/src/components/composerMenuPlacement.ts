interface Rect { left: number; top: number; bottom: number }
interface Viewport { left: number; top: number; width: number; height: number }

/** Fixed-position coordinates, including the visible viewport above a keyboard. */
export function composerMenuPlacement(anchor: Rect, viewport: Viewport, preferredWidth: number, height: number) {
  const margin = 8;
  const width = Math.max(0, Math.min(preferredWidth, viewport.width - margin * 2));
  const above = Math.max(0, anchor.top - viewport.top - margin * 2);
  const below = Math.max(0, viewport.top + viewport.height - anchor.bottom - margin * 2);
  const placeAbove = above >= Math.min(height, 345) || above >= below;
  const maxHeight = Math.min(345, placeAbove ? above : below);
  const renderedHeight = Math.min(height, maxHeight);
  return {
    left: Math.max(viewport.left + margin, Math.min(anchor.left, viewport.left + viewport.width - width - margin)),
    top: Math.max(viewport.top + margin, Math.min(
      placeAbove ? anchor.top - margin - renderedHeight : anchor.bottom + margin,
      viewport.top + viewport.height - margin - renderedHeight,
    )),
    width,
    maxHeight,
  };
}
