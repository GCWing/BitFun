/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { ChatInputBoostSubmenu } from './ChatInputBoostSubmenu';

describe('ChatInputBoostSubmenu', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('opens by click or keyboard and closes with the reverse arrow', async () => {
    await act(async () => {
      root.render(
        <ChatInputBoostSubmenu label="Additional modes" icon={<span>+</span>}>
          <button type="button" role="menuitem">Plan</button>
        </ChatInputBoostSubmenu>,
      );
    });

    const trigger = container.querySelector<HTMLElement>('[role="menuitem"]');
    const panel = container.querySelector<HTMLElement>('[role="menu"]');
    expect(trigger?.getAttribute('aria-expanded')).toBe('false');
    expect(panel?.getAttribute('aria-hidden')).toBe('true');

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(trigger?.getAttribute('aria-expanded')).toBe('true');
    expect(panel?.getAttribute('aria-hidden')).toBe('false');

    await act(async () => {
      trigger?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    });
    expect(trigger?.getAttribute('aria-expanded')).toBe('false');

    await act(async () => {
      trigger?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    });
    expect(trigger?.getAttribute('aria-expanded')).toBe('true');
  });
});
