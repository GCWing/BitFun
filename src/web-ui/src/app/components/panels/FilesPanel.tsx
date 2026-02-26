/**
 * Files panel component
 * Displays the file explorer for the current workspace
 */

import React, { useState, useCallback, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Search as SearchIcon, CaseSensitive, Regex, WholeWord, List, Loader2 } from 'lucide-react';
import { FileExplorer, useFileSystem } from '@/tools/file-system';
import { Search, IconButton, Tooltip } from '@/component-library';
import { useFileTreeGitSync } from '@/tools/file-system/hooks/useFileTreeGitSync';
import { FileSearchResults } from '@/tools/file-system/components/FileSearchResults';
import { useFileSearch } from '@/hooks';
import { workspaceAPI } from '@/infrastructure/api';
import type { FileSystemNode } from '@/tools/file-system/types';
import { globalEventBus } from '@/infrastructure/event-bus';
import { useNotification } from '@/shared/notification-system';
import { InputDialog, CubeLoading } from '@/component-library';
import { openFileInBestTarget } from '@/shared/utils/tabUtils';
import { PanelHeader } from './base';
import { createLogger } from '@/shared/utils/logger';
import '@/tools/file-system/styles/FileExplorer.scss';
import './FilesPanel.scss';

const log = createLogger('FilesPanel');

interface FilesPanelProps {
  workspacePath?: string;
  onFileSelect?: (filePath: string, fileName: string) => void;
  onFileDoubleClick?: (filePath: string) => void;
  hideHeader?: boolean;
  viewMode?: 'tree' | 'search';
  onViewModeChange?: (mode: 'tree' | 'search') => void;
}

const FilesPanel: React.FC<FilesPanelProps> = ({
  workspacePath,
  onFileSelect,
  onFileDoubleClick,
  hideHeader = false,
  viewMode: externalViewMode,
  onViewModeChange,
}) => {
  const { t } = useTranslation('panels/files');
  
  const panelRef = useRef<HTMLDivElement>(null);
  const [internalViewMode, setInternalViewMode] = useState<'tree' | 'search'>('tree');
  const viewMode = externalViewMode !== undefined ? externalViewMode : internalViewMode;
  
  const {
    query: searchQuery,
    setQuery: setSearchQuery,
    allResults: searchResults,
    searchPhase,
    isSearching,
    error: searchError,
    searchOptions,
    setSearchOptions,
    clearSearch,
  } = useFileSearch({
    workspacePath,
    enableContentSearch: true,
    contentSearchDebounce: 150,
    minSearchLength: 1,
  });

  const [renamingPath, setRenamingPath] = useState<string | null>(null);
  const [inputDialog, setInputDialog] = useState<{
    isOpen: boolean;
    type: 'newFile' | 'newFolder' | null;
    parentPath: string;
  }>({
    isOpen: false,
    type: null,
    parentPath: '',
  });

  const notification = useNotification();

  const {
    fileTree,
    selectedFile,
    expandedFolders,
    loading,
    silentRefreshing,
    error,
    loadFileTree,
    selectFile,
    expandFolder,
    expandFolderLazy,
    setFileTree
  } = useFileSystem({
    rootPath: workspacePath,
    autoLoad: true,
    enablePathCompression: true,
    showHiddenFiles: false,
    enableAutoWatch: true,
    enableLazyLoad: true
  });

  const handleTreeUpdate = useCallback((updatedTree: FileSystemNode[]) => {
    log.debug('File tree updated', { nodeCount: updatedTree.length });
    setFileTree(updatedTree);
  }, [setFileTree]);

  const handleNodeExpandLazy = useCallback((path: string, expanded: boolean) => {
    expandFolderLazy(path);
  }, [expandFolderLazy]);

  useFileTreeGitSync({
    workspacePath,
    fileTree,
    onTreeUpdate: handleTreeUpdate,
    autoRefresh: true
  });

  const prevWorkspacePathRef = useRef<string | undefined>(workspacePath);
  useEffect(() => {
    if (prevWorkspacePathRef.current !== undefined && prevWorkspacePathRef.current !== workspacePath) {
      log.debug('Workspace path changed, clearing local state', {
        from: prevWorkspacePathRef.current,
        to: workspacePath
      });
      
      clearSearch();
      setRenamingPath(null);
      setInputDialog({
        isOpen: false,
        type: null,
        parentPath: '',
      });
      if (onViewModeChange) {
        onViewModeChange('tree');
      } else {
        setInternalViewMode('tree');
      }
    }
    prevWorkspacePathRef.current = workspacePath;
  }, [workspacePath, clearSearch]);

  // ===== File Operation Handlers =====
  
  const handleOpenFile = useCallback((data: { path: string; line?: number; column?: number }) => {
    log.info('Opening file', { path: data.path, line: data.line, column: data.column });

    openFileInBestTarget({
      filePath: data.path,
      workspacePath,
      ...(data.line ? { jumpToLine: data.line } : {}),
      ...(data.column ? { jumpToColumn: data.column } : {}),
    });
  }, [workspacePath]);

  const handleNewFile = useCallback((data: { parentPath: string }) => {
    setInputDialog({
      isOpen: true,
      type: 'newFile',
      parentPath: data.parentPath,
    });
  }, []);

  const handleInputDialogClose = useCallback(() => {
    setInputDialog({
      isOpen: false,
      type: null,
      parentPath: '',
    });
  }, []);

  const handleConfirmNewFile = useCallback(async (fileName: string) => {
    const filePath = `${inputDialog.parentPath}${inputDialog.parentPath.endsWith('/') ? '' : '/'}${fileName}`;
    
    try {
      await workspaceAPI.createFile(filePath);
      log.info('File created', { path: filePath });
      handleInputDialogClose();
      loadFileTree(workspacePath || '', true);
    } catch (error) {
      log.error('Failed to create file', error);
      notification.error(t('notifications.createFileFailed', { error: String(error) }));
    }
  }, [inputDialog.parentPath, workspacePath, loadFileTree, notification, t, handleInputDialogClose]);

  const handleNewFolder = useCallback((data: { parentPath: string }) => {
    setInputDialog({
      isOpen: true,
      type: 'newFolder',
      parentPath: data.parentPath,
    });
  }, []);

  const handleConfirmNewFolder = useCallback(async (folderName: string) => {
    const folderPath = `${inputDialog.parentPath}${inputDialog.parentPath.endsWith('/') ? '' : '/'}${folderName}`;
    
    try {
      await workspaceAPI.createDirectory(folderPath);
      log.info('Directory created', { path: folderPath });
      handleInputDialogClose();
      loadFileTree(workspacePath || '', true);
    } catch (error) {
      log.error('Failed to create directory', error);
      notification.error(t('notifications.createFolderFailed', { error: String(error) }));
    }
  }, [inputDialog.parentPath, workspacePath, loadFileTree, notification, t, handleInputDialogClose]);

  const handleInputDialogConfirm = useCallback((value: string) => {
    if (inputDialog.type === 'newFile') {
      handleConfirmNewFile(value);
    } else if (inputDialog.type === 'newFolder') {
      handleConfirmNewFolder(value);
    }
  }, [inputDialog.type, handleConfirmNewFile, handleConfirmNewFolder]);

  const handleStartRename = useCallback((data: { path: string; name: string }) => {
    setRenamingPath(data.path);
  }, []);

  const handleExecuteRename = useCallback(async (oldPath: string, newName: string) => {
    const isWindows = oldPath.includes('\\');
    const separator = isWindows ? '\\' : '/';
    const pathParts = oldPath.split(separator);
    const oldName = pathParts[pathParts.length - 1];
    
    if (newName === oldName) {
      setRenamingPath(null);
      return;
    }
    
    pathParts[pathParts.length - 1] = newName;
    const newPath = pathParts.join(separator);
    
    try {
      await workspaceAPI.renameFile(oldPath, newPath);
      log.info('File renamed', { oldPath, newPath });
      setRenamingPath(null);
      loadFileTree(workspacePath || '', true);
    } catch (error) {
      log.error('Failed to rename file', error);
      notification.error(t('notifications.renameFailed', { error: String(error) }));
      setRenamingPath(null);
    }
  }, [workspacePath, loadFileTree, notification, t]);

  const handleCancelRename = useCallback(() => {
    setRenamingPath(null);
  }, []);

  const handleDelete = useCallback(async (data: { path: string; isDirectory: boolean }) => {
    try {
      if (data.isDirectory) {
        await workspaceAPI.deleteDirectory(data.path);
      } else {
        await workspaceAPI.deleteFile(data.path);
      }
      log.info('File deleted', { path: data.path, isDirectory: data.isDirectory });
      loadFileTree(workspacePath || '', true);
    } catch (error) {
      log.error('Failed to delete file', error);
      notification.error(t('notifications.deleteFailed', { error: String(error) }));
    }
  }, [workspacePath, loadFileTree, notification, t]);

  const handleReveal = useCallback(async (data: { path: string }) => {
    try {
      await workspaceAPI.revealInExplorer(data.path);
    } catch (error) {
      log.error('Failed to reveal in explorer', error);
      notification.error(t('notifications.openExplorerFailed', { error: String(error) }));
    }
  }, [notification, t]);

  const handleFileTreeRefresh = useCallback(() => {
    loadFileTree(undefined, true);
  }, [loadFileTree]);

  const handleNavigateToPath = useCallback((data: { path: string; scrollIntoView?: boolean }) => {
    if (!data.path || !workspacePath) {
      return;
    }
    
    log.debug('Navigating to path', { path: data.path, scrollIntoView: data.scrollIntoView });
    
    const normalizedTarget = data.path.replace(/\\/g, '/');
    const normalizedWorkspace = workspacePath.replace(/\\/g, '/');
    
    let relativePath = normalizedTarget;
    if (normalizedTarget.toLowerCase().startsWith(normalizedWorkspace.toLowerCase())) {
      relativePath = normalizedTarget.slice(normalizedWorkspace.length).replace(/^\//, '');
    }
    
    const parts = relativePath.split('/').filter(Boolean);
    let currentPath = normalizedWorkspace;
    const isWindowsPath = workspacePath.includes('\\');
    
    const targetPaths = new Set<string>();
    targetPaths.add(isWindowsPath ? normalizedWorkspace.replace(/\//g, '\\') : normalizedWorkspace);
    
    let finalExpandPath = '';
    const pathsToExpand: string[] = [];
    for (const part of parts) {
      currentPath = `${currentPath}/${part}`;
      const expandPath = isWindowsPath ? currentPath.replace(/\//g, '\\') : currentPath;
      finalExpandPath = expandPath;
      targetPaths.add(expandPath);
      pathsToExpand.push(expandPath);
    }
    
    expandedFolders.forEach(folderPath => {
      if (!targetPaths.has(folderPath)) {
        expandFolder(folderPath, false);
      }
    });
    
    for (const expandPath of pathsToExpand) {
      expandFolder(expandPath, true);
    }
    
    if (data.scrollIntoView && finalExpandPath) {
      setTimeout(() => {
        const escapedPath = finalExpandPath.replace(/\\/g, '\\\\');
        const targetElement = document.querySelector(`[data-file-path="${escapedPath}"]`);
        if (targetElement) {
          targetElement.scrollIntoView({ behavior: 'smooth', block: 'center' });
          targetElement.classList.add('bitfun-file-explorer__node-content--highlighted');
          setTimeout(() => {
            targetElement.classList.remove('bitfun-file-explorer__node-content--highlighted');
          }, 2000);
        }
      }, 300);
    }
  }, [workspacePath, expandFolder, expandedFolders]);

  const getParentDirectory = useCallback((filePath: string): string => {
    const isWindows = filePath.includes('\\');
    const separator = isWindows ? '\\' : '/';
    const parts = filePath.split(separator);
    parts.pop();
    return parts.join(separator);
  }, []);

  const findNode = useCallback((nodes: FileSystemNode[], path: string): FileSystemNode | null => {
    for (const node of nodes) {
      if (node.path === path) return node;
      if (node.children) {
        const found = findNode(node.children, path);
        if (found) return found;
      }
    }
    return null;
  }, []);

  const executePaste = useCallback(async (targetDir?: string) => {
    if (!workspacePath) {
      notification.warning(t('notifications.selectWorkspaceFirst'));
      return;
    }

    try {
      const { files, isCut } = await workspaceAPI.getClipboardFiles();
      
      if (files.length === 0) {
        notification.info(t('notifications.pasteNoFiles'));
        return;
      }

      let targetDirectory = targetDir || workspacePath;
      
      if (!targetDir && selectedFile) {
        const selectedNode = findNode(fileTree, selectedFile);
        if (selectedNode) {
          if (selectedNode.isDirectory) {
            targetDirectory = selectedFile;
          } else {
            targetDirectory = getParentDirectory(selectedFile);
          }
        }
      }

      notification.info(t('notifications.pastingFiles', { count: files.length, target: targetDirectory.split(/[/\\]/).pop() }));
      
      const result = await workspaceAPI.pasteFiles(files, targetDirectory, isCut);
      
      if (result.successCount > 0) {
        notification.success(t('notifications.pasteSuccess', { count: result.successCount }));
        loadFileTree(undefined, true);
        
        if (targetDirectory !== workspacePath) {
          expandFolder(targetDirectory, true);
        }
      }
      
      if (result.failedFiles.length > 0) {
        const failedNames = result.failedFiles.map(f => {
          const name = f.path.split(/[/\\]/).pop() || f.path;
          return `${name}: ${f.error}`;
        }).join('\n');
        notification.error(t('notifications.pasteFailed', { count: result.failedFiles.length }) + `:\n${failedNames}`, { duration: 5000 });
      }
      
    } catch (error) {
      log.error('Failed to paste files', error);
      notification.error(t('notifications.pasteFailed', { count: 1 }));
    }
  }, [workspacePath, selectedFile, fileTree, notification, loadFileTree, expandFolder, findNode, getParentDirectory, t]);

  const handlePasteFromContextMenu = useCallback((data: { targetDirectory: string }) => {
    executePaste(data.targetDirectory);
  }, [executePaste]);

  const handlePasteFromKeyboard = useCallback(() => {
    executePaste();
  }, [executePaste]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!panelRef.current?.contains(document.activeElement) && 
          !panelRef.current?.contains(e.target as Node)) {
        return;
      }

      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key === 'v') {
        e.preventDefault();
        e.stopPropagation();
        handlePasteFromKeyboard();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [handlePasteFromKeyboard]);

  useEffect(() => {
    globalEventBus.on('file:open', handleOpenFile);
    globalEventBus.on('file:new-file', handleNewFile);
    globalEventBus.on('file:new-folder', handleNewFolder);
    globalEventBus.on('file:rename', handleStartRename);
    globalEventBus.on('file:delete', handleDelete);
    globalEventBus.on('file:reveal', handleReveal);
    globalEventBus.on('file:paste', handlePasteFromContextMenu);
    globalEventBus.on('file-tree:refresh', handleFileTreeRefresh);
    globalEventBus.on('file-explorer:navigate', handleNavigateToPath);

    return () => {
      globalEventBus.off('file:open', handleOpenFile);
      globalEventBus.off('file:new-file', handleNewFile);
      globalEventBus.off('file:new-folder', handleNewFolder);
      globalEventBus.off('file:rename', handleStartRename);
      globalEventBus.off('file:delete', handleDelete);
      globalEventBus.off('file:reveal', handleReveal);
      globalEventBus.off('file:paste', handlePasteFromContextMenu);
      globalEventBus.off('file-tree:refresh', handleFileTreeRefresh);
      globalEventBus.off('file-explorer:navigate', handleNavigateToPath);
    };
  }, [handleOpenFile, handleNewFile, handleNewFolder, handleStartRename, handleDelete, handleReveal, handlePasteFromContextMenu, handleFileTreeRefresh, handleNavigateToPath]);

  const handleFileSelect = useCallback((filePath: string, fileName: string) => {
    selectFile(filePath);
    onFileSelect?.(filePath, fileName);
    
    const selectedNode = findNode(fileTree, filePath);
    if (selectedNode && !selectedNode.isDirectory) {
      openFileInBestTarget({
        filePath,
        fileName,
        workspacePath,
      }, { source: 'project-nav' });
    }
  }, [selectFile, onFileSelect, workspacePath, fileTree, findNode]);

  const handleFileDoubleClick = useCallback((filePath: string) => {
    onFileDoubleClick?.(filePath);
  }, [onFileDoubleClick]);

  const handleClearSearch = useCallback(() => {
    clearSearch();
  }, [clearSearch]);

  const handleToggleViewMode = useCallback(() => {
    const next = viewMode === 'tree' ? 'search' : 'tree';
    if (onViewModeChange) {
      onViewModeChange(next);
    } else {
      setInternalViewMode(next);
    }
  }, [viewMode, onViewModeChange]);

  return (
    <div 
      ref={panelRef}
      className="bitfun-files-panel"
      tabIndex={-1}
      onFocus={() => {}}
    >
      {!hideHeader && (
        <PanelHeader
          title={t('title')}
          className="bitfun-files-panel__header"
          actions={
            workspacePath && (
              <IconButton
                size="xs"
                onClick={handleToggleViewMode}
                tooltip={viewMode === 'tree' ? t('actions.switchToSearch') : t('actions.switchToTree')}
                tooltipPlacement="bottom"
              >
                {viewMode === 'tree' ? <SearchIcon size={14} /> : <List size={14} />}
              </IconButton>
            )
          }
        />
      )}
      
      <div className="bitfun-files-panel__content">
        {workspacePath && viewMode === 'search' && (
          <div className="bitfun-files-panel__search">
            <Search
              placeholder={t('search.placeholder')}
              value={searchQuery}
              onChange={(val) => setSearchQuery(val)}
              onClear={handleClearSearch}
              clearable
              size="small"
              loading={isSearching}
              suffixContent={
                <div className="bitfun-files-panel__search-options">
                  <Tooltip content={t('options.caseSensitive')}>
                    <button
                      className={`bitfun-files-panel__search-option ${searchOptions.caseSensitive ? 'active' : ''}`}
                      onClick={() => setSearchOptions(prev => ({ ...prev, caseSensitive: !prev.caseSensitive }))}
                    >
                      <CaseSensitive size={14} />
                    </button>
                  </Tooltip>
                  <Tooltip content={t('options.wholeWord')}>
                    <button
                      className={`bitfun-files-panel__search-option ${searchOptions.wholeWord ? 'active' : ''}`}
                      onClick={() => setSearchOptions(prev => ({ ...prev, wholeWord: !prev.wholeWord }))}
                    >
                      <WholeWord size={14} />
                    </button>
                  </Tooltip>
                  <Tooltip content={t('options.useRegex')}>
                    <button
                      className={`bitfun-files-panel__search-option ${searchOptions.useRegex ? 'active' : ''}`}
                      onClick={() => setSearchOptions(prev => ({ ...prev, useRegex: !prev.useRegex }))}
                    >
                      <Regex size={14} />
                    </button>
                  </Tooltip>
                </div>
              }
            />
          </div>
        )}

        <div className="bitfun-files-panel__main-content">
        {!workspacePath ? (
          <div className="bitfun-files-panel__placeholder">
            <div className="bitfun-files-panel__placeholder-icon">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                <polyline points="14,2 14,8 20,8"/>
                <line x1="16" y1="13" x2="8" y2="13"/>
                <line x1="16" y1="17" x2="8" y2="17"/>
                <polyline points="10,9 9,9 8,9"/>
              </svg>
            </div>
            <p>{t('empty.selectWorkspace')}</p>
          </div>
        ) : viewMode === 'search' ? (
          searchQuery ? (
            <div className="bitfun-files-panel__search-content">
              {isSearching && (
                <div className="bitfun-files-panel__search-status">
                  <Loader2 size={14} className="bitfun-files-panel__search-spinner" />
                  <span>
                    {!searchPhase.filenameComplete ? t('status.searchingFilename') : t('status.searchingContent')}
                  </span>
                </div>
              )}
              
              {searchError && (
                <div className="bitfun-files-panel__error">
                  <p>❌ {searchError}</p>
                  <button 
                    className="bitfun-files-panel__retry-button"
                    onClick={() => setSearchQuery(searchQuery)}
                  >
                    {t('actions.retry')}
                  </button>
                </div>
              )}
              
              {searchResults.length > 0 ? (
                <FileSearchResults
                  results={searchResults}
                  searchQuery={searchQuery}
                  onFileSelect={handleFileSelect}
                  className="bitfun-files-panel__search-results"
                />
              ) : (
                !isSearching && !searchError && (
                  <div className="bitfun-files-panel__placeholder">
                    <div className="bitfun-files-panel__placeholder-icon">
                      <SearchIcon size={32} />
                    </div>
                    <p>{t('search.noResults')}</p>
                  </div>
                )
              )}
            </div>
          ) : (
            <div className="bitfun-files-panel__placeholder">
              <div className="bitfun-files-panel__placeholder-icon">
                <SearchIcon size={32} />
              </div>
              <p>{t('search.enterKeyword')}</p>
            </div>
          )
        ) : (
          loading ? (
            <div className="bitfun-files-panel__loading">
              <CubeLoading size="medium" text={t('status.loadingFileTree')} />
            </div>
          ) : error ? (
            <div className="bitfun-files-panel__error">
              <p>❌ {error}</p>
              <button 
                className="bitfun-files-panel__retry-button"
                onClick={() => loadFileTree()}
              >
                {t('actions.retry')}
              </button>
            </div>
          ) : (
            <FileExplorer
              key={workspacePath || 'no-workspace'}
              fileTree={fileTree}
              selectedFile={selectedFile}
              expandedFolders={expandedFolders}
              onNodeExpand={handleNodeExpandLazy}
              onFileSelect={handleFileSelect}
              onFileDoubleClick={handleFileDoubleClick}
              className="bitfun-files-panel__explorer"
              enablePathCompression={true}
              renamingPath={renamingPath}
              onRename={handleExecuteRename}
              onCancelRename={handleCancelRename}
              workspacePath={workspacePath}
              onNewFile={handleNewFile}
              onNewFolder={handleNewFolder}
              onRefresh={() => loadFileTree(workspacePath || '', false)}
            />
          )
        )}
        </div>
      </div>

      <InputDialog
        isOpen={inputDialog.isOpen}
        onClose={handleInputDialogClose}
        onConfirm={handleInputDialogConfirm}
        title={inputDialog.type === 'newFile' ? t('dialog.newFile.title') : t('dialog.newFolder.title')}
        placeholder={inputDialog.type === 'newFile' ? t('dialog.newFile.placeholder') : t('dialog.newFolder.placeholder')}
        confirmText={inputDialog.type === 'newFile' ? t('dialog.newFile.confirm') : t('dialog.newFolder.confirm')}
        cancelText={inputDialog.type === 'newFile' ? t('dialog.newFile.cancel') : t('dialog.newFolder.cancel')}
        validator={(value) => {
          if (!/^[^<>:"/\\|?*\x00-\x1F]+$/.test(value)) {
            return t('validation.invalidFilename');
          }
          return null;
        }}
      />
    </div>
  );
};

export default FilesPanel;