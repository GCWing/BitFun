
import { useEffect, useRef } from 'react';
import { shortcutManager, type ShortcutCallback } from '@/infrastructure/services/ShortcutManager';
import type { ShortcutConfig } from '@/shared/types/shortcut';

export interface UseShortcutOptions {
  description?: string;
  priority?: number;
  /** Set to false to temporarily disable this shortcut without unregistering. */
  enabled?: boolean;
}

/**
 * Register a keyboard shortcut via ShortcutManager with automatic cleanup.
 *
 * Uses a stable ref pattern so that changes to `callback` never cause the
 * shortcut to be unregistered and re-registered — only changes to `id` or
 * `config` trigger a re-registration.
 *
 * @param id       Unique shortcut identifier (e.g. 'nav.toggleSearch')
 * @param config   Key + modifiers + scope config
 * @param callback Handler to invoke when the shortcut fires
 * @param options  Optional description, priority, enabled flag
 */
export function useShortcut(
  id: string,
  config: ShortcutConfig,
  callback: ShortcutCallback,
  options: UseShortcutOptions = {}
): void {
  const { description, priority, enabled = true } = options;

  // Keep the latest callback in a ref so changes don't trigger re-registration.
  const callbackRef = useRef<ShortcutCallback>(callback);
  callbackRef.current = callback;

  // Stable wrapper that delegates to the current callback ref.
  const stableCallback = useRef<ShortcutCallback>((e) => callbackRef.current(e));

  // Serialize config to a stable string for the dependency array.
  const configKey = JSON.stringify({
    key: config.key,
    ctrl: config.ctrl,
    shift: config.shift,
    alt: config.alt,
    meta: config.meta,
    scope: config.scope,
    allowInInput: config.allowInInput,
  });

  useEffect(() => {
    if (!enabled) return;

    const unregister = shortcutManager.register(id, config, stableCallback.current, {
      description,
      priority,
    });

    return unregister;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, configKey, enabled, description, priority]);
}
