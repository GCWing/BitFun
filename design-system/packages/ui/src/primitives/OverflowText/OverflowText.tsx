import {
  forwardRef,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ForwardedRef,
  type HTMLAttributes,
} from "react";
import { classNames } from "../../internal/classNames";
import styles from "./OverflowText.module.css";

const useIsomorphicLayoutEffect = typeof window === "undefined"
  ? useEffect
  : useLayoutEffect;

function assignRef<T>(ref: ForwardedRef<T>, value: T | null) {
  if (typeof ref === "function") ref(value);
  else if (ref) ref.current = value;
}

const MARQUEE_MIN_DURATION_MS = 2400;
const MARQUEE_PIXELS_PER_SECOND = 36;
const MARQUEE_EDGE_HOLD_MS = 1200;

export type OverflowTextBehavior = "fade" | "marquee";

export interface OverflowTextProps extends HTMLAttributes<HTMLSpanElement> {
  /** Visual treatment used only after the rendered content is actually clipped. */
  behavior?: OverflowTextBehavior;
}

interface OverflowMeasurement {
  distance: number;
  isOverflowing: boolean;
}

export const OverflowText = forwardRef<HTMLSpanElement, OverflowTextProps>(
  function OverflowText({
    behavior = "fade",
    children,
    className,
    style,
    ...props
  }, forwardedRef) {
    const elementRef = useRef<HTMLSpanElement | null>(null);
    const contentRef = useRef<HTMLSpanElement | null>(null);
    const measurementRef = useRef<OverflowMeasurement>({
      distance: 0,
      isOverflowing: false,
    });
    const [measurement, setMeasurement] = useState<OverflowMeasurement>(measurementRef.current);

    const setElementRef = useCallback((element: HTMLSpanElement | null) => {
      elementRef.current = element;
      assignRef(forwardedRef, element);
    }, [forwardedRef]);

    const updateOverflow = useCallback(() => {
      const element = elementRef.current;
      const content = contentRef.current;
      if (!element || !content) return;

      const distance = Math.max(0, content.scrollWidth - element.clientWidth);
      const isOverflowing = distance > 0;
      const current = measurementRef.current;
      if (current.distance === distance && current.isOverflowing === isOverflowing) return;

      const next = { distance, isOverflowing };
      measurementRef.current = next;
      setMeasurement(next);
    }, []);

    useIsomorphicLayoutEffect(() => {
      updateOverflow();
    }, [children, updateOverflow]);

    useEffect(() => {
      const element = elementRef.current;
      if (!element) return undefined;

      const resizeObserver = typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(updateOverflow);
      resizeObserver?.observe(element);
      if (contentRef.current) resizeObserver?.observe(contentRef.current);

      const fontSet = element.ownerDocument.fonts;
      fontSet?.addEventListener("loadingdone", updateOverflow);

      if (!resizeObserver) {
        element.ownerDocument.defaultView?.addEventListener("resize", updateOverflow);
      }

      return () => {
        resizeObserver?.disconnect();
        fontSet?.removeEventListener("loadingdone", updateOverflow);
        if (!resizeObserver) {
          element.ownerDocument.defaultView?.removeEventListener("resize", updateOverflow);
        }
      };
    }, [updateOverflow]);

    const marqueeDuration = Math.max(
      MARQUEE_MIN_DURATION_MS,
      Math.round(
        (measurement.distance / MARQUEE_PIXELS_PER_SECOND) * 1000
        + MARQUEE_EDGE_HOLD_MS,
      ),
    );
    const resolvedStyle = behavior === "marquee"
      ? ({
          ...style,
          "--_overflow-text-marquee-distance": `${measurement.distance}px`,
          "--_overflow-text-marquee-duration": `${marqueeDuration}ms`,
        } as CSSProperties)
      : style;

    return (
      <span
        {...props}
        className={classNames(styles.root, className)}
        data-overflow={measurement.isOverflowing ? "true" : "false"}
        data-overflow-behavior={behavior}
        ref={setElementRef}
        style={resolvedStyle}
      >
        <span className={styles.content} data-openbitfun-part="content" ref={contentRef}>
          {children}
        </span>
      </span>
    );
  },
);
