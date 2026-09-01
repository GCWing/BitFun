import {
  cloneElement,
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type FocusEvent as ReactFocusEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactElement,
  type ReactNode,
  type Ref,
} from "react";
import { classNames } from "../../internal/classNames";
import { Portal } from "../../overlay/Portal";
import { useDesignSystem } from "../../overlay/useDesignSystem";
import styles from "./Tooltip.module.css";

export type TooltipPlacement = "top" | "bottom" | "left" | "right";
export type TooltipTrigger = "hover" | "click" | "focus";

const DEFAULT_TOOLTIP_DELAY_MS = 450;
const INTERACTIVE_HIDE_DELAY_MS = 400;
/**
 * After a tooltip hides, tooltips shown again within this window skip the
 * open delay so scanning across adjacent triggers feels instant.
 */
const WARM_WINDOW_MS = 300;
let tooltipWarmUntil = 0;

/** Cursor offset when followCursor: right and down so the tooltip never covers the cursor. */
const CURSOR_OFFSET_X = 12;
const CURSOR_OFFSET_Y = 8;
const GAP = 8;
const VIEWPORT_PADDING = 8;

export interface TooltipProps {
  /** Single focusable trigger element the tooltip describes. */
  children: ReactElement;
  className?: string;
  content: ReactNode;
  /** Open delay in milliseconds. Falls back to the provider value, then 450ms. */
  delay?: number;
  disabled?: boolean;
  /** Position near the mouse cursor instead of the trigger element. */
  followCursor?: boolean;
  /** Keep the tooltip open while hovered so its content can be selected or clicked. */
  interactive?: boolean;
  /** Preferred side of the trigger; flips to the opposite side when space runs out. */
  placement?: TooltipPlacement;
  trigger?: TooltipTrigger;
}

function assignRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (!ref) return;
  if (typeof ref === "function") {
    ref(value);
    return;
  }
  (ref as { current: T | null }).current = value;
}

const OPPOSITE_PLACEMENT: Record<TooltipPlacement, TooltipPlacement> = {
  top: "bottom",
  bottom: "top",
  left: "right",
  right: "left",
};

function getAvailableSpace(triggerRect: DOMRect, placement: TooltipPlacement): number {
  switch (placement) {
    case "top":
      return triggerRect.top - VIEWPORT_PADDING;
    case "bottom":
      return window.innerHeight - triggerRect.bottom - VIEWPORT_PADDING;
    case "left":
      return triggerRect.left - VIEWPORT_PADDING;
    case "right":
      return window.innerWidth - triggerRect.right - VIEWPORT_PADDING;
  }
}

function getPositionForPlacement(
  triggerRect: DOMRect,
  tooltipRect: DOMRect,
  placement: TooltipPlacement,
): { top: number; left: number } {
  switch (placement) {
    case "top":
      return {
        top: triggerRect.top - tooltipRect.height - GAP,
        left: triggerRect.left + (triggerRect.width - tooltipRect.width) / 2,
      };
    case "bottom":
      return {
        top: triggerRect.bottom + GAP,
        left: triggerRect.left + (triggerRect.width - tooltipRect.width) / 2,
      };
    case "left":
      return {
        top: triggerRect.top + (triggerRect.height - tooltipRect.height) / 2,
        left: triggerRect.left - tooltipRect.width - GAP,
      };
    case "right":
      return {
        top: triggerRect.top + (triggerRect.height - tooltipRect.height) / 2,
        left: triggerRect.right + GAP,
      };
  }
}

function determineBestPlacement(
  triggerRect: DOMRect,
  tooltipRect: DOMRect,
  preferredPlacement: TooltipPlacement,
): TooltipPlacement {
  const requiredSpace = preferredPlacement === "top" || preferredPlacement === "bottom"
    ? tooltipRect.height + GAP
    : tooltipRect.width + GAP;

  const preferredSpace = getAvailableSpace(triggerRect, preferredPlacement);
  if (preferredSpace >= requiredSpace) return preferredPlacement;

  const oppositePlacement = OPPOSITE_PLACEMENT[preferredPlacement];
  const oppositeSpace = getAvailableSpace(triggerRect, oppositePlacement);
  if (oppositeSpace >= requiredSpace) return oppositePlacement;

  return oppositeSpace > preferredSpace ? oppositePlacement : preferredPlacement;
}

function applyBoundaryConstraints(
  position: { top: number; left: number },
  tooltipRect: DOMRect,
): { top: number; left: number } {
  let { top, left } = position;

  if (left < VIEWPORT_PADDING) {
    left = VIEWPORT_PADDING;
  } else if (left + tooltipRect.width > window.innerWidth - VIEWPORT_PADDING) {
    left = window.innerWidth - tooltipRect.width - VIEWPORT_PADDING;
  }

  if (top < VIEWPORT_PADDING) {
    top = VIEWPORT_PADDING;
  } else if (top + tooltipRect.height > window.innerHeight - VIEWPORT_PADDING) {
    top = window.innerHeight - tooltipRect.height - VIEWPORT_PADDING;
  }

  return { top, left };
}

interface TooltipLayout {
  top: number;
  left: number;
  placement: TooltipPlacement;
  ready: boolean;
}

export function Tooltip({
  children,
  className,
  content,
  delay,
  disabled = false,
  followCursor = false,
  interactive = false,
  placement = "top",
  trigger = "hover",
}: TooltipProps) {
  const designSystem = useDesignSystem();
  const resolvedDelayMs = delay ?? designSystem.tooltipDelay ?? DEFAULT_TOOLTIP_DELAY_MS;

  const tooltipId = useId();
  const [visible, setVisible] = useState(false);
  // Single layout state (position + placement + ready) so one recalculation
  // commits at most one re-render.
  const [layout, setLayout] = useState<TooltipLayout>({
    top: 0,
    left: 0,
    placement,
    ready: false,
  });
  const [mousePosition, setMousePosition] = useState<{ x: number; y: number } | null>(null);
  const triggerRef = useRef<HTMLElement | null>(null);
  const tooltipRef = useRef<HTMLDivElement | null>(null);
  const showTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hideTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latestMousePositionRef = useRef<{ x: number; y: number } | null>(null);
  const recalcFrameRef = useRef<number | null>(null);
  const instantRef = useRef(false);

  const calculatePosition = useCallback(() => {
    if (!tooltipRef.current) return;

    const tooltipRect = tooltipRef.current.getBoundingClientRect();

    if (followCursor && mousePosition) {
      const raw = {
        top: mousePosition.y + CURSOR_OFFSET_Y,
        left: mousePosition.x + CURSOR_OFFSET_X,
      };
      const pos = applyBoundaryConstraints(raw, tooltipRect);
      setLayout({ top: pos.top, left: pos.left, placement: "bottom", ready: true });
      return;
    }

    if (!triggerRef.current) return;

    const triggerRect = triggerRef.current.getBoundingClientRect();
    const bestPlacement = determineBestPlacement(triggerRect, tooltipRect, placement);
    const pos = applyBoundaryConstraints(
      getPositionForPlacement(triggerRect, tooltipRect, bestPlacement),
      tooltipRect,
    );

    setLayout({ top: pos.top, left: pos.left, placement: bestPlacement, ready: true });
  }, [placement, followCursor, mousePosition]);

  // rAF-merged recalculation for scroll/resize storms: at most one
  // getBoundingClientRect pass per frame.
  const scheduleCalculatePosition = useCallback(() => {
    if (recalcFrameRef.current !== null) return;
    recalcFrameRef.current = requestAnimationFrame(() => {
      recalcFrameRef.current = null;
      calculatePosition();
    });
  }, [calculatePosition]);

  const showTooltip = useCallback((event?: ReactMouseEvent) => {
    if (disabled) return;
    if (showTimeoutRef.current) clearTimeout(showTimeoutRef.current);
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
      hideTimeoutRef.current = null;
    }
    if (followCursor && event) {
      latestMousePositionRef.current = { x: event.clientX, y: event.clientY };
    }
    const openDelay = trigger === "hover" && Date.now() < tooltipWarmUntil
      ? 0
      : resolvedDelayMs;
    instantRef.current = openDelay === 0;
    showTimeoutRef.current = setTimeout(() => {
      showTimeoutRef.current = null;
      if (followCursor) {
        setMousePosition(latestMousePositionRef.current);
      }
      setLayout((prev) => (prev.ready ? { ...prev, ready: false } : prev));
      setVisible(true);
    }, openDelay);
  }, [disabled, followCursor, resolvedDelayMs, trigger]);

  const hideTooltip = useCallback(() => {
    if (showTimeoutRef.current) {
      clearTimeout(showTimeoutRef.current);
      showTimeoutRef.current = null;
    }
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
      hideTimeoutRef.current = null;
    }
    if (visible) {
      tooltipWarmUntil = Date.now() + WARM_WINDOW_MS;
    }
    setVisible(false);
    setLayout((prev) => (prev.ready ? { ...prev, ready: false } : prev));
    if (followCursor) {
      latestMousePositionRef.current = null;
      setMousePosition(null);
    }
  }, [followCursor, visible]);

  const scheduleHideTooltip = useCallback(() => {
    if (!interactive) {
      hideTooltip();
      return;
    }

    if (hideTimeoutRef.current) clearTimeout(hideTimeoutRef.current);
    hideTimeoutRef.current = setTimeout(() => {
      hideTimeoutRef.current = null;
      hideTooltip();
    }, INTERACTIVE_HIDE_DELAY_MS);
  }, [hideTooltip, interactive]);

  useEffect(() => {
    setLayout((prev) => (prev.placement === placement ? prev : { ...prev, placement }));
  }, [placement]);

  // When the tooltip becomes disabled (e.g. the parent opens a menu that
  // covers the trigger), cancel any pending show timer and force-hide so a
  // tooltip cannot appear or linger above the new overlay.
  useEffect(() => {
    if (disabled) hideTooltip();
  }, [disabled, hideTooltip]);

  useEffect(() => {
    if (!visible) return;

    scheduleCalculatePosition();
    if (!followCursor) {
      window.addEventListener("scroll", scheduleCalculatePosition, { capture: true, passive: true });
    }
    window.addEventListener("resize", scheduleCalculatePosition, { passive: true });
    return () => {
      if (!followCursor) {
        window.removeEventListener("scroll", scheduleCalculatePosition, { capture: true });
      }
      window.removeEventListener("resize", scheduleCalculatePosition);
      if (recalcFrameRef.current !== null) {
        cancelAnimationFrame(recalcFrameRef.current);
        recalcFrameRef.current = null;
      }
    };
  }, [visible, followCursor, scheduleCalculatePosition]);

  useEffect(() => () => {
    if (showTimeoutRef.current) clearTimeout(showTimeoutRef.current);
    if (hideTimeoutRef.current) clearTimeout(hideTimeoutRef.current);
  }, []);

  const childProps = children.props as Record<string, unknown>;
  const childRef = (children as ReactElement & { ref?: Ref<HTMLElement> }).ref;

  const handleTriggerRef = useCallback((node: HTMLElement | null) => {
    triggerRef.current = node;
    assignRef(childRef, node);
  }, [childRef]);

  const handleMouseEnter = (event: ReactMouseEvent) => {
    if (trigger === "hover") showTooltip(event);
    (childProps.onMouseEnter as ((event: ReactMouseEvent) => void) | undefined)?.(event);
  };

  const handleMouseLeave = (event: ReactMouseEvent) => {
    if (trigger === "hover") scheduleHideTooltip();
    (childProps.onMouseLeave as ((event: ReactMouseEvent) => void) | undefined)?.(event);
  };

  const handleMouseMove = (event: ReactMouseEvent) => {
    if (followCursor && !visible) {
      latestMousePositionRef.current = { x: event.clientX, y: event.clientY };
    }
    (childProps.onMouseMove as ((event: ReactMouseEvent) => void) | undefined)?.(event);
  };

  const handleClick = (event: ReactMouseEvent) => {
    // Always cancel any pending show timer so a click before the tooltip
    // appears cannot surface a stale tooltip after the trigger is covered
    // by a menu or backdrop (which prevents the natural mouseleave).
    if (showTimeoutRef.current) {
      clearTimeout(showTimeoutRef.current);
      showTimeoutRef.current = null;
    }
    if (visible) {
      hideTooltip();
    } else if (trigger === "click") {
      showTooltip();
    }
    (childProps.onClick as ((event: ReactMouseEvent) => void) | undefined)?.(event);
  };

  const handleFocus = (event: ReactFocusEvent) => {
    if (trigger === "focus") showTooltip();
    (childProps.onFocus as ((event: ReactFocusEvent) => void) | undefined)?.(event);
  };

  const handleBlur = (event: ReactFocusEvent) => {
    if (trigger === "focus") hideTooltip();
    (childProps.onBlur as ((event: ReactFocusEvent) => void) | undefined)?.(event);
  };

  const isShown = visible && layout.ready;

  const triggerElement = cloneElement(children as ReactElement<Record<string, unknown>>, {
    ref: handleTriggerRef,
    onMouseEnter: handleMouseEnter,
    onMouseLeave: handleMouseLeave,
    onMouseMove: followCursor ? handleMouseMove : childProps.onMouseMove,
    onClick: handleClick,
    onFocus: handleFocus,
    onBlur: handleBlur,
    "aria-describedby": isShown
      ? [childProps["aria-describedby"], tooltipId].filter(Boolean).join(" ")
      : childProps["aria-describedby"],
  } as Record<string, unknown>);

  return (
    <>
      {triggerElement}
      {visible && (
        <Portal ownerDocument={triggerRef.current?.ownerDocument}>
        <div
          ref={tooltipRef}
          id={tooltipId}
          role="tooltip"
          className={classNames(styles.root, className)}
          data-bf-component="tooltip"
          data-bf-placement={layout.placement}
          data-bf-interactive={interactive ? "true" : "false"}
          data-bf-state={isShown ? "visible" : undefined}
          data-instant={instantRef.current || undefined}
          onMouseEnter={interactive ? () => {
            if (hideTimeoutRef.current) {
              clearTimeout(hideTimeoutRef.current);
              hideTimeoutRef.current = null;
            }
          } : undefined}
          onMouseLeave={interactive ? scheduleHideTooltip : undefined}
          style={{
            top: `${layout.top}px`,
            left: `${layout.left}px`,
          }}
        >
          {!followCursor && (
            <div className={styles.arrow} data-bf-part="arrow" aria-hidden="true" />
          )}
          <div className={styles.content} data-bf-part="content">
            <div className={styles.body} data-bf-part="body">{content}</div>
          </div>
        </div>
        </Portal>
      )}
    </>
  );
}
