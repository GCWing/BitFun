import React, { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { Icon, IconButton, Tooltip } from '@openbitfun/ui';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { FilePlus, FolderPlus } from 'lucide-react';
import { VirtualFileTree } from './VirtualFileTree';
import { FileExplorerProps, FileSystemNode, FlatFileNode } from '../types';
import { flattenFileTree } from '../utils/treeFlattening';
import { getNewItemParentPath } from '../utils/getNewItemParentPath';
import { i18nService, useI18n } from '@/infrastructure/i18n';
import { expandedFoldersContains, pathsEquivalentFs } from '@/shared/utils/pathUtils';

import { filterTreeByPredicate, filterTreeBySearch } from '@/tools/file-explorer';
import { globalEventBus } from '@/infrastructure/event-bus';
import { commandExecutor } from '@/shared/context-menu-system/commands/CommandExecutor';
import { ContextType, type FileNodeContext } from '@/shared/context-menu-system/types/context.types';

function findNodeByPath(nodes: FileSystemNode[], path: string): FileSystemNode | undefined {
  for (const node of nodes) {
    if (pathsEquivalentFs(node.path, path)) {
      return node;
    }
    if (node.children) {
      const found = findNodeByPath(node.children, path);
      if (found) {
        return found;
      }
    }
  }
  return undefined;
}

function buildFileNodeContext(node: FileSystemNode, workspacePath?: string): FileNodeContext {
  return {
    type: node.isDirectory ? ContextType.FOLDER_NODE : ContextType.FILE_NODE,
    event: new MouseEvent('contextmenu'),
    targetElement: document.body,
    position: { x: 0, y: 0 },
    timestamp: Date.now(),
    metadata: {},
    filePath: node.path,
    fileName: node.name,
    isDirectory: node.isDirectory,
    isReadOnly: false,
    workspacePath,
  };
}

export const FileExplorer: React.FC<FileExplorerProps> = ({
  fileTree,
  selectedFile,
  onFileSelect,
  className = '',
  showFileSize = false,
  showLastModified = false,
  searchQuery,
  fileFilter,
  renamingPath,
  onRename,
  onCancelRename,
  expandedFolders: externalExpandedFolders,
  loadingPaths = new Set(),
  onNodeExpand: externalOnNodeExpand,
  workspacePath,
  onNewFile,
  onNewFolder,
  onRefresh,
  hideToolbar = false,
}) => {
  const { t } = useI18n('tools');
  const [internalExpandedFolders, setInternalExpandedFolders] = useState<Set<string>>(new Set());
  
  const expandedFolders = externalExpandedFolders || internalExpandedFolders;

  const emitFileSelect = useCallback((path: string, name: string) => {
    onFileSelect?.(path, name);
  }, [onFileSelect]);

  const setExpandedState = useCallback((path: string, expanded: boolean) => {
    if (externalOnNodeExpand) {
      externalOnNodeExpand(path, expanded);
    } else {
      setInternalExpandedFolders(prev => {
        const newSet = new Set(prev);
        if (expanded) {
          newSet.add(path);
        } else {
          newSet.delete(path);
        }
        return newSet;
      });
    }
  }, [externalOnNodeExpand]);

  const filteredFileTree = useMemo(() => {
    let result = fileTree;

    if (searchQuery && searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      result = filterTreeBySearch(result, query);
    }

    if (fileFilter) {
      result = filterTreeByPredicate(result, fileFilter);
    }

    return result;
  }, [fileTree, searchQuery, fileFilter]);

  const flatNodes = useMemo(() => {
    return flattenFileTree(filteredFileTree, expandedFolders, loadingPaths);
  }, [filteredFileTree, expandedFolders, loadingPaths]);

  // Keep hooks before any early returns (React Hooks rules).
  const containerRef = useRef<HTMLDivElement>(null);
  
  const toggleExpandedState = useCallback((path: string) => {
    const isCurrentlyExpanded = expandedFoldersContains(expandedFolders, path);
    setExpandedState(path, !isCurrentlyExpanded);
  }, [expandedFolders, setExpandedState]);

  const renderNodeContent = useCallback((node: FileSystemNode, _level: number) => {
    return (
      <div className="openbitfun-file-explorer__node-wrapper">
        <span className={`openbitfun-file-explorer__node-name ${node.isCompressed ? 'openbitfun-file-explorer__compressed-path' : ''}`}>
          {node.name}
        </span>
        
        {showFileSize && !node.isDirectory && node.size && (
          <span className="openbitfun-file-explorer__node-size">
            {formatFileSize(node.size)}
          </span>
        )}
        
        {showLastModified && node.lastModified && (
          <span className="openbitfun-file-explorer__node-modified">
            {formatDate(node.lastModified)}
          </span>
        )}
      </div>
    );
  }, [showFileSize, showLastModified]);

  const [isToolbarVisible, setIsToolbarVisible] = useState(false);
  const [isFocused, setIsFocused] = useState(false);
  
  const handleFocus = useCallback(() => {
    setIsFocused(true);
    setIsToolbarVisible(true);
  }, []);
  
  const handleBlur = useCallback((e: React.FocusEvent) => {
    const toolbar = e.currentTarget.querySelector('.openbitfun-file-explorer__toolbar');
    if (toolbar && toolbar.contains(e.relatedTarget as Node)) {
      return;
    }
    setTimeout(() => {
      const container = containerRef.current;
      if (container && !container.contains(document.activeElement)) {
        setIsFocused(false);
        setIsToolbarVisible(false);
      }
    }, 0);
  }, []);
  
  const handleContainerClick = useCallback((e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('.openbitfun-file-explorer__toolbar')) {
      return;
    }
    setIsFocused(true);
    setIsToolbarVisible(true);
  }, []);
  
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    
    const handleClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest('.openbitfun-file-explorer__toolbar')) {
        return;
      }
      setIsFocused(true);
      setIsToolbarVisible(true);
    };
    
    container.addEventListener('click', handleClick, true);
    
    return () => {
      container.removeEventListener('click', handleClick, true);
    };
  }, []);
  
  const handleNewFile = useCallback(() => {
    if (onNewFile) {
      const parentPath = getNewItemParentPath(workspacePath, selectedFile, fileTree);
      if (parentPath) {
        onNewFile({ parentPath });
      }
    }
  }, [onNewFile, workspacePath, selectedFile, fileTree]);
  
  const handleNewFolder = useCallback(() => {
    if (onNewFolder) {
      const parentPath = getNewItemParentPath(workspacePath, selectedFile, fileTree);
      if (parentPath) {
        onNewFolder({ parentPath });
      }
    }
  }, [onNewFolder, workspacePath, selectedFile, fileTree]);
  
  const handleRefresh = useCallback(() => {
    if (onRefresh) {
      onRefresh();
    }
  }, [onRefresh]);

  const handleRenameSelected = useCallback(() => {
    if (!selectedFile || renamingPath) {
      return;
    }

    const node = findNodeByPath(fileTree, selectedFile);
    if (!node) {
      return;
    }

    globalEventBus.emit('file:rename', {
      path: node.path,
      name: node.name,
    });
  }, [selectedFile, renamingPath, fileTree]);

  const handleDeleteSelected = useCallback(async () => {
    if (!selectedFile) {
      return;
    }

    const node = findNodeByPath(fileTree, selectedFile);
    if (!node) {
      return;
    }

    await commandExecutor.execute('file.delete', buildFileNodeContext(node, workspacePath));
  }, [selectedFile, fileTree, workspacePath]);

  useShortcut(
    'filetree.refresh',
    { key: 'F5', scope: 'filetree' },
    handleRefresh,
    { enabled: Boolean(onRefresh), description: 'keyboard.shortcuts.filetree.refresh' }
  );
  useShortcut(
    'filetree.newFile',
    { key: 'N', ctrl: true, scope: 'filetree' },
    handleNewFile,
    { enabled: Boolean(onNewFile), description: 'keyboard.shortcuts.filetree.newFile' }
  );
  useShortcut(
    'filetree.newFolder',
    { key: 'N', ctrl: true, shift: true, scope: 'filetree' },
    handleNewFolder,
    { enabled: Boolean(onNewFolder), description: 'keyboard.shortcuts.filetree.newFolder' }
  );
  useShortcut(
    'filetree.rename',
    { key: 'F2', scope: 'filetree' },
    handleRenameSelected,
    { enabled: Boolean(selectedFile) && !renamingPath, description: 'keyboard.shortcuts.filetree.rename' }
  );
  useShortcut(
    'filetree.delete',
    { key: 'Delete', scope: 'filetree' },
    () => {
      void handleDeleteSelected();
    },
    { enabled: Boolean(selectedFile), description: 'keyboard.shortcuts.filetree.delete' }
  );

  if (filteredFileTree.length === 0) {
    return (
      <div 
        className={`openbitfun-file-explorer openbitfun-file-explorer--empty ${className}`}
        data-area="file-explorer"
        data-workspace-root={workspacePath}
        data-shortcut-scope="filetree"
        tabIndex={0}
      >
        <div className="openbitfun-file-explorer__empty">
          <Icon name="folder" size="lg" className="openbitfun-file-explorer__empty-icon" />
          <p>{searchQuery ? t('fileTree.emptyFiltered') : t('fileTree.empty')}</p>
        </div>
      </div>
    );
  }

  return (
    <div 
      ref={containerRef}
      className={`openbitfun-file-explorer ${className}`}
      data-area="file-explorer"
      data-workspace-root={workspacePath}
      data-shortcut-scope="filetree"
      tabIndex={0}
      onMouseEnter={() => setIsToolbarVisible(true)}
      onMouseLeave={() => {
        if (!isFocused) {
          setIsToolbarVisible(false);
        }
      }}
      onFocus={handleFocus}
      onBlur={handleBlur}
      onClick={handleContainerClick}
    >
      {(onNewFile || onNewFolder || onRefresh) && !hideToolbar && (
        <div 
          className={`openbitfun-file-explorer__toolbar ${isToolbarVisible ? 'openbitfun-file-explorer__toolbar--visible' : ''}`}
          onClick={(e) => e.stopPropagation()}
          onMouseEnter={() => setIsToolbarVisible(true)}
          onMouseLeave={() => {
            if (!isFocused) {
              setIsToolbarVisible(false);
            }
          }}
        >
          {onNewFile && (
            <Tooltip content={t('fileTree.newFile')} placement="bottom">
              <IconButton
                size="sm"
                aria-label={t('fileTree.newFile')}
                icon={<FilePlus />}
                onClick={handleNewFile}
              />
            </Tooltip>
          )}
          {onNewFolder && (
            <Tooltip content={t('fileTree.newFolder')} placement="bottom">
              <IconButton
                size="sm"
                aria-label={t('fileTree.newFolder')}
                icon={<FolderPlus />}
                onClick={handleNewFolder}
              />
            </Tooltip>
          )}
          {onRefresh && (
            <Tooltip content={t('fileTree.refresh')} placement="bottom">
              <IconButton
                size="sm"
                aria-label={t('fileTree.refresh')}
                icon={<Icon name="refresh" size="lg" />}
                onClick={handleRefresh}
              />
            </Tooltip>
          )}
        </div>
      )}
      
      <VirtualFileTree
        flatNodes={flatNodes}
        selectedFile={selectedFile}
        expandedFolders={expandedFolders}
        onNodeSelect={(node: FlatFileNode) => emitFileSelect(node.path, node.name)}
        onToggleExpand={toggleExpandedState}
        className="openbitfun-file-explorer__tree"
        workspacePath={workspacePath}
        renamingPath={renamingPath}
        onRename={onRename}
        onCancelRename={onCancelRename}
        renderNodeContent={renderNodeContent}
      />
    </div>
  );
};

function formatFileSize(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(1)} ${units[unitIndex]}`;
}

function formatDate(date: Date): string {
  return i18nService.formatDate(date, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit'
  });
}

export default FileExplorer;
