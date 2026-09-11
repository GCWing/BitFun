// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { bindBtwTailFollow } from './btwTailFollow';

describe('embedded panel tail-follow intent', () => {
  let scroller: HTMLDivElement;
  let following: boolean;
  let dispose: () => void;
  const wheel = (deltaY: number) => scroller.dispatchEvent(new WheelEvent('wheel', { deltaY }));
  const scroll = (top: number) => {
    scroller.scrollTop = top;
    scroller.dispatchEvent(new Event('scroll'));
  };
  beforeEach(() => {
    vi.useFakeTimers();
    scroller = document.createElement('div');
    document.body.append(scroller);
    Object.defineProperties(scroller, {
      clientHeight: { value: 500 },
      scrollHeight: { value: 1500, configurable: true },
      clientWidth: { value: 500 },
    });
    scroller.scrollTop = 1000;
    following = true;
    dispose = bindBtwTailFollow(scroller, value => { following = value; }, vi.fn());
  });
  afterEach(() => { dispose(); scroller.remove(); vi.useRealTimers(); });

  it('keeps small upward gestures detached even inside the old 80px threshold', () => {
    wheel(-20);
    scroll(980);
    expect(following).toBe(false);
    // A measurement correction back to the tail is not downward user intent.
    scroll(1000);
    expect(following).toBe(false);
  });

  it('requires downward movement to the actual tail, not proximity', () => {
    wheel(-60); scroll(940);
    wheel(20); scroll(960);
    expect(following).toBe(false);
    wheel(40);
    expect(following).toBe(false);
    scroll(999);
    expect(following).toBe(true);
  });

  it('does not resume when streamed content moves the tail away', () => {
    wheel(-60); scroll(940); wheel(60);
    Object.defineProperty(scroller, 'scrollHeight', { value: 1600 });
    scroll(1000);
    expect(following).toBe(false);
  });

  it.each(['scrollend', 'timeout'])('expires downward intent on %s', end => {
    wheel(-60); scroll(940); wheel(20); scroll(960);
    if (end === 'scrollend') scroller.dispatchEvent(new Event('scrollend'));
    else vi.advanceTimersByTime(181);
    scroll(1000);
    expect(following).toBe(false);
  });

  it('replaces pending downward intent when the user reverses direction', () => {
    wheel(-60); scroll(940); wheel(20); scroll(960); wheel(-10);
    scroll(1000);
    expect(following).toBe(false);
  });

  it('supports keyboard scrolling but leaves nested controls alone', () => {
    scroller.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp' }));
    scroll(980);
    expect(following).toBe(false);
    const input = document.createElement('input');
    scroller.append(input);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
    scroll(1000);
    expect(following).toBe(false);
    scroll(980);
    scroller.dispatchEvent(new KeyboardEvent('keydown', { key: 'End' }));
    scroll(1000);
    expect(following).toBe(true);
  });

  it('supports touch direction followed by actual scrolling', () => {
    const touch = (type: string, clientY: number) => scroller.dispatchEvent(
      new TouchEvent(type, { touches: [{ clientY } as Touch] }),
    );
    touch('touchstart', 100); touch('touchmove', 120); scroll(980);
    expect(following).toBe(false);
    touch('touchmove', 100); scroll(1000);
    expect(following).toBe(true);
  });

  it('detaches a scrollbar drag and resumes only when dragged down to the tail', () => {
    scroller.dispatchEvent(new MouseEvent('pointerdown', { clientX: 505, button: 0 }));
    scroll(980);
    expect(following).toBe(false);
    scroll(1000);
    expect(following).toBe(true);
    document.dispatchEvent(new Event('pointerup'));
  });

  it('removes listeners and the expiry timer on disposal', () => {
    wheel(-20); scroll(980); wheel(20);
    dispose();
    expect(vi.getTimerCount()).toBe(0);
    scroll(1000);
    expect(following).toBe(false);
  });
});
