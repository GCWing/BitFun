 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type {
  ExplorerChildrenPageDto,
  ExplorerNodeDto,
  WorkspaceInfo,
  FileSearchResponse,
  FileSearchResult
} from './tauri-commands';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceAPI');

export class WorkspaceAPI {
   
  async openWorkspace(path: string): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('open_workspace', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('open_workspace', error, { path });
    }
  }

   
  async closeWorkspace(): Promise<void> {
    try {
      await api.invoke('close_workspace', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('close_workspace', error);
    }
  }

   
  async getWorkspaceInfo(): Promise<WorkspaceInfo> {
    try {
      return await api.invoke('get_workspace_info', { 
        request: {} 
      });
    } catch (error) {
      throw createTauriCommandError('get_workspace_info', error);
    }
  }

   
  async listFiles(path: string): Promise<any[]> {
    try {
      return await api.invoke('list_files', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('list_files', error, { path });
    }
  }

   
  async readFile(path: string): Promise<string> {
    try {
      return await api.invoke('read_file', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('read_file', error, { path });
    }
  }

   
  async writeFile(path: string, content: string): Promise<void> {
    try {
      await api.invoke('write_file', { 
        request: { path, content } 
      });
    } catch (error) {
      throw createTauriCommandError('write_file', error, { path, content });
    }
  }

   
  async writeFileContent(workspacePath: string, filePath: string, content: string): Promise<void> {
    try {
      
      
      await api.invoke('write_file_content', {
        request: { workspacePath, filePath, content }
      });
    } catch (error) {
      throw createTauriCommandError('write_file_content', error, { workspacePath, filePath, content });
    }
  }

  async resetWorkspacePersonaFiles(workspacePath: string): Promise<void> {
    try {
      await api.invoke('reset_workspace_persona_files', {
        request: { workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('reset_workspace_persona_files', error, { workspacePath });
    }
  }

   
  async createFile(path: string): Promise<void> {
    try {
      await api.invoke('create_file', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('create_file', error, { path });
    }
  }

   
  async deleteFile(path: string): Promise<void> {
    try {
      await api.invoke('delete_file', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('delete_file', error, { path });
    }
  }

   
  async createDirectory(path: string): Promise<void> {
    try {
      await api.invoke('create_directory', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('create_directory', error, { path });
    }
  }

   
  async deleteDirectory(path: string, recursive: boolean = true): Promise<void> {
    try {
      await api.invoke('delete_directory', { 
        request: { path, recursive } 
      });
    } catch (error) {
      throw createTauriCommandError('delete_directory', error, { path, recursive });
    }
  }

   
  async getFileTree(path: string, maxDepth?: number): Promise<ExplorerNodeDto[]> {
    try {
      return await api.invoke('get_file_tree', { 
        request: { path, maxDepth } 
      });
    } catch (error) {
      throw createTauriCommandError('get_file_tree', error, { path, maxDepth });
    }
  }

   
  async getDirectoryChildren(path: string): Promise<ExplorerNodeDto[]> {
    try {
      return await api.invoke('get_directory_children', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('get_directory_children', error, { path });
    }
  }

   
  async getDirectoryChildrenPaginated(
    path: string, 
    offset: number = 0, 
    limit: number = 100
  ): Promise<ExplorerChildrenPageDto> {
    try {
      return await api.invoke('get_directory_children_paginated', { 
        request: { path, offset, limit } 
      });
    } catch (error) {
      throw createTauriCommandError('get_directory_children_paginated', error, { path, offset, limit });
    }
  }

  async explorerGetFileTree(path: string, maxDepth?: number): Promise<ExplorerNodeDto[]> {
    try {
      return await api.invoke('explorer_get_file_tree', {
        request: { path, maxDepth }
      });
    } catch (error) {
      throw createTauriCommandError('explorer_get_file_tree', error, { path, maxDepth });
    }
  }

  async explorerGetChildren(path: string): Promise<ExplorerNodeDto[]> {
    try {
      return await api.invoke('explorer_get_children', {
        request: { path }
      });
    } catch (error) {
      throw createTauriCommandError('explorer_get_children', error, { path });
    }
  }

  async explorerGetChildrenPaginated(
    path: string,
    offset: number = 0,
    limit: number = 100
  ): Promise<ExplorerChildrenPageDto> {
    try {
      return await api.invoke('explorer_get_children_paginated', {
        request: { path, offset, limit }
      });
    } catch (error) {
      throw createTauriCommandError('explorer_get_children_paginated', error, { path, offset, limit });
    }
  }

   
  async readFileContent(filePath: string, encoding?: string): Promise<string> {
    try {
      return await api.invoke('read_file_content', { 
        request: { filePath, encoding } 
      });
    } catch (error) {
      throw createTauriCommandError('read_file_content', error, { filePath, encoding });
    }
  }

  private createSearchId(prefix: string): string {
    return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
  }

  async cancelSearch(searchId: string): Promise<void> {
    if (!searchId) {
      return;
    }

    try {
      await api.invoke('cancel_search', {
        request: { searchId }
      });
    } catch (error) {
      log.warn('Failed to cancel search', { searchId, error });
    }
  }

  private async raceCancelable<T>(
    commandName: string,
    resultPromise: Promise<T>,
    searchId: string,
    signal?: AbortSignal
  ): Promise<T> {
    if (!signal) {
      return resultPromise;
    }

    if (signal.aborted) {
      await this.cancelSearch(searchId);
      throw new DOMException('Search aborted', 'AbortError');
    }

    return await Promise.race([
      resultPromise,
      new Promise<T>((_, reject) => {
        const handleAbort = () => {
          void this.cancelSearch(searchId);
          reject(new DOMException(`${commandName} aborted`, 'AbortError'));
        };

        signal.addEventListener('abort', handleAbort, { once: true });
      })
    ]);
  }

  async searchFiles(
    rootPath: string, 
    pattern: string, 
    searchContent: boolean = true,
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    searchId?: string,
    maxResults?: number,
    includeDirectories?: boolean,
    signal?: AbortSignal
  ): Promise<FileSearchResult[]> {
    const effectiveSearchId = searchId ?? this.createSearchId(searchContent ? 'legacy-content' : 'legacy-filenames');

    try {
      const resultPromise = api.invoke<FileSearchResult[]>('search_files', { 
        request: { 
          rootPath, 
          pattern, 
          searchContent,
          searchId: effectiveSearchId,
          caseSensitive,
          useRegex,
          wholeWord,
          maxResults,
          includeDirectories,
        } 
      });

      return await this.raceCancelable('search_files', resultPromise, effectiveSearchId, signal);
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        throw error;
      }
      throw createTauriCommandError('search_files', error, {
        rootPath,
        pattern,
        searchContent,
        searchId: effectiveSearchId,
        caseSensitive,
        useRegex,
        wholeWord,
        maxResults,
        includeDirectories,
      });
    }
  }

  async searchFilenamesOnly(
    rootPath: string, 
    pattern: string, 
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    searchIdOrSignal?: string | AbortSignal,
    maxResults?: number,
    includeDirectories: boolean = true,
    signal?: AbortSignal
  ): Promise<FileSearchResult[]> {
    const response = await this.searchFilenamesOnlyDetailed(
      rootPath,
      pattern,
      caseSensitive,
      useRegex,
      wholeWord,
      searchIdOrSignal,
      maxResults,
      includeDirectories,
      signal
    );
    return response.results;
  }

  async searchFilenamesOnlyDetailed(
    rootPath: string,
    pattern: string,
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    searchIdOrSignal?: string | AbortSignal,
    maxResults?: number,
    includeDirectories: boolean = true,
    signal?: AbortSignal
  ): Promise<FileSearchResponse> {
    const effectiveSignal = searchIdOrSignal instanceof AbortSignal ? searchIdOrSignal : signal;
    const effectiveSearchId =
      typeof searchIdOrSignal === 'string' ? searchIdOrSignal : this.createSearchId('filenames');

    try {
      const resultPromise = api.invoke<FileSearchResponse>('search_filenames', {
        request: {
          rootPath,
          pattern,
          searchId: effectiveSearchId,
          caseSensitive,
          useRegex,
          wholeWord,
          maxResults,
          includeDirectories,
        }
      });

      return await this.raceCancelable('search_filenames', resultPromise, effectiveSearchId, effectiveSignal);
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        throw error;
      }

      throw createTauriCommandError('search_filenames', error, {
        rootPath,
        pattern,
        searchId: effectiveSearchId,
        caseSensitive,
        useRegex,
        wholeWord,
        maxResults,
        includeDirectories,
      });
    }
  }

  async searchContentOnly(
    rootPath: string, 
    pattern: string, 
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    searchIdOrSignal?: string | AbortSignal,
    maxResults?: number,
    signal?: AbortSignal
  ): Promise<FileSearchResult[]> {
    const response = await this.searchContentOnlyDetailed(
      rootPath,
      pattern,
      caseSensitive,
      useRegex,
      wholeWord,
      searchIdOrSignal,
      maxResults,
      signal
    );
    return response.results;
  }

  async searchContentOnlyDetailed(
    rootPath: string,
    pattern: string,
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    searchIdOrSignal?: string | AbortSignal,
    maxResults?: number,
    signal?: AbortSignal
  ): Promise<FileSearchResponse> {
    const effectiveSignal = searchIdOrSignal instanceof AbortSignal ? searchIdOrSignal : signal;
    const effectiveSearchId =
      typeof searchIdOrSignal === 'string' ? searchIdOrSignal : this.createSearchId('content');

    try {
      const resultPromise = api.invoke<FileSearchResponse>('search_file_contents', { 
        request: { 
          rootPath, 
          pattern, 
          searchId: effectiveSearchId,
          caseSensitive,
          useRegex,
          wholeWord,
          maxResults,
        } 
      });

      return await this.raceCancelable('search_file_contents', resultPromise, effectiveSearchId, effectiveSignal);
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        throw error;
      }

      throw createTauriCommandError('search_file_contents', error, {
        rootPath,
        pattern,
        searchId: effectiveSearchId,
        caseSensitive,
        useRegex,
        wholeWord,
        maxResults,
      });
    }
  }

   
  async renameFile(oldPath: string, newPath: string): Promise<void> {
    try {
      await api.invoke('rename_file', { 
        request: { oldPath, newPath } 
      });
    } catch (error) {
      throw createTauriCommandError('rename_file', error, { oldPath, newPath });
    }
  }

  /**
   * Copy a local file to another local path (binary-safe).
   */
  async exportLocalFileToPath(sourcePath: string, destinationPath: string): Promise<void> {
    try {
      await api.invoke('export_local_file_to_path', {
        request: { sourcePath, destinationPath },
      });
    } catch (error) {
      throw createTauriCommandError('export_local_file_to_path', error, {
        sourcePath,
        destinationPath,
      });
    }
  }

   
  async revealInExplorer(path: string): Promise<void> {
    try {
      await api.invoke('reveal_in_explorer', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('reveal_in_explorer', error, { path });
    }
  }

   
  async startFileWatch(path: string, recursive?: boolean): Promise<void> {
    try {
      await api.invoke('start_file_watch', { 
        path,
        recursive
      });
    } catch (error) {
      log.error('Failed to start file watch', { path, recursive, error });
      throw createTauriCommandError('start_file_watch', error, { path, recursive });
    }
  }

   
  async stopFileWatch(path: string): Promise<void> {
    try {
      await api.invoke('stop_file_watch', { 
        path
      });
    } catch (error) {
      log.error('Failed to stop file watch', { path, error });
      throw createTauriCommandError('stop_file_watch', error, { path });
    }
  }

   
  async getWatchedPaths(): Promise<string[]> {
    try {
      return await api.invoke('get_watched_paths', {});
    } catch (error) {
      throw createTauriCommandError('get_watched_paths', error);
    }
  }

   
  async getClipboardFiles(): Promise<{ files: string[]; isCut: boolean }> {
    try {
      return await api.invoke('get_clipboard_files');
    } catch (error) {
      throw createTauriCommandError('get_clipboard_files', error);
    }
  }

   
  async pasteFiles(
    sourcePaths: string[],
    targetDirectory: string,
    isCut: boolean = false
  ): Promise<{ successCount: number; failedFiles: Array<{ path: string; error: string }> }> {
    try {
      return await api.invoke('paste_files', {
        request: {
          sourcePaths,
          targetDirectory,
          isCut
        }
      });
    } catch (error) {
      throw createTauriCommandError('paste_files', error, { sourcePaths, targetDirectory, isCut });
    }
  }
}


export const workspaceAPI = new WorkspaceAPI();
