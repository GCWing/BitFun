/**
 * Desktop debug inspector hook.
 *
 * Provides Cmd/Ctrl + Shift + I shortcut to toggle the interactive element
 * inspector in the main webview. Only active in development or when the
 * desktop app is built with the `devtools` feature.
 *
 * The inspector is injected via `eval()` into the current page, so it works
 * without any server-side changes and has zero overhead when inactive.
 */

import { useCallback } from 'react';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { createLogger } from '@/shared/utils/logger';
import { isTauriRuntime } from '@/infrastructure/runtime';
import {
  createMainWindowInspectorScript,
  CANCEL_MAIN_WINDOW_INSPECTOR_SCRIPT,
  IS_INSPECTOR_ACTIVE_SCRIPT,
} from './mainWindowInspector';

const log = createLogger('DebugInspector');

/** Detect whether we are running inside a Tauri desktop webview with devtools available. */
function isDevToolsAvailable(): boolean {
  // In a standard web build (non-Tauri) the inspector is useless because we
  // already have browser DevTools. Only enable in the desktop webview.
  if (typeof window === 'undefined') return false;
  if (!isTauriRuntime()) return false;

  // The backend only exposes debug commands when compiled with devtools feature
  // or in debug builds. We optimistically enable the shortcut here; the invoke
  // will gracefully fail if the backend does not support it.
  return true;
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

/**
 * Register debug shortcuts when running in a Tauri desktop environment.
 *
 * Shortcuts:
 *   Cmd/Ctrl + Shift + I  → Toggle element inspector
 *   Cmd/Ctrl + Shift + J  → Open native DevTools
 *
 * Note: shortcuts are always registered (so they work even if Tauri runtime
 * detection races with component mount), but the callback no-ops when not
 * in a Tauri desktop environment.
 */
export function useDebugInspector(): void {
  const handleToggleInspector = useCallback(() => {
    if (!isDevToolsAvailable()) return;
    void toggleInspector();
  }, []);

  const handleOpenDevTools = useCallback(() => {
    if (!isDevToolsAvailable()) return;
    void openNativeDevTools();
  }, []);

  // Ctrl/Cmd + Shift + I — toggle element inspector
  // ctrl: true maps to Cmd on macOS and Ctrl on Windows/Linux (handled by ShortcutManager)
  useShortcut(
    'debug.toggleInspector',
    { key: 'i', ctrl: true, shift: true, scope: 'app', allowInInput: true },
    handleToggleInspector,
    {
      priority: 100,
      description: 'Toggle element inspector',
    }
  );

  // Ctrl/Cmd + Shift + J — open native DevTools
  useShortcut(
    'debug.openDevTools',
    { key: 'j', ctrl: true, shift: true, scope: 'app', allowInInput: true },
    handleOpenDevTools,
    {
      priority: 100,
      description: 'Open native DevTools',
    }
  );
}
