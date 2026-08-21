// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Virtualizer } from '@tanstack/react-virtual';
import { observeFlowChatViewportRect } from './useFlowChatVirtualizer';

class TestResizeObserver {
  static instances: TestResizeObserver[] = [];

  constructor(private readonly callback: ResizeObserverCallback) {
    TestResizeObserver.instances.push(this);
  }

  observe() {}

  unobserve() {}

  emit() {
    this.callback([], this as unknown as ResizeObserver);
  }
}

describe('observeFlowChatViewportRect', () => {
  afterEach(() => {
    TestResizeObserver.instances = [];
    vi.unstubAllGlobals();
  });

  it('keeps the last positive virtualizer rectangle while a WebView is minimized', () => {
    vi.stubGlobal('ResizeObserver', TestResizeObserver);
    let width = 1000;
    let height = 600;
    const element = document.createElement('div');
    Object.defineProperties(element, {
      offsetWidth: { configurable: true, get: () => width },
      offsetHeight: { configurable: true, get: () => height },
    });
    const instance = {
      scrollElement: element,
      targetWindow: window,
      options: { useAnimationFrameWithResizeObserver: false },
    } as unknown as Virtualizer<HTMLElement, HTMLElement>;
    const rectangles: Array<{ width: number; height: number }> = [];

    const cleanup = observeFlowChatViewportRect(instance, rectangle => {
      rectangles.push(rectangle);
    });

    expect(rectangles).toEqual([{ width: 1000, height: 600 }]);

    width = 390;
    height = 0;
    TestResizeObserver.instances[0].emit();
    expect(rectangles).toEqual([{ width: 1000, height: 600 }]);

    width = 1000;
    height = 700;
    TestResizeObserver.instances[0].emit();
    expect(rectangles).toEqual([
      { width: 1000, height: 600 },
      { width: 1000, height: 700 },
    ]);

    cleanup?.();
  });
});
