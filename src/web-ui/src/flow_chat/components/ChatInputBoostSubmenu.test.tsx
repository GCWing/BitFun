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

  it.each([
    { label: 'Skills', parentLeft: 30, submenuLeft: 260, gapX: 255 },
    { label: 'Additional modes', parentLeft: 650, submenuLeft: 420, gapX: 645 },
  ])('retains gap protection in the extracted $label submenu', ({ label, parentLeft, submenuLeft, gapX }) => {
    act(() => root.render(
      <ChatInputBoostSubmenu label={label} icon={<span>+</span>}>
        <button type="button" role="menuitem">Plan</button>
      </ChatInputBoostSubmenu>,
    ));

    const host = container.querySelector<HTMLElement>('.bitfun-chat-input__boost-submenu-host')!;
    const shell = container.querySelector<HTMLElement>('.bitfun-chat-input__boost-submenu-shell')!;
    const trigger = container.querySelector<HTMLElement>('[aria-haspopup="menu"]')!;
    host.getBoundingClientRect = () => new DOMRect(parentLeft, 20, 220, 34);
    shell.getBoundingClientRect = () => new DOMRect(submenuLeft, 20, 220, 200);

    act(() => trigger.dispatchEvent(new MouseEvent('pointerover', {
      bubbles: true, clientX: parentLeft + 20, clientY: 30,
    })));
    expect(trigger.getAttribute('aria-expanded')).toBe('true');

    for (const element of [host, shell]) {
      act(() => element.dispatchEvent(new MouseEvent('pointerout', {
        bubbles: true, relatedTarget: document.body, clientX: gapX, clientY: 80,
      })));
      act(() => vi.advanceTimersByTime(1000));
      expect(trigger.getAttribute('aria-expanded')).toBe('true');
      act(() => shell.dispatchEvent(new MouseEvent('pointerover', {
        bubbles: true, relatedTarget: document.body, clientX: submenuLeft + 20, clientY: 80,
      })));
    }

    act(() => document.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true, clientX: gapX, clientY: 80,
    })));
    act(() => document.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true, clientX: gapX, clientY: 400,
    })));
    act(() => vi.advanceTimersByTime(180));
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
  });
});
