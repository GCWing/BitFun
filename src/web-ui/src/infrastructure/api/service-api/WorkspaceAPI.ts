 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type {
  WorkspaceInfo,
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

   
  async getFileTree(path: string, maxDepth?: number): Promise<any[]> {
    try {
      return await api.invoke('get_file_tree', { 
        request: { path, maxDepth } 
      });
    } catch (error) {
      throw createTauriCommandError('get_file_tree', error, { path, maxDepth });
    }
  }

   
  async getDirectoryChildren(path: string): Promise<any[]> {
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
  ): Promise<{
    children: any[];
    total: number;
    hasMore: boolean;
    offset: number;
    limit: number;
  }> {
    try {
      return await api.invoke('get_directory_children_paginated', { 
        request: { path, offset, limit } 
      });
    } catch (error) {
      throw createTauriCommandError('get_directory_children_paginated', error, { path, offset, limit });
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

   
  async searchFiles(
    rootPath: string, 
    pattern: string, 
    searchContent: boolean = true,
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    signal?: AbortSignal
  ): Promise<FileSearchResult[]> {
    
    if (signal?.aborted) {
      throw new DOMException('Search aborted', 'AbortError');
    }

    try {
      const resultPromise = api.invoke('search_files', { 
        request: { 
          rootPath, 
          pattern, 
          searchContent,
          caseSensitive,
          useRegex,
          wholeWord
        } 
      });

      
      if (signal) {
        return await Promise.race([
          resultPromise,
          new Promise<FileSearchResult[]>((_, reject) => {
            signal.addEventListener('abort', () => {
              reject(new DOMException('Search aborted', 'AbortError'));
            });
          })
        ]);
      }

      return await resultPromise;
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        throw error;
      }
      throw createTauriCommandError('search_files', error, { rootPath, pattern, searchContent, caseSensitive, useRegex, wholeWord });
    }
  }

   
  async searchFilenamesOnly(
    rootPath: string, 
    pattern: string, 
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    signal?: AbortSignal
  ): Promise<FileSearchResult[]> {
    return this.searchFiles(
      rootPath,
      pattern,
      false, 
      caseSensitive,
      useRegex,
      wholeWord,
      signal
    );
  }

   
  async searchContentOnly(
    rootPath: string, 
    pattern: string, 
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    signal?: AbortSignal
  ): Promise<FileSearchResult[]> {
    
    if (signal?.aborted) {
      throw new DOMException('Search aborted', 'AbortError');
    }

    try {
      const resultPromise = api.invoke('search_files', { 
        request: { 
          rootPath, 
          pattern, 
          searchContent: true,
          caseSensitive,
          useRegex,
          wholeWord
        } 
      });

      
      if (signal) {
        const results = await Promise.race([
          resultPromise,
          new Promise<FileSearchResult[]>((_, reject) => {
            signal.addEventListener('abort', () => {
              reject(new DOMException('Search aborted', 'AbortError'));
            });
          })
        ]);
        
        return results.filter((r: FileSearchResult) => r.matchType === 'content');
      }

      const results = await resultPromise;
      return results.filter((r: FileSearchResult) => r.matchType === 'content');
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        throw error;
      }
      throw createTauriCommandError('search_files', error, { rootPath, pattern });
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

   
  async revealInExplorer(path: string): Promise<void> {
    try {
      await api.invoke('reveal_in_explorer', { 
        request: { path } 
      });
    } catch (error) {
      throw createTauriCommandError('reveal_in_explorer', error, { path });
    }
  }

  async openInExplorer(path: string): Promise<void> {
    try {
      await api.invoke('open_in_explorer', {
        request: { path }
      });
    } catch (error) {
      throw createTauriCommandError('open_in_explorer', error, { path });
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
