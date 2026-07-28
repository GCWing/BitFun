// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { FLOWCHAT_COLLAPSE_DURATION_MS } from './flowChatCollapseMotion';
import { SmoothHeightCollapse } from './SmoothHeightCollapse';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('SmoothHeightCollapse', () => {
  let container: HTMLDivElement;
  let root: Root;
  let measuredHeight: number;
  let requestAnimationFrameSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    measuredHeight = 0;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    vi.stubGlobal('ResizeObserver', class {
      observe = vi.fn();
      disconnect = vi.fn();
    });
    vi.stubGlobal('matchMedia', vi.fn(() => ({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })));
    requestAnimationFrameSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation(() => 1);
    vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(() => ({
      x: 0,
      y: 0,
      top: 0,
      right: 0,
      bottom: measuredHeight,
      left: 0,
      width: 0,
      height: measuredHeight,
      toJSON: () => ({}),
    }));
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('does not replay an opening animation when only animation mode changes', () => {
    act(() => {
      root.render(
        <SmoothHeightCollapse isOpen disableAnimation>
          <div>content</div>
        </SmoothHeightCollapse>,
      );
    });

    act(() => {
      root.render(
        <SmoothHeightCollapse isOpen disableAnimation={false}>
          <div>content</div>
        </SmoothHeightCollapse>,
      );
    });

    const collapse = container.querySelector('.smooth-height-collapse');
    expect(collapse?.classList.contains('smooth-height-collapse--open')).toBe(true);
    expect(requestAnimationFrameSpy).not.toHaveBeenCalled();
  });

  it('reverses an in-flight toggle from its rendered height', () => {
    act(() => {
      root.render(
        <SmoothHeightCollapse isOpen={false}>
          <div>content</div>
        </SmoothHeightCollapse>,
      );
    });

    act(() => {
      root.render(
        <SmoothHeightCollapse isOpen>
          <div>content</div>
        </SmoothHeightCollapse>,
      );
    });

    measuredHeight = 42;
    act(() => {
      root.render(
        <SmoothHeightCollapse isOpen={false}>
          <div>content</div>
        </SmoothHeightCollapse>,
      );
    });
    expect(container.querySelector<HTMLElement>('.smooth-height-collapse')?.style.height).toBe('42px');

    measuredHeight = 17;
    act(() => {
      root.render(
        <SmoothHeightCollapse isOpen>
          <div>content</div>
        </SmoothHeightCollapse>,
      );
    });
    expect(container.querySelector<HTMLElement>('.smooth-height-collapse')?.style.height).toBe('17px');
  });

  it('keeps height opacity and transform on the same collapse duration', () => {
    measuredHeight = 80;
    act(() => {
      root.render(
        <SmoothHeightCollapse isOpen>
          <div>content</div>
        </SmoothHeightCollapse>,
      );
    });

    act(() => {
      root.render(
        <SmoothHeightCollapse isOpen={false}>
          <div>content</div>
        </SmoothHeightCollapse>,
      );
    });

    const collapse = container.querySelector<HTMLElement>('.smooth-height-collapse');
    const expected = `${FLOWCHAT_COLLAPSE_DURATION_MS}ms, ${FLOWCHAT_COLLAPSE_DURATION_MS}ms, ${FLOWCHAT_COLLAPSE_DURATION_MS}ms`;
    expect(collapse?.style.transitionDuration).toBe(expected);
    expect(collapse?.classList.contains('smooth-height-collapse--closing')).toBe(true);
  });
});
