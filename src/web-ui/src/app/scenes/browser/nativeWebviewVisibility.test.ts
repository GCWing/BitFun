// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createNativeWebviewVisibility, hasNativeWebviewOccluder } from './nativeWebviewVisibility';

afterEach(() => {
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

function element(rect: DOMRect, occluder = false) {
  const node = document.createElement('div');
  if (occluder) node.setAttribute('data-openbitfun-native-webview-occlusion', '');
  node.getBoundingClientRect = () => rect;
  document.body.append(node);
  return node;
}

describe('native view occlusion geometry', () => {
  it('tracks multiple overlays and ignores hidden, non-overlapping and ancestor surfaces', () => {
    const bounds = new DOMRect(0, 0, 100, 100);
    const viewport = element(bounds);
    const first = element(new DOMRect(80, 80, 100, 100), true);
    const second = element(new DOMRect(10, 10, 20, 20), true);
    expect(hasNativeWebviewOccluder(viewport, bounds)).toBe(true);
    first.remove();
    expect(hasNativeWebviewOccluder(viewport, bounds)).toBe(true);
    second.style.visibility = 'hidden';
    expect(hasNativeWebviewOccluder(viewport, bounds)).toBe(false);
    second.style.visibility = 'visible';
    second.style.opacity = '0';
    expect(hasNativeWebviewOccluder(viewport, bounds)).toBe(true);
    second.style.opacity = '1';
    second.getBoundingClientRect = () => new DOMRect(100, 0, 20, 20);
    expect(hasNativeWebviewOccluder(viewport, bounds)).toBe(false);
    second.getBoundingClientRect = () => bounds;
    second.append(viewport);
    expect(hasNativeWebviewOccluder(viewport, bounds)).toBe(false);
  });
});

describe('native view visibility serialization', () => {
  it('rechecks occlusion before focusing when show is already in flight', async () => {
    let visible = true;
    let finishShow!: () => void;
    const calls: string[] = [];
    const view = {
      show: vi.fn(() => new Promise<void>(resolve => { calls.push('show'); finishShow = resolve; })),
      hide: vi.fn(async () => { calls.push('hide'); }),
      setFocus: vi.fn(async () => { calls.push('focus'); }),
    };
    const sync = createNativeWebviewVisibility(() => visible);
    const showing = sync(view, true);
    await vi.waitFor(() => expect(view.show).toHaveBeenCalledOnce());
    visible = false;
    const hiding = sync(view);
    finishShow();
    await Promise.all([showing, hiding]);
    expect(calls).toEqual(['show', 'hide']);
  });

  it('rechecks activation after a pending hide and suppresses redundant native calls', async () => {
    let visible = false;
    let finishHide!: () => void;
    const calls: string[] = [];
    const view = {
      show: async () => { calls.push('show'); },
      hide: () => new Promise<void>(resolve => { calls.push('hide'); finishHide = resolve; }),
      setFocus: async () => { calls.push('focus'); },
    };
    const sync = createNativeWebviewVisibility(() => visible);
    const hiding = sync(view);
    await vi.waitFor(() => expect(calls).toEqual(['hide']));
    visible = true;
    const showing = sync(view);
    finishHide();
    await Promise.all([hiding, showing]);
    await sync(view);
    expect(calls).toEqual(['hide', 'show']);
  });
});
