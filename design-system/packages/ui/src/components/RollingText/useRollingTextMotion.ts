import { useEffect, useLayoutEffect, useRef } from "react";

const useIsomorphicLayoutEffect = typeof window === "undefined" ? useEffect : useLayoutEffect;

interface RollingTextMotionOptions {
  active: boolean;
  returning: boolean;
  revision: number;
  text: string | number;
  onComplete: () => void;
}

function durationInMilliseconds(value: string): number {
  const duration = Number.parseFloat(value);
  return Number.isFinite(duration) ? duration * (value.trim().endsWith("ms") ? 1 : 1000) : 0;
}

/** One interruptible motion owns both the line positions and its intrinsic width. */
export function useRollingTextMotion({ active, returning, revision, text, onComplete }: RollingTextMotionOptions) {
  const rootRef = useRef<HTMLSpanElement>(null);
  const currentRef = useRef<HTMLSpanElement>(null);
  const outgoingRef = useRef<HTMLSpanElement>(null);
  const lastWidth = useRef<number | null>(null);
  const running = useRef<Animation[]>([]);
  const runningRevision = useRef<number | null>(null);

  useIsomorphicLayoutEffect(() => {
    const root = rootRef.current;
    const current = currentRef.current;
    const outgoing = outgoingRef.current;
    const view = root?.ownerDocument.defaultView;
    if (!root || !current || !view) return;

    // Read the actual in-flight positions before cancelling. A rapid replacement
    // adopts the latest text without jumping back to either starting position.
    const hasRunningMotion = running.current.length > 0;
    const interrupted = hasRunningMotion && runningRevision.current === revision;
    const fromWidth = hasRunningMotion ? root.getBoundingClientRect().width : lastWidth.current;
    const fromCurrent = interrupted ? view.getComputedStyle(current).transform : "translateY(100%)";
    const fromOutgoing = interrupted && outgoing ? view.getComputedStyle(outgoing).transform : "translateY(0)";
    const previous = running.current;
    running.current = [];
    previous.forEach(animation => animation.cancel());

    // Removing the width effect exposes the new natural, layout-constrained size.
    // Both reads happen before paint; no hidden clone or per-frame React state.
    const toWidth = root.getBoundingClientRect().width;
    lastWidth.current = toWidth;
    if (!active || !outgoing) return;

    const style = view.getComputedStyle(root);
    const duration = durationInMilliseconds(style.getPropertyValue("--_rolling-text-duration"));
    const easing = style.getPropertyValue("--_rolling-text-easing").trim();
    if (!root.animate || duration <= 0 || toWidth <= 0) {
      onComplete();
      return;
    }

    const options: KeyframeAnimationOptions = { duration, easing, fill: "both" };
    const animations = [
      current.animate([
        { transform: fromCurrent },
        { transform: returning ? "translateY(100%)" : "translateY(0)" },
      ], options),
      outgoing.animate([
        { transform: fromOutgoing },
        { transform: returning ? "translateY(0)" : "translateY(-100%)" },
      ], options),
      root.animate([
        { inlineSize: `${fromWidth ?? toWidth}px` },
        { inlineSize: `${toWidth}px` },
      ], options),
    ];
    running.current = animations;
    runningRevision.current = revision;
    void Promise.all(animations.map(animation => animation.finished)).then(() => {
      if (running.current === animations) onComplete();
    }, () => {
      // Replacement, reduced motion, and unmount cancel obsolete completions.
    });
  }, [active, returning, revision, text, onComplete]);

  useIsomorphicLayoutEffect(() => {
    const root = rootRef.current;
    const observer = root && typeof ResizeObserver !== "undefined" ? new ResizeObserver(() => {
      if (running.current.length === 0) lastWidth.current = root.getBoundingClientRect().width;
    }) : null;
    if (root) observer?.observe(root);

    return () => {
      observer?.disconnect();
      const animations = running.current;
      running.current = [];
      animations.forEach(animation => animation.cancel());
    };
  }, []);

  return { rootRef, currentRef, outgoingRef };
}
