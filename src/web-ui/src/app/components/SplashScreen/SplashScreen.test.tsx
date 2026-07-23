import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import SplashScreen from './SplashScreen';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('SplashScreen', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>');
    globalThis.window = dom.window as unknown as Window & typeof globalThis;
    globalThis.document = dom.window.document;
    container = document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.useRealTimers();
    dom.window.close();
  });

  it('reveals the subtle startup hint only after the delay', () => {
    act(() => {
      root.render(
        <SplashScreen
          isExiting={false}
          onExited={() => {}}
          delayedMessage="Starting BitFun..."
          delayedMessageMs={1000}
        />
      );
    });

    expect(container.querySelector('.splash-screen__message')).toBeNull();

    act(() => {
      vi.advanceTimersByTime(999);
    });
    expect(container.querySelector('.splash-screen__message')).toBeNull();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    const message = container.querySelector('.splash-screen__message');
    expect(message?.textContent).toBe('Starting BitFun...');
    expect(message?.classList.contains('splash-screen__message--visible')).toBe(true);
  });

  it('does not reveal the subtle startup hint during the normal startup splash window by default', () => {
    act(() => {
      root.render(
        <SplashScreen
          isExiting={false}
          onExited={() => {}}
          delayedMessage="Starting BitFun..."
        />
      );
    });

    expect(container.querySelector('.splash-screen__message')).toBeNull();

    act(() => {
      vi.advanceTimersByTime(1799);
    });
    expect(container.querySelector('.splash-screen__message')).toBeNull();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    const message = container.querySelector('.splash-screen__message');
    expect(message?.classList.contains('splash-screen__message--visible')).toBe(true);
  });

  it('does not show the delayed message while exiting', () => {
    act(() => {
      root.render(
        <SplashScreen
          isExiting={true}
          onExited={() => {}}
          delayedMessage="Starting BitFun..."
          delayedMessageMs={1000}
        />
      );
    });

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(container.querySelector('.splash-screen__message')).toBeNull();
  });
});
