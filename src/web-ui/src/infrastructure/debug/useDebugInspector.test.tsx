/**
 * @vitest-environment jsdom
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useDebugInspector } from './useDebugInspector';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

vi.mock('./mainWindowInspector', () => ({
  createMainWindowInspectorScript: () =>
    'window.__bitfunInspectorToggleCount = (window.__bitfunInspectorToggleCount || 0) + 1; window.__bitfun_main_inspector_active = true;',
  CANCEL_MAIN_WINDOW_INSPECTOR_SCRIPT:
    'window.__bitfun_main_inspector_active = false;',
  IS_INSPECTOR_ACTIVE_SCRIPT:
    'return Boolean(window.__bitfun_main_inspector_active);',
}));

function DebugInspectorHarness(): null {
  useDebugInspector();
  return null;
}

function setTauriRuntime(enabled: boolean): void {
  Object.defineProperty(window, '__TAURI_INTERNALS__', {
    configurable: true,
    value: enabled ? { invoke: vi.fn() } : undefined,
  });
}

function dispatchKey(init: KeyboardEventInit): KeyboardEvent {
  const event = new KeyboardEvent('keydown', {
    bubbles: true,
    cancelable: true,
    ...init,
  });
  document.body.dispatchEvent(event);
  return event;
}

describe('useDebugInspector', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.invoke.mockImplementation((command: string) => {
      if (command === 'debug_devtools_available') {
        return Promise.resolve(true);
      }
      return Promise.resolve(undefined);
    });
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllMocks();
    setTauriRuntime(false);
    delete (window as unknown as { __bitfunInspectorToggleCount?: number }).__bitfunInspectorToggleCount;
    delete (window as unknown as { __bitfun_main_inspector_active?: boolean }).__bitfun_main_inspector_active;
  });

  it('does not intercept DevTools shortcuts outside the desktop runtime', () => {
    setTauriRuntime(false);
    act(() => {
      root.render(<DebugInspectorHarness />);
    });

    const event = dispatchKey({ key: 'F12' });

    expect(event.defaultPrevented).toBe(false);
    expect(mocks.invoke).not.toHaveBeenCalled();
  });

  it('opens native DevTools with F12 in the desktop runtime', async () => {
    setTauriRuntime(true);
    act(() => {
      root.render(<DebugInspectorHarness />);
    });
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('debug_devtools_available'));
    mocks.invoke.mockClear();

    const event = dispatchKey({ key: 'F12' });

    expect(event.defaultPrevented).toBe(true);
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('debug_open_devtools'));
  });

  it('keeps Ctrl+Shift+I available for the element inspector', async () => {
    setTauriRuntime(true);
    act(() => {
      root.render(<DebugInspectorHarness />);
    });
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('debug_devtools_available'));
    mocks.invoke.mockClear();

    const event = dispatchKey({ key: 'i', ctrlKey: true, shiftKey: true });

    expect(event.defaultPrevented).toBe(true);
    await vi.waitFor(() =>
      expect((window as unknown as { __bitfunInspectorToggleCount?: number }).__bitfunInspectorToggleCount)
        .toBe(1)
    );
  });

  it('does not intercept shortcuts when desktop DevTools are unavailable', async () => {
    mocks.invoke.mockImplementation((command: string) => {
      if (command === 'debug_devtools_available') {
        return Promise.resolve(false);
      }
      return Promise.resolve(undefined);
    });
    setTauriRuntime(true);
    act(() => {
      root.render(<DebugInspectorHarness />);
    });
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('debug_devtools_available'));
    mocks.invoke.mockClear();

    const event = dispatchKey({ key: 'F12' });

    expect(event.defaultPrevented).toBe(false);
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
});
