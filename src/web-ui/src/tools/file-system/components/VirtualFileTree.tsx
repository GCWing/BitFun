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
  /** 同级已存在的名称列表（已排除当前节点原名）。 */
  renameSiblings?: string[];
  /** 当前是否为远程工作区，传入重命名校验上下文。 */
  isRemoteWorkspace?: boolean;
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
  renameSiblings,
  isRemoteWorkspace = false,
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
        renameSiblings={renameSiblings}
        isRemoteWorkspace={isRemoteWorkspace}
      />
    </div>
  );
});

VirtualFileRow.displayName = 'VirtualFileRow';

interface VirtualFileTreeWithRemoteProps extends VirtualFileTreeProps {
  /** 当前是否为远程工作区，传入重命名校验上下文。 */
  isRemoteWorkspace?: boolean;
}

export const VirtualFileTree = forwardRef<VirtuosoHandle, VirtualFileTreeWithRemoteProps>(({
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
  isRemoteWorkspace = false,
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

  // 按父目录分组，缓存每个节点的同级名称列表（重命名重名检测用）。
  const siblingsByParent = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const n of flatNodes) {
      const parent = n.parentPath ?? '';
      const list = map.get(parent);
      if (list) {
        list.push(n.name);
      } else {
        map.set(parent, [n.name]);
      }
    }
    return map;
  }, [flatNodes]);

  const itemContent = useCallback((_index: number, node: FlatFileNode) => {
    const isSelected = selectedFile === node.path;
    const isExpanded = expandedFoldersContains(expandedFolders, node.path);
    const siblings = siblingsByParent.get(node.parentPath ?? '')
      ?.filter((name) => name !== node.name);

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
        renameSiblings={siblings}
        isRemoteWorkspace={isRemoteWorkspace}
      />
    );
  }, [selectedFile, expandedFolders, handleNodeSelect, handleToggleExpand, renamingPath, onRename, onCancelRename, renderNodeContent, renderNodeActions, siblingsByParent, isRemoteWorkspace]);

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
