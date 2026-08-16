import React, { useLayoutEffect, useRef, useState } from 'react';
import './ViewTransitionBoundary.scss';

export const VIEW_TRANSITION_EXIT_MS = 200;

interface RetainedView {
  children: React.ReactNode;
  incomingKey: React.Key;
  outgoingKey: React.Key;
  phase: 'preparing' | 'running';
  sequence: number;
}

export interface ViewTransitionBoundaryProps
  extends Omit<React.HTMLAttributes<HTMLDivElement>, 'children'> {
  /** Identity of the visible view. A new key starts a pointer transition. */
  viewKey: React.Key;
  children: React.ReactNode;
  /** Keep false for keyboard, shortcut, and programmatic navigation. */
  animate?: boolean;
  /** Class applied to both the current and retained outgoing view wrappers. */
  viewClassName?: string;
}

function scheduleAnimationFrame(callback: FrameRequestCallback): number {
  if (typeof globalThis.requestAnimationFrame === 'function') {
    return globalThis.requestAnimationFrame(callback);
  }
  return window.setTimeout(() => (
    callback(globalThis.performance?.now() ?? Date.now())
  ), 16);
}

function cancelScheduledAnimationFrame(handle: number): void {
  if (typeof globalThis.cancelAnimationFrame === 'function') {
    globalThis.cancelAnimationFrame(handle);
    return;
  }
  window.clearTimeout(handle);
}

/**
 * Retains the outgoing React tree just long enough to bridge an occasional
 * pointer-driven content replacement. The outgoing tree is immediately inert;
 * keyboard and programmatic changes remain synchronous and single-mounted.
 */
export const ViewTransitionBoundary: React.FC<ViewTransitionBoundaryProps> = ({
  viewKey,
  children,
  animate = false,
  className = '',
  viewClassName = '',
  ...rootProps
}) => {
  const currentKeyRef = useRef<React.Key>(viewKey);
  const currentChildrenRef = useRef<React.ReactNode>(children);
  const sequenceRef = useRef(0);
  const frameRef = useRef<number | null>(null);
  const exitTimerRef = useRef<number | null>(null);
  const [transition, setTransition] = useState<RetainedView | null>(null);

  const keyChanged = currentKeyRef.current !== viewKey;
  if (!keyChanged) {
    currentChildrenRef.current = children;
  }

  const pendingTransition: RetainedView | null = keyChanged && animate
    ? {
        children: currentChildrenRef.current,
        incomingKey: viewKey,
        outgoingKey: currentKeyRef.current,
        phase: 'preparing',
        sequence: sequenceRef.current + 1,
      }
    : transition?.incomingKey === viewKey
      ? transition
      : null;

  useLayoutEffect(() => {
    if (currentKeyRef.current === viewKey) return;

    const outgoingKey = currentKeyRef.current;
    const outgoingChildren = currentChildrenRef.current;
    currentKeyRef.current = viewKey;
    currentChildrenRef.current = children;

    if (frameRef.current !== null) {
      cancelScheduledAnimationFrame(frameRef.current);
      frameRef.current = null;
    }
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
      exitTimerRef.current = null;
    }

    if (!animate) {
      setTransition(null);
      return;
    }

    const sequence = ++sequenceRef.current;
    setTransition({
      children: outgoingChildren,
      incomingKey: viewKey,
      outgoingKey,
      phase: 'preparing',
      sequence,
    });

    frameRef.current = scheduleAnimationFrame(() => {
      // A second frame guarantees the preparing styles have reached a paint.
      // React may otherwise flush an interaction effect before the browser
      // paints, collapsing the starting and running states into one frame.
      frameRef.current = scheduleAnimationFrame(() => {
        frameRef.current = null;
        setTransition(current => (
          current?.sequence === sequence
            ? { ...current, phase: 'running' }
            : current
        ));
        exitTimerRef.current = window.setTimeout(() => {
          exitTimerRef.current = null;
          setTransition(current => (
            current?.sequence === sequence ? null : current
          ));
        }, VIEW_TRANSITION_EXIT_MS);
      });
    });
  }, [animate, children, viewKey]);

  useLayoutEffect(() => () => {
    if (frameRef.current !== null) {
      cancelScheduledAnimationFrame(frameRef.current);
    }
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
    }
  }, []);

  const rootClassName = [
    'bitfun-view-transition-boundary',
    className,
  ].filter(Boolean).join(' ');
  const currentClassName = [
    'bitfun-view-transition-boundary__view',
    pendingTransition && 'bitfun-view-transition-boundary__view--incoming',
    viewClassName,
  ].filter(Boolean).join(' ');
  const outgoingClassName = [
    'bitfun-view-transition-boundary__view',
    'bitfun-view-transition-boundary__view--outgoing',
    viewClassName,
  ].filter(Boolean).join(' ');

  return (
    <div
      className={rootClassName}
      data-bf-component="view-transition-boundary"
      data-bf-part="root"
      {...rootProps}
      data-motion="presence"
      data-view-transition-phase={pendingTransition?.phase}
    >
      {pendingTransition ? (
        <div
          key={pendingTransition.outgoingKey}
          className={outgoingClassName}
          aria-hidden="true"
          data-bf-component="view-transition-boundary"
          data-bf-part="view"
          {...{ inert: '' }}
        >
          {pendingTransition.children}
        </div>
      ) : null}
      <div
        key={viewKey}
        className={currentClassName}
        data-bf-component="view-transition-boundary"
        data-bf-part="view"
      >
        {children}
      </div>
    </div>
  );
};

export default ViewTransitionBoundary;
