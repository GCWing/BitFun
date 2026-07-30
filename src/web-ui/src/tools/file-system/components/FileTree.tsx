import React, { useState, useCallback, useMemo } from 'react';
import { FileTreeNode } from './FileTreeNode';
import { FileTreeProps } from '../types';
import { lazyCompressFileTree, shouldCompressPaths, CompressedNode } from '../utils/pathCompression';
import { useI18n } from '@/infrastructure/i18n';
import { expandedFoldersContains } from '@/shared/utils/pathUtils';

interface FileTreeWithRemoteProps extends FileTreeProps {
  /** 当前是否为远程工作区，传入重命名校验上下文。 */
  isRemoteWorkspace?: boolean;
}

export const FileTree: React.FC<FileTreeWithRemoteProps> = ({
  nodes,
  selectedFile,
  expandedFolders: externalExpandedFolders,
  loadingPaths,
  onNodeSelect,
  onNodeExpand,
  className = '',
  level = 0,
  workspacePath,
  renderNodeContent,
  renderNodeActions,
  renamingPath,
  onRename,
  onCancelRename,
  isRemoteWorkspace = false,
}) => {
  const { t } = useI18n('tools');
  const [internalExpandedFolders, setInternalExpandedFolders] = useState<Set<string>>(new Set());

  const expandedFolders = externalExpandedFolders || internalExpandedFolders;

  const handleNodeExpand = useCallback((path: string) => {
    if (onNodeExpand) {
      const isCurrentlyExpanded = expandedFoldersContains(expandedFolders, path);
      onNodeExpand(path, !isCurrentlyExpanded);
    } else {
      setInternalExpandedFolders(prev => {
        const newSet = new Set(prev);
        if (newSet.has(path)) {
          newSet.delete(path);
        } else {
          newSet.add(path);
        }
        return newSet;
      });
    }
  }, [expandedFolders, onNodeExpand]);

  const processedNodes = useMemo(() => {
    if (!shouldCompressPaths()) {
      return nodes;
    }
    return lazyCompressFileTree(nodes, expandedFolders);
  }, [nodes, expandedFolders]);

  // 顶层节点之间互为同级（用于重命名重名检测）。
  const topSiblingNames = useMemo(
    () => processedNodes.map((n) => n.name),
    [processedNodes],
  );

  const renderNodes = (nodeList: CompressedNode[], currentLevel: number = level) => {
    return nodeList.map((node) => (
      <FileTreeNode
        key={node.path}
        node={node}
        level={currentLevel}
        isSelected={selectedFile === node.path}
        isExpanded={expandedFoldersContains(expandedFolders, node.path)}
        selectedFile={selectedFile}
        expandedFolders={expandedFolders}
        loadingPaths={loadingPaths}
        onSelect={onNodeSelect}
        onToggleExpand={handleNodeExpand}
        renamingPath={renamingPath}
        onRename={onRename}
        onCancelRename={onCancelRename}
        renderContent={renderNodeContent}
        renderActions={renderNodeActions}
        workspacePath={workspacePath}
        renameSiblings={topSiblingNames.filter((n) => n !== node.name)}
        isRemoteWorkspace={isRemoteWorkspace}
      />
    ));
  };

  return (
    <div
      className={`bitfun-file-explorer__tree ${className}`}
      tabIndex={0}
    >
      {processedNodes.length > 0 ? (
        renderNodes(processedNodes)
      ) : (
        <div className="bitfun-file-explorer__empty-message">
          <p>{t('fileTree.empty')}</p>
        </div>
      )}
    </div>
  );
};

export default FileTree;
