/**
 * Editor Type Definitions
 * 
 * Unified re-exports from config/types plus other editor-related types.
 * @module types
 */

export type {
  EditorConfig,
  EditorConfigPartial,
  EditorPresetName,
  EditorPresetConfig,
  MinimapConfig,
  GuidesConfig,
  ScrollbarConfig,
  HoverConfig,
  SuggestConfig,
  QuickSuggestionsConfig,
  InlayHintsConfig,
  DeepPartial,
  EditorConfigChangeEvent,
} from '../config/types';

/** File content representation */
export interface FileContent {
  name: string;
  content: string;
  language: string;
  encoding?: string;
  lineEnding?: 'lf' | 'crlf' | 'auto';
  isReadOnly?: boolean;
  isDirty?: boolean;
  lastModified?: Date;
  size?: number;
}

/** Editor state */
export interface EditorState {
  openFiles: FileContent[];
  activeFileIndex: number;
  isLoading: boolean;
  error: string | null;
  searchQuery: string;
  replaceQuery: string;
  searchResults: SearchResult[];
  currentSearchIndex: number;
}

/** Search result */
export interface SearchResult {
  fileIndex: number;
  line: number;
  column: number;
  length: number;
  match: string;
  context: string;
}

/** Editor action */
export interface EditorAction {
  type: 'insert' | 'delete' | 'replace' | 'format';
  position: Position;
  content?: string;
  range?: Range;
  timestamp: Date;
}

/** Position information */
export interface Position {
  line: number;
  column: number;
}

/** Range information */
export interface Range {
  start: Position;
  end: Position;
}

/** Editor event */
export type EditorEvent = 
  | { type: 'file:opened'; payload: FileContent }
  | { type: 'file:closed'; payload: { index: number } }
  | { type: 'file:saved'; payload: { index: number; content: string } }
  | { type: 'file:changed'; payload: { index: number; content: string } }
  | { type: 'selection:changed'; payload: { range: Range } }
  | { type: 'cursor:moved'; payload: { position: Position } }
  | { type: 'search:performed'; payload: { query: string; results: SearchResult[] } }
  | { type: 'config:changed'; payload: Record<string, unknown> };

/** Editor component props */
export interface EditorProps {
  content?: string;
  fileName?: string;
  language?: string;
  readOnly?: boolean;
  onContentChange?: (content: string) => void;
  onSave?: (content: string) => void;
  onSelectionChange?: (range: Range) => void;
  onCursorMove?: (position: Position) => void;
  className?: string;
  style?: React.CSSProperties;
}

/** Search options */
export interface SearchOptions {
  caseSensitive?: boolean;
  wholeWord?: boolean;
  regex?: boolean;
  fileIndex?: number;
}

/** Replace options */
export interface ReplaceOptions extends SearchOptions {
  replaceAll?: boolean;
}

/** Editor manager interface */
export interface IEditorManager {
  openFile(file: FileContent): Promise<number>;
  closeFile(index: number): Promise<void>;
  saveFile(index: number): Promise<void>;
  saveAllFiles(): Promise<void>;
  
  getActiveFile(): FileContent | null;
  getFile(index: number): FileContent | null;
  getAllFiles(): FileContent[];
  isFileDirty(index: number): boolean;
  
  search(query: string, options?: SearchOptions): SearchResult[];
  replace(query: string, replacement: string, options?: ReplaceOptions): number;
  
  addEventListener(listener: (event: EditorEvent) => void): () => void;
}

/** useEditor hook return type */
export interface UseEditorReturn {
  openFiles: FileContent[];
  activeFile: FileContent | null;
  activeFileIndex: number;
  isLoading: boolean;
  error: string | null;
  searchResults: SearchResult[];
  
  // Actions
  openFile: (file: FileContent) => Promise<number>;
  closeFile: (index: number) => Promise<void>;
  saveFile: (index?: number) => Promise<void>;
  switchToFile: (index: number) => void;
  updateFileContent: (index: number, content: string) => void;
  search: (query: string, options?: SearchOptions) => SearchResult[];
  replace: (query: string, replacement: string, options?: ReplaceOptions) => number;
  clearError: () => void;
}

/** Language detection result */
export interface LanguageDetectionResult {
  language: string;
  confidence: number;
  detected: boolean;
}

/** Editor theme definition */
export interface EditorTheme {
  id: string;
  name: string;
  type: 'light' | 'dark' | 'high-contrast';
  colors: {
    background: string;
    foreground: string;
    selection: string;
    lineHighlight: string;
    cursor: string;
    gutter: string;
    gutterForeground: string;
  };
  tokenColors: TokenColor[];
}

/** Syntax highlight token color */
export interface TokenColor {
  name: string;
  scope: string | string[];
  settings: {
    foreground?: string;
    background?: string;
    fontStyle?: 'italic' | 'bold' | 'underline';
  };
}

/** Completion item */
export interface CompletionItem {
  label: string;
  kind: CompletionItemKind;
  detail?: string;
  documentation?: string;
  insertText: string;
  range: Range;
}

/** Completion item kind */
export enum CompletionItemKind {
  Text = 1,
  Method = 2,
  Function = 3,
  Constructor = 4,
  Field = 5,
  Variable = 6,
  Class = 7,
  Interface = 8,
  Module = 9,
  Property = 10,
  Unit = 11,
  Value = 12,
  Enum = 13,
  Keyword = 14,
  Snippet = 15,
  Color = 16,
  File = 17,
  Reference = 18,
  Folder = 19,
  EnumMember = 20,
  Constant = 21,
  Struct = 22,
  Event = 23,
  Operator = 24,
  TypeParameter = 25
}

/** Diagnostic information (errors, warnings, etc.) */
export interface Diagnostic {
  range: Range;
  severity: DiagnosticSeverity;
  code?: string | number;
  source?: string;
  message: string;
  relatedInformation?: DiagnosticRelatedInformation[];
}

/** Diagnostic severity */
export enum DiagnosticSeverity {
  Error = 1,
  Warning = 2,
  Information = 3,
  Hint = 4
}

/** Diagnostic related information */
export interface DiagnosticRelatedInformation {
  location: {
    uri: string;
    range: Range;
  };
  message: string;
}

/** Markdown editor configuration */
export interface MarkdownEditorConfig {
  showPreview: boolean;
  previewPosition: 'right' | 'bottom';
  enableAutoSave: boolean;
  autoSaveDelay: number;
  theme: 'light' | 'dark' | 'nord';
  fontSize: number;
  lineHeight: number;
  enableSpellCheck: boolean;
  enableTableOfContents: boolean;
}

/** Markdown editor props */
export interface MarkdownEditorProps {
  content?: string;
  fileName?: string;
  filePath?: string;
  workspacePath?: string;
  readOnly?: boolean;
  config?: Partial<MarkdownEditorConfig>;
  onContentChange?: (content: string, hasChanges: boolean) => void;
  onSave?: (content: string) => void;
  className?: string;
  style?: React.CSSProperties;
}

/** Markdown render options */
export interface MarkdownRenderOptions {
  enableGFM: boolean;
  enableMath: boolean;
  enableDiagram: boolean;
  enableCodeHighlight: boolean;
  codeTheme: string;
}

/** Markdown document metadata */
export interface MarkdownMetadata {
  title?: string;
  author?: string;
  date?: Date;
  tags?: string[];
  description?: string;
  wordCount?: number;
  readingTime?: number;
}
