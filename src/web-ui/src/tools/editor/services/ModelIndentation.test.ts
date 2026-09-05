// @vitest-environment jsdom
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type * as Monaco from 'monaco-editor';
import { applyModelIndentation, readModelIndentation, setModelIndentation } from './ModelIndentation';

let monaco: typeof Monaco;
beforeAll(async () => {
  window.matchMedia ??= vi.fn().mockReturnValue({ matches: false, addEventListener() {}, removeEventListener() {} });
  monaco = await import('monaco-editor/esm/vs/editor/editor.api');
});

const defaults = { tab_size: 4, insert_spaces: true, detect_indentation: true };

describe('document indentation with real Monaco models', () => {
  it.each([
    ['', 4, true],
    ['function f() {\n  one();\n  two();\n}\n', 2, true],
    ['function f() {\n\tone();\n\ttwo();\n}\n', 4, false],
  ])('detects existing indentation and defaults for %j', (content, tabSize, insertSpaces) => {
    const model = monaco.editor.createModel(content);
    try {
      expect(applyModelIndentation(model, defaults)).toEqual({ tabSize, insertSpaces });
      expect(model.getValue()).toBe(content);
    } finally { model.dispose(); }
  });

  it('updates a supplied model, keeps tab and indent width in sync, and supports disabling detection', () => {
    const model = monaco.editor.createModel('if (ok) {\n  one();\n  two();\n}\n');
    try {
      applyModelIndentation(model, defaults);
      expect(readModelIndentation(model).tabSize).toBe(2);
      applyModelIndentation(model, { ...defaults, tab_size: 3, detect_indentation: false });
      expect(model.getOptions().indentSize).toBe(3);
      expect(model.normalizeIndentation('\t')).toBe('   ');
      applyModelIndentation(model, { ...defaults, insert_spaces: false, detect_indentation: false });
      expect(model.normalizeIndentation('    ')).toBe('\t');
    } finally { model.dispose(); }
  });

  it('keeps manual choices on the document across config refreshes without affecting another document', () => {
    const first = monaco.editor.createModel('');
    const second = monaco.editor.createModel('');
    try {
      setModelIndentation(first, { tabSize: 8, insertSpaces: false });
      expect(applyModelIndentation(first, defaults)).toEqual({ tabSize: 8, insertSpaces: false });
      expect(applyModelIndentation(second, defaults)).toEqual({ tabSize: 4, insertSpaces: true });
      first.setValue('  one();\n  two();');
      expect(applyModelIndentation(first, defaults, true)).toEqual({ tabSize: 8, insertSpaces: false });
    } finally { first.dispose(); second.dispose(); }
  });

  it('detects after asynchronous file loading but does not redetect on unrelated settings changes', () => {
    const model = monaco.editor.createModel('');
    try {
      applyModelIndentation(model, defaults);
      const detect = vi.spyOn(model, 'detectIndentation');
      model.setValue('if (ok) {\n  one();\n  two();\n}\n');
      expect(applyModelIndentation(model, defaults, true).tabSize).toBe(2);
      applyModelIndentation(model, { ...defaults });
      expect(detect).toHaveBeenCalledTimes(1);
    } finally { model.dispose(); }
  });

  it('preserves explicit indentation from an older host without the detection field', () => {
    const model = monaco.editor.createModel('  one();\n  two();\n');
    try {
      expect(applyModelIndentation(model, { tab_size: 8, insert_spaces: false }))
        .toEqual({ tabSize: 8, insertSpaces: false });
    } finally { model.dispose(); }
  });
});
