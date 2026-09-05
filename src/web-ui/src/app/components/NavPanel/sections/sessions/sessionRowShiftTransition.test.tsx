// @vitest-environment jsdom

import React, { useRef } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import {
  SESSION_ROW_SHIFT_DURATION_MS,
  useSessionRowRemovalTransition,
} from './sessionRowShift';

const ROW_HEIGHT = 26;

/** jsdom has no layout; stack rows by their index and size the list by count. */
function stubRowLayout(): () => void {
  const originalTop = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetTop');
  const originalHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
  Object.defineProperty(HTMLElement.prototype, 'offsetTop', {
    configurable: true,
    get(this: HTMLElement) {
      const parent = this.parentElement;
      if (!parent) return 0;
      return Array.prototype.indexOf.call(parent.children, this) * ROW_HEIGHT;
    },
  });
  Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
    configurable: true,
    get(this: HTMLElement) {
      return this.classList.contains('openbitfun-nav-panel__inline-list')
        ? this.children.length * ROW_HEIGHT
        : ROW_HEIGHT;
    },
  });
  const restore = (
    property: 'offsetTop' | 'offsetHeight',
    descriptor: PropertyDescriptor | undefined,
  ) => {
    if (descriptor) {
      Object.defineProperty(HTMLElement.prototype, property, descriptor);
    } else {
      delete (HTMLElement.prototype as unknown as Record<string, unknown>)[property];
    }
  };
  return () => {
    restore('offsetTop', originalTop);
    restore('offsetHeight', originalHeight);
  };
}

const SessionList: React.FC<{ sessionIds: string[] }> = ({ sessionIds }) => {
  const listRef = useRef<HTMLDivElement>(null);
  useSessionRowRemovalTransition(listRef, sessionIds.join('|'));
  return (
    <div className="openbitfun-nav-panel__inline-list" ref={listRef}>
      {sessionIds.map(sessionId => (
        <div
          key={sessionId}
          className="openbitfun-nav-panel__inline-item"
          data-session-id={sessionId}
        />
      ))}
    </div>
  );
};

const nextFrame = (): Promise<void> =>
  new Promise(resolve => requestAnimationFrame(() => resolve()));

const rowOf = (container: HTMLElement, sessionId: string): HTMLElement =>
  container.querySelector<HTMLElement>(`[data-session-id="${sessionId}"]`)!;

describe('useSessionRowRemovalTransition', () => {
  let container: HTMLDivElement;
  let root: Root;
  let restoreRowLayout: () => void;

  beforeEach(() => {
    restoreRowLayout = stubRowLayout();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    restoreRowLayout();
  });

  it('starts the rows below a deleted session from their previous slot', async () => {
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b', 'c', 'd']} />);
    });

    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'c', 'd']} />);
    });

    expect(rowOf(container, 'a').style.transform).toBe('');
    expect(rowOf(container, 'c').style.transform).toBe(`translateY(${ROW_HEIGHT}px)`);
    expect(rowOf(container, 'c').style.transition).toBe('none');
    expect(rowOf(container, 'd').style.transform).toBe(`translateY(${ROW_HEIGHT}px)`);

    await act(async () => { await nextFrame(); });

    expect(rowOf(container, 'c').style.transform).toBe('');
    expect(rowOf(container, 'c').style.transition).toContain(`${SESSION_ROW_SHIFT_DURATION_MS}ms`);
  });

  it('fades the backfilled row in instead of popping it into the freed slot', async () => {
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b', 'c']} />);
    });

    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'c', 'd']} />);
    });

    expect(rowOf(container, 'd').style.opacity).toBe('0');

    await act(async () => { await nextFrame(); });

    expect(rowOf(container, 'd').style.opacity).toBe('');
    expect(rowOf(container, 'd').style.transition).toContain('opacity');
  });

  it('keeps the slide running when a backfilled row lands mid-animation', async () => {
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b', 'c']} />);
    });
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'c']} />);
    });
    await act(async () => { await nextFrame(); });

    // A background metadata refresh appends the replacement row.
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'c', 'd']} />);
    });

    expect(rowOf(container, 'c').style.transition).toContain('transform');
  });

  it('closes the list height up with the rows so content below follows', async () => {
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b', 'c']} />);
    });

    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b']} />);
    });

    const list = container.querySelector<HTMLElement>('.openbitfun-nav-panel__inline-list')!;
    expect(list.style.height).toBe(`${3 * ROW_HEIGHT}px`);
    expect(list.style.overflow).toBe('hidden');

    await act(async () => { await nextFrame(); });

    expect(list.style.height).toBe(`${2 * ROW_HEIGHT}px`);
    expect(list.style.transition).toContain('height');
  });

  it('retargets the collapse when a row lands before it settles', async () => {
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b', 'c']} />);
    });
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b']} />);
    });
    await act(async () => { await nextFrame(); });

    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b', 'd']} />);
    });

    const list = container.querySelector<HTMLElement>('.openbitfun-nav-panel__inline-list')!;
    expect(list.style.height).toBe(`${3 * ROW_HEIGHT}px`);
  });

  it('does not animate when rows are only added', async () => {
    await act(async () => {
      root.render(<SessionList sessionIds={['b', 'c']} />);
    });
    await act(async () => {
      root.render(<SessionList sessionIds={['a', 'b', 'c']} />);
    });

    expect(rowOf(container, 'b').style.transform).toBe('');
    expect(rowOf(container, 'c').style.transform).toBe('');
  });
});
