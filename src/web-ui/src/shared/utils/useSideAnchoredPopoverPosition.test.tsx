// @vitest-environment jsdom
import React, { act, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import { Menu, MenuItem } from '@openbitfun/ui';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useSideAnchoredPopoverPosition } from './useSideAnchoredPopoverPosition';

function Harness({ revision = 0 }: { revision?: number }) {
  const [open, setOpen] = useState(false);
  const anchorRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const layout = useSideAnchoredPopoverPosition({ open, anchorRef, popoverRef, layoutRevision: revision });
  return <>
    <button ref={anchorRef} onClick={() => setOpen(value => !value)}>Menu</button>
    {open && createPortal(
      <Menu ref={popoverRef} style={{
        position: 'fixed', top: layout?.top ?? 0, left: layout?.left ?? 0,
        visibility: layout ? 'visible' : 'hidden',
      }}>
        <MenuItem>Action</MenuItem>
      </Menu>, document.body,
    )}
  </>;
}

describe('side anchored menu positioning', () => {
  let root: Root;
  let container: HTMLDivElement;
  let anchor: DOMRect;
  let width: number;
  let height: number;
  let scale: number;
  let notifyResize: () => void;
  const observed = new Set<Element>();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    vi.stubGlobal('innerWidth', 900);
    vi.stubGlobal('innerHeight', 600);
    anchor = new DOMRect(250, 510, 24, 24);
    width = 240;
    height = 400;
    scale = 1;
    observed.clear();
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback: () => void) { notifyResize = callback; }
      observe(element: Element) { observed.add(element); }
      disconnect() { observed.clear(); }
    });
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      return this.getAttribute('role') === 'menu'
        ? new DOMRect(0, 0, width * scale, height * scale)
        : anchor;
    });
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (this: HTMLElement) {
      return this.getAttribute('role') === 'menu' ? width : anchor.width;
    });
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (this: HTMLElement) {
      return this.getAttribute('role') === 'menu' ? height : anchor.height;
    });
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
    act(() => root.render(<Harness />));
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });
  const toggle = () => act(() => container.querySelector('button')!.click());
  const menu = () => document.querySelector<HTMLElement>('[role="menu"]')!;

  it('opens to the right above the footer on the first click using the full animated size', () => {
    scale = 0.98;
    toggle();
    // No animation frame or second click should be needed to reach the final geometry.
    expect(menu().style.visibility).toBe('visible');
    expect(menu().style.left).toBe('280px');
    expect(menu().style.top).toBe('134px');
    expect(Number.parseFloat(menu().style.top) + height).toBe(anchor.bottom);
    toggle();
    scale = 1;
    toggle();
    expect(menu().style.top).toBe('134px');
    expect(menu().style.left).toBe('280px');
  });

  it('flips left only when the right side lacks space without covering the trigger', () => {
    anchor = new DOMRect(850, 100, 24, 24);
    toggle();
    expect(menu().style.left).toBe('604px');
    expect(menu().style.top).toBe('100px');
    expect(Number.parseFloat(menu().style.left) + width).toBeLessThan(anchor.left);
  });

  it('repositions when async menu content grows', () => {
    height = 80;
    toggle();
    expect(menu().style.top).toBe('510px');
    expect(observed.has(menu())).toBe(true);
    height = 400;
    act(() => { notifyResize(); vi.advanceTimersByTime(20); });
    expect(menu().style.top).toBe('134px');
    expect(menu().style.left).toBe('280px');
  });

  it('remeasures a changed menu level before displaying it', () => {
    toggle();
    height = 80;
    act(() => root.render(<Harness revision={1} />));
    expect(menu().style.top).toBe('510px');
  });

  it('tracks scrolling and viewport changes and clears observers on close', () => {
    toggle();
    anchor = new DOMRect(300, 80, 24, 24);
    act(() => {
      container.dispatchEvent(new Event('scroll'));
      vi.advanceTimersByTime(20);
    });
    expect(menu().style.left).toBe('330px');
    expect(menu().style.top).toBe('80px');
    vi.stubGlobal('innerHeight', 430);
    act(() => { window.dispatchEvent(new Event('resize')); vi.advanceTimersByTime(20); });
    expect(Number.parseFloat(menu().style.top)).toBeGreaterThanOrEqual(8);
    expect(Number.parseFloat(menu().style.top) + height).toBeLessThanOrEqual(422);
    toggle();
    expect(observed.size).toBe(0);
    expect(menu()).toBeNull();
  });
});
