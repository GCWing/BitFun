import React from 'react';
import { FileTreeNodeProps } from '../types';
import { expandedFoldersContains } from '@/shared/utils/pathUtils';
import { FileTreeItem } from './FileTreeItem';
import { getPathDepth } from './fileTreeDepth';

interface ExtendedFileTreeNodeProps extends FileTreeNodeProps {
  selectedFile?: string;
  expandedFolders?: Set<string>;
  /** 重命名时用于重名检测的同级名称列表（已排除当前节点原名）。 */
  renameSiblings?: string[];
  /** 当前是否为远程工作区，传入重命名校验上下文。 */
  isRemoteWorkspace?: boolean;
}

export const FileTreeNode: React.FC<ExtendedFileTreeNodeProps> = ({
  node,
  level,
  isSelected = false,
  isExpanded = false,
  selectedFile,
  expandedFolders,
  loadingPaths,
  onSelect,
  onToggleExpand,
  className = '',
  workspacePath,
  renamingPath,
  onRename,
  onCancelRename,
  renderContent,
  renderActions,
  renameSiblings,
  isRemoteWorkspace = false,
}) => {
  const indentDepth = getPathDepth(node.path, workspacePath);

  return (
    <div className={`bitfun-file-explorer__node ${className}`}>
      <FileTreeItem
        node={node}
        level={level}
        indentPx={(indentDepth - 1) * 20 + 16}
        isSelected={isSelected}
        isExpanded={isExpanded}
        isLoading={loadingPaths?.has(node.path)}
        renamingPath={renamingPath}
        onRename={onRename}
        onCancelRename={onCancelRename}
        onSelect={() => onSelect?.(node)}
        onToggleExpand={() => onToggleExpand?.(node.path)}
        renderContent={renderContent}
        renderActions={renderActions}
        renameSiblings={renameSiblings}
        isRemoteWorkspace={isRemoteWorkspace}
      />

      {node.isDirectory && isExpanded && (
        <div className="bitfun-file-explorer__node-children">
          {(node.children ?? []).map(child => (
            <FileTreeNode
              key={child.path}
              node={child}
              level={level + 1}
              isSelected={selectedFile === child.path}
              isExpanded={
                expandedFolders ? expandedFoldersContains(expandedFolders, child.path) : false
              }
              selectedFile={selectedFile}
              expandedFolders={expandedFolders}
              loadingPaths={loadingPaths}
              onSelect={onSelect}
              onToggleExpand={onToggleExpand}
              workspacePath={workspacePath}
              renamingPath={renamingPath}
              onRename={onRename}
              onCancelRename={onCancelRename}
              renderContent={renderContent}
              renderActions={renderActions}
              renameSiblings={(node.children ?? []).filter(c => c.path !== child.path).map(c => c.name)}
              isRemoteWorkspace={isRemoteWorkspace}
            />
          ))}
        </div>
      )}
    </div>
  );
};

export default FileTreeNode;
