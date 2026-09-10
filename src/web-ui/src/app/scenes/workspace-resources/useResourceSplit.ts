import { useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from 'react';

export function useResourceSplit(
  containerRef: RefObject<HTMLDivElement>,
  fraction: number,
  commit: (fraction: number) => void,
) {
  const [preview, setPreview] = useState<number | null>(null);
  const drag = useRef<{ pointerId: number; y: number; fraction: number; height: number } | null>(null);
  const latest = useRef(fraction);
  const clamp = (value: number) => Math.max(0.15, Math.min(0.75, value));

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !containerRef.current) return;
    event.preventDefault();
    const height = containerRef.current.getBoundingClientRect().height;
    if (height <= 0) return;
    drag.current = { pointerId: event.pointerId, y: event.clientY, fraction, height };
    latest.current = fraction;
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const start = drag.current;
    if (!start || start.pointerId !== event.pointerId) return;
    latest.current = clamp(start.fraction - (event.clientY - start.y) / start.height);
    setPreview(latest.current);
  };
  const finish = (event: PointerEvent<HTMLDivElement>) => {
    if (drag.current?.pointerId !== event.pointerId) return;
    drag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    commit(latest.current);
    setPreview(null);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 0.1 : 0.03;
    const next = event.key === 'ArrowUp' ? fraction + step : event.key === 'ArrowDown' ? fraction - step
      : event.key === 'Home' ? 0.15 : event.key === 'End' ? 0.75 : null;
    if (next === null) return;
    event.preventDefault();
    commit(clamp(next));
  };
  return {
    fraction: preview ?? fraction,
    handlers: { onPointerDown, onPointerMove, onPointerUp: finish, onPointerCancel: finish, onLostPointerCapture: finish, onKeyDown },
  };
}
