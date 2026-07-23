/**
 * Editor jump service.
 *
 * Keeps track of editor instances and supports jumping to line/range locations.
 */
import type { LineRange } from '@/component-library/components/Markdown';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('EditorJumpService');

interface EditorInstance {
  editor: any; // Monaco editor instance
  filePath: string;
}

class EditorJumpService {
  private static instance: EditorJumpService;
  
  
  private editors = new Map<string, EditorInstance>();

  private constructor() {
  }

  static getInstance(): EditorJumpService {
    if (!EditorJumpService.instance) {
      EditorJumpService.instance = new EditorJumpService();
    }
    return EditorJumpService.instance;
  }

   
  private normalizePath(path: string): string {
    let normalized = path.replace(/\\/g, '/');
    
    
    normalized = normalized.replace(/^file:\/\/\//i, '');
    
    
    if (normalized.match(/^[A-Z]:/)) {
      normalized = normalized.charAt(0).toLowerCase() + normalized.slice(1);
    }
    
    return normalized;
  }

   
  registerEditor(filePath: string, editor: any): void {
    const normalized = this.normalizePath(filePath);
    this.editors.set(normalized, { editor, filePath });
  }

   
  unregisterEditor(filePath: string): void {
    const normalized = this.normalizePath(filePath);
    this.editors.delete(normalized);
  }

   
  private getEditor(filePath: string): any | null {
    const normalized = this.normalizePath(filePath);
    return this.editors.get(normalized)?.editor || null;
  }

   
  isFileOpen(filePath: string): boolean {
    const normalized = this.normalizePath(filePath);
    return this.editors.has(normalized);
  }

   
  jumpTo(filePath: string, line: number, column?: number, endLine?: number): boolean {
    const editor = this.getEditor(filePath);
    
    if (!editor) {
      log.warn('Editor not found', { filePath });
      return false;
    }

    const model = editor.getModel();
    if (!model) {
      log.warn('Model not found');
      return false;
    }

    
    const lineCount = model.getLineCount();
    const targetLine = Math.min(line, Math.max(1, lineCount));
    const targetEndLine = endLine ? Math.min(endLine, Math.max(1, lineCount)) : undefined;
    const targetColumn = column || 1;

    try {
      
      requestAnimationFrame(() => {
        
        editor.setPosition({
          lineNumber: targetLine,
          column: targetColumn
        });

        
        if (targetEndLine && targetEndLine > targetLine) {
          
          const endLineMaxColumn = model.getLineMaxColumn(targetEndLine);
          editor.setSelection({
            startLineNumber: targetLine,
            startColumn: 1,
            endLineNumber: targetEndLine,
            endColumn: endLineMaxColumn
          });
          
          
          editor.revealRangeInCenter({
            startLineNumber: targetLine,
            startColumn: 1,
            endLineNumber: targetEndLine,
            endColumn: endLineMaxColumn
          });
        } else {
          
          editor.revealLineInCenter(targetLine);
          
          try {
            const maxColumn = model.getLineMaxColumn(targetLine);
            editor.setSelection({
              startLineNumber: targetLine,
              startColumn: 1,
              endLineNumber: targetLine,
              endColumn: maxColumn
            });
          } catch (err) {
            log.warn('Failed to select line', err);
          }
        }

        
        editor.focus();
      });

      return true;
    } catch (error) {
      log.error('Jump failed', error);
      return false;
    }
  }

   
  jumpToRange(filePath: string, range: LineRange): boolean {
    return this.jumpTo(filePath, range.start, 1, range.end);
  }

   
  async jumpToFile(filePath: string, line: number, column?: number): Promise<void> {
    const normalized = this.normalizePath(filePath);
    
    
    if (this.isFileOpen(normalized)) {
      
      this.jumpTo(normalized, line, column);
    } else {
      
      
      const { fileTabManager } = await import('./FileTabManager');
      fileTabManager.openFileAndJump(filePath, line, column);
    }
  }

   
  async jumpToFileWithRange(filePath: string, range: LineRange): Promise<void> {
    const normalized = this.normalizePath(filePath);
    
    
    if (this.isFileOpen(normalized)) {
      
      this.jumpToRange(normalized, range);
    } else {
      
      const { fileTabManager } = await import('./FileTabManager');
      fileTabManager.openFileAndJumpToRange(filePath, range);
    }
  }

   
  getOpenFiles(): string[] {
    return Array.from(this.editors.keys());
  }
}


export const editorJumpService = EditorJumpService.getInstance();
