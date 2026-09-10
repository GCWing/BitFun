// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RollingText, TabGroup } from '@openbitfun/ui';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

interface ControlledAnimation {
  target: HTMLElement;
  frames: Keyframe[];
  options: KeyframeAnimationOptions;
  finished: Promise<Animation>;
  cancel: ReturnType<typeof vi.fn>;
  finish: () => void;
  transform?: string;
  width?: number;
}

// Exercise the package against the host's animation/layout boundary. These
// deterministic measurements verify lifecycle contracts, not visual fidelity.
describe('RollingText in the Web UI host', () => {
  let container: HTMLDivElement;
  let root: Root;
  let reduced: boolean;
  let mediaEvents: EventTarget;
  let animations: ControlledAnimation[];
  let active: Map<HTMLElement, ControlledAnimation>;
  let animateDescriptor: PropertyDescriptor | undefined;
  const widths: Record<string, number> = { Short: 64, 'Long session title': 184, Latest: 100, Renamed: 88 };

  beforeEach(() => {
    reduced = false;
    mediaEvents = new EventTarget();
    animations = [];
    active = new Map();
    vi.stubGlobal('matchMedia', () => ({
      get matches() { return reduced; },
      addEventListener: mediaEvents.addEventListener.bind(mediaEvents),
      removeEventListener: mediaEvents.removeEventListener.bind(mediaEvents),
    }));

    const computedStyle = window.getComputedStyle.bind(window);
    vi.spyOn(window, 'getComputedStyle').mockImplementation(element => {
      const style = computedStyle(element);
      return new Proxy(style, {
        get(target, property) {
          if (property === 'transform' && active.has(element as HTMLElement)) {
            return active.get(element as HTMLElement)?.transform ?? 'none';
          }
          if (property === 'getPropertyValue') return (name: string) => {
            if (name === '--_rolling-text-duration') return '320ms';
            if (name === '--_rolling-text-easing') return 'cubic-bezier(0.77, 0, 0.175, 1)';
            return target.getPropertyValue(name);
          };
          return Reflect.get(target, property);
        },
      });
    });
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      const title = this.querySelector('[data-rolling-text-part="current"]')?.textContent ?? '';
      const width = active.get(this)?.width ?? widths[title] ?? 0;
      return { width, height: 20, x: 0, y: 0, top: 0, left: 0, right: width, bottom: 20, toJSON() {} };
    });
    animateDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'animate');
    Object.defineProperty(HTMLElement.prototype, 'animate', {
      configurable: true,
      value(this: HTMLElement, frames: Keyframe[], options: KeyframeAnimationOptions) {
        let resolve!: (value: Animation) => void;
        let reject!: (reason: Error) => void;
        const animation: ControlledAnimation = {
          target: this,
          frames,
          options,
          transform: frames[0].transform as string | undefined,
          width: frames[0].inlineSize ? Number.parseFloat(String(frames[0].inlineSize)) : undefined,
          finished: new Promise<Animation>((yes, no) => { resolve = yes; reject = no; }),
          cancel: vi.fn(() => {
            if (active.get(this) === animation) active.delete(this);
            reject(new Error('Animation cancelled'));
          }),
          finish() {
            const last = frames[frames.length - 1];
            animation.transform = last.transform as string | undefined;
            animation.width = last.inlineSize ? Number.parseFloat(String(last.inlineSize)) : undefined;
            resolve(animation as unknown as Animation);
          },
        };
        active.set(this, animation);
        animations.push(animation);
        return animation;
      },
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    if (animateDescriptor) Object.defineProperty(HTMLElement.prototype, 'animate', animateDescriptor);
    else Reflect.deleteProperty(HTMLElement.prototype, 'animate');
  });

  function render(text: string, transitionKey?: string) {
    act(() => root.render(<RollingText transitionKey={transitionKey}>{text}</RollingText>));
  }

  function rolling() {
    return container.querySelector<HTMLElement>('[data-openbitfun-component="rolling-text"]')!;
  }

  async function finishCurrentMotion() {
    await act(async () => { [...active.values()].forEach(animation => animation.finish()); });
  }

  it('starts static and treats edits under the same identity as text updates', () => {
    render('Short', 'a');
    render('Renamed', 'a');
    expect(animations).toHaveLength(0);
    expect(rolling().textContent).toBe('Renamed');
    render('Latest', 'b');
    expect(container.querySelector('[data-rolling-text-part="outgoing"]')?.textContent).toBe('Renamed');
    expect(animations[2].frames).toEqual([{ inlineSize: '88px' }, { inlineSize: '100px' }]);
  });

  it('coordinates growing and shrinking widths with both lines, then releases layout', async () => {
    render('Short');
    render('Long session title');
    expect(animations).toHaveLength(3);
    expect(animations.map(animation => animation.options)).toEqual(Array(3).fill({
      duration: 320, easing: 'cubic-bezier(0.77, 0, 0.175, 1)', fill: 'both',
    }));
    expect(animations[2].frames).toEqual([{ inlineSize: '64px' }, { inlineSize: '184px' }]);
    const outgoing = container.querySelector('[data-rolling-text-part="outgoing"]')!;
    expect(outgoing.getAttribute('aria-hidden')).toBe('true');
    expect(container.querySelector('[data-rolling-text-part="current"]')?.textContent).toBe('Long session title');
    expect(container.querySelector('[data-rolling-text-part="current"] [data-overflow-behavior]')?.getAttribute('data-overflow-behavior')).toBe('fade');

    await finishCurrentMotion();
    expect(active.size).toBe(0);
    expect(rolling().dataset.transitioning).toBe('false');
    expect(rolling().style.inlineSize).toBe('');
    expect(container.querySelector('[data-overflow-behavior="marquee"]')).not.toBeNull();
    render('Short');
    expect(animations[5].frames).toEqual([{ inlineSize: '184px' }, { inlineSize: '64px' }]);
  });

  it('retargets from displayed positions and width while keeping only two text layers', async () => {
    render('Short', 'a');
    render('Long session title', 'b');
    const obsolete = [...animations];
    obsolete[0].transform = 'matrix(1, 0, 0, 1, 9)';
    obsolete[1].transform = 'matrix(1, 0, 0, 1, -11)';
    obsolete[2].width = 128;
    render('Latest', 'c');
    expect(animations[3].frames[0].transform).toBe('matrix(1, 0, 0, 1, 9)');
    expect(animations[4].frames[0].transform).toBe('matrix(1, 0, 0, 1, -11)');
    expect(animations[5].frames).toEqual([{ inlineSize: '128px' }, { inlineSize: '100px' }]);
    expect(obsolete.every(animation => animation.cancel.mock.calls.length === 1)).toBe(true);
    expect(container.querySelectorAll('[data-rolling-text-part]')).toHaveLength(2);
    expect(container.querySelector('[data-rolling-text-part="current"]')?.textContent).toBe('Latest');
    await act(async () => { obsolete.forEach(animation => animation.finish()); });
    expect(rolling().dataset.transitioning).toBe('true');
    await finishCurrentMotion();
    expect(rolling().textContent).toBe('Latest');
  });

  it('reverses smoothly when returning to the outgoing resource', async () => {
    render('Short', 'a');
    render('Long session title', 'b');
    animations[2].width = 124;
    render('Short', 'a');
    expect(animations[3].frames[1].transform).toBe('translateY(100%)');
    expect(animations[4].frames[1].transform).toBe('translateY(0)');
    expect(animations[5].frames).toEqual([{ inlineSize: '124px' }, { inlineSize: '64px' }]);
    await finishCurrentMotion();
    expect(rolling().textContent).toBe('Short');
    expect(rolling().dataset.transitioning).toBe('false');
  });

  it('cancels on reduced-motion changes and never replays a completed replacement', () => {
    render('Short', 'a');
    render('Long session title', 'b');
    act(() => { reduced = true; mediaEvents.dispatchEvent(new Event('change')); });
    expect(rolling().textContent).toBe('Long session title');
    expect(rolling().dataset.transitioning).toBe('false');
    expect(active.size).toBe(0);
    render('Latest', 'c');
    expect(animations).toHaveLength(3);
    act(() => { reduced = false; mediaEvents.dispatchEvent(new Event('change')); });
    expect(animations).toHaveLength(3);
  });

  it('renders immediately on hosts without Web Animations and cancels on unmount', () => {
    render('Short');
    render('Latest');
    act(() => root.render(null));
    expect(active.size).toBe(0);
    expect(animations.every(animation => animation.cancel.mock.calls.length === 1)).toBe(true);
    Object.defineProperty(HTMLElement.prototype, 'animate', { configurable: true, value: undefined });
    render('Short');
    render('Latest');
    expect(rolling().dataset.transitioning).toBe('false');
    expect(rolling().textContent).toBe('Latest');
  });

  it('preserves the focused tab and close action while its resource title rolls', () => {
    function renderTabs(title: string, id: string) {
      act(() => root.render(<TabGroup items={[
        { value: 'workspace', label: title, labelTransitionKey: id, endAction: <button>Close</button> },
        { value: 'settings', label: 'Settings' },
      ]} />));
    }
    renderTabs('Short', 'a');
    const tab = container.querySelector<HTMLButtonElement>('[role="tab"]')!;
    const close = container.querySelector('[data-openbitfun-part="endAction"] button');
    tab.focus();
    renderTabs('Long session title', 'b');
    expect(container.querySelector('[role="tab"]')).toBe(tab);
    expect(document.activeElement).toBe(tab);
    expect(container.querySelector('[data-openbitfun-part="endAction"] button')).toBe(close);
    expect(tab.contains(close)).toBe(false);
    expect(tab.getAttribute('aria-selected')).toBe('true');
    expect(container.querySelectorAll('[data-openbitfun-component="rolling-text"]')).toHaveLength(1);
  });
});
