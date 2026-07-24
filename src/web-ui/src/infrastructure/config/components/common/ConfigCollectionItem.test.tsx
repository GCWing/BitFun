// @vitest-environment jsdom

import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
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
  });

  it('uses an independent native button for expandable details', () => {
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
    expect(container.textContent).not.toContain('Configuration location');

    act(() => {
      toggle?.click();
    });

    expect(toggle?.getAttribute('aria-expanded')).toBe('true');
    expect(container.textContent).toContain('Configuration location');
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
    expect(container.textContent).not.toContain('Configuration location');
  });
});
