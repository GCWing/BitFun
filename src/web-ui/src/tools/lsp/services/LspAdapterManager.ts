/**
 * LSP Adapter Manager
 * 
 * Manages `MonacoLspAdapter` instances with a 1:1 binding to Monaco models.
 *
 * - One adapter per model (long-lived).
 * - An adapter can serve multiple editors.
 * - Adapters are disposed when the model is disposed.
 */

import * as monaco from 'monaco-editor';
import { MonacoLspAdapter } from './MonacoLspAdapter';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('LspAdapterManager');

interface AdapterMetadata {
  adapter: MonacoLspAdapter;
  model: monaco.editor.ITextModel;
  language: string;
  workspacePath: string;
  createdAt: number;
  editorCount: number;
}

class LspAdapterManager {
  private static instance: LspAdapterManager;
  
  /** Adapter metadata keyed by model URI. */
  private adapters = new Map<string, AdapterMetadata>();
  
  /** Model dispose listeners keyed by model URI. */
  private modelDisposeListeners = new Map<string, monaco.IDisposable>();
  
  private constructor() {}
  
  public static getInstance(): LspAdapterManager {
    if (!LspAdapterManager.instance) {
      LspAdapterManager.instance = new LspAdapterManager();
    }
    return LspAdapterManager.instance;
  }
  
  /** Get or create an adapter bound to a model. */
  public getOrCreateAdapter(
    model: monaco.editor.ITextModel,
    language: string,
    filePath: string,
    workspacePath: string
  ): MonacoLspAdapter {
    const uri = model.uri.toString();
    
    const existing = this.adapters.get(uri);
    if (existing) {
      return existing.adapter;
    }
    
    const adapter = new MonacoLspAdapter(
      model,
      language,
      filePath,
      workspacePath
    );
    
    const metadata: AdapterMetadata = {
      adapter,
      model,
      language,
      workspacePath,
      createdAt: Date.now(),
      editorCount: 0
    };
    this.adapters.set(uri, metadata);
    
    const disposeListener = model.onWillDispose(() => {
      this.disposeAdapter(uri);
    });
    this.modelDisposeListeners.set(uri, disposeListener);
    
    return adapter;
  }
  
  /** Get an existing adapter (no creation). */
  public getAdapter(uri: string): MonacoLspAdapter | undefined {
    return this.adapters.get(uri)?.adapter;
  }
  
  /** Register an editor to an adapter. */
  public registerEditor(
    model: monaco.editor.ITextModel,
    editor: monaco.editor.IStandaloneCodeEditor
  ): void {
    const uri = model.uri.toString();
    const metadata = this.adapters.get(uri);
    
    if (!metadata) {
      log.warn('Cannot register editor: adapter not found', { uri });
      return;
    }
    
    metadata.adapter.registerEditor(editor);
    metadata.editorCount++;
  }
  
  /** Unregister an editor from an adapter. */
  public unregisterEditor(
    model: monaco.editor.ITextModel,
    editor: monaco.editor.IStandaloneCodeEditor
  ): void {
    const uri = model.uri.toString();
    const metadata = this.adapters.get(uri);
    
    if (!metadata) {
      log.warn('Cannot unregister editor: adapter not found', { uri });
      return;
    }
    
    metadata.adapter.unregisterEditor(editor);
    metadata.editorCount = Math.max(0, metadata.editorCount - 1);
  }
  
  private disposeAdapter(uri: string): void {
    const metadata = this.adapters.get(uri);
    if (!metadata) {
      return;
    }
    
    metadata.adapter.dispose();
    
    const disposeListener = this.modelDisposeListeners.get(uri);
    if (disposeListener) {
      disposeListener.dispose();
      this.modelDisposeListeners.delete(uri);
    }
    
    this.adapters.delete(uri);
  }
  
  /** Get statistics. */
  public getStatistics(): {
    totalAdapters: number;
    activeAdapters: number;
    totalEditors: number;
    adapters: Array<{
      uri: string;
      language: string;
      editorCount: number;
      age: number;
    }>;
  } {
    let activeAdapters = 0;
    let totalEditors = 0;
    const adapters: any[] = [];
    
    this.adapters.forEach((metadata, uri) => {
      if (metadata.editorCount > 0) {
        activeAdapters++;
      }
      totalEditors += metadata.editorCount;
      
      adapters.push({
        uri,
        language: metadata.language,
        editorCount: metadata.editorCount,
        age: Date.now() - metadata.createdAt
      });
    });
    
    return {
      totalAdapters: this.adapters.size,
      activeAdapters,
      totalEditors,
      adapters
    };
  }
  
  public disposeAll(): void {
    const uris = Array.from(this.adapters.keys());
    uris.forEach(uri => this.disposeAdapter(uri));
  }
}

// Singleton export
export const lspAdapterManager = LspAdapterManager.getInstance();
export default LspAdapterManager;

