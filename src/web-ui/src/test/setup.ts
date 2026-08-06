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
