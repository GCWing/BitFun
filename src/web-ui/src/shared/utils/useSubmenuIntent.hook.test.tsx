// @vitest-environment jsdom

import React, { act, useRef, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useSubmenuIntent } from './useSubmenuIntent';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const Harness: React.FC = () => {
  const [activeId, setActiveId] = useState<string | null>(null);
  const parentRef = useRef<HTMLDivElement>(null);
  const submenuRef = useRef<HTMLDivElement>(null);
  const intent = useSubmenuIntent({
    activeId,
    onActiveIdChange: setActiveId,
    parentRef,
    submenuRef,
  });

  return (
    <div>
      <div ref={parentRef} data-testid="parent">
        <button
          data-testid="first"
          onPointerEnter={event => intent.requestChange('first', event)}
          onPointerLeave={intent.requestClose}
        >
          First
        </button>
        <button
          data-testid="second"
          onPointerEnter={event => intent.requestChange('second', event)}
          onPointerLeave={intent.requestClose}
        >
          Second
        </button>
      </div>
      {activeId ? (
        <div
          ref={submenuRef}
          data-testid="submenu"
          data-active-id={activeId}
          onPointerEnter={intent.keepOpen}
          onPointerLeave={intent.requestClose}
        />
      ) : null}
    </div>
  );
};

const dispatchPointerOver = (element: Element, clientX: number, clientY: number) => {
  element.dispatchEvent(new MouseEvent('pointerover', {
    bubbles: true,
    clientX,
    clientY,
  }));
};

const dispatchPointerMove = (clientX: number, clientY: number) => {
  document.dispatchEvent(new MouseEvent('pointermove', {
    bubbles: true,
    clientX,
    clientY,
  }));
};

const dispatchPointerOut = (element: Element, clientX: number, clientY: number) => {
  element.dispatchEvent(new MouseEvent('pointerout', {
    bubbles: true,
    relatedTarget: document.body,
    clientX,
    clientY,
  }));
};

const setBounds = (element: HTMLElement, left: number, right: number) => {
  element.getBoundingClientRect = () => ({
    left, right, top: 20, bottom: 220,
    width: right - left, height: 200, x: left, y: 20,
    toJSON: () => ({}),
  });
};

describe('useSubmenuIntent', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  const openFirstSubmenu = () => {
    const first = container.querySelector('[data-testid="first"]')!;
    act(() => dispatchPointerOver(first, 150, 40));
    act(() => vi.advanceTimersByTime(120));

    const submenu = container.querySelector<HTMLElement>('[data-testid="submenu"]')!;
    submenu.getBoundingClientRect = () => ({
      left: 220,
      right: 440,
      top: 20,
      bottom: 220,
      width: 220,
      height: 200,
      x: 220,
      y: 20,
      toJSON: () => ({}),
    });
    return submenu;
  };

  it('keeps the current submenu open while the pointer crosses a sibling toward it', () => {
    const submenu = openFirstSubmenu();
    const second = container.querySelector('[data-testid="second"]')!;

    act(() => dispatchPointerMove(170, 48));
    act(() => dispatchPointerOver(second, 190, 82));
    expect(submenu.dataset.activeId).toBe('first');

    act(() => dispatchPointerOver(submenu, 225, 110));
    act(() => vi.advanceTimersByTime(400));
    expect(container.querySelector<HTMLElement>('[data-testid="submenu"]')?.dataset.activeId).toBe('first');
  });

  it('switches immediately once the pointer turns away from the open submenu', () => {
    openFirstSubmenu();
    const second = container.querySelector('[data-testid="second"]')!;

    act(() => dispatchPointerMove(170, 48));
    act(() => dispatchPointerOver(second, 190, 82));
    expect(container.querySelector<HTMLElement>('[data-testid="submenu"]')?.dataset.activeId).toBe('first');

    act(() => dispatchPointerMove(165, 100));
    expect(container.querySelector<HTMLElement>('[data-testid="submenu"]')?.dataset.activeId).toBe('second');
  });

  it.each([
    { side: 'right', parentLeft: 0, parentRight: 215, gapX: 218 },
    { side: 'left', parentLeft: 445, parentRight: 665, gapX: 442 },
  ])('keeps a $side-opening submenu open while the pointer rests in the gap', ({ parentLeft, parentRight, gapX }) => {
    openFirstSubmenu();
    setBounds(container.querySelector<HTMLElement>('[data-testid="parent"]')!, parentLeft, parentRight);
    const first = container.querySelector('[data-testid="first"]')!;

    act(() => dispatchPointerOut(first, gapX, 80));
    act(() => dispatchPointerMove(gapX, 80));
    act(() => vi.advanceTimersByTime(1000));

    expect(container.querySelector<HTMLElement>('[data-testid="submenu"]')?.dataset.activeId).toBe('first');
  });

  it('keeps the gap open when returning from the submenu, but closes after leaving the bridge', () => {
    const submenu = openFirstSubmenu();
    setBounds(container.querySelector<HTMLElement>('[data-testid="parent"]')!, 0, 215);
    act(() => dispatchPointerOver(submenu, 225, 80));
    act(() => dispatchPointerOut(submenu, 218, 80));
    act(() => dispatchPointerMove(218, 80));
    act(() => vi.advanceTimersByTime(1000));
    expect(container.querySelector('[data-testid="submenu"]')).not.toBeNull();

    act(() => dispatchPointerMove(218, 400));
    act(() => vi.advanceTimersByTime(300));
    expect(container.querySelector('[data-testid="submenu"]')).toBeNull();
  });

  it('protects a gap stop even if pointerleave is not followed by pointermove', () => {
    openFirstSubmenu();
    setBounds(container.querySelector<HTMLElement>('[data-testid="parent"]')!, 0, 215);
    act(() => dispatchPointerOut(container.querySelector('[data-testid="first"]')!, 218, 80));
    act(() => vi.advanceTimersByTime(1000));
    expect(container.querySelector('[data-testid="submenu"]')).not.toBeNull();
  });

  it('keeps the gap protected during slow sub-pixel and reverse movements', () => {
    openFirstSubmenu();
    setBounds(container.querySelector<HTMLElement>('[data-testid="parent"]')!, 0, 215);
    act(() => dispatchPointerOut(container.querySelector('[data-testid="first"]')!, 218, 80));
    for (const x of [218.5, 217.5, 218, 217]) {
      act(() => dispatchPointerMove(x, 80));
      act(() => vi.advanceTimersByTime(500));
      expect(container.querySelector('[data-testid="submenu"]')).not.toBeNull();
    }
  });

  it('clears pending sibling activation when crossing into the bridge', () => {
    openFirstSubmenu();
    setBounds(container.querySelector<HTMLElement>('[data-testid="parent"]')!, 0, 215);
    act(() => dispatchPointerMove(170, 48));
    act(() => dispatchPointerOver(container.querySelector('[data-testid="second"]')!, 190, 82));
    act(() => dispatchPointerMove(218, 110));
    act(() => vi.advanceTimersByTime(1000));
    expect(container.querySelector<HTMLElement>('[data-testid="submenu"]')?.dataset.activeId).toBe('first');
  });

  it.each(['blur', 'pointerleave'])('does not retain bridge protection after %s exits the window', eventName => {
    openFirstSubmenu();
    setBounds(container.querySelector<HTMLElement>('[data-testid="parent"]')!, 0, 215);
    act(() => dispatchPointerOut(container.querySelector('[data-testid="first"]')!, 218, 80));
    act(() => {
      (eventName === 'blur' ? window : document).dispatchEvent(new Event(eventName));
    });
    act(() => vi.advanceTimersByTime(300));
    expect(container.querySelector('[data-testid="submenu"]')).toBeNull();
  });
});
