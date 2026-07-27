import React, { ReactNode, useLayoutEffect, useRef, useState } from 'react';

interface SmoothHeightCollapseProps {
  isOpen: boolean;
  children?: ReactNode;
  className?: string;
  innerClassName?: string;
  durationMs?: number;
  disableAnimation?: boolean;
}

type CollapsePhase = 'open' | 'opening' | 'closed' | 'closing';

export const SmoothHeightCollapse: React.FC<SmoothHeightCollapseProps> = ({
  isOpen,
  children,
  className = '',
  innerClassName = '',
  durationMs = 260,
  disableAnimation = false,
}) => {
  const outerRef = useRef<HTMLDivElement>(null);
  const innerRef = useRef<HTMLDivElement>(null);
  const hasMountedRef = useRef(false);
  const previousIsOpenRef = useRef(isOpen);
  const [phase, setPhase] = useState<CollapsePhase>(() => (isOpen ? 'open' : 'closed'));
  const [height, setHeight] = useState<string>(() => (isOpen ? 'auto' : '0px'));
  const shouldRender = isOpen || phase !== 'closed';
  const prefersReducedMotion =
    typeof window !== 'undefined' &&
    (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false);
  const shouldAnimate = !disableAnimation && !prefersReducedMotion;

  useLayoutEffect(() => {
    const previousIsOpen = previousIsOpenRef.current;
    previousIsOpenRef.current = isOpen;

    if (!hasMountedRef.current) {
      hasMountedRef.current = true;
      return;
    }

    const inner = innerRef.current;
    if (!inner) {
      return;
    }

    let frameId = 0;
    let timeoutId = 0;

    if (!shouldAnimate || previousIsOpen === isOpen) {
      setPhase(isOpen ? 'open' : 'closed');
      setHeight(isOpen ? 'auto' : '0px');
      return;
    }

    if (isOpen) {
      // Preserve the rendered height when a rapid click reverses an in-flight
      // close. Restarting from zero makes the body visibly snap.
      const startHeight = outerRef.current?.getBoundingClientRect().height ?? 0;
      setPhase('opening');
      setHeight(`${startHeight}px`);
      frameId = window.requestAnimationFrame(() => {
        setHeight(`${inner.scrollHeight}px`);
      });
      timeoutId = window.setTimeout(() => {
        setPhase('open');
        setHeight('auto');
      }, durationMs);
    } else {
      const startHeight =
        outerRef.current?.getBoundingClientRect().height ??
        inner.getBoundingClientRect().height;
      setPhase('closing');
      setHeight(`${startHeight}px`);
      frameId = window.requestAnimationFrame(() => {
        setHeight('0px');
      });
      timeoutId = window.setTimeout(() => {
        setPhase('closed');
      }, durationMs);
    }

    return () => {
      window.cancelAnimationFrame(frameId);
      window.clearTimeout(timeoutId);
    };
  }, [disableAnimation, durationMs, isOpen, shouldAnimate]);

  useLayoutEffect(() => {
    const inner = innerRef.current;
    if (
      !inner ||
      (phase !== 'opening' && phase !== 'open') ||
      !shouldAnimate ||
      typeof ResizeObserver === 'undefined'
    ) {
      return;
    }

    const observer = new ResizeObserver(() => {
      setHeight(phase === 'opening' ? `${inner.scrollHeight}px` : 'auto');
    });
    observer.observe(inner);
    return () => observer.disconnect();
  }, [phase, shouldAnimate]);

  return (
    <div
      ref={outerRef}
      className={[
        'smooth-height-collapse',
        `smooth-height-collapse--${phase}`,
        !shouldAnimate ? 'smooth-height-collapse--instant' : '',
        className,
      ].filter(Boolean).join(' ')}
      style={{
        height,
        transitionDuration: `${durationMs}ms, 180ms, ${durationMs}ms`,
      }}
      aria-hidden={!isOpen}
    >
      {shouldRender && (
        <div ref={innerRef} className={`smooth-height-collapse__inner ${innerClassName}`.trim()}>
          {children}
        </div>
      )}
    </div>
  );
};
