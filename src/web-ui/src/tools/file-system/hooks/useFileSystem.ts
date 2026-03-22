import { useState, useEffect, useCallback, useRef } from 'react';
import { FileSystemNode, FileSystemState, FileSystemOptions } from '../types';
import { fileSystemService } from '../services/FileSystemService';
import { directoryCache } from '../services/DirectoryCache';
import { createLogger } from '@/shared/utils/logger';
import { useI18n } from '@/infrastructure/i18n';
import { globalEventBus } from '@/infrastructure/event-bus';

const log = createLogger('useFileSystem');

const EMPTY_FILE_TREE: FileSystemNode[] = [];

function findNodeByPath(nodes: FileSystemNode[], targetPath: string): FileSystemNode | undefined {
  for (const node of nodes) {
    if (node.path === targetPath) return node;
    if (node.children) {
      const found = findNodeByPath(node.children, targetPath);
      if (found) return found;
    }
  }
  return undefined;
}

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
  searchFiles: (query: string) => void;
  refreshFileTree: () => Promise<void>;
  setFileTree: (tree: FileSystemNode[]) => void;
  updateOptions: (options: Partial<FileSystemOptions>) => void;
}

export function useFileSystem(options: UseFileSystemOptions = {}): UseFileSystemReturn {
  const { t } = useI18n('tools');
  const {
    rootPath,
    autoLoad = true,
    enableAutoWatch = true,
    enableLazyLoad = true,
    enablePathCompression = true,
    showHiddenFiles = false,
    sortBy = 'name',
    sortOrder = 'asc',
    maxDepth,
    excludePatterns = []
  } = options;

  const [state, setState] = useState<FileSystemState>({
    fileTree: [],
    expandedFolders: new Set(),
    loading: false,
    silentRefreshing: false,
    options: {
      enablePathCompression,
      showHiddenFiles,
      sortBy,
      sortOrder,
      maxDepth,
      excludePatterns
    }
  });

  const [loadingPaths, setLoadingPaths] = useState<Set<string>>(new Set());
  const loadedPathsRef = useRef<Set<string>>(new Set());

  const abortControllerRef = useRef<AbortController | null>(null);
  const rootPathRef = useRef<string | undefined>(rootPath);
  const isLoadingRef = useRef(false);
  const optionsRef = useRef(state.options);
  
  useEffect(() => {
    optionsRef.current = state.options;
  }, [state.options]);

  const loadFileTreeLazy = useCallback(async (path?: string, silent = false) => {
    const targetPath = path || rootPath;
    if (!targetPath) return;
    
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    
    const controller = new AbortController();
    abortControllerRef.current = controller;
    
    isLoadingRef.current = true;
    if (silent) {
      setState(prev => ({ ...prev, silentRefreshing: true }));
    } else {
      setState(prev => ({ ...prev, loading: true, error: undefined }));
    }

    try {
      const children = await fileSystemService.getDirectoryChildren(targetPath);
      
      if (controller.signal.aborted) {
        return;
      }
      
      const rootName = targetPath.split(/[/\\]/).filter(Boolean).pop() || targetPath;
      const rootNode: FileSystemNode = {
        path: targetPath,
        name: rootName,
        isDirectory: true,
        children: children.map(child => ({
          ...child,
          children: child.isDirectory ? undefined : undefined,
        })),
      };
      
      const fileTree = [rootNode];
      
      if (rootPathRef.current !== targetPath) {
        return;
      }
      
      directoryCache.set(targetPath, children);
      loadedPathsRef.current.add(targetPath);
      
      setState(prev => {
        const newExpandedFolders = new Set(prev.expandedFolders);
        newExpandedFolders.add(targetPath);
        
        return {
          ...prev,
          fileTree,
          expandedFolders: newExpandedFolders,
          loading: false,
          silentRefreshing: false
        };
      });
      
      if (silent) {
        isLoadingRef.current = false;
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('file-tree:silent-refresh-completed', { 
          path: targetPath,
          fileTree: fileTree
        });
      }
    } catch (error) {
      if (controller.signal.aborted) {
        return;
      }
      
      const errorMessage = error instanceof Error
        ? t('fileTree.errors.loadTreeFailedWithMessage', { message: error.message })
        : t('fileTree.errors.loadTreeFailed');
      if (silent) {
        log.warn('Lazy load silent refresh failed', { path: targetPath, error });
        setState(prev => ({ ...prev, silentRefreshing: false }));
      } else {
        setState(prev => ({
          ...prev,
          loading: false,
          error: errorMessage
        }));
      }
    } finally {
      isLoadingRef.current = false;
    }
  }, [rootPath, t]);

  const loadFileTree = useCallback(async (path?: string, silent = false) => {
    const targetPath = path || rootPath;
    if (!targetPath) return;

    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    
    const controller = new AbortController();
    abortControllerRef.current = controller;
    
    isLoadingRef.current = true;
    if (silent) {
      setState(prev => ({ ...prev, silentRefreshing: true }));
    } else {
      setState(prev => ({ ...prev, loading: true, error: undefined }));
    }

    try {
      const fileTree = await fileSystemService.loadFileTree(targetPath, optionsRef.current);
      
      if (controller.signal.aborted) {
        return;
      }
      
      if (rootPathRef.current !== targetPath) {
        return;
      }
      
      setState(prev => {
        const newExpandedFolders = new Set(prev.expandedFolders);
        if (fileTree.length > 0 && fileTree[0].isDirectory) {
          newExpandedFolders.add(fileTree[0].path);
        }
        
        return {
          ...prev,
          fileTree,
          expandedFolders: newExpandedFolders,
          loading: false,
          silentRefreshing: false
        };
      });
      
      if (silent) {
        isLoadingRef.current = false;
        
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('file-tree:silent-refresh-completed', { 
          path: targetPath,
          fileTree: fileTree
        });
      }
    } catch (error) {
      if (controller.signal.aborted) {
        return;
      }
      
      const errorMessage = error instanceof Error
        ? t('fileTree.errors.loadTreeFailedWithMessage', { message: error.message })
        : t('fileTree.errors.loadTreeFailed');
      if (silent) {
        log.warn('Silent refresh failed', { path: targetPath, error });
        setState(prev => ({ ...prev, silentRefreshing: false }));
      } else {
        log.error('Failed to load file tree', { path: targetPath, error });
        setState(prev => ({
          ...prev,
          loading: false,
          error: errorMessage
        }));
      }
    } finally {
      isLoadingRef.current = false;
    }
  }, [rootPath, t]);

  const selectFile = useCallback((filePath: string) => {
    setState(prev => ({
      ...prev,
      selectedFile: filePath
    }));
  }, []);

  const expandFolder = useCallback((folderPath: string, expanded?: boolean) => {
    setState(prev => {
      const newExpandedFolders = new Set(prev.expandedFolders);
      const shouldExpand = expanded !== undefined ? expanded : !newExpandedFolders.has(folderPath);
      
      if (shouldExpand) {
        newExpandedFolders.add(folderPath);
      } else {
        newExpandedFolders.delete(folderPath);
      }

      return {
        ...prev,
        expandedFolders: newExpandedFolders
      };
    });
  }, []);

  const updateNodeChildrenInTree = useCallback((
    nodes: FileSystemNode[],
    targetPath: string,
    children: FileSystemNode[]
  ): FileSystemNode[] => {
    return nodes.map(node => {
      if (node.path === targetPath) {
        return {
          ...node,
          children: children,
        };
      }
      
      if (node.children) {
        const updatedChildren = updateNodeChildrenInTree(node.children, targetPath, children);
        if (updatedChildren !== node.children) {
          return {
            ...node,
            children: updatedChildren,
          };
        }
      }
      
      return node;
    });
  }, []);

  const refreshDirectoryInTree = useCallback(async (dirPath: string) => {
    try {
      const newChildren = await fileSystemService.getDirectoryChildren(dirPath);
      directoryCache.set(dirPath, newChildren);
      loadedPathsRef.current.add(dirPath);

      setState(prev => {
        const mergedChildren = newChildren.map(newChild => {
          if (!newChild.isDirectory) return newChild;
          const existingChild = findNodeByPath(prev.fileTree, newChild.path);
          if (existingChild?.children) {
            return { ...newChild, children: existingChild.children };
          }
          return newChild;
        });

        return {
          ...prev,
          fileTree: updateNodeChildrenInTree(prev.fileTree, dirPath, mergedChildren)
        };
      });
    } catch (error) {
      log.warn('Failed to refresh directory after file change', { dirPath, error });
    }
  }, [updateNodeChildrenInTree]);

  const expandFolderLazy = useCallback(async (folderPath: string) => {
    if (state.expandedFolders.has(folderPath)) {
      setState(prev => {
        const newExpandedFolders = new Set(prev.expandedFolders);
        newExpandedFolders.delete(folderPath);
        return {
          ...prev,
          expandedFolders: newExpandedFolders
        };
      });
      return;
    }

    const cachedChildren = directoryCache.get(folderPath);
    const needsLoading = !loadedPathsRef.current.has(folderPath) && !cachedChildren;

    setState(prev => {
      const newExpandedFolders = new Set(prev.expandedFolders);
      newExpandedFolders.add(folderPath);
      return {
        ...prev,
        expandedFolders: newExpandedFolders
      };
    });

    if (cachedChildren) {
      setState(prev => ({
        ...prev,
        fileTree: updateNodeChildrenInTree(prev.fileTree, folderPath, cachedChildren)
      }));
      loadedPathsRef.current.add(folderPath);
      return;
    }

    if (!needsLoading) {
      return;
    }

    setLoadingPaths(prev => {
      const newSet = new Set(prev);
      newSet.add(folderPath);
      return newSet;
    });

    try {
      const children = await fileSystemService.getDirectoryChildren(folderPath);
      
      directoryCache.set(folderPath, children);
      
      setState(prev => ({
        ...prev,
        fileTree: updateNodeChildrenInTree(prev.fileTree, folderPath, children)
      }));

      loadedPathsRef.current.add(folderPath);
    } catch (error) {
      log.error('Failed to load directory', { folderPath, error });
      // Revert expanded state after load failure.
      setState(prev => {
        const newExpandedFolders = new Set(prev.expandedFolders);
        newExpandedFolders.delete(folderPath);
        return {
          ...prev,
          expandedFolders: newExpandedFolders
        };
      });
    } finally {
      setLoadingPaths(prev => {
        const newSet = new Set(prev);
        newSet.delete(folderPath);
        return newSet;
      });
    }
  }, [state.expandedFolders, updateNodeChildrenInTree]);

  const searchFiles = useCallback((query: string) => {
    setState(prev => ({
      ...prev,
      searchQuery: query
    }));
  }, []);

  const refreshFileTree = useCallback(async () => {
    await loadFileTree();
  }, [loadFileTree]);

  const updateOptions = useCallback((newOptions: Partial<FileSystemOptions>) => {
    setState(prev => ({
      ...prev,
      options: {
        ...prev.options,
        ...newOptions
      }
    }));
  }, []);

  // Defensive setFileTree to avoid accidental clobbering during async loads.
  const setFileTree = useCallback((tree: FileSystemNode[]) => {
    if (isLoadingRef.current) {
      return;
    }
    
    setState(prev => {
      if (tree.length === 0 && prev.fileTree.length > 0) {
        return prev;
      }
      return { ...prev, fileTree: tree };
    });
  }, []);

  useEffect(() => {
    return () => {
      abortControllerRef.current?.abort();
    };
  }, []);
  
  useEffect(() => {
    if (autoLoad && rootPath) {
      rootPathRef.current = rootPath;
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
        abortControllerRef.current = null;
      }
      isLoadingRef.current = false;

      directoryCache.clear();
      loadedPathsRef.current.clear();

      setState(prev => ({
        ...prev,
        fileTree: [],
        expandedFolders: new Set(),
        selectedFile: undefined,
        error: undefined,
        loading: false,
        silentRefreshing: false
      }));
      
      if (enableLazyLoad) {
        loadFileTreeLazy();
      } else {
        loadFileTree();
      }
    } else if (!rootPath) {
      rootPathRef.current = undefined;
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
        abortControllerRef.current = null;
      }
      isLoadingRef.current = false;
      
      directoryCache.clear();
      loadedPathsRef.current.clear();
      
      setState(prev => ({
        ...prev,
        fileTree: [],
        expandedFolders: new Set(),
        selectedFile: undefined,
        error: undefined,
        loading: false,
        silentRefreshing: false
      }));
    }
  }, [autoLoad, rootPath, enableLazyLoad]);

  useEffect(() => {
    if (rootPath && state.fileTree.length > 0) {
      loadFileTree();
    }
  }, [state.options.showHiddenFiles, state.options.excludePatterns]);

  useEffect(() => {
    if (!enableAutoWatch || !rootPath) {
      return;
    }

    let debounceTimer: ReturnType<typeof setTimeout> | null = null;
    let pendingPaths: string[] = [];

    const handleFileChange = (eventPath: string) => {
      pendingPaths.push(eventPath);
      
      directoryCache.invalidate(eventPath);
      
      loadedPathsRef.current.delete(eventPath);
      const parentPath = eventPath.replace(/\\/g, '/').split('/').slice(0, -1).join('/');
      if (parentPath) {
        loadedPathsRef.current.delete(parentPath);
      }

      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }

      debounceTimer = setTimeout(() => {
        if (pendingPaths.length > 0 && rootPath) {
          if (enableLazyLoad) {
            const affectedParents = new Set<string>();

            pendingPaths.forEach(changedPath => {
              directoryCache.invalidate(changedPath);
              loadedPathsRef.current.delete(changedPath);
              const parentPath = changedPath.replace(/[\\/][^\\/]+$/, '');
              if (parentPath && parentPath !== changedPath) {
                directoryCache.invalidate(parentPath);
                loadedPathsRef.current.delete(parentPath);
                affectedParents.add(parentPath);
              }
            });
            pendingPaths = [];

            for (const parentPath of affectedParents) {
              refreshDirectoryInTree(parentPath);
            }
          } else {
            pendingPaths = [];
            loadFileTree(rootPath, true);
          }
        }
      }, 200);
    };

    const unwatch = fileSystemService.watchFileChanges(rootPath, (event) => {
      handleFileChange(event.path);

      if (
        event.type === 'modified' ||
        event.type === 'created' ||
        event.type === 'renamed'
      ) {
        globalEventBus.emit('editor:file-changed', { filePath: event.path });
      }
    });

    return () => {
      unwatch();
      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
    };
  }, [enableAutoWatch, rootPath, enableLazyLoad, loadFileTree, loadFileTreeLazy, refreshDirectoryInTree]);

  const effectiveFileTree =
    rootPathRef.current === rootPath ? state.fileTree : EMPTY_FILE_TREE;

  return {
    fileTree: effectiveFileTree,
    selectedFile: state.selectedFile,
    expandedFolders: state.expandedFolders,
    loading: state.loading,
    silentRefreshing: state.silentRefreshing,
    error: state.error,
    loadingPaths,
    
    loadFileTree,
    loadFileTreeLazy,
    selectFile,
    expandFolder,
    expandFolderLazy,
    searchFiles,
    refreshFileTree,
    setFileTree,
    updateOptions
  };
}
