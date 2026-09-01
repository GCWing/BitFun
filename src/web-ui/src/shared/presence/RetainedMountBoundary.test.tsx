// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_RETAINED_MOUNT_MS,
  RetainedMountBoundary,
} from './RetainedMountBoundary';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('RetainedMountBoundary', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  const renderBoundary = (
    present: boolean,
    retainForMs?: number,
    minimumRetainMs?: number,
  ) => {
    root.render(
      <RetainedMountBoundary
        present={present}
        retainForMs={retainForMs}
        minimumRetainMs={minimumRetainMs}
      >
        <div data-testid="retained-child">Dialog owner</div>
      </RetainedMountBoundary>,
    );
  };

  it('does not render before its first activation', () => {
    act(() => renderBoundary(false));

    expect(container.querySelector('[data-testid="retained-child"]')).toBeNull();
  });

  it('retains children for at least 200 ms after deactivation', () => {
    act(() => renderBoundary(true, 50));
    expect(container.querySelector('[data-testid="retained-child"]')).not.toBeNull();

    act(() => renderBoundary(false, 50));
    act(() => vi.advanceTimersByTime(DEFAULT_RETAINED_MOUNT_MS - 1));
    expect(container.querySelector('[data-testid="retained-child"]')).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(container.querySelector('[data-testid="retained-child"]')).toBeNull();
  });

  it('honors a longer owner-specific exit duration', () => {
    act(() => renderBoundary(true, 320));
    act(() => renderBoundary(false, 320));
    act(() => vi.advanceTimersByTime(319));
    expect(container.querySelector('[data-testid="retained-child"]')).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(container.querySelector('[data-testid="retained-child"]')).toBeNull();
  });

  it('supports an explicit shorter floor for compact primitives', () => {
    act(() => renderBoundary(true, 120, 120));
    act(() => renderBoundary(false, 120, 120));
    act(() => vi.advanceTimersByTime(119));
    expect(container.querySelector('[data-testid="retained-child"]')).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(container.querySelector('[data-testid="retained-child"]')).toBeNull();
  });

  it('cancels a pending unmount when reactivated', () => {
    act(() => renderBoundary(true));
    act(() => renderBoundary(false));
    act(() => vi.advanceTimersByTime(120));
    act(() => renderBoundary(true));
    act(() => vi.advanceTimersByTime(DEFAULT_RETAINED_MOUNT_MS));

    expect(container.querySelector('[data-testid="retained-child"]')).not.toBeNull();
  });
});
