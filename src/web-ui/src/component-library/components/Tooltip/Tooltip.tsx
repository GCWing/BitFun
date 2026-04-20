import React, { useState, useRef, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import './Tooltip.scss';

export type TooltipPlacement = 'top' | 'bottom' | 'left' | 'right';
const DEFAULT_TOOLTIP_DELAY = 450;

export interface TooltipProps {
  content: React.ReactNode;
  children: React.ReactElement;
  /** Preferred side of trigger (or of cursor when followCursor). */
  placement?: TooltipPlacement;
  /** When true, tooltip appears near the mouse cursor instead of the trigger element. */
  followCursor?: boolean;
  trigger?: 'hover' | 'click' | 'focus';
  delay?: number;
  disabled?: boolean;
  className?: string;
  interactive?: boolean;
}

const assignRef = <T,>(ref: React.Ref<T> | undefined, value: T | null): void => {
  if (!ref) return;

  if (typeof ref === 'function') {
    ref(value);
    return;
  }

  (ref as React.MutableRefObject<T | null>).current = value;
};

const getOppositePlacement = (placement: TooltipPlacement): TooltipPlacement => {
  const opposites: Record<TooltipPlacement, TooltipPlacement> = {
    top: 'bottom',
    bottom: 'top',
    left: 'right',
    right: 'left',
  };
  return opposites[placement];
};

const getAvailableSpace = (
  triggerRect: DOMRect,
  placement: TooltipPlacement,
  viewportPadding: number
): number => {
  switch (placement) {
    case 'top':
      return triggerRect.top - viewportPadding;
    case 'bottom':
      return window.innerHeight - triggerRect.bottom - viewportPadding;
    case 'left':
      return triggerRect.left - viewportPadding;
    case 'right':
      return window.innerWidth - triggerRect.right - viewportPadding;
  }
};

const getPositionForPlacement = (
  triggerRect: DOMRect,
  tooltipRect: DOMRect,
  placement: TooltipPlacement,
  gap: number
): { top: number; left: number } => {
  let top = 0;
  let left = 0;

  switch (placement) {
    case 'top':
      top = triggerRect.top - tooltipRect.height - gap;
      left = triggerRect.left + (triggerRect.width - tooltipRect.width) / 2;
      break;
    case 'bottom':
      top = triggerRect.bottom + gap;
      left = triggerRect.left + (triggerRect.width - tooltipRect.width) / 2;
      break;
    case 'left':
      top = triggerRect.top + (triggerRect.height - tooltipRect.height) / 2;
      left = triggerRect.left - tooltipRect.width - gap;
      break;
    case 'right':
      top = triggerRect.top + (triggerRect.height - tooltipRect.height) / 2;
      left = triggerRect.right + gap;
      break;
  }

  return { top, left };
};

const determineBestPlacement = (
  triggerRect: DOMRect,
  tooltipRect: DOMRect,
  preferredPlacement: TooltipPlacement,
  gap: number,
  viewportPadding: number
): TooltipPlacement => {
  const requiredSpace = 
    preferredPlacement === 'top' || preferredPlacement === 'bottom'
      ? tooltipRect.height + gap
      : tooltipRect.width + gap;

  const preferredSpace = getAvailableSpace(triggerRect, preferredPlacement, viewportPadding);

  if (preferredSpace >= requiredSpace) {
    return preferredPlacement;
  }

  const oppositePlacement = getOppositePlacement(preferredPlacement);
  const oppositeSpace = getAvailableSpace(triggerRect, oppositePlacement, viewportPadding);

  if (oppositeSpace >= requiredSpace) {
    return oppositePlacement;
  }

  return oppositeSpace > preferredSpace ? oppositePlacement : preferredPlacement;
};

const applyBoundaryConstraints = (
  position: { top: number; left: number },
  tooltipRect: DOMRect,
  viewportPadding: number
): { top: number; left: number } => {
  let { top, left } = position;

  if (left < viewportPadding) {
    left = viewportPadding;
  } else if (left + tooltipRect.width > window.innerWidth - viewportPadding) {
    left = window.innerWidth - tooltipRect.width - viewportPadding;
  }

  if (top < viewportPadding) {
    top = viewportPadding;
  } else if (top + tooltipRect.height > window.innerHeight - viewportPadding) {
    top = window.innerHeight - tooltipRect.height - viewportPadding;
  }

  return { top, left };
};

/** Cursor offset when followCursor: right 12px, down 8px so tooltip doesn't cover cursor */
const CURSOR_OFFSET_X = 12;
const CURSOR_OFFSET_Y = 8;

export const Tooltip: React.FC<TooltipProps> = ({
  content,
  children,
  placement = 'top',
  followCursor = false,
  trigger = 'hover',
  delay = DEFAULT_TOOLTIP_DELAY,
  disabled = false,
  className = '',
  interactive = false,
}) => {
  const [visible, setVisible] = useState(false);
  const [position, setPosition] = useState({ top: 0, left: 0 });
  const [positionReady, setPositionReady] = useState(false);
  const [actualPlacement, setActualPlacement] = useState<TooltipPlacement>(placement);
  const [mousePosition, setMousePosition] = useState<{ x: number; y: number } | null>(null);
  const triggerRef = useRef<HTMLElement | null>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hideTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latestMousePositionRef = useRef<{ x: number; y: number } | null>(null);

  const gap = 8;
  const viewportPadding = 8;

  const calculatePosition = useCallback(() => {
    if (!tooltipRef.current) return;

    const tooltipRect = tooltipRef.current.getBoundingClientRect();

    if (followCursor && mousePosition) {
      const left = mousePosition.x + CURSOR_OFFSET_X;
      const top = mousePosition.y + CURSOR_OFFSET_Y;
      const pos = applyBoundaryConstraints({ top, left }, tooltipRect, viewportPadding);
      setActualPlacement('bottom');
      setPosition(pos);
      setPositionReady(true);
      return;
    }

    if (!triggerRef.current) return;

    const triggerRect = triggerRef.current.getBoundingClientRect();

    const bestPlacement = determineBestPlacement(
      triggerRect,
      tooltipRect,
      placement,
      gap,
      viewportPadding
    );

    let pos = getPositionForPlacement(triggerRect, tooltipRect, bestPlacement, gap);

    pos = applyBoundaryConstraints(pos, tooltipRect, viewportPadding);

    setActualPlacement(bestPlacement);
    setPosition(pos);
    setPositionReady(true);
  }, [placement, followCursor, mousePosition]);

  const showTooltip = (e?: React.MouseEvent) => {
    if (disabled) return;
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
      hideTimeoutRef.current = null;
    }
    if (followCursor && e) {
      latestMousePositionRef.current = { x: e.clientX, y: e.clientY };
    }
    timeoutRef.current = setTimeout(() => {
      timeoutRef.current = null;
      if (followCursor) {
        setMousePosition(latestMousePositionRef.current);
      }
      setPositionReady(false);
      setVisible(true);
    }, delay);
  };

  const hideTooltip = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
      hideTimeoutRef.current = null;
    }
    setVisible(false);
    setPositionReady(false);
    if (followCursor) {
      latestMousePositionRef.current = null;
      setMousePosition(null);
    }
  }, [followCursor]);

  const scheduleHideTooltip = useCallback(() => {
    if (!interactive) {
      hideTooltip();
      return;
    }

    if (hideTimeoutRef.current) {
      clearTimeout(hideTimeoutRef.current);
    }

    hideTimeoutRef.current = setTimeout(() => {
      hideTimeoutRef.current = null;
      hideTooltip();
    }, 150);
  }, [hideTooltip, interactive]);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (followCursor && !visible) {
        latestMousePositionRef.current = { x: e.clientX, y: e.clientY };
      }
      const childProps = children.props as Record<string, unknown>;
      if (typeof childProps.onMouseMove === 'function') {
        (childProps.onMouseMove as (e: React.MouseEvent) => void)(e);
      }
    },
    [followCursor, visible, children.props]
  );

  useEffect(() => {
    setActualPlacement(placement);
  }, [placement]);

  // When the tooltip becomes disabled (e.g. parent opens a menu/popover that
  // covers the trigger), cancel any pending show timer and force-hide so a
  // tooltip cannot appear or linger above the new overlay.
  useEffect(() => {
    if (disabled) {
      hideTooltip();
    }
  }, [disabled, hideTooltip]);

  useEffect(() => {
    if (visible) {
      requestAnimationFrame(() => {
        calculatePosition();
      });
      if (!followCursor) {
        window.addEventListener('scroll', calculatePosition, true);
      }
      window.addEventListener('resize', calculatePosition);
      return () => {
        if (!followCursor) {
          window.removeEventListener('scroll', calculatePosition, true);
        }
        window.removeEventListener('resize', calculatePosition);
      };
    }
  }, [visible, followCursor, calculatePosition]);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      if (hideTimeoutRef.current) {
        clearTimeout(hideTimeoutRef.current);
      }
    };
  }, []);

  const childProps = children.props as Record<string, unknown>;
  const childRef = (children as React.ReactElement & { ref?: React.Ref<HTMLElement> }).ref;

  const handleTriggerRef = useCallback((node: HTMLElement | null) => {
    triggerRef.current = node;
    assignRef(childRef, node);
  }, [childRef]);

  const handleMouseEnter = (e: React.MouseEvent) => {
    if (trigger === 'hover') showTooltip(e);
    if (typeof childProps.onMouseEnter === 'function') {
      (childProps.onMouseEnter as (e: React.MouseEvent) => void)(e);
    }
  };

  const handleMouseLeave = (e: React.MouseEvent) => {
    if (trigger === 'hover') scheduleHideTooltip();
    if (typeof childProps.onMouseLeave === 'function') {
      (childProps.onMouseLeave as (e: React.MouseEvent) => void)(e);
    }
  };

  const handleClick = (e: React.MouseEvent) => {
    // Always cancel any pending show timer so a click before the tooltip
    // appears won't surface a stale tooltip after the trigger is covered
    // by a menu/backdrop (which prevents the natural mouseleave).
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    // Always hide tooltip on click for better UX (e.g., button tooltips)
    if (visible) {
      hideTooltip();
    } else if (trigger === 'click') {
      showTooltip();
    }
    if (typeof childProps.onClick === 'function') {
      (childProps.onClick as (e: React.MouseEvent) => void)(e);
    }
  };

  const handleFocus = (e: React.FocusEvent) => {
    if (trigger === 'focus') showTooltip();
    if (typeof childProps.onFocus === 'function') {
      (childProps.onFocus as (e: React.FocusEvent) => void)(e);
    }
  };

  const handleBlur = (e: React.FocusEvent) => {
    if (trigger === 'focus') hideTooltip();
    if (typeof childProps.onBlur === 'function') {
      (childProps.onBlur as (e: React.FocusEvent) => void)(e);
    }
  };

  const triggerElement = React.cloneElement(children as React.ReactElement<any>, {
    ref: handleTriggerRef,
    onMouseEnter: handleMouseEnter,
    onMouseLeave: handleMouseLeave,
    onMouseMove: followCursor ? handleMouseMove : (children.props as Record<string, unknown>).onMouseMove,
    onClick: handleClick,
    onFocus: handleFocus,
    onBlur: handleBlur,
  });

  const tooltipClass = [
    'bitfun-tooltip',
    `bitfun-tooltip--${actualPlacement}`,
    visible && positionReady && 'bitfun-tooltip--visible',
    interactive && 'bitfun-tooltip--interactive',
    className
  ].filter(Boolean).join(' ');

  return (
    <>
      {triggerElement}
      {visible && createPortal(
        <div
          ref={tooltipRef}
          className={tooltipClass}
          onMouseEnter={interactive ? () => {
            if (hideTimeoutRef.current) {
              clearTimeout(hideTimeoutRef.current);
              hideTimeoutRef.current = null;
            }
          } : undefined}
          onMouseLeave={interactive ? scheduleHideTooltip : undefined}
          style={{
            position: 'fixed',
            top: `${position.top}px`,
            left: `${position.left}px`,
            zIndex: 9999,
          }}
        >
          <div className="bitfun-tooltip__content">{content}</div>
        </div>,
        document.body
      )}
    </>
  );
};

Tooltip.displayName = 'Tooltip';
