import React, { useLayoutEffect, useRef, useState } from 'react';
import './NavigationTransitionBoundary.scss';

export const NAVIGATION_TRANSITION_EXIT_MS = 200;

interface RetainedView {
  children: React.ReactNode;
  incomingKey: React.Key;
  outgoingKey: React.Key;
  phase: 'preparing' | 'running';
  sequence: number;
}

export interface NavigationTransitionBoundaryProps
  extends Omit<React.HTMLAttributes<HTMLDivElement>, 'children'> {
  /** Identity of the visible view. A new key starts a pointer transition. */
  transitionKey: React.Key;
  children: React.ReactNode;
  /** Only direct pointer navigation retains and animates the outgoing tree. */
  motion?: 'none' | 'pointer';
  /** Class applied to both the current and retained outgoing view wrappers. */
  layerClassName?: string;
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
export const NavigationTransitionBoundary: React.FC<NavigationTransitionBoundaryProps> = ({
  transitionKey,
  children,
  motion = 'none',
  className = '',
  layerClassName = '',
  ...rootProps
}) => {
  const currentKeyRef = useRef<React.Key>(transitionKey);
  const currentChildrenRef = useRef<React.ReactNode>(children);
  const sequenceRef = useRef(0);
  const frameRef = useRef<number | null>(null);
  const exitTimerRef = useRef<number | null>(null);
  const [transition, setTransition] = useState<RetainedView | null>(null);

  const keyChanged = currentKeyRef.current !== transitionKey;
  if (!keyChanged) {
    currentChildrenRef.current = children;
  }

  const pendingTransition: RetainedView | null = keyChanged && motion === 'pointer'
    ? {
        children: currentChildrenRef.current,
        incomingKey: transitionKey,
        outgoingKey: currentKeyRef.current,
        phase: 'preparing',
        sequence: sequenceRef.current + 1,
      }
    : transition?.incomingKey === transitionKey
      ? transition
      : null;

  useLayoutEffect(() => {
    if (currentKeyRef.current === transitionKey) return;

    const outgoingKey = currentKeyRef.current;
    const outgoingChildren = currentChildrenRef.current;
    currentKeyRef.current = transitionKey;
    currentChildrenRef.current = children;

    if (frameRef.current !== null) {
      cancelScheduledAnimationFrame(frameRef.current);
      frameRef.current = null;
    }
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
      exitTimerRef.current = null;
    }

    if (motion !== 'pointer') {
      setTransition(null);
      return;
    }

    const sequence = ++sequenceRef.current;
    setTransition({
      children: outgoingChildren,
      incomingKey: transitionKey,
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
        }, NAVIGATION_TRANSITION_EXIT_MS);
      });
    });
  }, [children, motion, transitionKey]);

  useLayoutEffect(() => () => {
    if (frameRef.current !== null) {
      cancelScheduledAnimationFrame(frameRef.current);
    }
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
    }
  }, []);

  const rootClassName = [
    'bitfun-navigation-transition-boundary',
    className,
  ].filter(Boolean).join(' ');
  const currentClassName = [
    'bitfun-navigation-transition-boundary__layer',
    pendingTransition && 'bitfun-navigation-transition-boundary__layer--incoming',
    layerClassName,
  ].filter(Boolean).join(' ');
  const outgoingClassName = [
    'bitfun-navigation-transition-boundary__layer',
    'bitfun-navigation-transition-boundary__layer--outgoing',
    layerClassName,
  ].filter(Boolean).join(' ');

  return (
    <div
      className={rootClassName}
      data-bf-component="navigation-transition-boundary"
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
          data-bf-component="navigation-transition-boundary"
          data-bf-part="layer"
          {...{ inert: '' }}
        >
          {pendingTransition.children}
        </div>
      ) : null}
      <div
        key={transitionKey}
        className={currentClassName}
        data-bf-component="navigation-transition-boundary"
        data-bf-part="layer"
      >
        {children}
      </div>
    </div>
  );
};

export default NavigationTransitionBoundary;
