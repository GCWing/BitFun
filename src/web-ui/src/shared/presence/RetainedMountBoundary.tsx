import React, { useEffect, useState } from 'react';

/** Default grace period for product surfaces that animate while closing. */
export const DEFAULT_RETAINED_MOUNT_MS = 200;

export interface RetainedMountBoundaryProps {
  present: boolean;
  children: React.ReactNode;
  /** Owner-specific retention window, clamped to `minimumRetainMs`. */
  retainForMs?: number;
  /** Minimum retention floor for the owning product surface. */
  minimumRetainMs?: number;
}

/**
 * Delays owner-level unmounting without eagerly rendering lazy children.
 * Reopening during the retention window cancels the pending unmount.
 */
export const RetainedMountBoundary: React.FC<RetainedMountBoundaryProps> = ({
  present,
  children,
  retainForMs,
  minimumRetainMs = DEFAULT_RETAINED_MOUNT_MS,
}) => {
  const [isRetained, setIsRetained] = useState(present);

  useEffect(() => {
    if (present) {
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
        minimumRetainMs,
        retainForMs ?? minimumRetainMs,
      ),
    );
    return () => window.clearTimeout(timer);
  }, [isRetained, minimumRetainMs, present, retainForMs]);

  if (!present && !isRetained) {
    return null;
  }

  return <>{children}</>;
};

export default RetainedMountBoundary;
