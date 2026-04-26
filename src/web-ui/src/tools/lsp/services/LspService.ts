/**
 * Thin frontend wrapper around backend LSP APIs.
 */

import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/shared/utils/logger';
import { measureAsync } from '@/shared/utils/timing';
import type { LspPlugin, CompletionItem, TextEdit } from '../types';

const log = createLogger('LspService');

export class LspService {
  private static instance: LspService;
  private initialized = false;

  private constructor() {}

  static getInstance(): LspService {
    if (!LspService.instance) {
      LspService.instance = new LspService();
    }
    return LspService.instance;
  }

  /** Initialize the backend LSP system once. */
  async initialize(): Promise<void> {
    if (this.initialized) {
      return;
    }
    
    try {
      const result = await measureAsync(() => invoke('lsp_initialize'));
      this.initialized = true;
      log.info('LSP system initialized', { durationMs: result.durationMs });
    } catch (error) {
      log.error('Failed to initialize LSP system', error);
      throw error;
    }
  }

  /** Open a workspace (supersedes the old setWorkspaceRoot flow). */
  async openWorkspace(workspacePath: string): Promise<void> {
    try {
      await invoke('lsp_open_workspace', {
        request: { workspace_path: workspacePath }
      });
      log.debug('Workspace opened', { workspacePath });
    } catch (error) {
      log.error('Failed to open workspace', error);
      throw error;
    }
  }

  /** Start a language server for a given file path. */
  async startServerForFile(filePath: string): Promise<void> {
    try {
      const result = await measureAsync(() => invoke('lsp_start_server_for_file', {
        request: { filePath }
      }));
      log.debug('LSP server started for file', { filePath, durationMs: result.durationMs });
    } catch (error) {
      log.error('Failed to start LSP server for file', error);
      throw error;
    }
  }

  /** Stop a language server. */
  async stopServer(language: string): Promise<void> {
    try {
      await invoke('lsp_stop_server', {
        request: { language }
      });
      log.debug('LSP server stopped', { language });
    } catch (error) {
      log.error('Failed to stop LSP server', error);
      throw error;
    }
  }

  /** Notify didOpen. */
  async didOpen(language: string, uri: string, text: string): Promise<void> {
    try {
      await invoke('lsp_did_open', {
        request: { language, uri, text }
      });
    } catch (error) {
      log.error('Failed to notify document open', error);
      throw error;
    }
  }

  /** Notify didChange. */
  async didChange(language: string, uri: string, version: number, text: string): Promise<void> {
    try {
      await invoke('lsp_did_change', {
        request: { language, uri, version, text }
      });
    } catch (error) {
      log.error('Failed to notify document change', error);
      throw error;
    }
  }

  /** Notify didSave. */
  async didSave(language: string, uri: string): Promise<void> {
    try {
      await invoke('lsp_did_save', {
        request: { language, uri }
      });
    } catch (error) {
      log.error('Failed to notify document save', error);
      throw error;
    }
  }

  /** Notify didClose. */
  async didClose(language: string, uri: string): Promise<void> {
    try {
      await invoke('lsp_did_close', {
        request: { language, uri }
      });
    } catch (error) {
      log.error('Failed to notify document close', error);
      throw error;
    }
  }

  /** Request completion items. */
  async getCompletions(
    language: string,
    uri: string,
    line: number,
    character: number
  ): Promise<CompletionItem[]> {
    try {
      const result = await measureAsync<CompletionItem[]>(() => invoke('lsp_get_completions', {
        request: { language, uri, line, character }
      }) as Promise<CompletionItem[]>);
      log.debug('Got completions', { 
        count: Array.isArray(result.value) ? result.value.length : 0, 
        durationMs: result.durationMs,
      });
      return result.value;
    } catch (error) {
      log.error('Failed to get completions', error);
      throw error;
    }
  }

  /** Request hover info. */
  async getHover(
    language: string,
    uri: string,
    line: number,
    character: number
  ): Promise<any> {
    try {
      const result = await invoke('lsp_get_hover', {
        request: { language, uri, line, character }
      });
      return result;
    } catch (error) {
      log.error('Failed to get hover', error);
      throw error;
    }
  }

  /** Request definition location(s). */
  async gotoDefinition(
    language: string,
    uri: string,
    line: number,
    character: number
  ): Promise<any> {
    try {
      const result = await measureAsync(() => invoke('lsp_goto_definition', {
        request: { language, uri, line, character }
      }));
      log.debug('Found definition', { durationMs: result.durationMs });
      return result.value;
    } catch (error) {
      log.error('Failed to go to definition', error);
      throw error;
    }
  }

  /** Request references. */
  async findReferences(
    language: string,
    uri: string,
    line: number,
    character: number
  ): Promise<any> {
    try {
      const result = await measureAsync(() => invoke('lsp_find_references', {
        request: { language, uri, line, character }
      }));
      const count = Array.isArray(result.value) ? result.value.length : 0;
      log.debug('Found references', { count, durationMs: result.durationMs });
      return result.value;
    } catch (error) {
      log.error('Failed to find references', error);
      throw error;
    }
  }

  /** Request full-document formatting edits. */
  async formatDocument(
    language: string,
    uri: string,
    tabSize?: number,
    insertSpaces?: boolean
  ): Promise<TextEdit[]> {
    try {
      const result = await measureAsync<TextEdit[]>(() => invoke('lsp_format_document', {
        request: { language, uri, tabSize, insertSpaces }
      }) as Promise<TextEdit[]>);
      const count = Array.isArray(result.value) ? result.value.length : 0;
      log.debug('Document formatted', { editCount: count, durationMs: result.durationMs });
      return result.value;
    } catch (error) {
      log.error('Failed to format document', error);
      throw error;
    }
  }

  /** Install an LSP plugin package. */
  async installPlugin(packagePath: string): Promise<string> {
    try {
      const result = await measureAsync<string>(() => invoke('lsp_install_plugin', {
        request: { packagePath }
      }) as Promise<string>);
      log.info('Plugin installed', { pluginId: result.value, durationMs: result.durationMs });
      return result.value;
    } catch (error) {
      log.error('Failed to install plugin', error);
      throw error;
    }
  }

  /** Uninstall an installed plugin by ID. */
  async uninstallPlugin(pluginId: string): Promise<void> {
    try {
      await invoke('lsp_uninstall_plugin', {
        request: { pluginId }
      });
      log.info('Plugin uninstalled', { pluginId });
    } catch (error) {
      log.error('Failed to uninstall plugin', error);
      throw error;
    }
  }

  /** List installed plugins. */
  async listPlugins(): Promise<LspPlugin[]> {
    try {
      const plugins = await invoke('lsp_list_plugins') as LspPlugin[];
      const count = Array.isArray(plugins) ? plugins.length : 0;
      log.debug('Listed plugins', { count });
      return plugins;
    } catch (error) {
      log.error('Failed to list LSP plugins', error);
      throw error;
    }
  }

  /** Get plugin info by ID. */
  async getPlugin(pluginId: string): Promise<LspPlugin | null> {
    return await invoke('lsp_get_plugin', {
      request: { pluginId }
    });
  }

  /** Get server capabilities for a language. */
  async getServerCapabilities(language: string): Promise<any> {
    return await invoke('lsp_get_server_capabilities', {
      request: { language }
    });
  }

  /** Convert a file path to a file:// URI. */
  static filePathToUri(filePath: string): string {
    // Windows paths
    if (filePath.match(/^[a-zA-Z]:/)) {
      return `file:///${filePath.replace(/\\/g, '/')}`;
    }
    // Unix paths
    return `file://${filePath}`;
  }

  /** Convert a file:// URI back to a file path. */
  static uriToFilePath(uri: string): string {
    if (uri.startsWith('file:///')) {
      const path = uri.substring(8);
      // Windows paths
      if (path.match(/^[a-zA-Z]:/)) {
        return path.replace(/\//g, '\\');
      }
      return path;
    }
    if (uri.startsWith('file://')) {
      return uri.substring(7);
    }
    return uri;
  }
}

// Singleton export
export const lspService = LspService.getInstance();
