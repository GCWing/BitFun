/**
 * Monaco Model Manager (singleton).
 *
 * Global model pool manager ensuring one Model per URI, reference counting,
 * metadata tracking (dirty state, version), and unified lifecycle management.
 *
 * Design: Models are the data layer (globally unique, persistent).
 * Editors are the view layer (created/destroyed freely).
 * One Model can be used by multiple Editors.
 */

import * as monaco from 'monaco-editor';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MonacoModelManager');

export interface ModelMetadata {
  /** How many Editors are using this Model */
  referenceCount: number;
  isDirty: boolean;
  /** Version ID when saved (for dirty detection) */
  savedVersionId: number;
  originalContent: string;
  createdAt: number;
  lastAccessedAt: number;
  /** Normalized absolute file path */
  filePath: string;
  workspacePath?: string;
  languageId: string;
}

interface ModelLoadState {
  isLoading: boolean;
  promise?: Promise<void>;
  resolve?: () => void;
  reject?: (error: Error) => void;
}

export interface ModelCreatedEvent {
  uri: string;
  filePath: string;
  language: string;
  model: monaco.editor.ITextModel;
}

export interface ModelContentChangedEvent {
  uri: string;
  filePath: string;
  content: string;
  model: monaco.editor.ITextModel;
}

export interface ModelDisposedEvent {
  uri: string;
  filePath: string;
}

export interface ModelContentReadyEvent {
  uri: string;
  filePath: string;
  content: string;
  model: monaco.editor.ITextModel;
}

type EventListener<T> = (event: T) => void;

class MonacoModelManager {
  private static instance: MonacoModelManager;
  
  private modelMetadata = new Map<string, ModelMetadata>();
  private modelLoadStates = new Map<string, ModelLoadState>();
  private contentChangeListeners = new Map<string, monaco.IDisposable>();
  private disposalTimers = new Map<string, ReturnType<typeof setTimeout>>();
  /** Disposal delay (ms), 0 for immediate */
  private disposalDelay = 10000;
  
  private modelCreatedListeners: Array<EventListener<ModelCreatedEvent>> = [];
  private modelContentChangedListeners: Array<EventListener<ModelContentChangedEvent>> = [];
  private modelDisposedListeners: Array<EventListener<ModelDisposedEvent>> = [];
  private modelContentReadyListeners: Array<EventListener<ModelContentReadyEvent>> = [];
  
  private constructor() {
    this.setupGlobalListeners();
  }
  
  public static getInstance(): MonacoModelManager {
    if (!MonacoModelManager.instance) {
      MonacoModelManager.instance = new MonacoModelManager();
    }
    return MonacoModelManager.instance;
  }
  
  private setupGlobalListeners(): void {
    // Sync cleanup when Model is disposed externally
    monaco.editor.onWillDisposeModel((model) => {
      const uri = model.uri.toString();
      this.cleanupMetadata(uri);
    });
  }
  
  public getOrCreateModel(
    filePath: string,
    language: string,
    initialContent: string = '',
    workspacePath?: string
  ): monaco.editor.ITextModel {
    const uri = this.normalizeUri(filePath);
    const uriString = uri.toString();
    
    this.cancelDisposalTimer(uriString);
    
    let model = monaco.editor.getModel(uri);
    
    if (model) {
      const metadata = this.modelMetadata.get(uriString);
      if (metadata) {
        metadata.referenceCount++;
        metadata.lastAccessedAt = Date.now();
      } else {
        // Model exists but no metadata (externally created)
        this.createMetadata(uriString, filePath, language, initialContent, workspacePath, model);
      }
      
      const existingContent = model.getValue();
      if (existingContent) {
        this.emitModelContentReady({
          uri: uriString,
          filePath,
          content: existingContent,
          model
        });
      }
      
      if (initialContent && model.getValue() === '') {
        model.setValue(initialContent);
        
        const metadata = this.modelMetadata.get(uriString);
        if (metadata) {
          metadata.savedVersionId = model.getAlternativeVersionId();
          metadata.originalContent = initialContent;
          metadata.isDirty = false;
        }
      }
      
      return model;
    }
    
    model = monaco.editor.createModel(initialContent, language, uri);
    
    this.createMetadata(uriString, filePath, language, initialContent, workspacePath, model);
    this.setupContentChangeListener(uriString, model);
    
    this.emitModelCreated({
      uri: uriString,
      filePath,
      language,
      model
    });
    
    if (initialContent) {
      this.emitModelContentReady({
        uri: uriString,
        filePath,
        content: initialContent,
        model
      });
    }
    
    return model;
  }
  
  private createMetadata(
    uriString: string,
    filePath: string,
    languageId: string,
    originalContent: string,
    workspacePath: string | undefined,
    model: monaco.editor.ITextModel
  ): void {
    const now = Date.now();
    const metadata: ModelMetadata = {
      referenceCount: 1,
      isDirty: false,
      savedVersionId: model.getAlternativeVersionId(),
      originalContent,
      createdAt: now,
      lastAccessedAt: now,
      filePath,
      workspacePath,
      languageId
    };
    
    this.modelMetadata.set(uriString, metadata);
  }
  
  private setupContentChangeListener(
    uriString: string,
    model: monaco.editor.ITextModel
  ): void {
    const listener = model.onDidChangeContent(() => {
      const metadata = this.modelMetadata.get(uriString);
      if (metadata) {
        const currentVersionId = model.getAlternativeVersionId();
        metadata.isDirty = currentVersionId !== metadata.savedVersionId;
        
        window.dispatchEvent(new CustomEvent('monaco-model-dirty-changed', {
          detail: {
            uri: uriString,
            filePath: metadata.filePath,
            isDirty: metadata.isDirty
          }
        }));
        
        this.emitModelContentChanged({
          uri: uriString,
          filePath: metadata.filePath,
          content: model.getValue(),
          model
        });
      }
    });
    
    this.contentChangeListeners.set(uriString, listener);
  }
  
  public releaseModel(filePath: string, immediate: boolean = false): void {
    const uri = this.normalizeUri(filePath);
    const uriString = uri.toString();
    
    const metadata = this.modelMetadata.get(uriString);
    if (!metadata) {
      log.warn('Trying to release non-existent model', { filePath });
      return;
    }
    
    metadata.referenceCount = Math.max(0, metadata.referenceCount - 1);
    metadata.lastAccessedAt = Date.now();
    
    if (metadata.referenceCount === 0) {
      if (immediate || this.disposalDelay === 0) {
        this.disposeModel(uriString);
      } else {
        this.scheduleDisposal(uriString);
      }
    }
  }
  
  private scheduleDisposal(uriString: string): void {
    this.cancelDisposalTimer(uriString);
    
    const timer = setTimeout(() => {
      const metadata = this.modelMetadata.get(uriString);
      if (metadata && metadata.referenceCount === 0) {
        this.disposeModel(uriString);
      }
    }, this.disposalDelay);
    
    this.disposalTimers.set(uriString, timer);
  }
  
  private cancelDisposalTimer(uriString: string): void {
    const timer = this.disposalTimers.get(uriString);
    if (timer) {
      clearTimeout(timer);
      this.disposalTimers.delete(uriString);
    }
  }
  
  private disposeModel(uriString: string): void {
    const metadata = this.modelMetadata.get(uriString);
    const model = monaco.editor.getModel(monaco.Uri.parse(uriString));
    
    if (!model) {
      log.warn('Model already disposed', { uri: uriString });
      this.cleanupMetadata(uriString);
      return;
    }
    
    if (metadata) {
      this.emitModelDisposed({
        uri: uriString,
        filePath: metadata.filePath
      });
    }
    
    const listener = this.contentChangeListeners.get(uriString);
    if (listener) {
      listener.dispose();
      this.contentChangeListeners.delete(uriString);
    }
    
    this.cleanupMetadata(uriString);
    model.dispose();
  }
  
  private cleanupMetadata(uriString: string): void {
    this.modelMetadata.delete(uriString);
    this.modelLoadStates.delete(uriString);
    this.cancelDisposalTimer(uriString);
  }
  
  public updateModelContent(
    filePath: string,
    content: string,
    markAsSaved: boolean = false
  ): void {
    const uri = this.normalizeUri(filePath);
    const uriString = uri.toString();
    const model = monaco.editor.getModel(uri);
    
    if (!model) {
      log.warn('Cannot update non-existent model', { filePath });
      return;
    }
    
    const loadState = this.modelLoadStates.get(uriString);
    const isLoadingState = loadState && loadState.isLoading;
    
    const metadata = this.modelMetadata.get(uriString);
    const wasEmpty = !metadata || metadata.originalContent === '';
    const isFirstContentSet = wasEmpty && content.length > 0;
    
    model.setValue(content);
    
    if (metadata) {
      if (markAsSaved) {
        metadata.savedVersionId = model.getAlternativeVersionId();
        metadata.originalContent = content;
        metadata.isDirty = false;
      }
      metadata.lastAccessedAt = Date.now();
    }
    
    this.markLoadingComplete(uriString);
    
    if (isLoadingState || isFirstContentSet) {
      this.emitModelContentReady({
        uri: uriString,
        filePath,
        content,
        model
      });
    }
  }
  
  public markAsSaved(filePath: string): void {
    const uri = this.normalizeUri(filePath);
    const uriString = uri.toString();
    const model = monaco.editor.getModel(uri);
    const metadata = this.modelMetadata.get(uriString);
    
    if (model && metadata) {
      metadata.savedVersionId = model.getAlternativeVersionId();
      metadata.originalContent = model.getValue();
      metadata.isDirty = false;
      metadata.lastAccessedAt = Date.now();
      
      window.dispatchEvent(new CustomEvent('monaco-model-dirty-changed', {
        detail: {
          uri: uriString,
          filePath: metadata.filePath,
          isDirty: false
        }
      }));
    }
  }
  
  public getModelMetadata(filePath: string): ModelMetadata | undefined {
    const uri = this.normalizeUri(filePath);
    const uriString = uri.toString();
    return this.modelMetadata.get(uriString);
  }
  
  public getModel(filePath: string): monaco.editor.ITextModel | null {
    const uri = this.normalizeUri(filePath);
    return monaco.editor.getModel(uri);
  }
  
  public async waitForModelContent(filePath: string, timeout: number = 5000): Promise<void> {
    const uri = this.normalizeUri(filePath);
    const uriString = uri.toString();
    const model = monaco.editor.getModel(uri);
    
    if (!model) {
      throw new Error(`Model not found: ${filePath}`);
    }
    
    if (model.getLineCount() > 1 || model.getLineContent(1).length > 0) {
      return;
    }
    
    let loadState = this.modelLoadStates.get(uriString);
    if (!loadState) {
      loadState = { isLoading: true };
      loadState.promise = new Promise((resolve, reject) => {
        loadState!.resolve = resolve;
        loadState!.reject = reject;
      });
      this.modelLoadStates.set(uriString, loadState);
    }
    
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error(`Model content load timeout: ${filePath}`)), timeout);
    });
    
    try {
      await Promise.race([loadState.promise, timeoutPromise]);
    } catch (error) {
      log.error('Failed to wait for model content', { filePath, error });
      throw error;
    }
  }
  
  private markLoadingComplete(uriString: string): void {
    const loadState = this.modelLoadStates.get(uriString);
    if (loadState && loadState.resolve) {
      loadState.resolve();
      loadState.isLoading = false;
    }
  }
  
  public normalizeUri(filePath: string): monaco.Uri {
    try {
      if (filePath.includes('%')) {
        filePath = decodeURIComponent(filePath);
      }
    } catch (err) {
      log.warn('Failed to decode path', { filePath, error: err });
    }
    
    let normalizedPath = filePath.replace(/\\/g, '/');
    
    if (normalizedPath.match(/^[a-zA-Z]:/)) {
      normalizedPath = normalizedPath.charAt(0).toLowerCase() + normalizedPath.slice(1);
    }
    
    return monaco.Uri.file(normalizedPath);
  }
  
  public getStatistics(): {
    totalModels: number;
    activeModels: number;
    dirtyModels: number;
    totalReferences: number;
  } {
    let activeModels = 0;
    let dirtyModels = 0;
    let totalReferences = 0;
    
    this.modelMetadata.forEach((metadata) => {
      if (metadata.referenceCount > 0) {
        activeModels++;
      }
      if (metadata.isDirty) {
        dirtyModels++;
      }
      totalReferences += metadata.referenceCount;
    });
    
    return {
      totalModels: this.modelMetadata.size,
      activeModels,
      dirtyModels,
      totalReferences
    };
  }
  
  public setDisposalDelay(delay: number): void {
    this.disposalDelay = Math.max(0, delay);
  }
  
  public cleanupUnusedModels(): void {
    const toDispose: string[] = [];
    this.modelMetadata.forEach((metadata, uri) => {
      if (metadata.referenceCount === 0) {
        toDispose.push(uri);
      }
    });
    
    toDispose.forEach(uri => this.disposeModel(uri));
  }
  
  public onModelCreated(listener: EventListener<ModelCreatedEvent>): () => void {
    this.modelCreatedListeners.push(listener);
    return () => {
      const index = this.modelCreatedListeners.indexOf(listener);
      if (index > -1) {
        this.modelCreatedListeners.splice(index, 1);
      }
    };
  }
  
  public onModelContentChanged(listener: EventListener<ModelContentChangedEvent>): () => void {
    this.modelContentChangedListeners.push(listener);
    return () => {
      const index = this.modelContentChangedListeners.indexOf(listener);
      if (index > -1) {
        this.modelContentChangedListeners.splice(index, 1);
      }
    };
  }
  
  public onModelDisposed(listener: EventListener<ModelDisposedEvent>): () => void {
    this.modelDisposedListeners.push(listener);
    return () => {
      const index = this.modelDisposedListeners.indexOf(listener);
      if (index > -1) {
        this.modelDisposedListeners.splice(index, 1);
      }
    };
  }
  
  public onModelContentReady(listener: EventListener<ModelContentReadyEvent>): () => void {
    this.modelContentReadyListeners.push(listener);
    return () => {
      const index = this.modelContentReadyListeners.indexOf(listener);
      if (index > -1) {
        this.modelContentReadyListeners.splice(index, 1);
      }
    };
  }
  
  private emitModelCreated(event: ModelCreatedEvent): void {
    this.modelCreatedListeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Error in modelCreated listener', error);
      }
    });
  }
  
  private emitModelContentChanged(event: ModelContentChangedEvent): void {
    this.modelContentChangedListeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Error in modelContentChanged listener', error);
      }
    });
  }
  
  private emitModelDisposed(event: ModelDisposedEvent): void {
    this.modelDisposedListeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Error in modelDisposed listener', error);
      }
    });
  }
  
  private emitModelContentReady(event: ModelContentReadyEvent): void {
    this.modelContentReadyListeners.forEach(listener => {
      try {
        listener(event);
      } catch (error) {
        log.error('Error in modelContentReady listener', error);
      }
    });
  }
}

export const monacoModelManager = MonacoModelManager.getInstance();
export default MonacoModelManager;

export { monacoModelManager as monacoGlobalManager };
export { MonacoModelManager as MonacoGlobalManager };
