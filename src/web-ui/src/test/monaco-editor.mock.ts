class MockRange {
  constructor(
    public startLineNumber: number,
    public startColumn: number,
    public endLineNumber: number,
    public endColumn: number,
  ) {}
}

const disposable = {
  dispose: () => undefined,
};

const mockEditor = {
  getDomNode: () => null,
  getSelection: () => null,
  getModel: () => null,
  getPosition: () => null,
  getVisibleRanges: () => [],
};

export const Range = MockRange;

export const Uri = {
  parse: (value: string) => ({
    toString: () => value,
    path: value,
  }),
  file: (value: string) => ({
    toString: () => `file://${value}`,
    path: value,
  }),
};

export const KeyMod = {
  CtrlCmd: 2048,
  Shift: 1024,
  Alt: 512,
  WinCtrl: 256,
};

export const KeyCode = {};

export const editor = {
  defineTheme: () => undefined,
  setTheme: () => undefined,
  getEditors: () => [],
  create: () => mockEditor,
  createDiffEditor: () => mockEditor,
  createModel: () => mockEditor,
  setModelLanguage: () => undefined,
  getModel: () => null,
  getModels: () => [],
  onDidCreateModel: () => disposable,
  onWillDisposeModel: () => disposable,
};

export const languages = {
  register: () => undefined,
  setMonarchTokensProvider: () => disposable,
  setLanguageConfiguration: () => disposable,
  registerCompletionItemProvider: () => disposable,
  registerHoverProvider: () => disposable,
  registerDefinitionProvider: () => disposable,
  registerDocumentFormattingEditProvider: () => disposable,
};

export default {
  Range,
  Uri,
  KeyMod,
  KeyCode,
  editor,
  languages,
};
