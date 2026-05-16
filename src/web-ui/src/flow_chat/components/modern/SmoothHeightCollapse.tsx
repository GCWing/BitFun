import React, { ReactNode, useLayoutEffect, useRef, useState } from 'react';

interface SmoothHeightCollapseProps {
  isOpen: boolean;
  children?: ReactNode;
  className?: string;
  innerClassName?: string;
  durationMs?: number;
}

type CollapsePhase = 'open' | 'opening' | 'closed' | 'closing';

export const SmoothHeightCollapse: React.FC<SmoothHeightCollapseProps> = ({
  isOpen,
  children,
  className = '',
  innerClassName = '',
  durationMs = 260,
}) => {
  const innerRef = useRef<HTMLDivElement>(null);
  const [phase, setPhase] = useState<CollapsePhase>(() => (isOpen ? 'open' : 'closed'));
  const [height, setHeight] = useState<string>(() => (isOpen ? 'auto' : '0px'));
  const shouldRender = isOpen || phase !== 'closed';

  useLayoutEffect(() => {
    const inner = innerRef.current;
    if (!inner) {
      return;
    }

    let frameId = 0;
    let timeoutId = 0;

    const reduceMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
    if (reduceMotion) {
      setPhase(isOpen ? 'open' : 'closed');
      setHeight(isOpen ? 'auto' : '0px');
      return;
    }

    if (isOpen) {
      setPhase('opening');
      setHeight('0px');
      frameId = window.requestAnimationFrame(() => {
        setHeight(`${inner.scrollHeight}px`);
      });
      timeoutId = window.setTimeout(() => {
        setPhase('open');
        setHeight('auto');
      }, durationMs);
    } else {
      const startHeight = inner.getBoundingClientRect().height;
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
  }, [durationMs, isOpen]);

  useLayoutEffect(() => {
    const inner = innerRef.current;
    if (!inner || phase !== 'open') {
      return;
    }

    const observer = new ResizeObserver(() => {
      setHeight('auto');
    });
    observer.observe(inner);
    return () => observer.disconnect();
  }, [phase, children]);

  return (
    <div
      className={`smooth-height-collapse smooth-height-collapse--${phase} ${className}`.trim()}
      style={{
        height,
        ['--smooth-height-collapse-duration' as string]: `${durationMs}ms`,
      }}
      aria-hidden={!isOpen && phase === 'closed'}
    >
      {shouldRender && (
        <div ref={innerRef} className={`smooth-height-collapse__inner ${innerClassName}`.trim()}>
          {children}
        </div>
      )}
    </div>
  );
};
