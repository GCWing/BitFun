import React from 'react';
import { Loader2 } from 'lucide-react';
import { FileTreeNodeProps } from '../types';
import { expandedFoldersContains } from '@/shared/utils/pathUtils';
import { FileTreeItem, getPathDepth } from './FileTreeItem';
import { useI18n } from '@/infrastructure/i18n';

interface ExtendedFileTreeNodeProps extends FileTreeNodeProps {
  selectedFile?: string;
  expandedFolders?: Set<string>;
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
  onLoadMore,
  className = '',
  workspacePath,
  renamingPath,
  onRename,
  onCancelRename,
  renderContent,
  renderActions
}) => {
  const { t } = useI18n('tools');
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
              onLoadMore={onLoadMore}
              workspacePath={workspacePath}
              renamingPath={renamingPath}
              onRename={onRename}
              onCancelRename={onCancelRename}
              renderContent={renderContent}
              renderActions={renderActions}
            />
          ))}
          {node.hasMoreChildren && onLoadMore && (
            <button
              type="button"
              className="bitfun-file-explorer__load-more"
              style={{ paddingLeft: `${indentPxForLoadMore(indentDepth + 1)}px` }}
              onClick={() => onLoadMore(node.path)}
              disabled={loadingPaths?.has(node.path)}
            >
              {loadingPaths?.has(node.path) ? (
                <Loader2 size={14} className="bitfun-file-explorer__loading-icon" />
              ) : null}
              <span>
                {t('fileTree.loadMore', {
                  loaded: node.loadedChildrenCount ?? node.children?.length ?? 0,
                  total: node.totalChildren ?? node.children?.length ?? 0,
                  defaultValue: `Load more (${node.loadedChildrenCount ?? node.children?.length ?? 0}/${node.totalChildren ?? node.children?.length ?? 0})`,
                })}
              </span>
            </button>
          )}
        </div>
      )}
    </div>
  );
};

function indentPxForLoadMore(depth: number): number {
  return (Math.max(depth, 1) - 1) * 20 + 40;
}

export default FileTreeNode;
