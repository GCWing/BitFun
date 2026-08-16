import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ContextMenu } from './ContextMenu';
import type { ContextMenuItem } from './types';

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({ error: vi.fn() }),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('ContextMenu presence', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>');
    globalThis.window = dom.window as unknown as Window & typeof globalThis;
    globalThis.document = dom.window.document;
    window.requestAnimationFrame = callback => window.setTimeout(() => callback(0), 0);
    window.cancelAnimationFrame = handle => window.clearTimeout(handle);
    container = document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    vi.useRealTimers();
    dom.window.close();
  });

  it('keeps the menu mounted while its exit transition runs', () => {
    const trigger = document.createElement('button');
    document.body.appendChild(trigger);
    trigger.focus();
    const props = {
      items: [{ id: 'copy', label: 'Copy' }],
      position: { x: 20, y: 20 },
      context: {},
      onClose: vi.fn(),
    };

    act(() => root.render(<ContextMenu {...props} visible />));
    act(() => vi.runOnlyPendingTimers());
    act(() => vi.runOnlyPendingTimers());
    expect(container.querySelector('[role="menu"]')?.getAttribute('data-state')).toBe('entered');

    act(() => root.render(<ContextMenu {...props} visible={false} />));
    const exitingMenu = container.querySelector('[role="menu"]');
    expect(exitingMenu?.getAttribute('data-state')).toBe('exiting');
    expect(exitingMenu?.getAttribute('aria-hidden')).toBe('true');
    expect(exitingMenu?.hasAttribute('inert')).toBe(true);
    expect(document.activeElement).toBe(trigger);

    act(() => vi.advanceTimersByTime(100));
    expect(container.querySelector('[role="menu"]')).toBeNull();
  });

  it('uses roving focus and skips disabled items and separators', () => {
    const items: ContextMenuItem[] = [
      { id: 'disabled', label: 'Disabled', disabled: true },
      { id: 'separator', label: '', separator: true },
      { id: 'copy', label: 'Copy' },
      { id: 'rename', label: 'Rename' },
      { id: 'delete', label: 'Delete' },
    ];

    act(() => root.render(
      <ContextMenu items={items} position={{ x: 0, y: 0 }} visible onClose={vi.fn()} />,
    ));

    const menuItems = Array.from(container.querySelectorAll<HTMLElement>('[role="menuitem"]'));
    expect(document.activeElement?.textContent).toContain('Copy');
    expect(menuItems[0].tabIndex).toBe(-1);
    expect(menuItems[1].tabIndex).toBe(0);

    act(() => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true })));
    expect(document.activeElement?.textContent).toContain('Rename');

    act(() => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'End', bubbles: true })));
    expect(document.activeElement?.textContent).toContain('Delete');

    act(() => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Home', bubbles: true })));
    expect(document.activeElement?.textContent).toContain('Copy');

    act(() => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true })));
    expect(document.activeElement?.textContent).toContain('Delete');
    expect(container.querySelector('[role="separator"]')).not.toBeNull();
  });

  it('activates focused items with Enter and Space', async () => {
    const onCopy = vi.fn();
    const onDelete = vi.fn();
    const onItemClick = vi.fn();
    const onClose = vi.fn();
    const trigger = document.createElement('button');
    document.body.appendChild(trigger);
    trigger.focus();
    const items: ContextMenuItem[] = [
      { id: 'copy', label: 'Copy', onClick: onCopy },
      { id: 'delete', label: 'Delete', onClick: onDelete },
    ];

    act(() => root.render(
      <ContextMenu
        items={items}
        position={{ x: 0, y: 0 }}
        visible
        onClose={onClose}
        onItemClick={onItemClick}
      />,
    ));

    await act(async () => {
      document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    expect(onCopy).toHaveBeenCalledOnce();
    expect(onItemClick).toHaveBeenCalledWith(items[0], undefined);
    expect(onClose).toHaveBeenCalledOnce();
    expect(document.activeElement).toBe(trigger);

    act(() => {
      const firstItem = container.querySelector<HTMLElement>('[role="menuitem"]');
      firstItem?.focus();
      document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    });
    await act(async () => {
      document.dispatchEvent(new window.KeyboardEvent('keydown', { key: ' ', bubbles: true }));
    });
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it('restores the pre-open focus snapshot on Escape', () => {
    const trigger = document.createElement('button');
    document.body.appendChild(trigger);
    trigger.focus();
    const onClose = vi.fn();

    act(() => root.render(
      <ContextMenu
        items={[{ id: 'copy', label: 'Copy' }]}
        position={{ x: 0, y: 0 }}
        visible
        onClose={onClose}
      />,
    ));
    expect(document.activeElement?.getAttribute('role')).toBe('menuitem');

    act(() => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true })));

    expect(onClose).toHaveBeenCalledOnce();
    expect(document.activeElement).toBe(trigger);
  });

  it('opens a submenu from the keyboard and keeps keyboard handling in the owning menu', () => {
    const items: ContextMenuItem[] = [{
      id: 'share',
      label: 'Share',
      submenu: [
        { id: 'unavailable', label: 'Unavailable', disabled: true },
        { id: 'email', label: 'Email' },
        { id: 'link', label: 'Copy link' },
      ],
    }];

    act(() => root.render(
      <ContextMenu items={items} position={{ x: 0, y: 0 }} visible onClose={vi.fn()} />,
    ));
    const parentItem = container.querySelector<HTMLElement>('[role="menuitem"]');
    expect(parentItem?.getAttribute('aria-haspopup')).toBe('menu');

    act(() => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true })));
    act(() => vi.runOnlyPendingTimers());

    expect(parentItem?.getAttribute('aria-expanded')).toBe('true');
    expect(document.activeElement?.textContent).toContain('Email');

    act(() => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true })));
    expect(document.activeElement?.textContent).toContain('Copy link');

    act(() => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true })));
    expect(document.activeElement).toBe(parentItem);
    expect(parentItem?.getAttribute('aria-expanded')).toBe('false');
  });
});
