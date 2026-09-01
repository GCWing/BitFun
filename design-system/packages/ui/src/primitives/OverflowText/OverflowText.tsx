import {
  forwardRef,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
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

export type OverflowTextProps = HTMLAttributes<HTMLSpanElement>;

export const OverflowText = forwardRef<HTMLSpanElement, OverflowTextProps>(
  function OverflowText({ className, ...props }, forwardedRef) {
    const elementRef = useRef<HTMLSpanElement | null>(null);
    const [isOverflowing, setIsOverflowing] = useState(false);

    const setElementRef = useCallback((element: HTMLSpanElement | null) => {
      elementRef.current = element;
      assignRef(forwardedRef, element);
    }, [forwardedRef]);

    const updateOverflow = useCallback(() => {
      const element = elementRef.current;
      if (!element) return;

      const nextIsOverflowing = element.scrollWidth > element.clientWidth;
      setIsOverflowing((current) => (
        current === nextIsOverflowing ? current : nextIsOverflowing
      ));
    }, []);

    useIsomorphicLayoutEffect(() => {
      updateOverflow();
    });

    useEffect(() => {
      const element = elementRef.current;
      if (!element) return undefined;

      const resizeObserver = typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(updateOverflow);
      resizeObserver?.observe(element);

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

    return (
      <span
        {...props}
        className={classNames(styles.root, className)}
        data-overflow={isOverflowing ? "true" : "false"}
        ref={setElementRef}
      />
    );
  },
);
