import React, { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { useShortcut } from '@/infrastructure/hooks/useShortcut';
import { Folder, ChevronRight, FilePlus, FolderPlus, RefreshCw } from 'lucide-react';
import { FileTree } from './FileTree';
import { VirtualFileTree } from './VirtualFileTree';
import { FileExplorerProps, FileSystemNode, FlatFileNode } from '../types';
import { flattenFileTree } from '../utils/treeFlattening';
import { getNewItemParentPath } from '../utils/getNewItemParentPath';
import { i18nService, useI18n } from '@/infrastructure/i18n';
import { expandedFoldersContains, pathsEquivalentFs } from '@/shared/utils/pathUtils';
import { IconButton } from '@/component-library';
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

const VIRTUAL_SCROLL_THRESHOLD = 100;

interface ScrollBreadcrumbProps {
  containerRef: React.RefObject<HTMLDivElement>;
  workspacePath?: string;
  onNavigate?: (path: string) => void;
}

const ScrollBreadcrumb: React.FC<ScrollBreadcrumbProps> = ({ containerRef, workspacePath, onNavigate }) => {
  const [visiblePath, setVisiblePath] = useState<string | null>(null);
  
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    
    const detectCurrentDirectory = () => {
      const treeContainer = container.querySelector('.bitfun-file-explorer__tree');
      if (!treeContainer) return;
      
      const containerRect = treeContainer.getBoundingClientRect();
      
      const expandedDirNodes = treeContainer.querySelectorAll('[data-is-directory="true"][data-is-expanded="true"]');
      
      const activeDirs: { path: string; top: number }[] = [];
      
      expandedDirNodes.forEach((node) => {
        const rect = node.getBoundingClientRect();
        const relativeTop = rect.top - containerRect.top;
        const path = node.getAttribute('data-file-path');
        
        if (!path) return;
        
        if (relativeTop >= 0) return;
        
        const nodeElement = node.closest('.bitfun-file-explorer__node');
        const childrenContainer = nodeElement?.querySelector(':scope > .bitfun-file-explorer__node-children');
        
        if (childrenContainer) {
          const childrenRect = childrenContainer.getBoundingClientRect();
          const childrenBottom = childrenRect.bottom - containerRect.top;
          
          if (childrenBottom > 0) {
            activeDirs.push({ path, top: relativeTop });
          }
        }
      });
      
      if (activeDirs.length > 0) {
        activeDirs.sort((a, b) => b.top - a.top);
        setVisiblePath(activeDirs[0].path);
      } else {
        setVisiblePath(null);
      }
    };
    
    detectCurrentDirectory();
    
    const treeContainer = container.querySelector('.bitfun-file-explorer__tree');
    if (treeContainer) {
      treeContainer.addEventListener('scroll', detectCurrentDirectory, { passive: true });
      return () => treeContainer.removeEventListener('scroll', detectCurrentDirectory);
    }
  }, [containerRef]);
  
  if (!visiblePath) return null;
  
  let relativePath = visiblePath;
  if (workspacePath && visiblePath.startsWith(workspacePath)) {
    relativePath = visiblePath.slice(workspacePath.length).replace(/^[/\\]/, '');
  }
  
  const parts = relativePath.split(/[/\\]/).filter(Boolean);
  if (parts.length === 0) return null;
  
  const pathSegments: { name: string; fullPath: string }[] = [];
  let currentPath = workspacePath || '';
  
  for (const part of parts) {
    currentPath = currentPath ? `${currentPath}/${part}` : part;
    pathSegments.push({ name: part, fullPath: currentPath });
  }
  
  const displaySegments = pathSegments.length > 4 
    ? [{ name: '…', fullPath: '' }, ...pathSegments.slice(-4)]
    : pathSegments;
  
  return (
    <div className="bitfun-file-explorer__breadcrumb">
      {displaySegments.map((segment, index) => (
        <React.Fragment key={segment.fullPath || index}>
          {index > 0 && (
            <ChevronRight size={10} className="bitfun-file-explorer__breadcrumb-separator" />
          )}
          <span 
            className={`bitfun-file-explorer__breadcrumb-item ${segment.fullPath ? 'bitfun-file-explorer__breadcrumb-item--clickable' : ''}`}
            onClick={() => segment.fullPath && onNavigate?.(segment.fullPath)}
            title={segment.fullPath || undefined}
          >
            {segment.name}
          </span>
        </React.Fragment>
      ))}
    </div>
  );
};

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

  const useVirtualScroll = flatNodes.length > VIRTUAL_SCROLL_THRESHOLD;
  
  const toggleExpandedState = useCallback((path: string) => {
    const isCurrentlyExpanded = expandedFoldersContains(expandedFolders, path);
    setExpandedState(path, !isCurrentlyExpanded);
  }, [expandedFolders, setExpandedState]);

  const renderNodeContent = useCallback((node: FileSystemNode, _level: number) => {
    return (
      <div className="bitfun-file-explorer__node-wrapper">
        <span className={`bitfun-file-explorer__node-name ${node.isCompressed ? 'bitfun-file-explorer__compressed-path' : ''}`}>
          {node.name}
        </span>
        
        {showFileSize && !node.isDirectory && node.size && (
          <span className="bitfun-file-explorer__node-size">
            {formatFileSize(node.size)}
          </span>
        )}
        
        {showLastModified && node.lastModified && (
          <span className="bitfun-file-explorer__node-modified">
            {formatDate(node.lastModified)}
          </span>
        )}
      </div>
    );
  }, [showFileSize, showLastModified]);

  // Keep hooks before any early returns (React Hooks rules).
  const containerRef = useRef<HTMLDivElement>(null);
  
  const [isToolbarVisible, setIsToolbarVisible] = useState(false);
  const [isFocused, setIsFocused] = useState(false);
  
  const handleFocus = useCallback(() => {
    setIsFocused(true);
    setIsToolbarVisible(true);
  }, []);
  
  const handleBlur = useCallback((e: React.FocusEvent) => {
    const toolbar = e.currentTarget.querySelector('.bitfun-file-explorer__toolbar');
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
    if (target.closest('.bitfun-file-explorer__toolbar')) {
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
      if (target.closest('.bitfun-file-explorer__toolbar')) {
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

  const handleBreadcrumbNavigate = useCallback((path: string) => {
    if (externalOnNodeExpand) {
      externalOnNodeExpand(path, true);
    } else {
      setInternalExpandedFolders(prev => {
        const newSet = new Set(prev);
        newSet.add(path);
        return newSet;
      });
    }
  }, [externalOnNodeExpand]);

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
        className={`bitfun-file-explorer bitfun-file-explorer--empty ${className}`}
        data-area="file-explorer"
        data-workspace-root={workspacePath}
        data-shortcut-scope="filetree"
        tabIndex={0}
      >
        <div className="bitfun-file-explorer__empty">
          <Folder size={48} className="bitfun-file-explorer__empty-icon" />
          <p>{searchQuery ? t('fileTree.emptyFiltered') : t('fileTree.empty')}</p>
        </div>
      </div>
    );
  }

  return (
    <div 
      ref={containerRef}
      className={`bitfun-file-explorer ${className}`}
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
          className={`bitfun-file-explorer__toolbar ${isToolbarVisible ? 'bitfun-file-explorer__toolbar--visible' : ''}`}
          onClick={(e) => e.stopPropagation()}
          onMouseEnter={() => setIsToolbarVisible(true)}
          onMouseLeave={() => {
            if (!isFocused) {
              setIsToolbarVisible(false);
            }
          }}
        >
          {onNewFile && (
            <IconButton
              size="xs"
              variant="ghost"
              onClick={handleNewFile}
              tooltip={t('fileTree.newFile')}
              tooltipPlacement="bottom"
            >
              <FilePlus size={14} />
            </IconButton>
          )}
          {onNewFolder && (
            <IconButton
              size="xs"
              variant="ghost"
              onClick={handleNewFolder}
              tooltip={t('fileTree.newFolder')}
              tooltipPlacement="bottom"
            >
              <FolderPlus size={14} />
            </IconButton>
          )}
          {onRefresh && (
            <IconButton
              size="xs"
              variant="ghost"
              onClick={handleRefresh}
              tooltip={t('fileTree.refresh')}
              tooltipPlacement="bottom"
            >
              <RefreshCw size={14} />
            </IconButton>
          )}
        </div>
      )}
      
      {!useVirtualScroll && (
        <ScrollBreadcrumb 
          containerRef={containerRef}
          workspacePath={workspacePath}
          onNavigate={handleBreadcrumbNavigate}
        />
      )}
      
      {useVirtualScroll ? (
        <VirtualFileTree
          flatNodes={flatNodes}
          selectedFile={selectedFile}
          expandedFolders={expandedFolders}
          onNodeSelect={(node: FlatFileNode) => emitFileSelect(node.path, node.name)}
          onToggleExpand={toggleExpandedState}
          className="bitfun-file-explorer__tree"
          workspacePath={workspacePath}
          renamingPath={renamingPath}
          onRename={onRename}
          onCancelRename={onCancelRename}
          renderNodeContent={renderNodeContent}
        />
      ) : (
        <FileTree
          nodes={filteredFileTree}
          selectedFile={selectedFile}
          expandedFolders={expandedFolders}
          loadingPaths={loadingPaths}
          onNodeSelect={(node: FileSystemNode) => emitFileSelect(node.path, node.name)}
          onNodeExpand={setExpandedState}
          renderNodeContent={renderNodeContent}
          className="bitfun-file-explorer__tree"
          renamingPath={renamingPath}
          onRename={onRename}
          onCancelRename={onCancelRename}
          workspacePath={workspacePath}
        />
      )}
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
