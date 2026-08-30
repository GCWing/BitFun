// @vitest-environment jsdom
import React, { act, useRef, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MenuPopover, ThemeRoot, type MenuEntry } from '@bitfun/ui';

describe('public MenuPopover', () => {
  let host: HTMLDivElement;
  let root: Root;
  const selected = vi.fn();
  const items: MenuEntry[] = [
    { id: 'disabled', label: 'Unavailable', disabled: true },
    { id: 'copy', label: 'Copy', onSelect: selected },
    { id: 'share', label: 'Share', submenu: [{ id: 'email', label: 'Email', onSelect: selected }, { id: 'link', label: 'Link' }] },
  ];
  const key = (value: string, isComposing = false) => act(() => { document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: value, bubbles: true, isComposing })); });
  function Harness() {
    const anchorRef = useRef<HTMLButtonElement>(null);
    const [open, setOpen] = useState(false);
    return <ThemeRoot><button ref={anchorRef} onClick={() => setOpen(!open)}>Commands</button><MenuPopover items={items} open={open} onClose={() => setOpen(false)} anchorRef={anchorRef} aria-label="Commands" /></ThemeRoot>;
  }
  const open = () => {
    act(() => root.render(<Harness />));
    const trigger = host.querySelector<HTMLButtonElement>('button')!;
    act(() => { trigger.focus(); trigger.click(); });
    return trigger;
  };
  beforeEach(() => { host = document.createElement('div'); document.body.append(host); root = createRoot(host); selected.mockClear(); });
  afterEach(() => { act(() => root.unmount()); host.remove(); vi.restoreAllMocks(); });

  it('portals inside the theme scope, supports typeahead and dispatches after focus return', () => {
    const trigger = open();
    expect(host.querySelector('[role="menu"]')?.closest('[data-bf-design-system-root]')).not.toBeNull();
    expect(document.activeElement?.textContent).toBe('Copy');
    key('s'); expect(document.activeElement?.textContent).toBe('Share');
    key('Home'); key('Enter', true); expect(selected).not.toHaveBeenCalled();
    key('Enter'); expect(selected).toHaveBeenCalledOnce(); expect(document.activeElement).toBe(trigger);
  });
  it('opens and backs out of nested menus and closes the tree on Tab', () => {
    const trigger = open(); key('End'); key('ArrowRight');
    expect(document.activeElement?.textContent).toBe('Email');
    key('ArrowDown'); expect(document.activeElement?.textContent).toBe('Link');
    key('ArrowLeft'); expect(document.activeElement?.textContent).toBe('Share');
    key('Tab'); expect(document.activeElement).toBe(trigger);
    expect(host.querySelector('[role="menu"]')?.getAttribute('aria-hidden')).toBe('true');
  });
  it('flips the root above its anchor and submenus left near viewport edges', () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1000 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 600 });
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      if (this.getAttribute('role') === 'menu') return new DOMRect(0, 0, 220, 200);
      return new DOMRect(940, 560, 40, 32);
    });
    open();
    const menu = host.querySelector<HTMLElement>('[role="menu"]')!;
    expect(menu.dataset.placement).toBe('top');
    expect(Number.parseFloat(menu.style.left)).toBeLessThanOrEqual(772);
    key('End'); key('ArrowRight');
    const submenu = host.querySelector<HTMLElement>('[role="menu"][aria-label="Share"]')!;
    expect(submenu.dataset.placement).toBe('left');
    expect(Number.parseFloat(submenu.style.top)).toBeLessThanOrEqual(392);
  });
  it('opens a coordinate menu initially and releases it on outside interaction', () => {
    const close = vi.fn();
    act(() => root.render(<MenuPopover items={items} open onClose={close} position={{ x: 50, y: 50 }} />));
    expect(document.activeElement?.textContent).toBe('Copy');
    act(() => document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true })));
    expect(close).toHaveBeenCalledOnce();
  });
});
