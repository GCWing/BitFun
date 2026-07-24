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

  it('detects semantic card buttons while ignoring ordinary controls', () => {
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
});
