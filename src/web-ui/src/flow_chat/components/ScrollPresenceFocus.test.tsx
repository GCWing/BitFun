/** @vitest-environment jsdom */

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ScrollToBottomButton } from './ScrollToBottomButton';
import { ScrollToLatestBar } from './ScrollToLatestBar';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@openbitfun/ui', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => children,
}));

describe('retained scroll controls', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it('returns focus from the scroll-to-bottom button before its retained exit', () => {
    const focusTarget = document.createElement('div');
    focusTarget.tabIndex = -1;
    document.body.appendChild(focusTarget);
    const focusReturnRef = { current: focusTarget };
    act(() => {
      root.render(
        <ScrollToBottomButton
          visible
          onClick={vi.fn()}
          focusReturnRef={focusReturnRef}
        />,
      );
    });
    const button = container.querySelector<HTMLButtonElement>('button');
    button?.focus();
    expect(document.activeElement).toBe(button);

    act(() => {
      root.render(
        <ScrollToBottomButton
          visible={false}
          onClick={vi.fn()}
          focusReturnRef={focusReturnRef}
        />,
      );
    });

    expect(document.activeElement).toBe(focusTarget);
    expect(button?.getAttribute('aria-hidden')).toBe('true');
    expect(button?.hasAttribute('inert')).toBe(true);
    expect(button?.tabIndex).toBe(-1);
    act(() => vi.advanceTimersByTime(199));
    expect(container.querySelector('button')).not.toBeNull();
    act(() => vi.advanceTimersByTime(1));
    expect(container.querySelector('button')).toBeNull();
    focusTarget.remove();
  });

  it('returns focus from the scroll-to-latest bar before its retained exit', () => {
    const focusTarget = document.createElement('div');
    focusTarget.tabIndex = -1;
    document.body.appendChild(focusTarget);
    const focusReturnRef = { current: focusTarget };
    act(() => {
      root.render(
        <ScrollToLatestBar
          visible
          onClick={vi.fn()}
          focusReturnRef={focusReturnRef}
        />,
      );
    });
    const bar = container.querySelector<HTMLElement>('[role="button"]');
    bar?.focus();
    expect(document.activeElement).toBe(bar);

    act(() => {
      root.render(
        <ScrollToLatestBar
          visible={false}
          onClick={vi.fn()}
          focusReturnRef={focusReturnRef}
        />,
      );
    });

    expect(document.activeElement).toBe(focusTarget);
    expect(bar?.getAttribute('aria-hidden')).toBe('true');
    expect(bar?.hasAttribute('inert')).toBe(true);
    expect(bar?.tabIndex).toBe(-1);
    act(() => vi.advanceTimersByTime(199));
    expect(container.querySelector('[role="button"]')).not.toBeNull();
    act(() => vi.advanceTimersByTime(1));
    expect(container.querySelector('[role="button"]')).toBeNull();
    focusTarget.remove();
  });

  it('cancels removal when a scroll affordance reappears quickly', () => {
    act(() => {
      root.render(<ScrollToBottomButton visible onClick={vi.fn()} unreadCount={2} />);
    });
    act(() => {
      root.render(<ScrollToBottomButton visible={false} onClick={vi.fn()} unreadCount={2} />);
    });
    act(() => vi.advanceTimersByTime(100));
    act(() => {
      root.render(<ScrollToBottomButton visible onClick={vi.fn()} unreadCount={3} />);
    });
    act(() => vi.advanceTimersByTime(200));

    const button = container.querySelector<HTMLButtonElement>('button');
    expect(button).not.toBeNull();
    expect(button?.getAttribute('aria-hidden')).toBe('false');
    expect(button?.hasAttribute('inert')).toBe(false);
    expect(button?.textContent).toContain('3');
  });
});
