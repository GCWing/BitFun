/**
 * Workspace-scoped LSP manager.
 *
 * Provides a thin API over backend workspace LSP commands and bridges backend
 * events to frontend consumers (e.g. diagnostics subscriptions, notifications).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('WorkspaceLspManager');

interface LspEvent {
  type: 'ServerStateChanged' | 'DocumentOpened' | 'DocumentClosed' | 'WorkspaceOpened' | 'WorkspaceClosed' | 'ServerError' | 'Diagnostics' | 'IndexingProgress' | 'IndexingComplete' | 'ProjectDetected';
  data: {
    workspace_path?: string;
    language?: string;
    plugin_name?: string;
    status?: string;
    message?: string;
    uri?: string;
    error?: string;
    diagnostics?: any[];
    progress?: number;
    project_info?: any;
  };
}

interface ServerState {
  status: 'stopped' | 'starting' | 'running' | 'failed' | 'restarting';
  language: string;
  startedAt?: number;
  lastError?: string;
  restartCount: number;
  documentCount: number;
}

export class WorkspaceLspManager {
  private static instances = new Map<string, WorkspaceLspManager>();
  
  private workspacePath: string;
  private eventUnlisten?: UnlistenFn;
  private isInitialized = false;
  

  private startingLanguages = new Set<string>();
  private languageReadyPromises = new Map<string, Promise<void>>();
  

  private diagnosticsCallbacks = new Map<string, Array<(diagnostics: any[]) => void>>();
  

  private indexingProgressNotifications = new Map<string, {
    updateMessage: (message: string) => void;
    complete: (message?: string) => void;
  }>();
  
  private constructor(workspacePath: string) {
    this.workspacePath = workspacePath;
  }
  
  
  getWorkspacePath(): string {
    return this.workspacePath;
  }
  
  
  static getOrCreate(workspacePath: string): WorkspaceLspManager {
    if (!this.instances.has(workspacePath)) {
      const manager = new WorkspaceLspManager(workspacePath);
      this.instances.set(workspacePath, manager);
    }
    return this.instances.get(workspacePath)!;
  }
  
  
  static get(workspacePath: string): WorkspaceLspManager | undefined {
    return this.instances.get(workspacePath);
  }
  
  
  static async remove(workspacePath: string): Promise<void> {
    const manager = this.instances.get(workspacePath);
    if (manager) {
      await manager.dispose();
      this.instances.delete(workspacePath);
    }
  }
  
  
  async initialize(): Promise<void> {
    if (this.isInitialized) {
      return;
    }

    try {

      await invoke('lsp_open_workspace', {
        request: { workspacePath: this.workspacePath }
      });
      

      this.eventUnlisten = await listen<LspEvent>('lsp-event', (event) => {
        this.handleLspEvent(event.payload);
      });
      
      this.isInitialized = true;
      log.info('LSP initialized', { workspacePath: this.workspacePath });
    } catch (error) {
      log.error('LSP initialization failed', { workspacePath: this.workspacePath, error });
      throw error;
    }
  }
  
  
  private handleLspEvent(event: LspEvent) {

    if (event.data.workspace_path !== this.workspacePath) {
      return;
    }

    switch (event.type) {
      case 'ServerStateChanged':
        this.onServerStateChanged(event.data);
        break;
      case 'ServerError':
        this.onServerError(event.data);
        break;
      case 'ProjectDetected':
        this.onProjectDetected(event.data);
        break;
      case 'IndexingProgress':
        this.onIndexingProgress(event.data);
        break;
      case 'IndexingComplete':
        this.onIndexingComplete(event.data);
        break;
      case 'Diagnostics':
        this.onDiagnosticsReceived(event.data);
        break;
      default:
        log.warn('Unknown LSP event', { eventType: event.type });
    }
  }

  
  private normalizeUri(uri: string): string {
    if (!uri) return uri;
    


    return uri.toLowerCase();
  }

  
  private onDiagnosticsReceived(data: LspEvent['data']) {
    const { uri, diagnostics } = data;
    
    if (!uri || !diagnostics) {
      log.error('Invalid diagnostics data', { uri, hasDiagnostics: !!diagnostics });
      return;
    }
    

    const normalizedUri = this.normalizeUri(uri);
    

    const callbacks = this.diagnosticsCallbacks.get(normalizedUri);
    if (callbacks && callbacks.length > 0) {
      callbacks.forEach((cb) => {
        try {
          cb(diagnostics);
        } catch (error) {
          log.error('Diagnostics callback error', { uri, error });
        }
      });
    }
  }

  
  private onServerStateChanged(data: LspEvent['data']) {
    const { language, status, message } = data;
    
    if (!language) return;


    switch (status) {
      case 'starting':

        notificationService.progress({
          title: i18nService.t('settings/lsp:server.starting', { language }),
          message: i18nService.t('settings/lsp:server.startingMessage'),
          initialProgress: 20
        });
        break;
      case 'running':



        break;
      case 'failed':
        notificationService.error(
          message || i18nService.t('settings/lsp:server.checkConfig'),
          {
            title: i18nService.t('settings/lsp:server.startFailed', { language }),
            duration: 5000
          }
        );
        break;
    }
  }

  
  private onServerError(data: LspEvent['data']) {
    const { language, error } = data;
    
    if (!language || !error) return;

    log.error('Server error', { language, error });
    
    notificationService.error(
      error,
      {
        title: i18nService.t('settings/lsp:server.error', { language }),
        duration: 5000
      }
    );
  }

  
  private onProjectDetected(_data: LspEvent['data']) {


  }

  
  private onIndexingProgress(data: LspEvent['data']) {
    const { language, plugin_name, message, progress } = data;
    
    if (!language || !message) return;
    

    const displayName = plugin_name || i18nService.t('settings/lsp:server.languageService', { language });
    

    if (!this.indexingProgressNotifications.has(language)) {
      const loadingNotif = notificationService.loading({
        title: displayName,
        message: message
      });
      this.indexingProgressNotifications.set(language, loadingNotif);
    } else {

      const loadingNotif = this.indexingProgressNotifications.get(language);
      if (loadingNotif) {
        loadingNotif.updateMessage(message);
      }
    }
    

    if (progress !== undefined && progress >= 100) {
      const loadingNotif = this.indexingProgressNotifications.get(language);
      if (loadingNotif) {
        loadingNotif.complete();
        this.indexingProgressNotifications.delete(language);
      }
    }
  }

  
  private onIndexingComplete(data: LspEvent['data']) {
    const { language } = data;
    
    if (!language) return;
    

    const progressNotif = this.indexingProgressNotifications.get(language);
    if (progressNotif) {
      progressNotif.complete();
      this.indexingProgressNotifications.delete(language);
    }
    

    this.startingLanguages.delete(language);
  }
  
  
  async openDocument(uri: string, language: string, content: string): Promise<string> {

    if (this.startingLanguages.has(language)) {
      const readyPromise = this.languageReadyPromises.get(language);
      if (readyPromise) {
        await readyPromise;
      }
    }
    
    if (!this.isInitialized) {
      try {
        await this.initialize();
      } catch (initError) {
        log.error('Failed to initialize manager', { workspacePath: this.workspacePath, error: initError });
        throw initError;
      }
    }
    

    if (!this.startingLanguages.has(language)) {
      this.startingLanguages.add(language);
      

      const readyPromise = new Promise<void>((resolve) => {

        setTimeout(() => {
          this.startingLanguages.delete(language);
          this.languageReadyPromises.delete(language);
          resolve();
        }, 5000);
      });
      this.languageReadyPromises.set(language, readyPromise);
    }
    
    try {

      const timeoutPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error('LSP open document timeout after 30s')), 30000);
      });
      
      const openPromise = invoke('lsp_open_document', {
        request: {
          workspacePath: this.workspacePath,
          uri,
          language,
          content
        }
      });
      
      await Promise.race([openPromise, timeoutPromise]);
      


      return language;
    } catch (error) {
      log.error('Failed to open document', { workspacePath: this.workspacePath, uri, language, error });
      throw error;
    }
  }
  
  
  
  async changeDocument(uri: string, content: string): Promise<void> {
    try {
      await invoke('lsp_change_document', {
        request: {
          workspacePath: this.workspacePath,
          uri,
          content
        }
      });
    } catch (error) {
      log.error('Failed to notify document change', { uri, error });
    }
  }
  
  
  async saveDocument(uri: string): Promise<void> {
    try {
      await invoke('lsp_save_document', {
        request: {
          workspacePath: this.workspacePath,
          uri
        }
      });
    } catch (error) {
      log.error('Failed to notify document save', { uri, error });
    }
  }
  
  
  async closeDocument(uri: string): Promise<void> {
    try {
      await invoke('lsp_close_document', {
        request: {
          workspacePath: this.workspacePath,
          uri
        }
      });
    } catch (error) {
      log.error('Failed to close document', { uri, error });
    }
  }

  
  async getCompletions(language: string, uri: string, line: number, character: number): Promise<any[]> {
    try {
      const result = await invoke<any[]>('lsp_get_completions_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          line,
          character
        }
      });
      return result || [];
    } catch (error) {
      log.error('Failed to get completions', { workspacePath: this.workspacePath, language, uri, line, character, error });
      return [];
    }
  }

  
  async getHover(language: string, uri: string, line: number, character: number): Promise<any> {
    try {
      return await invoke('lsp_get_hover_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          line,
          character
        }
      });
    } catch (error: any) {


      const errorStr = error?.message || String(error);
      if (errorStr.includes('Missing result')) {

        return null;
      }
      log.error('Failed to get hover', { workspacePath: this.workspacePath, language, uri, line, character, error });
      return null;
    }
  }

  
  async gotoDefinition(language: string, uri: string, line: number, character: number): Promise<any> {
    try {
      return await invoke('lsp_goto_definition_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          line,
          character
        }
      });
    } catch (error) {
      log.error('Failed to goto definition', { workspacePath: this.workspacePath, language, uri, line, character, error });
      return null;
    }
  }

  
  async findReferences(language: string, uri: string, line: number, character: number): Promise<any> {
    try {
      return await invoke('lsp_find_references_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          line,
          character
        }
      });
    } catch (error) {
      log.error('Failed to find references', { workspacePath: this.workspacePath, language, uri, line, character, error });
      return null;
    }
  }

  
  async getSignatureHelp(language: string, uri: string, line: number, character: number): Promise<any> {
    try {
      const result = await invoke('lsp_get_signature_help_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          line,
          character
        }
      });
      return result;
    } catch (error) {
      log.error('Failed to get signature help', { workspacePath: this.workspacePath, language, uri, line, character, error });
      return null;
    }
  }

  
  onDiagnostics(uri: string, callback: (diagnostics: any[]) => void): void {

    const normalizedUri = this.normalizeUri(uri);
    
    if (!this.diagnosticsCallbacks.has(normalizedUri)) {
      this.diagnosticsCallbacks.set(normalizedUri, []);
    }
    this.diagnosticsCallbacks.get(normalizedUri)!.push(callback);
    

    this.requestDiagnostics(uri).catch(() => {
      // silent
    });
  }

  
  private async requestDiagnostics(uri: string): Promise<void> {
    try {
      const diagnostics = await invoke<any[]>('lsp_get_diagnostics', {
        request: {
          workspacePath: this.workspacePath,
          uri
        }
      });
      

      const callbacks = this.diagnosticsCallbacks.get(uri);
      if (callbacks && diagnostics) {
        callbacks.forEach(cb => cb(diagnostics));
      }
    } catch (_error) {

    }
  }

  
  async formatDocument(language: string, uri: string, tabSize: number = 2, insertSpaces: boolean = true): Promise<any> {
    try {
      return await invoke('lsp_format_document_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          tabSize,
          insertSpaces
        }
      });
    } catch (error) {
      log.error('Failed to format document', { workspacePath: this.workspacePath, language, uri, tabSize, insertSpaces, error });
      return null;
    }
  }

  
  async getInlayHints(
    language: string, 
    uri: string, 
    startLine: number, 
    startCharacter: number,
    endLine: number,
    endCharacter: number
  ): Promise<any[]> {
    try {
      return await invoke('lsp_get_inlay_hints_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          startLine,
          startCharacter,
          endLine,
          endCharacter
        }
      });
    } catch (error) {
      log.error('Failed to get inlay hints', { workspacePath: this.workspacePath, language, uri, error });
      return [];
    }
  }

  
  async rename(language: string, uri: string, line: number, character: number, newName: string): Promise<any> {
    try {
      return await invoke('lsp_rename_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          line,
          character,
          newName
        }
      });
    } catch (error) {
      log.error('Failed to rename', { workspacePath: this.workspacePath, language, uri, line, character, newName, error });
      return null;
    }
  }

  
  async getCodeActions(language: string, uri: string, range: any, context: any): Promise<any> {
    try {
      return await invoke('lsp_get_code_actions_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          range,
          context
        }
      });
    } catch (error) {
      log.error('Failed to get code actions', { workspacePath: this.workspacePath, language, uri, error });
      return null;
    }
  }

  
  async getDocumentSymbols(language: string, uri: string): Promise<any> {
    try {
      return await invoke('lsp_get_document_symbols_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri
        }
      });
    } catch (error) {
      log.error('Failed to get document symbols', { workspacePath: this.workspacePath, language, uri, error });
      return null;
    }
  }

  
  async getWorkspaceSymbols(query: string): Promise<any> {
    try {
      return await invoke('lsp_get_workspace_symbols', {
        request: {
          workspacePath: this.workspacePath,
          query
        }
      });
    } catch (error) {
      log.error('Failed to search workspace symbols', { workspacePath: this.workspacePath, query, error });
      return null;
    }
  }

  
  async getDocumentHighlight(language: string, uri: string, line: number, character: number): Promise<any> {
    try {
      return await invoke('lsp_get_document_highlight_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          line,
          character
        }
      });
    } catch (error) {
      log.error('Failed to get document highlight', { workspacePath: this.workspacePath, language, uri, line, character, error });
      return null;
    }
  }

  
  async getSemanticTokens(language: string, uri: string): Promise<any> {
    try {
      return await invoke('lsp_get_semantic_tokens_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri
        }
      });
    } catch (error) {
      log.error('Failed to get semantic tokens', { workspacePath: this.workspacePath, language, uri, error });
      return null;
    }
  }

  
  async getSemanticTokensRange(
    language: string, 
    uri: string, 
    startLine: number, 
    startCharacter: number,
    endLine: number,
    endCharacter: number
  ): Promise<any> {
    try {
      return await invoke('lsp_get_semantic_tokens_range_workspace', {
        request: {
          workspacePath: this.workspacePath,
          language,
          uri,
          startLine,
          startCharacter,
          endLine,
          endCharacter
        }
      });
    } catch (error) {
      log.error('Failed to get semantic tokens range', { workspacePath: this.workspacePath, language, uri, error });
      return null;
    }
  }
  
  
  async getServerState(language: string): Promise<ServerState | null> {
    try {
      return await invoke<ServerState>('lsp_get_server_state', {
        request: {
          workspacePath: this.workspacePath,
          language
        }
      });
    } catch (error) {
      log.error('Failed to get server state', { workspacePath: this.workspacePath, language, error });
      return null;
    }
  }

  
  async getAllServerStates(): Promise<Record<string, ServerState>> {
    try {
      return await invoke<Record<string, ServerState>>('lsp_get_all_server_states', {
        request: {
          workspacePath: this.workspacePath
        }
      });
    } catch (error) {
      log.error('Failed to get all server states', { workspacePath: this.workspacePath, error });
      return {};
    }
  }
  
  
  async dispose(): Promise<void> {
    try {

      if (this.eventUnlisten) {
        this.eventUnlisten();
        this.eventUnlisten = undefined;
      }


      await invoke('lsp_close_workspace', {
        request: { workspacePath: this.workspacePath }
      });
      
      this.isInitialized = false;
    } catch (error) {
      log.error('Error during dispose', { workspacePath: this.workspacePath, error });
    }
  }
}
