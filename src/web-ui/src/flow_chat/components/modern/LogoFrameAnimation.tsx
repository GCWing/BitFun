import React, { useEffect, useMemo, useState } from 'react';
import './LogoFrameAnimation.scss';

interface LogoFrameAnimationProps {
  size?: number;
  intervalMs?: number;
  className?: string;
}

const FRAME_COUNT = 8;
const FRAME_PATHS = Array.from(
  { length: FRAME_COUNT },
  (_, index) => `/logo-animation/logo-frame-${String(index + 1).padStart(2, '0')}.png`,
);

export const LogoFrameAnimation: React.FC<LogoFrameAnimationProps> = ({
  size = 28,
  intervalMs = 160,
  className,
}) => {
  const [frameIndex, setFrameIndex] = useState(0);

  const prefersReducedMotion = useMemo(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return false;
    }
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  }, []);

  useEffect(() => {
    FRAME_PATHS.forEach((src) => {
      const image = new Image();
      image.src = src;
    });
  }, []);

  useEffect(() => {
    if (prefersReducedMotion) {
      setFrameIndex(0);
      return;
    }

    const timer = window.setInterval(() => {
      setFrameIndex((prev) => (prev + 1) % FRAME_PATHS.length);
    }, intervalMs);

    return () => window.clearInterval(timer);
  }, [intervalMs, prefersReducedMotion]);

  const classNames = ['logo-frame-animation', className].filter(Boolean).join(' ');

  return (
    <span
      className={classNames}
      style={{ '--logo-frame-animation-size': `${size}px` } as React.CSSProperties}
      aria-hidden="true"
    >
      <img
        className="logo-frame-animation__image"
        src={FRAME_PATHS[frameIndex]}
        alt=""
        draggable={false}
      />
    </span>
  );
};
