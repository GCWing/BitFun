 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type {
  ExplorerChildrenPageDto,
  ExplorerNodeDto,
  WorkspaceInfo,
  FileSearchResponse,
  FileSearchResult,
  FileSearchCompleteEvent,
  FileSearchErrorEvent,
  FileSearchProgressEvent,
  FileSearchResultGroup,
  FileSearchStreamKind,
  FileSearchStreamStartResponse,
  SearchRepoIndexRequest,
  WorkspaceSearchIndexStatus,
  WorkspaceSearchIndexTaskHandle,
} from './tauri-commands';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WorkspaceAPI');

const FILE_SEARCH_PROGRESS_EVENT = 'file-search://progress';
const FILE_SEARCH_COMPLETE_EVENT = 'file-search://complete';
const FILE_SEARCH_ERROR_EVENT = 'file-search://error';

interface FileSearchStreamCallbacks {
  onProgress?: (event: FileSearchProgressEvent) => void;
}

export interface FileMetadata {
  path: string;
  resolvedPath?: string;
  modified: number;
  size: number;
  isFile: boolean;
  isDir: boolean;
  isRemote?: boolean;
  isRuntimeArtifact?: boolean;
}

interface WorkspaceSearchRepoStatusRaw {
  repoId: string;
  repoPath: string;
  storageRoot: string;
  baseSnapshotRoot: string;
  workspaceOverlayRoot: string;
  phase: WorkspaceSearchIndexStatus['repoStatus']['phase'];
  snapshotKey?: string | null;
  lastProbeUnixSecs?: number | null;
  lastRebuildUnixSecs?: number | null;
  dirtyFiles: {
    modified: number;
    deleted: number;
    new: number;
  };
  rebuildRecommended: boolean;
  activeTaskId?: string | null;
  probeHealthy: boolean;
  lastError?: string | null;
  overlay?: WorkspaceSearchIndexStatus['repoStatus']['overlay'] | null;
}

interface WorkspaceSearchTaskStatusRaw {
  taskId: string;
  workspaceId: string;
  kind: NonNullable<WorkspaceSearchIndexStatus['activeTask']>['kind'];
  state: NonNullable<WorkspaceSearchIndexStatus['activeTask']>['state'];
  phase?: NonNullable<WorkspaceSearchIndexStatus['activeTask']>['phase'] | null;
  message: string;
  processed: number;
  total?: number | null;
  startedUnixSecs: number;
  updatedUnixSecs: number;
  finishedUnixSecs?: number | null;
  cancellable: boolean;
  error?: string | null;
}

interface WorkspaceSearchIndexStatusRaw {
  repoStatus: WorkspaceSearchRepoStatusRaw;
  activeTask?: WorkspaceSearchTaskStatusRaw | null;
}

interface WorkspaceSearchIndexTaskHandleRaw {
  task: WorkspaceSearchTaskStatusRaw;
  repoStatus: WorkspaceSearchRepoStatusRaw;
}

function groupSearchResultsByFile(results: FileSearchResult[]): FileSearchResultGroup[] {
  const groups = new Map<string, FileSearchResultGroup>();

  for (const result of results) {
    const existing = groups.get(result.path);
    if (existing) {
      if (result.matchType === 'fileName') {
        existing.fileNameMatch = result;
      } else {
        existing.contentMatches.push(result);
      }
      continue;
    }

    groups.set(result.path, {
      path: result.path,
      name: result.name,
      isDirectory: result.isDirectory,
      fileNameMatch: result.matchType === 'fileName' ? result : undefined,
      contentMatches: result.matchType === 'content' ? [result] : [],
    });
  }

  return Array.from(groups.values());
}

function mapWorkspaceSearchRepoStatus(raw: WorkspaceSearchRepoStatusRaw): WorkspaceSearchIndexStatus['repoStatus'] {
  return {
    repoId: raw.repoId,
    repoPath: raw.repoPath,
    storageRoot: raw.storageRoot,
    baseSnapshotRoot: raw.baseSnapshotRoot,
    workspaceOverlayRoot: raw.workspaceOverlayRoot,
    phase: raw.phase,
    snapshotKey: raw.snapshotKey ?? null,
    lastProbeUnixSecs: raw.lastProbeUnixSecs ?? null,
    lastRebuildUnixSecs: raw.lastRebuildUnixSecs ?? null,
    dirtyFiles: raw.dirtyFiles,
    rebuildRecommended: raw.rebuildRecommended,
    activeTaskId: raw.activeTaskId ?? null,
    probeHealthy: raw.probeHealthy,
    lastError: raw.lastError ?? null,
    overlay: raw.overlay ?? null,
  };
}

function mapWorkspaceSearchTaskStatus(
  raw: WorkspaceSearchTaskStatusRaw
): NonNullable<WorkspaceSearchIndexStatus['activeTask']> {
  return {
    taskId: raw.taskId,
    workspaceId: raw.workspaceId,
    kind: raw.kind,
    state: raw.state,
    phase: raw.phase ?? null,
    message: raw.message,
    processed: raw.processed,
    total: raw.total ?? null,
    startedUnixSecs: raw.startedUnixSecs,
    updatedUnixSecs: raw.updatedUnixSecs,
    finishedUnixSecs: raw.finishedUnixSecs ?? null,
    cancellable: raw.cancellable,
    error: raw.error ?? null,
  };
}

function mapWorkspaceSearchIndexStatus(raw: WorkspaceSearchIndexStatusRaw): WorkspaceSearchIndexStatus {
  return {
    repoStatus: mapWorkspaceSearchRepoStatus(raw.repoStatus),
    activeTask: raw.activeTask ? mapWorkspaceSearchTaskStatus(raw.activeTask) : null,
  };
}

function mapWorkspaceSearchIndexTaskHandle(
  raw: WorkspaceSearchIndexTaskHandleRaw
): WorkspaceSearchIndexTaskHandle {
  return {
    task: mapWorkspaceSearchTaskStatus(raw.task),
    repoStatus: mapWorkspaceSearchRepoStatus(raw.repoStatus),
  };
}

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

  async explorerGetChildren(path: string): Promise<ExplorerNodeDto[]> {
    try {
      return await api.invoke('explorer_get_children', {
        request: { path }
      });
    } catch (error) {
      throw createTauriCommandError('explorer_get_children', error, { path });
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

  async getFileMetadata(path: string): Promise<FileMetadata> {
    try {
      const raw = await api.invoke<Record<string, unknown>>('get_file_metadata', {
        request: { path }
      });
      return {
        path: String(raw.path ?? path),
        resolvedPath: typeof raw.resolvedPath === 'string' ? raw.resolvedPath : undefined,
        modified: Number(raw.modified ?? 0),
        size: Number(raw.size ?? 0),
        isFile: raw.isFile === true,
        isDir: raw.isDir === true,
        isRemote: typeof raw.isRemote === 'boolean' ? raw.isRemote : undefined,
        isRuntimeArtifact:
          typeof raw.isRuntimeArtifact === 'boolean' ? raw.isRuntimeArtifact : undefined,
      };
    } catch (error) {
      throw createTauriCommandError('get_file_metadata', error, { path });
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

  private supportsSearchStreamEvents(): boolean {
    return typeof window !== 'undefined' && '__TAURI__' in window;
  }

  private async runSearchStream(
    commandName: 'start_search_filenames_stream' | 'start_search_file_contents_stream',
    searchKind: FileSearchStreamKind,
    request: {
      rootPath: string;
      pattern: string;
      searchId: string;
      caseSensitive: boolean;
      useRegex: boolean;
      wholeWord: boolean;
      maxResults?: number;
      includeDirectories?: boolean;
    },
    callbacks: FileSearchStreamCallbacks = {},
    signal?: AbortSignal
  ): Promise<FileSearchCompleteEvent> {
    if (!this.supportsSearchStreamEvents()) {
      throw new Error(`Search streaming is unavailable for ${searchKind} searches outside Tauri`);
    }

    if (signal?.aborted) {
      await this.cancelSearch(request.searchId);
      throw new DOMException(`${commandName} aborted`, 'AbortError');
    }

    const { listen } = await import('@tauri-apps/api/event');

    return await new Promise<FileSearchCompleteEvent>((resolve, reject) => {
      let settled = false;

      const cleanupCallbacks: Array<() => void> = [];
      const cleanup = () => {
        while (cleanupCallbacks.length > 0) {
          const callback = cleanupCallbacks.pop();
          try {
            callback?.();
          } catch (error) {
            log.warn('Failed to cleanup search stream listener', {
              searchId: request.searchId,
              searchKind,
              error,
            });
          }
        }
      };

      const settleResolve = (event: FileSearchCompleteEvent) => {
        if (settled) {
          return;
        }
        settled = true;
        cleanup();
        resolve(event);
      };

      const settleReject = (error: unknown) => {
        if (settled) {
          return;
        }
        settled = true;
        cleanup();
        reject(error);
      };

      const handleAbort = () => {
        void this.cancelSearch(request.searchId);
        settleReject(new DOMException(`${commandName} aborted`, 'AbortError'));
      };

      if (signal) {
        signal.addEventListener('abort', handleAbort, { once: true });
        cleanupCallbacks.push(() => {
          signal.removeEventListener('abort', handleAbort);
        });
      }

      void (async () => {
        cleanupCallbacks.push(await listen<FileSearchProgressEvent>(FILE_SEARCH_PROGRESS_EVENT, (tauriEvent) => {
          const event = tauriEvent.payload;
          if (event.searchId !== request.searchId || event.searchKind !== searchKind) {
            return;
          }

          callbacks.onProgress?.(event);
        }));

        cleanupCallbacks.push(await listen<FileSearchCompleteEvent>(FILE_SEARCH_COMPLETE_EVENT, (tauriEvent) => {
          const event = tauriEvent.payload;
          if (event.searchId !== request.searchId || event.searchKind !== searchKind) {
            return;
          }

          settleResolve(event);
        }));

        cleanupCallbacks.push(await listen<FileSearchErrorEvent>(FILE_SEARCH_ERROR_EVENT, (tauriEvent) => {
          const event = tauriEvent.payload;
          if (event.searchId !== request.searchId || event.searchKind !== searchKind) {
            return;
          }

          settleReject(new Error(event.error));
        }));

        await api.invoke<FileSearchStreamStartResponse>(commandName, { request });
      })().catch((error) => {
        settleReject(
          createTauriCommandError(commandName, error, {
            rootPath: request.rootPath,
            pattern: request.pattern,
            searchId: request.searchId,
            searchKind,
          })
        );
      });
    });
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

  async searchFilenamesOnlyStreamDetailed(
    rootPath: string,
    pattern: string,
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    searchIdOrSignal?: string | AbortSignal,
    maxResults?: number,
    includeDirectories: boolean = true,
    callbacks: FileSearchStreamCallbacks = {},
    signal?: AbortSignal
  ): Promise<FileSearchCompleteEvent> {
    const effectiveSignal = searchIdOrSignal instanceof AbortSignal ? searchIdOrSignal : signal;
    const effectiveSearchId =
      typeof searchIdOrSignal === 'string' ? searchIdOrSignal : this.createSearchId('filenames');

    if (!this.supportsSearchStreamEvents()) {
      const response = await this.searchFilenamesOnlyDetailed(
        rootPath,
        pattern,
        caseSensitive,
        useRegex,
        wholeWord,
        effectiveSearchId,
        maxResults,
        includeDirectories,
        effectiveSignal
      );
      const groupedResults = groupSearchResultsByFile(response.results);
      const event: FileSearchCompleteEvent = {
        searchId: effectiveSearchId,
        searchKind: 'filenames',
        limit: response.limit,
        truncated: response.truncated,
        totalResults: groupedResults.length,
      };
      if (groupedResults.length > 0) {
        callbacks.onProgress?.({
          searchId: effectiveSearchId,
          searchKind: 'filenames',
          results: groupedResults,
        });
      }
      return event;
    }

    return await this.runSearchStream(
      'start_search_filenames_stream',
      'filenames',
      {
        rootPath,
        pattern,
        searchId: effectiveSearchId,
        caseSensitive,
        useRegex,
        wholeWord,
        maxResults,
        includeDirectories,
      },
      callbacks,
      effectiveSignal
    );
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

  async searchContentOnlyStreamDetailed(
    rootPath: string,
    pattern: string,
    caseSensitive: boolean = false,
    useRegex: boolean = false,
    wholeWord: boolean = false,
    searchIdOrSignal?: string | AbortSignal,
    maxResults?: number,
    callbacks: FileSearchStreamCallbacks = {},
    signal?: AbortSignal
  ): Promise<FileSearchCompleteEvent> {
    const effectiveSignal = searchIdOrSignal instanceof AbortSignal ? searchIdOrSignal : signal;
    const effectiveSearchId =
      typeof searchIdOrSignal === 'string' ? searchIdOrSignal : this.createSearchId('content');

    if (!this.supportsSearchStreamEvents()) {
      const response = await this.searchContentOnlyDetailed(
        rootPath,
        pattern,
        caseSensitive,
        useRegex,
        wholeWord,
        effectiveSearchId,
        maxResults,
        effectiveSignal
      );
      const groupedResults = groupSearchResultsByFile(response.results);
      const event: FileSearchCompleteEvent = {
        searchId: effectiveSearchId,
        searchKind: 'content',
        limit: response.limit,
        truncated: response.truncated,
        totalResults: groupedResults.length,
        searchMetadata: response.searchMetadata,
      };
      if (groupedResults.length > 0) {
        callbacks.onProgress?.({
          searchId: effectiveSearchId,
          searchKind: 'content',
          results: groupedResults,
        });
      }
      return event;
    }

    return await this.runSearchStream(
      'start_search_file_contents_stream',
      'content',
      {
        rootPath,
        pattern,
        searchId: effectiveSearchId,
        caseSensitive,
        useRegex,
        wholeWord,
        maxResults,
      },
      callbacks,
      effectiveSignal
    );
  }

  async getSearchRepoStatus(rootPath: string): Promise<WorkspaceSearchIndexStatus> {
    const request: SearchRepoIndexRequest = { rootPath };
    try {
      const raw = await api.invoke<WorkspaceSearchIndexStatusRaw>('search_get_repo_status', { request });
      return mapWorkspaceSearchIndexStatus(raw);
    } catch (error) {
      throw createTauriCommandError('search_get_repo_status', error, { rootPath });
    }
  }

  async buildSearchIndex(rootPath: string): Promise<WorkspaceSearchIndexTaskHandle> {
    const request: SearchRepoIndexRequest = { rootPath };
    try {
      const raw = await api.invoke<WorkspaceSearchIndexTaskHandleRaw>('search_build_index', { request });
      return mapWorkspaceSearchIndexTaskHandle(raw);
    } catch (error) {
      throw createTauriCommandError('search_build_index', error, { rootPath });
    }
  }

  async rebuildSearchIndex(rootPath: string): Promise<WorkspaceSearchIndexTaskHandle> {
    const request: SearchRepoIndexRequest = { rootPath };
    try {
      const raw = await api.invoke<WorkspaceSearchIndexTaskHandleRaw>('search_rebuild_index', { request });
      return mapWorkspaceSearchIndexTaskHandle(raw);
    } catch (error) {
      throw createTauriCommandError('search_rebuild_index', error, { rootPath });
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
