/**
 * Monaco runtime instance holder.
 *
 * The app loads Monaco at runtime via the AMD loader (`loader.init()` in
 * MonacoInitManager). Source files must NOT value-import 'monaco-editor'
 * (that would bundle a second, ESM copy of Monaco into the JS chunks).
 *
 * Use `import type * as monaco from 'monaco-editor'` for types, and access
 * the runtime API through this module.
 */

import type * as Monaco from 'monaco-editor';

let runtime: typeof Monaco | null = null;

/** Called by MonacoInitManager once `loader.init()` resolves. */
export function setMonacoRuntime(monaco: typeof Monaco | null): void {
  runtime = monaco;
}

/** Returns the Monaco runtime, or null when Monaco has not been loaded yet. */
export function getMonacoRuntime(): typeof Monaco | null {
  if (runtime) {
    return runtime;
  }
  // Fallback: the AMD loader exposes the instance on window.monaco.
  const globalMonaco = (globalThis as { monaco?: typeof Monaco }).monaco;
  return globalMonaco ?? null;
}

/** Returns the Monaco runtime, throwing when Monaco has not been loaded yet. */
export function requireMonaco(): typeof Monaco {
  const monaco = getMonacoRuntime();
  if (!monaco) {
    throw new Error(
      'Monaco runtime is not initialized. Call monacoInitManager.initialize() before using Monaco APIs.'
    );
  }
  return monaco;
}

/**
 * Lazy proxy over the Monaco runtime namespace.
 *
 * Lets call sites keep `monacoApi.editor.create(...)` / `monacoApi.KeyMod.CtrlCmd`
 * syntax without eagerly resolving the runtime at module-evaluation time.
 * Accessing any property before Monaco is initialized throws.
 */
export const monacoApi: typeof Monaco = new Proxy({} as typeof Monaco, {
  get(_target, prop) {
    return (requireMonaco() as unknown as Record<PropertyKey, unknown>)[prop];
  },
  has(_target, prop) {
    return prop in (requireMonaco() as unknown as Record<PropertyKey, unknown>);
  },
});
