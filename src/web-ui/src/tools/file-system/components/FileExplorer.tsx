import React, { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { Folder, ChevronRight, FilePlus, FolderPlus, RefreshCw } from 'lucide-react';
import { FileTree } from './FileTree';
import { VirtualFileTree } from './VirtualFileTree';
import { FileExplorerProps, FileSystemNode, FlatFileNode } from '../types';
import { GitStatusIndicator } from './GitStatusIndicator';
import { flattenFileTree } from '../utils/treeFlattening';
import { getNewItemParentPath } from '../utils/getNewItemParentPath';
import { i18nService, useI18n } from '@/infrastructure/i18n';
import { IconButton } from '@/component-library';

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

  const handleNodeSelect = useCallback((node: FileSystemNode) => {
    if (onFileSelect) {
      onFileSelect(node.path, node.name);
    }
  }, [onFileSelect]);

  const handleNodeExpand = useCallback((path: string, expanded: boolean) => {
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
    return flattenFileTree(filteredFileTree, expandedFolders);
  }, [filteredFileTree, expandedFolders]);

  const useVirtualScroll = flatNodes.length > VIRTUAL_SCROLL_THRESHOLD;

  const handleVirtualNodeSelect = useCallback((node: FlatFileNode) => {
    if (onFileSelect) {
      onFileSelect(node.path, node.name);
    }
  }, [onFileSelect]);

  const handleVirtualToggleExpand = useCallback((path: string) => {
    const isCurrentlyExpanded = expandedFolders.has(path);
    if (externalOnNodeExpand) {
      externalOnNodeExpand(path, !isCurrentlyExpanded);
    } else {
      setInternalExpandedFolders(prev => {
        const newSet = new Set(prev);
        if (isCurrentlyExpanded) {
          newSet.delete(path);
        } else {
          newSet.add(path);
        }
        return newSet;
      });
    }
  }, [expandedFolders, externalOnNodeExpand]);

  const renderNodeContent = useCallback((node: FileSystemNode, _level: number) => {
    return (
      <div className="bitfun-file-explorer__node-wrapper">
        <span className={`bitfun-file-explorer__node-name ${node.isCompressed ? 'bitfun-file-explorer__compressed-path' : ''}`}>
          {node.name}
        </span>
        
        {(() => {
          if (!node.isDirectory && node.gitStatus) {
            return <GitStatusIndicator status={node.gitStatus} compact={true} />;
          }
          
          return null;
        })()}
        
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

  if (filteredFileTree.length === 0) {
    return (
      <div 
        className={`bitfun-file-explorer bitfun-file-explorer--empty ${className}`}
        data-area="file-explorer"
        data-workspace-root={workspacePath}
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
          onNodeSelect={handleVirtualNodeSelect}
          onToggleExpand={handleVirtualToggleExpand}
          className="bitfun-file-explorer__tree"
          workspacePath={workspacePath}
          renamingPath={renamingPath}
          onRename={onRename}
          onCancelRename={onCancelRename}
        />
      ) : (
        <FileTree
          nodes={filteredFileTree}
          selectedFile={selectedFile}
          expandedFolders={expandedFolders}
          onNodeSelect={handleNodeSelect}
          onNodeExpand={handleNodeExpand}
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

function filterTreeBySearch(nodes: FileSystemNode[], query: string): FileSystemNode[] {
  const result: FileSystemNode[] = [];

  for (const node of nodes) {
    if (node.name.toLowerCase().includes(query)) {
      result.push(node);
    } else if (node.isDirectory && node.children) {
      const filteredChildren = filterTreeBySearch(node.children, query);
      if (filteredChildren.length > 0) {
        result.push({
          ...node,
          children: filteredChildren
        });
      }
    }
  }

  return result;
}

function filterTreeByPredicate(nodes: FileSystemNode[], predicate: (node: FileSystemNode) => boolean): FileSystemNode[] {
  const result: FileSystemNode[] = [];

  for (const node of nodes) {
    if (predicate(node)) {
      if (node.isDirectory && node.children) {
        const filteredChildren = filterTreeByPredicate(node.children, predicate);
        result.push({
          ...node,
          children: filteredChildren
        });
      } else {
        result.push(node);
      }
    } else if (node.isDirectory && node.children) {
      const filteredChildren = filterTreeByPredicate(node.children, predicate);
      if (filteredChildren.length > 0) {
        result.push({
          ...node,
          children: filteredChildren
        });
      }
    }
  }

  return result;
}

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
