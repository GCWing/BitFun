// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ScrollAnchor } from './ScrollAnchor';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('../../store/modernFlowChatStore', () => ({
  useVirtualItems: () => [{
    type: 'user-message',
    turnId: 'turn-a',
    data: {
      id: 'user-turn-a',
      content: 'turn-a',
      timestamp: 1,
    },
  }],
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    formatDate: () => 'now',
  },
}));

describe('ScrollAnchor', () => {
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
    vi.useRealTimers();
  });

  it('rebinds scroll listeners when the concrete scroller element changes', () => {
    vi.useFakeTimers();
    const firstScroller = document.createElement('div');
    const secondScroller = document.createElement('div');
    const onAnchorNavigate = vi.fn();

    act(() => {
      root.render(
        <ScrollAnchor
          onAnchorNavigate={onAnchorNavigate}
          scrollerElement={firstScroller}
        />,
      );
    });

    act(() => {
      firstScroller.dispatchEvent(new Event('scroll'));
    });
    expect(container.querySelector('.scroll-anchor')?.className).toContain('scrolling');

    act(() => {
      vi.advanceTimersByTime(801);
    });
    expect(container.querySelector('.scroll-anchor')?.className).not.toContain('scrolling');

    act(() => {
      root.render(
        <ScrollAnchor
          onAnchorNavigate={onAnchorNavigate}
          scrollerElement={secondScroller}
        />,
      );
    });

    act(() => {
      firstScroller.dispatchEvent(new Event('scroll'));
    });
    expect(container.querySelector('.scroll-anchor')?.className).not.toContain('scrolling');

    act(() => {
      secondScroller.dispatchEvent(new Event('scroll'));
    });
    expect(container.querySelector('.scroll-anchor')?.className).toContain('scrolling');

  });
});
