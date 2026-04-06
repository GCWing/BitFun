import { useCallback, useEffect, useMemo, useState } from 'react';
import { useExplorerController, useExplorerSnapshot } from '@/tools/file-explorer';
import { workspaceAPI } from '@/infrastructure/api';
import type { FileSystemNode, FileSystemOptions } from '../types';

export interface UseFileSystemOptions extends FileSystemOptions {
  rootPath?: string;
  autoLoad?: boolean;
  enableAutoWatch?: boolean;
  enableLazyLoad?: boolean;
}

export interface UseFileSystemReturn {
  fileTree: FileSystemNode[];
  selectedFile?: string;
  expandedFolders: Set<string>;
  loading: boolean;
  error?: string;
  silentRefreshing?: boolean;
  loadingPaths: Set<string>;
  loadFileTree: (path?: string, silent?: boolean) => Promise<void>;
  loadFileTreeLazy: (path?: string, silent?: boolean) => Promise<void>;
  selectFile: (filePath: string) => void;
  expandFolder: (folderPath: string, expanded?: boolean) => void;
  expandFolderLazy: (folderPath: string) => Promise<void>;
  loadMoreFolder: (folderPath: string) => Promise<void>;
  searchFiles: (query: string) => Promise<FileSystemNode[]>;
  refreshFileTree: () => Promise<void>;
  setFileTree: (tree: FileSystemNode[]) => void;
  updateOptions: (options: Partial<FileSystemOptions>) => void;
}

export function useFileSystem(options: UseFileSystemOptions = {}): UseFileSystemReturn {
  const [optionOverrides, setOptionOverrides] = useState<Partial<FileSystemOptions>>({});

  useEffect(() => {
    setOptionOverrides({});
  }, [options.rootPath]);

  const controllerConfig = useMemo(() => ({
    rootPath: options.rootPath,
    autoLoad: options.autoLoad ?? true,
    enableAutoWatch: options.enableAutoWatch ?? true,
    enableLazyLoad: options.enableLazyLoad ?? true,
    enablePathCompression: optionOverrides.enablePathCompression ?? options.enablePathCompression ?? true,
    showHiddenFiles: optionOverrides.showHiddenFiles ?? options.showHiddenFiles ?? false,
    sortBy: optionOverrides.sortBy ?? options.sortBy ?? 'name',
    sortOrder: optionOverrides.sortOrder ?? options.sortOrder ?? 'asc',
    maxDepth: optionOverrides.maxDepth ?? options.maxDepth,
    excludePatterns: optionOverrides.excludePatterns ?? options.excludePatterns ?? [],
  }), [
    options.rootPath,
    options.autoLoad,
    options.enableAutoWatch,
    options.enableLazyLoad,
    options.enablePathCompression,
    options.showHiddenFiles,
    options.sortBy,
    options.sortOrder,
    options.maxDepth,
    options.excludePatterns,
    optionOverrides,
  ]);
  const controller = useExplorerController(controllerConfig);
  const snapshot = useExplorerSnapshot(controller);

  const loadFileTree = useCallback((path?: string, silent = false) => {
    return controller.loadFileTree(path, silent);
  }, [controller]);

  const loadFileTreeLazy = useCallback((path?: string, silent = false) => {
    return controller.loadFileTreeLazy(path, silent);
  }, [controller]);

  const selectFile = useCallback((filePath: string) => {
    controller.selectFile(filePath);
  }, [controller]);

  const expandFolder = useCallback((folderPath: string, expanded?: boolean) => {
    controller.expandFolder(folderPath, expanded);
  }, [controller]);

  const expandFolderLazy = useCallback((folderPath: string) => {
    return controller.expandFolderLazy(folderPath);
  }, [controller]);

  const loadMoreFolder = useCallback((folderPath: string) => {
    return controller.loadMoreFolder(folderPath);
  }, [controller]);

  const refreshFileTree = useCallback(() => {
    return controller.loadFileTree(undefined, false);
  }, [controller]);

  const searchFiles = useCallback(async (query: string) => {
    const rootPath = controllerConfig.rootPath;
    if (!rootPath || !query.trim()) {
      return [];
    }

    const results = await workspaceAPI.searchFilenamesOnly(rootPath, query.trim());
    return results.map((result) => ({
      path: result.path,
      name: result.name,
      isDirectory: result.isDirectory,
    }));
  }, [controllerConfig.rootPath]);

  const setFileTree = useCallback((_tree: FileSystemNode[]) => {
    controller.replaceTree(_tree);
  }, [controller]);

  const updateOptions = useCallback((nextOptions: Partial<FileSystemOptions>) => {
    setOptionOverrides(prev => ({
      ...prev,
      ...nextOptions,
      excludePatterns: nextOptions.excludePatterns ?? prev.excludePatterns,
    }));
  }, []);

  return {
    fileTree: snapshot.fileTree,
    selectedFile: snapshot.selectedFile,
    expandedFolders: snapshot.expandedFolders,
    loading: snapshot.loading,
    error: snapshot.error,
    silentRefreshing: snapshot.silentRefreshing,
    loadingPaths: snapshot.loadingPaths,
    loadFileTree,
    loadFileTreeLazy,
    selectFile,
    expandFolder,
    expandFolderLazy,
    loadMoreFolder,
    searchFiles,
    refreshFileTree,
    setFileTree,
    updateOptions,
  };
}
