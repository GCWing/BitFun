/**
 * LSP type definitions shared by services/components.
 */

export interface LspPlugin {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  server: ServerConfig;
  languages: string[];
  file_extensions: string[];
  capabilities: CapabilitiesConfig;
  settings: Record<string, any>;
  checksum: string;
  min_bitfun_version: string;
}

export interface ServerConfig {
  command: string;
  args: string[];
  env: Record<string, string>;
}

export interface CapabilitiesConfig {
  completion: boolean;
  hover: boolean;
  definition: boolean;
  references: boolean;
  rename: boolean;
  formatting: boolean;
  diagnostics: boolean;
  inlayHints?: boolean;
}

export interface CompletionItem {
  label: string;
  kind?: number;
  detail?: string;
  documentation?: any;
  sortText?: string;
  filterText?: string;
  insertText?: string;
  insertTextFormat?: number;
}

export interface Position {
  line: number;
  character: number;
}

export interface Range {
  start: Position;
  end: Position;
}

export interface Diagnostic {
  range: Range;
  severity?: number;
  code?: any;
  source?: string;
  message: string;
}

export interface HoverInfo {
  contents: any;
  range?: Range;
}

export interface Location {
  uri: string;
  range: Range;
}

export interface TextEdit {
  range: Range;
  newText: string;
}

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
}

export enum DiagnosticSeverity {
  Error = 1,
  Warning = 2,
  Information = 3,
  Hint = 4,
}

export enum InlayHintKind {
  Type = 1,
  Parameter = 2,
}

export interface InlayHintLabelPart {
  value: string;
  tooltip?: any;
  location?: Location;
}

export type InlayHintLabel = string | InlayHintLabelPart[];

export interface InlayHint {
  position: Position;
  label: InlayHintLabel;
  kind?: InlayHintKind;
  tooltip?: any;
  paddingLeft?: boolean;
  paddingRight?: boolean;
  textEdits?: TextEdit[];
}

