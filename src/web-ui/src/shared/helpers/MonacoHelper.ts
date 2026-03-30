 

import * as monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MonacoHelper');

export interface EditorSelection {
   
  hasSelection: boolean;
   
  selectedText?: string;
   
  range?: {
    startLine: number;
    startColumn: number;
    endLine: number;
    endColumn: number;
  };
}

export interface EditorPosition {
  line: number;
  column: number;
}

export interface EditorFileInfo {
   
  uri: string;
   
  filePath: string;
   
  relativePath?: string;
   
  language: string;
}

export interface EditorContextInfo {
   
  editor: monaco.editor.IStandaloneCodeEditor;
   
  fileInfo: EditorFileInfo;
   
  selection: EditorSelection;
   
  cursorPosition: EditorPosition;
   
  wordAtCursor?: string;
}

export class MonacoHelper {
   
  static getEditorFromElement(element: HTMLElement): monaco.editor.IStandaloneCodeEditor | null {
    try {
      
      let current: HTMLElement | null = element;
      while (current) {
        if (current.classList.contains('monaco-editor')) {
          
          const editor = (current as any)['__monaco_editor__'];
          if (editor) {
            return editor as monaco.editor.IStandaloneCodeEditor;
          }
          break;
        }
        current = current.parentElement;
      }

      
      const allEditors = monaco.editor.getEditors();

      for (const editor of allEditors) {
        const domNode = editor.getDomNode();
        if (domNode && (domNode === element || domNode.contains(element))) {
          return editor as monaco.editor.IStandaloneCodeEditor;
        }
      }

      return null;
    } catch (error) {
      log.error('Failed to get editor from element', error as Error);
      return null;
    }
  }

   
  static getSelection(editor: monaco.editor.IStandaloneCodeEditor): EditorSelection {
    const selection = editor.getSelection();

    if (!selection || selection.isEmpty()) {
      return {
        hasSelection: false
      };
    }

    const model = editor.getModel();
    if (!model) {
      return {
        hasSelection: false
      };
    }

    const selectedText = model.getValueInRange(selection);

    return {
      hasSelection: true,
      selectedText,
      range: {
        startLine: selection.startLineNumber,
        startColumn: selection.startColumn,
        endLine: selection.endLineNumber,
        endColumn: selection.endColumn
      }
    };
  }

   
  static getCursorPosition(editor: monaco.editor.IStandaloneCodeEditor): EditorPosition | null {
    const position = editor.getPosition();
    if (!position) {
      return null;
    }

    return {
      line: position.lineNumber,
      column: position.column
    };
  }

   
  static getWordAtCursor(editor: monaco.editor.IStandaloneCodeEditor): string | undefined {
    const model = editor.getModel();
    const position = editor.getPosition();

    if (!model || !position) {
      return undefined;
    }

    const word = model.getWordAtPosition(position);
    return word?.word;
  }

   
  static getFileInfo(editor: monaco.editor.IStandaloneCodeEditor): EditorFileInfo | null {
    const model = editor.getModel();
    if (!model) {
      return null;
    }

    const uri = model.uri.toString();

    
    // file:///d:/path/to/file.ts -> d:/path/to/file.ts
    let filePath = uri;
    if (uri.startsWith('file:///')) {
      filePath = uri.substring(8); 
    } else if (uri.startsWith('file://')) {
      filePath = uri.substring(7); 
    }

    
    try {
      filePath = decodeURIComponent(filePath);
    } catch (_error) {
      log.debug('Failed to decode URI', { filePath });
    }

    
    const relativePath = filePath.split('/').pop() || filePath;

    return {
      uri,
      filePath,
      relativePath,
      language: model.getLanguageId()
    };
  }

   
  static getContextInfo(editor: monaco.editor.IStandaloneCodeEditor): EditorContextInfo | null {
    const fileInfo = this.getFileInfo(editor);
    if (!fileInfo) {
      return null;
    }

    const selection = this.getSelection(editor);
    const cursorPosition = this.getCursorPosition(editor);

    if (!cursorPosition) {
      return null;
    }

    const wordAtCursor = this.getWordAtCursor(editor);

    return {
      editor,
      fileInfo,
      selection,
      cursorPosition,
      wordAtCursor
    };
  }

   
  static isInMonacoEditor(element: HTMLElement): boolean {
    let current: HTMLElement | null = element;
    while (current) {
      if (current.classList.contains('monaco-editor')) {
        return true;
      }
      current = current.parentElement;
    }
    return false;
  }

   
  static getVisibleRange(editor: monaco.editor.IStandaloneCodeEditor): monaco.Range | null {
    const visibleRanges = editor.getVisibleRanges();
    return visibleRanges.length > 0 ? visibleRanges[0] : null;
  }

   
  static getLineContent(editor: monaco.editor.IStandaloneCodeEditor, lineNumber: number): string | null {
    const model = editor.getModel();
    if (!model) {
      return null;
    }

    try {
      return model.getLineContent(lineNumber);
    } catch (_error) {
      log.debug('Failed to get line content', { lineNumber });
      return null;
    }
  }

   
  static getContextCode(
    editor: monaco.editor.IStandaloneCodeEditor,
    startLine: number,
    endLine: number,
    contextLines: number = 3
  ): string | null {
    const model = editor.getModel();
    if (!model) {
      return null;
    }

    const totalLines = model.getLineCount();
    const contextStart = Math.max(1, startLine - contextLines);
    const contextEnd = Math.min(totalLines, endLine + contextLines);

    try {
      return model.getValueInRange({
        startLineNumber: contextStart,
        startColumn: 1,
        endLineNumber: contextEnd,
        endColumn: model.getLineMaxColumn(contextEnd)
      });
    } catch (_error) {
      log.debug('Failed to get context code');
      return null;
    }
  }
}
