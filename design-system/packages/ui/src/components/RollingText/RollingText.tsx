import { forwardRef, useCallback, useImperativeHandle, useState } from "react";
import { classNames } from "../../internal/classNames";
import { useReducedMotion } from "../../internal/useReducedMotion";
import { OverflowText, type OverflowTextProps } from "../../primitives/OverflowText";
import styles from "./RollingText.module.css";
import { useRollingTextMotion } from "./useRollingTextMotion";

export interface RollingTextProps extends Omit<OverflowTextProps, "children"> {
  children: string | number;
  /** Change this identity to roll; text edits under the same identity stay still. */
  transitionKey?: string | number;
}

interface TextFrame {
  identity: string | number;
  text: string | number;
  outgoing: string | number | null;
  revision: number;
}

/** A single-line vertical replacement: old text exits up, new text enters below. */
export const RollingText = forwardRef<HTMLSpanElement, RollingTextProps>(
  function RollingText({
    behavior = "marquee",
    children,
    className,
    marqueeActive,
    transitionKey = children,
    ...props
  }, ref) {
    const reducedMotion = useReducedMotion();
    const [frame, setFrame] = useState<TextFrame>(() => ({
      identity: transitionKey,
      text: children,
      outgoing: null,
      revision: 0,
    }));

    if (frame.identity !== transitionKey || frame.text !== children
      || (reducedMotion && frame.outgoing !== null)) {
      const startsTransition = !reducedMotion
        && frame.identity !== transitionKey
        && frame.text !== children
        && frame.outgoing === null;

      // Capture before commit, so the new identity never paints with stale text.
      // During a roll, retain one outgoing snapshot and adopt the latest target.
      setFrame({
        identity: transitionKey,
        text: children,
        outgoing: reducedMotion
          ? null
          : startsTransition ? frame.text : frame.outgoing,
        revision: frame.revision + (startsTransition ? 1 : 0),
      });
    }

    const transitioning = frame.outgoing !== null;
    const completeTransition = useCallback(() => {
      setFrame(current => current.revision === frame.revision && current.outgoing !== null
        ? { ...current, outgoing: null }
        : current);
    }, [frame.revision]);
    const { rootRef, currentRef, outgoingRef } = useRollingTextMotion({
      active: transitioning,
      returning: frame.text === frame.outgoing,
      revision: frame.revision,
      text: frame.text,
      onComplete: completeTransition,
    });
    useImperativeHandle(ref, () => rootRef.current!, [rootRef]);

    return (
      <span
        {...props}
        className={classNames(styles.root, className)}
        data-openbitfun-component="rolling-text"
        data-transitioning={transitioning ? "true" : "false"}
        ref={rootRef}
      >
        {transitioning && (
          <span aria-hidden="true" className={styles.outgoing} data-rolling-text-part="outgoing" ref={outgoingRef}>
            <OverflowText behavior="fade">{frame.outgoing}</OverflowText>
          </span>
        )}
        <span
          className={styles.current}
          data-rolling-text-part="current"
          key={frame.revision}
          ref={currentRef}
        >
          <OverflowText
            behavior={transitioning ? "fade" : behavior}
            marqueeActive={marqueeActive}
          >
            {frame.text}
          </OverflowText>
        </span>
      </span>
    );
  },
);
