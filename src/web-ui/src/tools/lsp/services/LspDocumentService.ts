/**
 * Connects Monaco model lifecycle to LSP document sync.
 *
 * Sends didOpen/didChange/didClose and triggers lightweight UI refreshes
 * (e.g. semantic tokens, inlay hints) after edits.
 */

import { monacoModelManager, type ModelCreatedEvent, type ModelContentChangedEvent, type ModelContentReadyEvent, type ModelDisposedEvent } from '@/tools/editor/services/MonacoModelManager';
import { WorkspaceLspManager } from './WorkspaceLspManager';
import { lspRefreshManager } from './LspRefreshManager';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('LspDocumentService');

interface DocumentSyncState {
  /** Document URI. */
  uri: string;
  /** File path. */
  filePath: string;
  /** Language ID. */
  language: string;
  /** Workspace root path. */
  workspacePath?: string;
  /** Whether didOpen has been sent. */
  isOpen: boolean;
  /** Last synced content. */
  lastSyncedContent: string;
  /** Debounce timer for content changes. */
  changeTimer?: NodeJS.Timeout;
  /** Associated editor instance (used to trigger refresh actions). */
  editor?: any; // monaco.editor.IStandaloneCodeEditor
}

class LspDocumentService {
  private static instance: LspDocumentService;
  
  /** Per-document sync state. */
  private documentStates = new Map<string, DocumentSyncState>();
  
  /** Debounce delay for content changes (ms). */
  private readonly CHANGE_DEBOUNCE = 300;
  
  /** Batch sync timers by URI. */
  private syncTimers = new Map<string, NodeJS.Timeout>();
  /** Batch window (ms). */
  private readonly SYNC_DELAY = 100;
  
  /** Unsubscribe callbacks. */
  private unsubscribeFunctions: Array<() => void> = [];
  
  private constructor() {
    this.setupEventListeners();
  }
  
  public static getInstance(): LspDocumentService {
    if (!LspDocumentService.instance) {
      LspDocumentService.instance = new LspDocumentService();
    }
    return LspDocumentService.instance;
  }
  
  private setupEventListeners(): void {
    const unsubCreated = monacoModelManager.onModelCreated((event) => {
      this.handleModelCreated(event);
    });
    this.unsubscribeFunctions.push(unsubCreated);
    
    const unsubReady = monacoModelManager.onModelContentReady((event) => {
      this.handleModelContentReady(event);
    });
    this.unsubscribeFunctions.push(unsubReady);
    
    const unsubChanged = monacoModelManager.onModelContentChanged((event) => {
      this.handleModelContentChanged(event);
    });
    this.unsubscribeFunctions.push(unsubChanged);
    
    const unsubDisposed = monacoModelManager.onModelDisposed((event) => {
      this.handleModelDisposed(event);
    });
    this.unsubscribeFunctions.push(unsubDisposed);
  }
  
  private handleModelCreated(event: ModelCreatedEvent): void {
    const state: DocumentSyncState = {
      uri: event.uri,
      filePath: event.filePath,
      language: event.language,
      isOpen: false,
      lastSyncedContent: ''
    };
    
    this.documentStates.set(event.uri, state);
    
    // Intentionally wait for content-ready before sending didOpen.
  }
  
  private async handleModelContentReady(event: ModelContentReadyEvent): Promise<void> {
    const state = this.documentStates.get(event.uri);
    if (!state) {
      log.warn('Document state not found', { uri: event.uri });
      return;
    }
    
    if (state.isOpen) {
      return;
    }
    
    const workspacePath = this.detectWorkspacePath(event.filePath);
    state.workspacePath = workspacePath;
    
    if (!workspacePath) {
      log.warn('No workspace detected', { filePath: event.filePath });
      return;
    }
    
    const manager = WorkspaceLspManager.getOrCreate(workspacePath);
    
    try {
      const openResult = await manager.openDocument(
        event.uri,
        state.language,
        event.content
      );

      if (!openResult.opened) {
        return;
      }
      
      state.isOpen = true;
      state.lastSyncedContent = event.content;
    } catch (error) {
      log.error('Failed to open document', { uri: event.uri, filePath: event.filePath, error });
    }
  }
  
  private handleModelContentChanged(event: ModelContentChangedEvent): void {
    const state = this.documentStates.get(event.uri);
    if (!state) {
      return;
    }
    
    if (!state.isOpen) {
      return;
    }
    
    if (state.changeTimer) {
      clearTimeout(state.changeTimer);
    }
    
    state.changeTimer = setTimeout(() => {
      this.syncContentChange(event, state);
      state.changeTimer = undefined;
    }, this.CHANGE_DEBOUNCE);
  }
  
  /** Associate an editor instance with a document URI. */
  public associateEditor(uri: string, editor: any): void {
    const state = this.documentStates.get(uri);
    if (state) {
      state.editor = editor;
    }
  }
  
  public disassociateEditor(uri: string): void {
    const state = this.documentStates.get(uri);
    if (state) {
      state.editor = undefined;
    }
  }
  
  /**
   * Sync content changes to LSP with a small batching window to reduce IPC calls.
   */
  private async syncContentChange(
    event: ModelContentChangedEvent,
    state: DocumentSyncState
  ): Promise<void> {
    if (event.content === state.lastSyncedContent) {
      return;
    }
    
    if (!state.workspacePath) {
      return;
    }
    
    const manager = WorkspaceLspManager.get(state.workspacePath);
    if (!manager) {
      return;
    }
    
    const oldTimer = this.syncTimers.get(event.uri);
    if (oldTimer) {
      clearTimeout(oldTimer);
    }
    
    const timer = setTimeout(async () => {
      try {
        await manager.changeDocument(event.uri, event.content);
        
        state.lastSyncedContent = event.content;
        
        if (state.editor) {
          lspRefreshManager.onDocumentChange(event.uri, state.editor, {
            refreshSemanticTokens: true,
            refreshInlayHints: true,
            refreshDiagnostics: false
          });
        }
      } catch (error) {
        log.error('Failed to sync content', { uri: event.uri, error });
      } finally {
        this.syncTimers.delete(event.uri);
      }
    }, this.SYNC_DELAY);
    
    this.syncTimers.set(event.uri, timer);
  }
  
  /** Ensure any pending batched sync is flushed before closing. */
  private async flushPendingSync(uri: string): Promise<void> {
    const timer = this.syncTimers.get(uri);
    if (!timer) {
      return;
    }
    
    clearTimeout(timer);
    this.syncTimers.delete(uri);
    
    const state = this.documentStates.get(uri);
    if (!state || !state.workspacePath) {
      return;
    }
    
    const manager = WorkspaceLspManager.get(state.workspacePath);
    if (!manager) {
      return;
    }
    
    try {
      await manager.changeDocument(uri, state.lastSyncedContent);
    } catch (error) {
      log.error('Failed to flush pending sync', { uri, error });
    }
  }
  
  private async handleModelDisposed(event: ModelDisposedEvent): Promise<void> {
    const state = this.documentStates.get(event.uri);
    if (!state) {
      return;
    }
    
    await this.flushPendingSync(event.uri);
    
    if (state.changeTimer) {
      clearTimeout(state.changeTimer);
    }
    
    if (state.isOpen && state.workspacePath) {
      const manager = WorkspaceLspManager.get(state.workspacePath);
      if (manager) {
        try {
          await manager.closeDocument(event.uri);
        } catch (error) {
          log.error('Failed to close document', { uri: event.uri, error });
        }
      }
    }
    
    this.documentStates.delete(event.uri);
  }
  
  private detectWorkspacePath(filePath: string): string | undefined {
    const metadata = monacoModelManager.getModelMetadata(filePath);
    if (metadata?.workspacePath) {
      return metadata.workspacePath;
    }
    
    const normalizedPath = filePath.replace(/\\/g, '/');
    const lastSlashIndex = normalizedPath.lastIndexOf('/');
    if (lastSlashIndex > 0) {
      const parentDir = normalizedPath.substring(0, lastSlashIndex);
      return parentDir;
    }
    
    return undefined;
  }
  
  public getDocumentState(uri: string): DocumentSyncState | undefined {
    return this.documentStates.get(uri);
  }
  
  public getOpenDocuments(): string[] {
    const openDocs: string[] = [];
    this.documentStates.forEach((state, uri) => {
      if (state.isOpen) {
        openDocs.push(uri);
      }
    });
    return openDocs;
  }
  
  public async dispose(): Promise<void> {
    const flushPromises = Array.from(this.syncTimers.keys()).map(uri =>
      this.flushPendingSync(uri)
    );
    await Promise.all(flushPromises);
    
    this.unsubscribeFunctions.forEach(unsub => unsub());
    this.unsubscribeFunctions = [];
    
    this.documentStates.forEach(state => {
      if (state.changeTimer) {
        clearTimeout(state.changeTimer);
      }
    });
    
    this.documentStates.clear();
    this.syncTimers.clear();
  }
}

// Singleton export
export const lspDocumentService = LspDocumentService.getInstance();
export default LspDocumentService;

