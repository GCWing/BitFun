import { useLayoutEffect, useState, type CSSProperties, type RefObject } from "react";

export type LayerPlacement = "top" | "bottom" | "left" | "right";
export type PortalTarget = Element | DocumentFragment | (() => Element | DocumentFragment | null) | null;

/** Default portals stay inside the nearest theme scope; hosts may supply an overlay root. */
export function resolveLayerPortal(target: PortalTarget | undefined, anchor: HTMLElement | null) {
  if (typeof target === "function") return target();
  return target ?? anchor?.closest("[data-openbitfun-design-system-root]") ?? anchor?.ownerDocument.body ?? null;
}

export function useAnchoredLayer({ open, anchorRef, layerRef, placement = "bottom", matchWidth = false, revision, point }: {
  open: boolean;
  anchorRef: RefObject<HTMLElement | null>;
  layerRef: RefObject<HTMLElement | null>;
  placement?: LayerPlacement;
  matchWidth?: boolean;
  revision?: unknown;
  point?: { x: number; y: number };
}) {
  const [layout, setLayout] = useState<{ style: CSSProperties; placement: LayerPlacement } | null>(null);
  useLayoutEffect(() => {
    if (!open) { setLayout(null); return; }
    const anchor = anchorRef.current;
    const layer = layerRef.current;
    const view = layer?.ownerDocument.defaultView;
    if (!layer || !view || (!anchor && !point)) return;
    const update = () => {
      const padding = 8;
      const gap = point ? 0 : 4;
      const rect = point
        ? { left: point.x, right: point.x, top: point.y, bottom: point.y, width: 0 }
        : anchor!.getBoundingClientRect();
      const viewport = view.visualViewport;
      const vx = viewport?.offsetLeft ?? 0;
      const vy = viewport?.offsetTop ?? 0;
      const vw = viewport?.width ?? view.innerWidth;
      const vh = viewport?.height ?? view.innerHeight;
      const width = matchWidth ? Math.min(rect.width, vw - padding * 2) : undefined;
      layer.style.maxWidth = `${Math.max(0, vw - padding * 2)}px`;
      if (width !== undefined) layer.style.width = `${width}px`;
      const box = layer.getBoundingClientRect();
      const horizontal = placement === "left" || placement === "right";
      const before = horizontal ? rect.left - vx : rect.top - vy;
      const after = horizontal ? vx + vw - rect.right : vy + vh - rect.bottom;
      const needs = (horizontal ? box.width : box.height) + gap + padding;
      const preferBefore = placement === "top" || placement === "left";
      const useBefore = preferBefore ? before >= needs || before > after : after < needs && before > after;
      const side: LayerPlacement = horizontal ? (useBefore ? "left" : "right") : (useBefore ? "top" : "bottom");
      const maxHeight = horizontal ? vh - padding * 2 : Math.max(0, (useBefore ? before : after) - gap - padding);
      const height = Math.min(box.height, maxHeight);
      const left = horizontal ? (useBefore ? rect.left - box.width - gap : rect.right + gap) : rect.left;
      const top = horizontal ? rect.top : (useBefore ? rect.top - height - gap : rect.bottom + gap);
      const style: CSSProperties = {
        position: "fixed", width, maxWidth: vw - padding * 2, maxHeight,
        left: Math.max(vx + padding, Math.min(left, vx + vw - box.width - padding)),
        top: Math.max(vy + padding, Math.min(top, vy + vh - height - padding)),
      };
      setLayout(previous => JSON.stringify(previous) === JSON.stringify({ style, placement: side }) ? previous : { style, placement: side });
    };
    update();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(update);
    if (anchor) observer?.observe(anchor);
    observer?.observe(layer);
    view.addEventListener("resize", update);
    view.addEventListener("scroll", update, true);
    view.visualViewport?.addEventListener("resize", update);
    view.visualViewport?.addEventListener("scroll", update);
    return () => {
      observer?.disconnect();
      view.removeEventListener("resize", update);
      view.removeEventListener("scroll", update, true);
      view.visualViewport?.removeEventListener("resize", update);
      view.visualViewport?.removeEventListener("scroll", update);
    };
  }, [open, anchorRef, layerRef, placement, matchWidth, revision, point?.x, point?.y]);
  return layout;
}
