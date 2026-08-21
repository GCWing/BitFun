/**
 * Vitest setup: provide an in-memory `localStorage` for the Node test runtime.
 *
 * Node >= 22 exposes an experimental webstorage `localStorage` global. Without a
 * valid `--localstorage-file` path (the default on Node 25) it is a method-less
 * shell, so code guarding with `typeof localStorage === 'undefined'` (zustand
 * persist, dispatchJobStore, FlowChatStore) treats it as real storage and
 * throws `localStorage.getItem is not a function`. Replace the shell with a
 * working in-memory Storage before any store module loads.
 */
import { enableMapSet } from 'immer';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Enable React's act() environment for every test. Without this, React logs
// "Warning: The current testing environment is not configured to support
// act(...)" once per act()/render() call (~1100+ lines in CI logs), and ~80
// test files each set the flag manually. Setting it once here removes the
// per-file duplication and silences the warning globally.
(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// Flow-chat stores use Map-based state (contract §2.2).
// immer needs the MapSet plugin enabled explicitly to draft-change Maps.
enableMapSet();

// W4 production/test initialization consistency (2026-08-13, plan v1.1 sec 4.1,
// Mengqing P2-1 fix): every global plugin/initialization must be enabled on
// BOTH main.tsx and setup.ts (prevent the divergence regression - setup had
// enableMapSet but the production entry lacked it, crashing at runtime). Both
// sides read the **source** and check call existence (not typeof - after an
// import, typeof is always 'function' and the check is a no-op). The reverse
// branch (main enabled, setup missing) is reachable - no dead code.

/** Pure logic (testable): returns the divergence message, or null when in sync. */
export function detectMapSetDivergence(mainSource: string, setupSource: string): string | null {
  const mainHasMapSet = mainSource.includes('enableMapSet()');
  const setupHasMapSet = setupSource.includes('enableMapSet()');
  if (setupHasMapSet && !mainHasMapSet) {
    return (
      'Global plugin initialization divergence: test/setup.ts enables enableMapSet() but ' +
      'production main.tsx does not. Add it to the global-plugin-enablement checklist in main.tsx.'
    );
  }
  // Reverse branch (main enabled, setup missing) - testable, not dead code.
  if (!setupHasMapSet && mainHasMapSet) {
    return (
      'Global plugin initialization divergence: main.tsx enables enableMapSet() but ' +
      'test/setup.ts does not. Keep both ends in sync (see main.tsx checklist).'
    );
  }
  return null;
}

function assertGlobalPluginInitializationConsistency(): void {
  const mainSource = readFileSync(resolve(__dirname, '../main.tsx'), 'utf8');
  const setupSource = readFileSync(resolve(__dirname, 'setup.ts'), 'utf8');
  const divergence = detectMapSetDivergence(mainSource, setupSource);
  if (divergence) {
    throw new Error(divergence);
  }
}
assertGlobalPluginInitializationConsistency();

if (
  typeof globalThis.localStorage === 'undefined'
  || typeof globalThis.localStorage.getItem !== 'function'
) {
  const values = new Map<string, string>();
  const memoryStorage: Storage = {
    get length(): number {
      return values.size;
    },
    clear(): void {
      values.clear();
    },
    getItem(key: string): string | null {
      return values.get(key) ?? null;
    },
    key(index: number): string | null {
      return Array.from(values.keys())[index] ?? null;
    },
    removeItem(key: string): void {
      values.delete(key);
    },
    setItem(key: string, value: string): void {
      values.set(key, String(value));
    },
  };
  Object.defineProperty(globalThis, 'localStorage', {
    value: memoryStorage,
    configurable: true,
    writable: true,
  });
}
