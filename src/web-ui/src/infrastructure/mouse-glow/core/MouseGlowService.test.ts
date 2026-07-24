// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_MOUSE_GLOW_ENABLED,
  MOUSE_GLOW_STORAGE_KEY,
  MouseGlowService,
} from './MouseGlowService';

describe('MouseGlowService', () => {
  let service: MouseGlowService;
  let nextFrame: FrameRequestCallback | undefined;

  beforeEach(() => {
    const storedValues = new Map<string, string>();
    Object.defineProperty(window, 'localStorage', {
      configurable: true,
      value: {
        clear: () => storedValues.clear(),
        getItem: (key: string) => storedValues.get(key) ?? null,
        key: (index: number) => Array.from(storedValues.keys())[index] ?? null,
        removeItem: (key: string) => storedValues.delete(key),
        setItem: (key: string, value: string) => storedValues.set(key, value),
        get length() {
          return storedValues.size;
        },
      } satisfies Storage,
    });
    document.documentElement.removeAttribute('data-mouse-glow-enabled');
    document.getElementById('bitfun-mouse-glow-overlay')?.remove();

    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: vi.fn(() => ({
        matches: false,
        media: '(prefers-reduced-motion: reduce)',
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      nextFrame = callback;
      return 1;
    });
    vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);

    service = new MouseGlowService();
  });

  afterEach(() => {
    service.dispose();
    vi.restoreAllMocks();
  });

  it('defaults to enabled when no preference has been stored', () => {
    service.initialize();

    expect(service.getEnabled()).toBe(DEFAULT_MOUSE_GLOW_ENABLED);
    expect(document.documentElement.hasAttribute('data-mouse-glow-enabled')).toBe(true);
  });

  it('restores and updates a disabled preference', () => {
    window.localStorage.setItem(MOUSE_GLOW_STORAGE_KEY, 'false');
    service.initialize();

    expect(service.getEnabled()).toBe(false);
    expect(document.documentElement.hasAttribute('data-mouse-glow-enabled')).toBe(false);

    service.setEnabled(true);

    expect(window.localStorage.getItem(MOUSE_GLOW_STORAGE_KEY)).toBe('true');
    expect(document.documentElement.hasAttribute('data-mouse-glow-enabled')).toBe(true);
  });

  it('updates the shared pointer variables at most once per animation frame', () => {
    const surface = document.createElement('div');
    surface.setAttribute('data-mouse-glow-surface', '');
    surface.getBoundingClientRect = () => ({
      bottom: 128,
      height: 80,
      left: 20,
      right: 220,
      top: 48,
      width: 200,
      x: 20,
      y: 48,
      toJSON: () => ({}),
    });
    document.body.appendChild(surface);
    service.initialize();
    surface.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 72,
      clientY: 68,
    }));

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.hasAttribute('data-active')).toBe(false);
    expect(overlay?.hidden).toBe(true);
    nextFrame?.(0);

    expect(overlay?.style.width).toBe('200px');
    expect(overlay?.style.height).toBe('80px');
    expect(overlay?.style.getPropertyValue('--mouse-glow-local-x')).toBe('52px');
    expect(overlay?.style.getPropertyValue('--mouse-glow-local-y')).toBe('20px');
    expect(overlay?.hasAttribute('data-active')).toBe(true);
    expect(overlay?.hidden).toBe(false);
    surface.remove();
  });

  it('clears the previous glow immediately when the pointer leaves its surface', () => {
    const surface = document.createElement('div');
    surface.setAttribute('data-mouse-glow-surface', '');
    surface.getBoundingClientRect = () => ({
      bottom: 128,
      height: 80,
      left: 20,
      right: 220,
      top: 48,
      width: 200,
      x: 20,
      y: 48,
      toJSON: () => ({}),
    });
    const plainElement = document.createElement('span');
    document.body.append(surface, plainElement);
    service.initialize();

    surface.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 72,
      clientY: 68,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.hasAttribute('data-active')).toBe(true);

    plainElement.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 260,
      clientY: 68,
    }));

    expect(overlay?.hasAttribute('data-active')).toBe(false);
    expect(overlay?.hidden).toBe(true);
    surface.remove();
    plainElement.remove();
  });

  it('clears the glow when the pointer enters an iframe', () => {
    const surface = document.createElement('div');
    surface.setAttribute('data-mouse-glow-surface', '');
    surface.getBoundingClientRect = () => ({
      bottom: 128,
      height: 80,
      left: 20,
      right: 220,
      top: 48,
      width: 200,
      x: 20,
      y: 48,
      toJSON: () => ({}),
    });
    const iframe = document.createElement('iframe');
    document.body.append(surface, iframe);
    service.initialize();

    surface.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 72,
      clientY: 68,
    }));
    nextFrame?.(0);
    surface.dispatchEvent(new MouseEvent('pointerout', {
      bubbles: true,
      relatedTarget: iframe,
    }));

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.hasAttribute('data-active')).toBe(false);
    expect(overlay?.hidden).toBe(true);
    surface.remove();
    iframe.remove();
  });

  it('keeps background glow below floating controls and highlights the floating layer itself', () => {
    const stackingHost = document.createElement('div');
    stackingHost.style.position = 'relative';
    stackingHost.style.zIndex = '1';
    const surface = document.createElement('div');
    surface.setAttribute('data-mouse-glow-surface', '');
    surface.getBoundingClientRect = () => ({
      bottom: 128,
      height: 80,
      left: 20,
      right: 220,
      top: 48,
      width: 200,
      x: 20,
      y: 48,
      toJSON: () => ({}),
    });
    const listbox = document.createElement('div');
    listbox.setAttribute('role', 'listbox');
    listbox.style.position = 'absolute';
    listbox.style.zIndex = '60';
    listbox.style.border = '1px solid black';
    listbox.getBoundingClientRect = () => ({
      bottom: 300,
      height: 160,
      left: 180,
      right: 380,
      top: 140,
      width: 200,
      x: 180,
      y: 140,
      toJSON: () => ({}),
    });
    const option = document.createElement('div');
    option.setAttribute('role', 'option');
    listbox.appendChild(option);
    surface.appendChild(listbox);
    stackingHost.appendChild(surface);
    document.body.appendChild(stackingHost);
    service.initialize();

    surface.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 72,
      clientY: 68,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.hasAttribute('data-active')).toBe(true);
    expect(overlay?.parentElement).toBe(stackingHost);
    expect(Number(window.getComputedStyle(listbox).zIndex)).toBeGreaterThan(49);

    option.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 220,
      clientY: 180,
    }));
    nextFrame?.(16);

    expect(overlay?.hasAttribute('data-active')).toBe(true);
    expect(overlay?.parentElement).toBe(listbox);
    expect(overlay?.hasAttribute('data-local-position')).toBe(true);
    expect(overlay?.style.width).toBe('198px');
    expect(overlay?.style.height).toBe('158px');
    expect(overlay?.style.transform).toBe('translate3d(0px, 0px, 0)');
    stackingHost.remove();
  });

  it('highlights a nearby single-border divider as a line', () => {
    const divider = document.createElement('section');
    divider.style.display = 'block';
    divider.style.opacity = '1';
    divider.style.visibility = 'visible';
    divider.style.border = '0 solid transparent';
    divider.style.borderBottom = '1px solid black';
    divider.getBoundingClientRect = () => ({
      bottom: 120,
      height: 80,
      left: 40,
      right: 360,
      top: 40,
      width: 320,
      x: 40,
      y: 40,
      toJSON: () => ({}),
    });
    const content = document.createElement('span');
    divider.appendChild(content);
    document.body.appendChild(divider);
    service.initialize();

    content.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 96,
      clientY: 60,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.hasAttribute('data-active')).toBe(true);
    expect(overlay?.hasAttribute('data-divider')).toBe(true);
    expect(overlay?.style.width).toBe('320px');
    expect(overlay?.style.height).toBe('1px');
    expect(overlay?.style.transform).toBe('translate3d(40px, 119px, 0)');
    expect(overlay?.style.getPropertyValue('--mouse-glow-local-y')).toBe('-59px');
    divider.remove();
  });

  it('uses local coordinates inside a transformed input host', () => {
    const transformedHost = document.createElement('div');
    transformedHost.style.position = 'absolute';
    transformedHost.style.zIndex = '100';
    transformedHost.style.transform = 'translateX(-50%)';
    transformedHost.getBoundingClientRect = () => ({
      bottom: 700,
      height: 200,
      left: 100,
      right: 800,
      top: 500,
      width: 700,
      x: 100,
      y: 500,
      toJSON: () => ({}),
    });
    const inputSurface = document.createElement('div');
    inputSurface.setAttribute('data-mouse-glow-surface', '');
    inputSurface.style.border = '1px solid black';
    inputSurface.style.borderRadius = '22px';
    inputSurface.getBoundingClientRect = () => ({
      bottom: 584,
      height: 44,
      left: 120,
      right: 720,
      top: 540,
      width: 600,
      x: 120,
      y: 540,
      toJSON: () => ({}),
    });
    const editor = document.createElement('div');
    editor.setAttribute('contenteditable', 'true');
    inputSurface.appendChild(editor);
    transformedHost.appendChild(inputSurface);
    document.body.appendChild(transformedHost);
    service.initialize();

    editor.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 180,
      clientY: 560,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.parentElement).toBe(transformedHost);
    expect(overlay?.hasAttribute('data-local-position')).toBe(true);
    expect(overlay?.style.transform).toBe('translate3d(20px, 40px, 0)');
    expect(overlay?.style.getPropertyValue('--mouse-glow-local-x')).toBe('60px');
    expect(overlay?.style.getPropertyValue('--mouse-glow-local-y')).toBe('20px');
    transformedHost.remove();
  });

  it('automatically detects bordered product surfaces without an explicit marker', () => {
    const surface = document.createElement('section');
    surface.className = 'workspace-card';
    surface.style.display = 'block';
    surface.style.opacity = '1';
    surface.style.visibility = 'visible';
    surface.style.border = '1px solid black';
    surface.style.borderRadius = '12px';
    surface.style.background = 'black';
    surface.getBoundingClientRect = () => ({
      bottom: 180,
      height: 140,
      left: 40,
      right: 360,
      top: 40,
      width: 320,
      x: 40,
      y: 40,
      toJSON: () => ({}),
    });
    const content = document.createElement('span');
    surface.appendChild(content);
    document.body.appendChild(surface);
    service.initialize();

    content.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 96,
      clientY: 72,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.style.width).toBe('320px');
    expect(overlay?.style.borderRadius).toBe('12px');
    expect(overlay?.hasAttribute('data-active')).toBe(true);
    surface.remove();
  });

  it('detects semantic borderless cards', () => {
    const cardButton = document.createElement('button');
    cardButton.className = 'nursery-template-card';
    cardButton.style.borderRadius = '15px';
    cardButton.style.background = 'black';
    cardButton.getBoundingClientRect = () => ({
      bottom: 200,
      height: 160,
      left: 50,
      right: 350,
      top: 40,
      width: 300,
      x: 50,
      y: 40,
      toJSON: () => ({}),
    });
    document.body.appendChild(cardButton);
    service.initialize();

    cardButton.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 90,
      clientY: 80,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.style.width).toBe('300px');
    expect(overlay?.hasAttribute('data-active')).toBe(true);
    cardButton.remove();
  });

  it('automatically detects an inset-shadow search control', () => {
    const searchButton = document.createElement('button');
    searchButton.style.borderRadius = '9999px';
    searchButton.style.boxShadow = 'inset 0 0 0 1px black';
    searchButton.getBoundingClientRect = () => ({
      bottom: 72,
      height: 32,
      left: 16,
      right: 240,
      top: 40,
      width: 224,
      x: 16,
      y: 40,
      toJSON: () => ({}),
    });
    document.body.appendChild(searchButton);
    service.initialize();

    searchButton.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 80,
      clientY: 56,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.hasAttribute('data-active')).toBe(true);
    expect(overlay?.style.width).toBe('224px');
    expect(overlay?.style.height).toBe('32px');
    expect(overlay?.style.borderRadius).toBe('9999px');
    searchButton.remove();
  });

  it('automatically detects small bordered controls', () => {
    const control = document.createElement('input');
    control.style.border = '1px solid black';
    control.style.borderRadius = '6px';
    control.getBoundingClientRect = () => ({
      bottom: 60,
      height: 28,
      left: 20,
      right: 48,
      top: 32,
      width: 28,
      x: 20,
      y: 32,
      toJSON: () => ({}),
    });
    document.body.appendChild(control);
    service.initialize();

    control.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 34,
      clientY: 46,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.hasAttribute('data-active')).toBe(true);
    expect(overlay?.style.width).toBe('28px');
    expect(overlay?.style.height).toBe('28px');
    control.remove();
  });

  it('ignores resize interaction handles', () => {
    const resizer = document.createElement('div');
    resizer.setAttribute('role', 'separator');
    resizer.style.cursor = 'col-resize';
    resizer.style.borderLeft = '1px solid black';
    resizer.getBoundingClientRect = () => ({
      bottom: 600,
      height: 600,
      left: 240,
      right: 248,
      top: 0,
      width: 8,
      x: 240,
      y: 0,
      toJSON: () => ({}),
    });
    document.body.appendChild(resizer);
    service.initialize();

    resizer.dispatchEvent(new MouseEvent('pointermove', {
      bubbles: true,
      clientX: 244,
      clientY: 300,
    }));
    nextFrame?.(0);

    const overlay = document.getElementById('bitfun-mouse-glow-overlay');
    expect(overlay?.hasAttribute('data-active')).toBe(false);
    expect(overlay?.hidden).toBe(true);
    resizer.remove();
  });
});
