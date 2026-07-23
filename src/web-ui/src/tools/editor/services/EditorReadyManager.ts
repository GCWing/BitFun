/**
 * Editor ready manager.
 * Solves editor jump delay issues using Promise-based mechanism
 * to ensure editor is fully ready before navigating.
 *
 * Uses requestAnimationFrame for performance, with timeout protection
 * and fallback mechanisms.
 */

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('EditorReadyManager');

interface EditorReadyEntry {
  promise: Promise<any>;
  resolve: (editor: any) => void;
  reject: (error: Error) => void;
}

class EditorReadyManager {
  private readyPromises = new Map<string, EditorReadyEntry>();
  private timeouts = new Map<string, NodeJS.Timeout>();

  private normalizePath(filePath: string): string {
    let normalized = filePath.replace(/\\/g, '/');
    normalized = normalized.replace(/\/+/g, '/');
    return normalized;
  }

  registerEditor(filePath: string): Promise<any> {
    const normalizedPath = this.normalizePath(filePath);
    
    if (this.readyPromises.has(normalizedPath)) {
      return this.readyPromises.get(normalizedPath)!.promise;
    }

    let resolveFunc: (editor: any) => void;
    let rejectFunc: (error: Error) => void;
    
    const promise = new Promise<any>((resolve, reject) => {
      resolveFunc = resolve;
      rejectFunc = reject;
    });

    this.readyPromises.set(normalizedPath, {
      promise,
      resolve: resolveFunc!,
      reject: rejectFunc!
    });

    return promise;
  }

  markEditorReady(filePath: string, editor: any): void {
    const normalizedPath = this.normalizePath(filePath);
    const entry = this.readyPromises.get(normalizedPath);
    if (entry) {
      entry.resolve(editor);
      
      const timeout = this.timeouts.get(normalizedPath);
      if (timeout) {
        clearTimeout(timeout);
        this.timeouts.delete(normalizedPath);
      }
    } else {
      log.warn('Editor ready but not registered', { path: normalizedPath, registeredPaths: Array.from(this.readyPromises.keys()) });
    }
  }

  cleanup(filePath: string): void {
    const normalizedPath = this.normalizePath(filePath);
    this.readyPromises.delete(normalizedPath);
    
    const timeout = this.timeouts.get(normalizedPath);
    if (timeout) {
      clearTimeout(timeout);
      this.timeouts.delete(normalizedPath);
    }
  }

  async waitAndJump(
    filePath: string, 
    lineNumber: number,
    timeout = 3000
  ): Promise<void> {
    const normalizedPath = this.normalizePath(filePath);

    try {
      // Check if editor already exists (switching to existing tab)
      const existingEditor = await this.getExistingEditor(normalizedPath);
      
      if (existingEditor) {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            this.performJump(existingEditor, lineNumber, normalizedPath);
          });
        });
        return;
      }

      const timeoutPromise = new Promise<never>((_, reject) => {
        const timer = setTimeout(() => {
          reject(new Error(`Editor ready timeout: ${normalizedPath}`));
        }, timeout);
        this.timeouts.set(normalizedPath, timer);
      });

      const editor = await Promise.race([
        this.registerEditor(normalizedPath),
        timeoutPromise
      ]);

      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          this.performJump(editor, lineNumber, normalizedPath);
        });
      });
    } catch (error) {
      log.warn('Jump failed, using fallback', { path: normalizedPath, lineNumber, error });
      
      // Fallback: use a fixed delay plus an event.
      this.fallbackJump(normalizedPath, lineNumber);
    }
  }

  private async getExistingEditor(filePath: string): Promise<any> {
    try {
      const { GlobalAdapterRegistry } = await import('@/tools/lsp/services/MonacoLspAdapter');
      const { pathToUri } = await import('@/shared/utils/pathUtils');
      const uri = pathToUri(filePath);
      
      const adapter = GlobalAdapterRegistry.get(uri);
      
      if (adapter) {
        const editor = adapter.getActiveEditor();
        if (editor) {
          return editor;
        }
      }
      
      return null;
    } catch (error) {
      log.warn('Failed to find existing editor', { filePath, error });
      return null;
    }
  }

  private async performJump(editor: any, lineNumber: number, filePath: string): Promise<void> {
    if (!editor) {
      log.warn('Editor not available for jump', { filePath, lineNumber });
      return;
    }

    try {
      const model = editor.getModel();
      if (!model) {
        log.warn('Model not available for jump', { filePath, lineNumber });
        return;
      }

      // Wait for content to load (max 2s)
      const maxRetries = 20;
      let retries = 0;
      
      while (retries < maxRetries) {
        const lineCount = model.getLineCount();
        
        if (lineCount >= lineNumber) {
          break;
        }
        
        if (retries > 5 && lineCount > 0) {
          lineNumber = Math.min(lineNumber, lineCount);
          break;
        }
        
        await new Promise(resolve => setTimeout(resolve, 100));
        retries++;
      }
      
      const lineCount = model.getLineCount();
      if (lineNumber > lineCount) {
        log.warn('Target line exceeds file length, jumping to last line', { targetLine: lineNumber, actualLines: lineCount, filePath });
        lineNumber = lineCount;
      }
      
      editor.revealLineInCenter(lineNumber);
      editor.setPosition({ lineNumber, column: 1 });
      
      try {
        const maxColumn = model.getLineMaxColumn(lineNumber);
        editor.setSelection({
          startLineNumber: lineNumber,
          startColumn: 1,
          endLineNumber: lineNumber,
          endColumn: maxColumn
        });
      } catch (err) {
        log.warn('Failed to select line, using basic jump', { filePath, lineNumber, error: err });
      }
      
      editor.focus();
    } catch (error) {
      log.error('Failed to perform jump', { filePath, lineNumber, error });
    }
  }

  private fallbackJump(filePath: string, lineNumber: number): void {
    setTimeout(() => {
      window.dispatchEvent(new CustomEvent('editor-goto-line-fallback', {
        detail: { filePath, lineNumber }
      }));
    }, 300);
  }

  tryImmediateJump(filePath: string, lineNumber: number): boolean {
    const normalizedPath = this.normalizePath(filePath);
    const entry = this.readyPromises.get(normalizedPath);
    
    if (entry) {
      entry.promise.then(editor => {
        if (editor && !editor.isDisposed?.()) {
          requestAnimationFrame(() => {
            this.performJump(editor, lineNumber, normalizedPath);
          });
          return true;
        }
        return false;
      }).catch(() => false);
    }
    
    return false;
  }
}

export const editorReadyManager = new EditorReadyManager();

