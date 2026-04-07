import React, { useCallback, useMemo, useRef, forwardRef } from 'react';
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { VirtualFileTreeProps, FlatFileNode, FileSystemNode } from '../types';
import { useI18n } from '@/infrastructure/i18n';
import { expandedFoldersContains } from '@/shared/utils/pathUtils';
import { FileTreeItem } from './FileTreeItem';

interface VirtualFileRowProps {
  node: FlatFileNode;
  isSelected: boolean;
  isExpanded: boolean;
  onSelect: (node: FlatFileNode) => void;
  onToggleExpand: (path: string) => void;
  renamingPath?: string | null;
  onRename?: (oldPath: string, newName: string) => void;
  onCancelRename?: () => void;
  renderContent?: (node: FileSystemNode, level: number) => React.ReactNode;
  renderActions?: (node: FileSystemNode) => React.ReactNode;
}

const VirtualFileRow = React.memo<VirtualFileRowProps>(({
  node,
  isSelected,
  isExpanded,
  onSelect,
  onToggleExpand,
  renamingPath,
  onRename,
  onCancelRename,
  renderContent,
  renderActions,
}) => {
  const indentPx = node.depth * 20 + 16;

  const nodeForIcon: FileSystemNode = useMemo(() => ({
    path: node.path,
    name: node.name,
    isDirectory: node.isDirectory,
    extension: node.extension,
    size: node.size,
    lastModified: node.lastModified,
    isCompressed: node.isCompressed,
  }), [node]);

  return (
    <div className="bitfun-file-explorer__node">
      <FileTreeItem
        node={nodeForIcon}
        level={node.depth}
        indentPx={indentPx}
        isSelected={isSelected}
        isExpanded={isExpanded}
        isLoading={node.isLoading}
        renamingPath={renamingPath}
        onRename={onRename}
        onCancelRename={onCancelRename}
        onSelect={() => onSelect(node)}
        onToggleExpand={() => onToggleExpand(node.path)}
        renderContent={renderContent}
        renderActions={renderActions}
      />
    </div>
  );
});

VirtualFileRow.displayName = 'VirtualFileRow';

export const VirtualFileTree = forwardRef<VirtuosoHandle, VirtualFileTreeProps>(({
  flatNodes,
  selectedFile,
  expandedFolders,
  onNodeSelect,
  onToggleExpand,
  height = '100%',
  className = '',
  renamingPath,
  onRename,
  onCancelRename,
  renderNodeContent,
  renderNodeActions,
}, ref) => {
  const { t } = useI18n('tools');
  const virtuosoRef = useRef<VirtuosoHandle>(null);

  React.useImperativeHandle(ref, () => virtuosoRef.current!, []);

  const handleNodeSelect = useCallback((node: FlatFileNode) => {
    onNodeSelect?.(node);
  }, [onNodeSelect]);

  const handleToggleExpand = useCallback((path: string) => {
    onToggleExpand?.(path);
  }, [onToggleExpand]);

  const itemContent = useCallback((_index: number, node: FlatFileNode) => {
    const isSelected = selectedFile === node.path;
    const isExpanded = expandedFoldersContains(expandedFolders, node.path);

    return (
      <VirtualFileRow
        node={node}
        isSelected={isSelected}
        isExpanded={isExpanded}
        onSelect={handleNodeSelect}
        onToggleExpand={handleToggleExpand}
        renamingPath={renamingPath}
        onRename={onRename}
        onCancelRename={onCancelRename}
        renderContent={renderNodeContent}
        renderActions={renderNodeActions}
      />
    );
  }, [selectedFile, expandedFolders, handleNodeSelect, handleToggleExpand, renamingPath, onRename, onCancelRename, renderNodeContent, renderNodeActions]);

  if (flatNodes.length === 0) {
    return (
      <div className={`bitfun-file-explorer__tree bitfun-file-explorer__tree--empty ${className}`}>
        <div className="bitfun-file-explorer__empty-message">
          <p>{t('fileTree.empty')}</p>
        </div>
      </div>
    );
  }

  return (
    <div 
      className={`bitfun-file-explorer__tree bitfun-file-explorer__tree--virtual ${className}`}
      style={{ height }}
      tabIndex={0}
    >
      <Virtuoso
        ref={virtuosoRef}
        data={flatNodes}
        itemContent={itemContent}
        overscan={50}
        increaseViewportBy={{ top: 100, bottom: 200 }}
        style={{ height: '100%' }}
        computeItemKey={(_index, node) => node.path}
      />
    </div>
  );
});

VirtualFileTree.displayName = 'VirtualFileTree';

export default VirtualFileTree;
