// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import ConfigCollectionItem from './ConfigCollectionItem';

describe('ConfigCollectionItem', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it('uses an independent native button for expandable details', () => {
    vi.useFakeTimers();
    act(() => {
      root.render(
        <ConfigCollectionItem
          label="OpenCode MCP"
          control={<button type="button">Active</button>}
          details={<span>Configuration location</span>}
        />,
      );
    });

    const row = container.querySelector<HTMLElement>('.bitfun-collection-item__row');
    const toggle = container.querySelector<HTMLButtonElement>('.bitfun-collection-item__details-toggle');
    const control = Array.from(container.querySelectorAll('button'))
      .find((button) => button.textContent === 'Active');
    expect(row?.getAttribute('role')).toBeNull();
    expect(toggle?.type).toBe('button');
    expect(toggle?.getAttribute('aria-expanded')).toBe('false');
    expect(toggle?.getAttribute('aria-controls')).toBeTruthy();

    act(() => {
      control?.click();
    });
    expect(container.querySelector('.bitfun-collection-item__details-collapse')).toBeNull();

    act(() => {
      toggle?.click();
    });

    const details = container.querySelector<HTMLElement>('.bitfun-collection-item__details-collapse');
    expect(toggle?.getAttribute('aria-expanded')).toBe('true');
    expect(container.textContent).toContain('Configuration location');
    expect(details?.dataset.open).toBe('true');
    expect(details?.getAttribute('aria-hidden')).toBe('false');

    act(() => {
      toggle?.click();
    });
    expect(details?.dataset.open).toBe('false');
    expect(details?.getAttribute('aria-hidden')).toBe('true');
    expect(details?.hasAttribute('inert')).toBe(true);

    act(() => {
      vi.advanceTimersByTime(180);
    });
    expect(container.querySelector('.bitfun-collection-item__details-collapse')).toBeNull();
  });

  it('does not expose disabled details as an interactive control', () => {
    act(() => {
      root.render(
        <ConfigCollectionItem
          label="Unavailable MCP"
          control={<span>Unavailable</span>}
          details={<span>Configuration location</span>}
          disabled
        />,
      );
    });

    const toggle = container.querySelector<HTMLButtonElement>('.bitfun-collection-item__details-toggle');
    expect(toggle?.disabled).toBe(true);
    expect(toggle?.getAttribute('aria-expanded')).toBe('false');

    act(() => {
      toggle?.click();
    });

    expect(toggle?.getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelector('.bitfun-collection-item__details-collapse')).toBeNull();
  });
});
