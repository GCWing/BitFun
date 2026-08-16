// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  VIEW_TRANSITION_EXIT_MS,
  ViewTransitionBoundary,
} from './ViewTransitionBoundary';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('ViewTransitionBoundary', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => (
      window.setTimeout(() => callback(performance.now()), 16)
    ));
    vi.stubGlobal('cancelAnimationFrame', (handle: number) => window.clearTimeout(handle));
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  const renderView = (viewKey: string, animate: boolean) => {
    root.render(
      <ViewTransitionBoundary viewKey={viewKey} animate={animate}>
        <button type="button" data-testid={`view-${viewKey}`}>{viewKey}</button>
      </ViewTransitionBoundary>,
    );
  };

  it('switches keyboard and programmatic views immediately', () => {
    act(() => renderView('first', false));
    act(() => renderView('second', false));

    expect(container.querySelector('[data-testid="view-first"]')).toBeNull();
    expect(container.querySelector('[data-testid="view-second"]')).not.toBeNull();
  });

  it('retains an inert outgoing view for a pointer transition', () => {
    act(() => renderView('first', false));
    act(() => renderView('second', true));

    const outgoing = container.querySelector('.bitfun-view-transition-boundary__view--outgoing');
    expect(outgoing?.hasAttribute('inert')).toBe(true);
    expect(outgoing?.getAttribute('aria-hidden')).toBe('true');
    expect(container.querySelectorAll('.bitfun-view-transition-boundary__view')).toHaveLength(2);

    act(() => vi.advanceTimersByTime(32));
    expect(container.querySelector('[data-view-transition-phase="running"]')).not.toBeNull();

    act(() => vi.advanceTimersByTime(VIEW_TRANSITION_EXIT_MS));
    expect(container.querySelector('[data-testid="view-first"]')).toBeNull();
    expect(container.querySelectorAll('.bitfun-view-transition-boundary__view')).toHaveLength(1);
  });

  it('retargets a rapid pointer switch to the latest view', () => {
    act(() => renderView('first', false));
    act(() => renderView('second', true));
    act(() => vi.advanceTimersByTime(32));
    act(() => renderView('third', true));
    act(() => vi.advanceTimersByTime(32 + VIEW_TRANSITION_EXIT_MS));

    expect(container.querySelector('[data-testid="view-first"]')).toBeNull();
    expect(container.querySelector('[data-testid="view-second"]')).toBeNull();
    expect(container.querySelector('[data-testid="view-third"]')).not.toBeNull();
  });
});
