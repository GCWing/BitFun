import type { editor } from 'monaco-editor';
import type { EditorConfig } from '@/infrastructure/config/types';
import { DEFAULT_EDITOR_CONFIG } from '../config/defaults';

export interface Indentation {
  tabSize: number;
  insertSpaces: boolean;
}

type IndentationConfig = Pick<Partial<EditorConfig>, 'tab_size' | 'insert_spaces' | 'detect_indentation'>;

// Model identity scopes choices to one document, including split views and editor remounts.
// Weak keys release state when MonacoModelManager disposes a document.
const states = new WeakMap<editor.ITextModel, { signature?: string; override?: Indentation }>();

export function readModelIndentation(model: editor.ITextModel): Indentation {
  const { tabSize, insertSpaces } = model.getOptions();
  return { tabSize, insertSpaces };
}

export function applyModelIndentation(
  model: editor.ITextModel,
  config: IndentationConfig,
  contentReplaced = false,
): Indentation {
  const state = states.get(model) ?? {};
  states.set(model, state);
  if (state.override) {
    model.updateOptions({ ...state.override, indentSize: 'tabSize' });
    return readModelIndentation(model);
  }

  const tabSize = config.tab_size ?? DEFAULT_EDITOR_CONFIG.tabSize;
  const insertSpaces = config.insert_spaces ?? DEFAULT_EDITOR_CONFIG.insertSpaces;
  // An older host omits this field; retain its explicit indentation behavior.
  const detect = config.detect_indentation === true;
  const signature = `${tabSize}:${insertSpaces}:${detect}`;
  if (contentReplaced || state.signature !== signature) {
    state.signature = signature;
    model.updateOptions({ tabSize, insertSpaces, indentSize: 'tabSize' });
    if (detect) model.detectIndentation(insertSpaces, tabSize);
  }
  return readModelIndentation(model);
}

export function setModelIndentation(model: editor.ITextModel, indentation: Indentation): void {
  const state = states.get(model) ?? {};
  state.override = indentation;
  states.set(model, state);
  model.updateOptions({ ...indentation, indentSize: 'tabSize' });
}
