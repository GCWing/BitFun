/**
 * Desktop debug inspector hook.
 *
 * Provides desktop-only shortcuts for the native DevTools window and the
 * interactive element inspector. Only active in development or when the
 * desktop app is built with the `devtools` feature.
 *
 * The inspector is injected via `eval()` into the current page, so it works
 * without any server-side changes and has zero overhead when inactive.
 */

import { useEffect } from 'react';
import { createLogger } from '@/shared/utils/logger';
import { isTauriRuntime } from '@/infrastructure/runtime';
import {
  createMainWindowInspectorScript,
  CANCEL_MAIN_WINDOW_INSPECTOR_SCRIPT,
  IS_INSPECTOR_ACTIVE_SCRIPT,
} from './mainWindowInspector';

const log = createLogger('DebugInspector');

type DebugShortcutAction = 'toggleInspector' | 'openDevTools';

/** Detect whether the desktop backend exposes debug commands in this build. */
async function loadDevToolsAvailable(): Promise<boolean> {
  // In a standard web build (non-Tauri) the inspector is useless because we
  // already have browser DevTools. Only enable in the desktop webview.
  if (typeof window === 'undefined') return false;
  if (!isTauriRuntime()) return false;

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<boolean>('debug_devtools_available');
  } catch (error) {
    log.error('Failed to detect DevTools availability', error);
    return false;
  }
}

/** Toggle the element inspector by eval-ing the inspector script into the page. */
async function toggleInspector(): Promise<void> {
  try {
    // Check if already active
    const isActive = await evalInPage<boolean>(IS_INSPECTOR_ACTIVE_SCRIPT);
    if (isActive) {
      await evalInPage<void>(CANCEL_MAIN_WINDOW_INSPECTOR_SCRIPT);
      log.info('Element inspector deactivated');
      return;
    }

    // Inject and activate
    const script = createMainWindowInspectorScript();
    await evalInPage<void>(script);
    log.info('Element inspector activated — hover to highlight, click to capture, Escape to exit');
  } catch (error) {
    log.error('Failed to toggle element inspector', error);
  }
}

/** Eval a JS snippet in the current page context. */
async function evalInPage<T>(script: string): Promise<T> {
  // We use the Function constructor to run in the page's global scope
  // rather than the current module scope. The script may be a void IIFE,
  // so we wrap it to ensure it is evaluated as an expression.
  const fn = new Function(script);
  return fn() as T;
}

/** Open the native webview DevTools window. */
async function openNativeDevTools(): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('debug_open_devtools');
    log.info('Native DevTools opened');
  } catch (error) {
    log.error('Failed to open native DevTools', error);
  }
}

function isPrimaryModifier(event: KeyboardEvent): boolean {
  const isMac = typeof navigator !== 'undefined'
    && navigator.platform.toUpperCase().includes('MAC');
  return isMac ? event.metaKey : event.ctrlKey;
}

function getDebugShortcutAction(event: KeyboardEvent): DebugShortcutAction | null {
  if (
    event.key === 'F12' &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    !event.altKey
  ) {
    return 'openDevTools';
  }

  if (!isPrimaryModifier(event) || !event.shiftKey || event.altKey) {
    return null;
  }

  const key = event.key.toLowerCase();
  if (key === 'i') {
    return 'toggleInspector';
  }
  if (key === 'j') {
    return 'openDevTools';
  }
  return null;
}

/**
 * Register debug shortcuts when running in a Tauri desktop environment.
 *
 * Shortcuts:
 *   Cmd/Ctrl + Shift + I  → Toggle element inspector
 *   Cmd/Ctrl + Shift + J  → Open native DevTools
 *   F12                   → Open native DevTools
 *
 * These shortcuts intentionally bypass the user-configurable product shortcut
 * manager so development tools stay available even when app shortcuts change.
 */
export function useDebugInspector(): void {
  useEffect(() => {
    if (typeof window === 'undefined') return;

    let cancelled = false;
    let unregister: (() => void) | null = null;

    void (async () => {
      const available = await loadDevToolsAvailable();
      if (cancelled || !available) return;

      const handleKeyDown = (event: KeyboardEvent) => {
        const action = getDebugShortcutAction(event);
        if (!action) return;

        event.preventDefault();
        event.stopPropagation();
        event.stopImmediatePropagation();

        if (action === 'toggleInspector') {
          void toggleInspector();
          return;
        }
        void openNativeDevTools();
      };

      window.addEventListener('keydown', handleKeyDown, { capture: true });
      unregister = () => window.removeEventListener('keydown', handleKeyDown, { capture: true });
    })();

    return () => {
      cancelled = true;
      unregister?.();
    };
  }, []);
}
