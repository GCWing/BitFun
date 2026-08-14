import React, { useEffect, useState } from 'react';

/**
 * The shared Modal exit animation lasts 180 ms. Keep its owner mounted a little
 * longer so the Modal can finish its own exit lifecycle before React removes it.
 */
export const PRESENCE_BOUNDARY_MIN_EXIT_MS = 200;

export interface PresenceBoundaryProps {
  active: boolean;
  children: React.ReactNode;
  /** Owner-specific exit window, clamped to `minimumExitDurationMs`. */
  exitDurationMs?: number;
  /**
   * Component-specific retention floor. Keep the 200 ms default for Modal;
   * shorter primitives may opt into their own measured CSS exit window.
   */
  minimumExitDurationMs?: number;
}

/**
 * Delays owner-level unmounting without eagerly rendering lazy children.
 * Reopening during the retention window cancels the pending unmount.
 */
export const PresenceBoundary: React.FC<PresenceBoundaryProps> = ({
  active,
  children,
  exitDurationMs,
  minimumExitDurationMs = PRESENCE_BOUNDARY_MIN_EXIT_MS,
}) => {
  const [isRetained, setIsRetained] = useState(active);

  useEffect(() => {
    if (active) {
      setIsRetained(true);
      return;
    }

    if (!isRetained) {
      return;
    }

    const timer = window.setTimeout(
      () => setIsRetained(false),
      Math.max(
        0,
        minimumExitDurationMs,
        exitDurationMs ?? minimumExitDurationMs,
      ),
    );
    return () => window.clearTimeout(timer);
  }, [active, exitDurationMs, isRetained, minimumExitDurationMs]);

  if (!active && !isRetained) {
    return null;
  }

  return <>{children}</>;
};

export default PresenceBoundary;
