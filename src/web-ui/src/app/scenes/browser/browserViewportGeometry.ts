export type BrowserViewportBounds = { left: number; top: number; width: number; height: number };

/** Share one physical-pixel rectangle between the native view and its DOM frame. */
export function alignBrowserViewport(rect: BrowserViewportBounds, scale: number): BrowserViewportBounds {
  const dpr = Number.isFinite(scale) && scale > 0 ? scale : 1;
  const left = Math.round(rect.left * dpr);
  const top = Math.round(rect.top * dpr);
  const right = Math.round((rect.left + rect.width) * dpr);
  const bottom = Math.round((rect.top + rect.height) * dpr);
  return { left: left / dpr, top: top / dpr, width: (right - left) / dpr, height: (bottom - top) / dpr };
}
