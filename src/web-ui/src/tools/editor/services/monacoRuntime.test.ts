import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
import type * as Monaco from 'monaco-editor';
import {
  getMonacoRuntime,
  monacoApi,
  requireMonaco,
  setMonacoRuntime,
} from './monacoRuntime';

const globalRef = globalThis as { monaco?: typeof Monaco };

const SRC_ROOT = fileURLToPath(new URL('../../../', import.meta.url));

/**
 * `import * as x from 'monaco-editor'` / `import x from 'monaco-editor'` — but not
 * `import type ...`. Anchored to the start of a line so prose inside doc comments
 * (which mentions the very import forms this guards) is not mistaken for code.
 */
const MONACO_VALUE_IMPORT = /^\s*import\s+(?!type\b)[^;\n]*from\s+['"]monaco-editor['"]/m;

function collectSourceFiles(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      collectSourceFiles(fullPath, out);
    } else if (/\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name)) {
      out.push(fullPath);
    }
  }
  return out;
}

function clearRuntime(): void {
  setMonacoRuntime(null);
  delete globalRef.monaco;
}

afterEach(() => {
  clearRuntime();
});

describe('monacoRuntime', () => {
  it('fails loudly (not silently as undefined) when Monaco is not initialized', () => {
    clearRuntime();

    expect(getMonacoRuntime()).toBeNull();
    expect(() => requireMonaco()).toThrow(/Monaco runtime is not initialized/);
    // The lazy proxy must throw on *any* property access rather than yielding
    // undefined, which would surface much later as an opaque TypeError.
    expect(() => monacoApi.editor).toThrow(/Monaco runtime is not initialized/);
    expect(() => monacoApi.KeyMod).toThrow(/Monaco runtime is not initialized/);
    expect(() => monacoApi.Uri).toThrow(/Monaco runtime is not initialized/);
  });

  it('resolves value namespaces through the proxy once the runtime is set', () => {
    const fakeMonaco = {
      KeyMod: { CtrlCmd: 2048 },
      KeyCode: { KeyS: 49 },
      MarkerSeverity: { Error: 8 },
      Range: class FakeRange {
        constructor(public readonly startLineNumber: number) {}
      },
      editor: { setTheme: () => 'called' },
      languages: { getLanguages: () => [{ id: 'toml' }] },
    } as unknown as typeof Monaco;

    setMonacoRuntime(fakeMonaco);

    expect(getMonacoRuntime()).toBe(fakeMonaco);
    expect(requireMonaco()).toBe(fakeMonaco);
    expect(monacoApi.KeyMod.CtrlCmd | monacoApi.KeyCode.KeyS).toBe(2048 | 49);
    expect(monacoApi.MarkerSeverity.Error).toBe(8);
    expect(new monacoApi.Range(1, 1, 1, 1).startLineNumber).toBe(1);
    expect(monacoApi.editor.setTheme('x' as never) as unknown).toBe('called');
    expect(monacoApi.languages.getLanguages()).toEqual([{ id: 'toml' }]);
  });

  it('falls back to the AMD loader global and clears on reset', () => {
    clearRuntime();
    const globalMonaco = { editor: {} } as unknown as typeof Monaco;
    globalRef.monaco = globalMonaco;

    expect(getMonacoRuntime()).toBe(globalMonaco);

    // MonacoInitManager.reset() calls setMonacoRuntime(null); with no global
    // present the runtime must go back to "not initialized".
    delete globalRef.monaco;
    setMonacoRuntime(null);
    expect(getMonacoRuntime()).toBeNull();
  });

  // The whole point of this module: Monaco is loaded at runtime through the AMD
  // loader, so a single value-import anywhere in src/ silently bundles a second
  // ESM copy of Monaco into the JS chunks. Guard the invariant repo-wide.
  it('is the only way source files reach the Monaco runtime (no value imports)', () => {
    const offenders = collectSourceFiles(SRC_ROOT).filter(file =>
      MONACO_VALUE_IMPORT.test(readFileSync(file, 'utf8'))
    );

    expect(offenders.map(file => file.slice(SRC_ROOT.length).replace(/\\/g, '/'))).toEqual([]);
  });
});
