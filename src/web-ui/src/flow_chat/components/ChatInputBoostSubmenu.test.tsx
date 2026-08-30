/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ChatInputBoostSubmenu } from './ChatInputBoostSubmenu';

describe('ChatInputBoostSubmenu', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
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
    expect(trigger?.getAttribute('aria-expanded')).toBe('false');
    expect(document.body.querySelector('.bitfun-chat-input__boost-submenu-panel')).toBeNull();

    await act(async () => {
      trigger?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(trigger?.getAttribute('aria-expanded')).toBe('true');
    expect(document.body.querySelector('.bitfun-chat-input__boost-submenu-panel')).not.toBeNull();

    await act(async () => {
      trigger?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    });
    expect(trigger?.getAttribute('aria-expanded')).toBe('false');

    await act(async () => {
      trigger?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    });
    expect(trigger?.getAttribute('aria-expanded')).toBe('true');
  });

  it.each([
    { label: 'Skills' },
    { label: 'Additional modes' },
  ])('keeps the extracted $label submenu open across pointer movement', ({ label }) => {
    act(() => root.render(
      <ChatInputBoostSubmenu label={label} icon={<span>+</span>}>
        <button type="button" role="menuitem">Plan</button>
      </ChatInputBoostSubmenu>,
    ));

    const trigger = container.querySelector<HTMLElement>('[aria-haspopup="menu"]')!;
    act(() => trigger.click());
    expect(trigger.getAttribute('aria-expanded')).toBe('true');

    act(() => document.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true, clientX: 0, clientY: 0,
    })));
    act(() => vi.advanceTimersByTime(1000));
    expect(trigger.getAttribute('aria-expanded')).toBe('true');

    act(() => trigger.click());
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
  });
});
