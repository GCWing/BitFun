/**
 * Editor Extension Type Definitions
 * 
 * Defines extension interfaces for AI completion, Tab completion, etc.
 * @module extensions/types
 */

import type * as monaco from 'monaco-editor';

/** Extension interface (re-exported from EditorExtensionManager) */
export type { EditorExtension, EditorExtensionContext } from '../services/EditorExtensionManager';

/** Extension priority constants */
export const ExtensionPriority = {
  /** Highest priority - core functionality */
  CRITICAL: 0,
  /** High priority - language services like LSP */
  HIGH: 10,
  /** Medium priority - AI completion and enhancements */
  MEDIUM: 50,
  /** Low priority - decorative features */
  LOW: 100,
  /** Lowest priority - optional features */
  OPTIONAL: 200,
} as const;

/** LSP extension configuration */
export interface LspExtensionConfig {
  /** Whether enabled */
  enabled: boolean;
  /** Workspace path */
  workspacePath?: string;
  /** Languages to skip (use Monaco built-in services) */
  skipLanguages?: string[];
}

/** AI completion extension configuration (reserved) */
export interface AiCompletionExtensionConfig {
  /** Whether enabled */
  enabled: boolean;
  /** Trigger characters */
  triggerCharacters?: string[];
  /** Debounce delay (milliseconds) */
  debounceMs?: number;
}

/** Tab completion extension configuration (reserved) */
export interface TabCompletionExtensionConfig {
  /** Whether enabled */
  enabled: boolean;
  /** Accept key */
  acceptKey?: 'Tab' | 'Enter';
}

/** Extension state */
export interface ExtensionState {
  /** Whether ready */
  isReady: boolean;
  /** Whether active */
  isActive: boolean;
  /** Error message */
  error?: string;
}

/** Extension lifecycle event */
export interface ExtensionLifecycleEvent {
  type: 'registered' | 'unregistered' | 'activated' | 'deactivated' | 'error';
  extensionId: string;
  timestamp: number;
  error?: Error;
}

/** Editor decorations extension interface */
export interface DecorationsExtension {
  /** Add decorations */
  addDecorations(
    editor: monaco.editor.IStandaloneCodeEditor,
    decorations: monaco.editor.IModelDeltaDecoration[]
  ): string[];
  
  /** Remove decorations */
  removeDecorations(
    editor: monaco.editor.IStandaloneCodeEditor,
    decorationIds: string[]
  ): void;
  
  /** Clear all decorations */
  clearDecorations(editor: monaco.editor.IStandaloneCodeEditor): void;
}

/** Editor commands extension interface */
export interface CommandsExtension {
  /** Register command */
  registerCommand(
    editor: monaco.editor.IStandaloneCodeEditor,
    commandId: string,
    handler: () => void,
    keybinding?: number
  ): monaco.IDisposable;
  
  /** Execute command */
  executeCommand(commandId: string, ...args: unknown[]): void;
}
