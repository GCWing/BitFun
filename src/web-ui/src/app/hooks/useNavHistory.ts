/**
 * useNavHistory — in-app navigation history (back / forward).
 *
 * Records navigation events dispatched via the `nav-push` custom event
 * and exposes goBack / goForward actions.
 *
 * Usage (push a new entry from anywhere):
 *   window.dispatchEvent(new CustomEvent('nav-push', { detail: { key: 'scene:git' } }))
 *
 * Components that want to restore state on back/forward should listen for
 * the `nav-restore` event:
 *   window.dispatchEvent(new CustomEvent('nav-restore', { detail: entry }))
 */

import { useState, useEffect, useCallback, useRef } from 'react';

export interface NavEntry {
  key: string;
  [extra: string]: unknown;
}

interface NavHistoryState {
  stack: NavEntry[];
  cursor: number;
}

// ── Singleton event bus so all hook instances share state ─────────────────

let _state: NavHistoryState = { stack: [], cursor: -1 };
const _listeners = new Set<() => void>();

function _notify() {
  _listeners.forEach(fn => fn());
}

function _push(entry: NavEntry) {
  // Drop forward entries after the cursor
  const trimmed = _state.stack.slice(0, _state.cursor + 1);
  // Avoid pushing a duplicate of the current entry
  const current = trimmed[trimmed.length - 1];
  if (current && current.key === entry.key) return;
  _state = { stack: [...trimmed, entry], cursor: trimmed.length };
  _notify();
}

/**
 * pushNavEntry — public imperative API for non-hook callers (e.g. stores).
 * Adds an entry to the history stack directly without going through window events.
 */
export function pushNavEntry(entry: NavEntry): void {
  _push(entry);
}

function _back(): NavEntry | undefined {
  if (_state.cursor <= 0) return undefined;
  _state = { ..._state, cursor: _state.cursor - 1 };
  _notify();
  return _state.stack[_state.cursor];
}

function _forward(): NavEntry | undefined {
  if (_state.cursor >= _state.stack.length - 1) return undefined;
  _state = { ..._state, cursor: _state.cursor + 1 };
  _notify();
  return _state.stack[_state.cursor];
}

export function useNavHistory() {
  const [, forceUpdate] = useState(0);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    const refresh = () => { if (mountedRef.current) forceUpdate(n => n + 1); };
    _listeners.add(refresh);

    // Listen for external push events
    const handlePush = (e: Event) => {
      const entry = (e as CustomEvent<NavEntry>).detail;
      if (entry?.key) _push(entry);
    };
    window.addEventListener('nav-push', handlePush);

    return () => {
      mountedRef.current = false;
      _listeners.delete(refresh);
      window.removeEventListener('nav-push', handlePush);
    };
  }, []);

  const goBack = useCallback(() => {
    const entry = _back();
    if (entry) {
      window.dispatchEvent(new CustomEvent('nav-restore', { detail: entry }));
    }
  }, []);

  const goForward = useCallback(() => {
    const entry = _forward();
    if (entry) {
      window.dispatchEvent(new CustomEvent('nav-restore', { detail: entry }));
    }
  }, []);

  return {
    canGoBack: _state.cursor > 0,
    canGoForward: _state.cursor < _state.stack.length - 1,
    goBack,
    goForward,
  };
}
