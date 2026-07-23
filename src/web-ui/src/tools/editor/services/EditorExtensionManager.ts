/**
 * Editor extension manager (singleton).
 * Handles extension registration, lifecycle hook dispatch,
 * and provides extension points for AI completion, Tab completion, etc.
 */

import type * as monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('EditorExtensionManager');
/**
 * Extension interface for pluggable editor features.
 * AI completion, Tab completion, etc. can implement this interface.
 */
export interface EditorExtension {
  id: string;
  name: string;
  /** Lower number = higher priority */
  priority: number;
  
  /**
   * Called when editor is created. Returns optional cleanup function.
   */
  onEditorCreated?(
    editor: monaco.editor.IStandaloneCodeEditor,
    model: monaco.editor.ITextModel,
    context: EditorExtensionContext
  ): void | monaco.IDisposable | (() => void);
  
  onEditorWillDispose?(
    editor: monaco.editor.IStandaloneCodeEditor,
    model: monaco.editor.ITextModel,
    context: EditorExtensionContext
  ): void;
  
  onModelChanged?(
    editor: monaco.editor.IStandaloneCodeEditor,
    oldModel: monaco.editor.ITextModel | null,
    newModel: monaco.editor.ITextModel | null,
    context: EditorExtensionContext
  ): void;
  
  /** Called on content change (optional, high frequency). */
  onContentChanged?(
    editor: monaco.editor.IStandaloneCodeEditor,
    model: monaco.editor.ITextModel,
    event: monaco.editor.IModelContentChangedEvent,
    context: EditorExtensionContext
  ): void;
}

export interface EditorExtensionContext {
  filePath: string;
  language: string;
  workspacePath?: string;
  readOnly: boolean;
  enableLsp: boolean;
}

interface ExtensionRegistration {
  extension: EditorExtension;
  disposables: Map<string, monaco.IDisposable | (() => void)>;
}

class EditorExtensionManager {
  private static instance: EditorExtensionManager;
  
  private extensions = new Map<string, ExtensionRegistration>();
  private editorIdCounter = 0;
  
  private constructor() {}
  
  public static getInstance(): EditorExtensionManager {
    if (!EditorExtensionManager.instance) {
      EditorExtensionManager.instance = new EditorExtensionManager();
    }
    return EditorExtensionManager.instance;
  }
  
  public register(extension: EditorExtension): () => void {
    if (this.extensions.has(extension.id)) {
      log.warn('Extension already registered, replacing', { extensionId: extension.id });
      this.unregister(extension.id);
    }
    
    this.extensions.set(extension.id, {
      extension,
      disposables: new Map(),
    });
    
    log.debug('Extension registered', { extensionId: extension.id, priority: extension.priority });
    
    return () => this.unregister(extension.id);
  }
  
  public unregister(extensionId: string): void {
    const registration = this.extensions.get(extensionId);
    if (!registration) {
      return;
    }
    
    // Dispose all disposables
    registration.disposables.forEach((disposable) => {
      try {
        if (typeof disposable === 'function') {
          disposable();
        } else {
          disposable.dispose();
        }
      } catch (error) {
        log.error('Error disposing extension', { extensionId, error });
      }
    });
    
    this.extensions.delete(extensionId);
  }
  
  public notifyEditorCreated(
    editor: monaco.editor.IStandaloneCodeEditor,
    model: monaco.editor.ITextModel,
    context: EditorExtensionContext
  ): string {
    const editorId = `editor-${++this.editorIdCounter}`;
    
    const sortedExtensions = this.getSortedExtensions();
    
    for (const registration of sortedExtensions) {
      const { extension } = registration;
      
      if (!extension.onEditorCreated) {
        continue;
      }
      
      try {
        const result = extension.onEditorCreated(editor, model, context);
        
        if (result) {
          registration.disposables.set(editorId, result);
        }
      } catch (error) {
        log.error('Error in extension onEditorCreated', { extensionId: extension.id, error });
      }
    }
    
    return editorId;
  }
  
  public notifyEditorWillDispose(
    editorId: string,
    editor: monaco.editor.IStandaloneCodeEditor,
    model: monaco.editor.ITextModel,
    context: EditorExtensionContext
  ): void {
    const sortedExtensions = this.getSortedExtensions();
    
    for (const registration of sortedExtensions) {
      const { extension } = registration;
      
      if (extension.onEditorWillDispose) {
        try {
          extension.onEditorWillDispose(editor, model, context);
        } catch (error) {
          log.error('Error in extension onEditorWillDispose', { extensionId: extension.id, error });
        }
      }
      
      const disposable = registration.disposables.get(editorId);
      if (disposable) {
        try {
          if (typeof disposable === 'function') {
            disposable();
          } else {
            disposable.dispose();
          }
        } catch (error) {
          log.error('Error disposing extension', { extensionId: extension.id, error });
        }
        registration.disposables.delete(editorId);
      }
    }
  }
  
  public notifyModelChanged(
    editor: monaco.editor.IStandaloneCodeEditor,
    oldModel: monaco.editor.ITextModel | null,
    newModel: monaco.editor.ITextModel | null,
    context: EditorExtensionContext
  ): void {
    const sortedExtensions = this.getSortedExtensions();
    
    for (const registration of sortedExtensions) {
      const { extension } = registration;
      
      if (!extension.onModelChanged) {
        continue;
      }
      
      try {
        extension.onModelChanged(editor, oldModel, newModel, context);
      } catch (error) {
        log.error('Error in extension onModelChanged', { extensionId: extension.id, error });
      }
    }
  }
  
  public notifyContentChanged(
    editor: monaco.editor.IStandaloneCodeEditor,
    model: monaco.editor.ITextModel,
    event: monaco.editor.IModelContentChangedEvent,
    context: EditorExtensionContext
  ): void {
    const sortedExtensions = this.getSortedExtensions();
    
    for (const registration of sortedExtensions) {
      const { extension } = registration;
      
      if (!extension.onContentChanged) {
        continue;
      }
      
      try {
        extension.onContentChanged(editor, model, event, context);
      } catch (error) {
        log.error('Error in extension onContentChanged', { extensionId: extension.id, error });
      }
    }
  }
  
  public getExtensions(): EditorExtension[] {
    return Array.from(this.extensions.values()).map((r) => r.extension);
  }
  
  public hasExtension(extensionId: string): boolean {
    return this.extensions.has(extensionId);
  }
  
  private getSortedExtensions(): ExtensionRegistration[] {
    return Array.from(this.extensions.values()).sort(
      (a, b) => a.extension.priority - b.extension.priority
    );
  }
}

export const editorExtensionManager = EditorExtensionManager.getInstance();
export default EditorExtensionManager;
