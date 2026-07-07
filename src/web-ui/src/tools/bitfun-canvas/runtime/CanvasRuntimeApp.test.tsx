import { afterEach, describe, expect, it, vi } from 'vitest';

import { installBitfunCanvasRuntimeApp } from './CanvasRuntimeApp';

type TestWindow = Window & {
  BitfunCanvasSDK?: Record<string, unknown>;
  BitfunCanvasSDKAdapters?: Record<string, unknown>;
  BitfunCanvasRuntime?: Record<string, any>;
  ReactDOM?: {
    createRoot: (element: HTMLElement) => { render: (node: unknown) => void };
  };
};

const originalWindow = (globalThis as typeof globalThis & { window?: Window }).window;
const originalDocument = (globalThis as typeof globalThis & { document?: Document }).document;

afterEach(() => {
  (globalThis as typeof globalThis & { window?: Window }).window = originalWindow;
  (globalThis as typeof globalThis & { document?: Document }).document = originalDocument;
});

function installTestDom() {
  const render = vi.fn();
  const postMessage = vi.fn();
  const rootElement = {
    innerHTML: '',
    querySelector: vi.fn(),
  } as unknown as HTMLElement;
  const testWindow = {
    parent: { postMessage },
    setTimeout: vi.fn(),
    clearTimeout: vi.fn(),
    BitfunCanvasSDK: { Fallback: true },
    BitfunCanvasSDKAdapters: { Adapter: true },
    BitfunCanvasRuntime: { fallback: true },
    ReactDOM: {
      createRoot: vi.fn(() => ({ render })),
    },
  } as unknown as TestWindow;
  const testDocument = {
    getElementById: vi.fn(() => rootElement),
    querySelector: vi.fn(() => ({ getAttribute: () => 'rev_test' })),
  } as unknown as Document;

  (globalThis as typeof globalThis & { window?: TestWindow }).window = testWindow;
  (globalThis as typeof globalThis & { document?: Document }).document = testDocument;

  return { render, postMessage, rootElement, testWindow };
}

describe('CanvasRuntimeApp', () => {
  it('installs runtime hooks without removing fallback runtime fields', () => {
    const { testWindow } = installTestDom();

    installBitfunCanvasRuntimeApp();

    expect(testWindow.BitfunCanvasRuntime?.fallback).toBe(true);
    expect(testWindow.BitfunCanvasRuntime?.h).toBeTypeOf('function');
    expect(testWindow.BitfunCanvasRuntime?.Fragment).toBeTruthy();
  });

  it('merges SDK adapters on module start and posts startup event', () => {
    const { postMessage, testWindow } = installTestDom();

    installBitfunCanvasRuntimeApp();
    testWindow.BitfunCanvasRuntime?.moduleStarted();

    expect(testWindow.BitfunCanvasSDK).toMatchObject({ Fallback: true, Adapter: true });
    expect(postMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'bitfun-canvas-module-started',
        sourceRevisionSeen: 'rev_test',
      }),
      '*',
    );
  });

  it('mounts user components through the bundled React runtime app', () => {
    const { render, postMessage, testWindow } = installTestDom();
    function Canvas() {
      return null;
    }

    installBitfunCanvasRuntimeApp();
    testWindow.BitfunCanvasRuntime?.mount(Canvas);

    expect(testWindow.ReactDOM?.createRoot).toHaveBeenCalled();
    expect(render).toHaveBeenCalled();
    expect(postMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'bitfun-canvas-ready',
        sourceRevisionSeen: 'rev_test',
      }),
      '*',
    );
  });
});
