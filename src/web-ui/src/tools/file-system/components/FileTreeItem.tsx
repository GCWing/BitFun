import React, { useEffect, useState } from 'react';
import { ChevronRight, ChevronDown, FolderOpen, FileText, Loader2 } from 'lucide-react';
import { Input } from '../../../component-library/components/Input';
import { dragManager } from '../../../shared/services/DragManager';
import { fileTreeDragSource } from '../../../shared/context-system/drag-drop/FileTreeDragSource';
import { useI18n } from '@/infrastructure/i18n';
import { FileSystemNode } from '../types';
import { getFileIcon, getFileIconClass } from '../utils/fileIcons';
import { getCompressionTooltip } from '../utils/pathCompression';

interface RenameInputProps {
  node: FileSystemNode;
  onRename: (newName: string) => void;
  onCancel?: () => void;
}

const RenameInput: React.FC<RenameInputProps> = ({ node, onRename, onCancel }) => {
  const [value, setValue] = useState(node.name);

  useEffect(() => {
    const timer = setTimeout(() => {
      const input = document.querySelector('.bitfun-file-explorer__rename-input-wrapper input') as HTMLInputElement | null;
      if (!input) {
        return;
      }

      input.focus();
      const dotIndex = node.name.lastIndexOf('.');
      if (dotIndex > 0 && !node.isDirectory) {
        input.setSelectionRange(0, dotIndex);
      } else {
        input.select();
      }
    }, 10);

    return () => clearTimeout(timer);
  }, [node.name, node.isDirectory]);

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      const newName = value.trim();
      if (newName && newName !== node.name) {
        onRename(newName);
      } else {
        onCancel?.();
      }
      return;
    }

    if (event.key === 'Escape') {
      event.preventDefault();
      onCancel?.();
    }
  };

  const handleBlur = () => {
    const newName = value.trim();
    if (newName && newName !== node.name) {
      onRename(newName);
    } else {
      onCancel?.();
    }
  };

  return (
    <div className="bitfun-file-explorer__rename-input-wrapper" onClick={(event) => event.stopPropagation()}>
      <Input
        type="text"
        variant="filled"
        inputSize="small"
        value={value}
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={handleBlur}
        prefix={node.isDirectory ? <FolderOpen size={14} /> : <FileText size={14} />}
        autoFocus
      />
    </div>
  );
};

export interface FileTreeItemProps {
  node: FileSystemNode;
  level: number;
  indentPx: number;
  isSelected?: boolean;
  isExpanded?: boolean;
  isLoading?: boolean;
  className?: string;
  renamingPath?: string | null;
  onRename?: (path: string, newName: string) => void;
  onCancelRename?: () => void;
  onSelect?: () => void;
  onToggleExpand?: () => void;
  renderContent?: (node: FileSystemNode, level: number) => React.ReactNode;
  renderActions?: (node: FileSystemNode) => React.ReactNode;
}

export const FileTreeItem: React.FC<FileTreeItemProps> = ({
  node,
  level,
  indentPx,
  isSelected = false,
  isExpanded = false,
  isLoading = false,
  className = '',
  renamingPath,
  onRename,
  onCancelRename,
  onSelect,
  onToggleExpand,
  renderContent,
  renderActions,
}) => {
  const { t } = useI18n('tools');
  const dragImageRef = React.useRef<HTMLDivElement | null>(null);

  const isCompressed = node.isCompressed;
  const tooltip = isCompressed ? getCompressionTooltip(node as any) : node.path;
  const isRenaming = renamingPath === node.path;

  const handleClick = (event: React.MouseEvent) => {
    if (event.button !== 0) {
      return;
    }

    event.stopPropagation();

    const target = event.currentTarget as HTMLElement;
    if (typeof target.focus === 'function') {
      target.focus();
    }

    if (node.isDirectory) {
      onToggleExpand?.();
    }
    onSelect?.();
  };

  const handleExpandClick = (event: React.MouseEvent) => {
    event.stopPropagation();
    onToggleExpand?.();
  };

  const handleDragStart = (event: React.DragEvent) => {
    const dragImage = document.createElement('div');
    dragImage.textContent = t('fileTree.draggingFile', { name: node.name });
    dragImage.style.position = 'absolute';
    dragImage.style.top = '-1000px';
    dragImage.style.padding = '8px';
    dragImage.style.background = 'rgba(0, 0, 0, 0.8)';
    dragImage.style.color = 'white';
    dragImage.style.borderRadius = '4px';
    document.body.appendChild(dragImage);
    dragImageRef.current = dragImage;

    event.dataTransfer.setDragImage(dragImage, 0, 0);
    event.dataTransfer.effectAllowed = 'copy';

    const payload = fileTreeDragSource.createPayload(node);
    dragManager.startDrag(fileTreeDragSource, payload, event.nativeEvent);
  };

  const handleDragEnd = (event: React.DragEvent) => {
    if (dragImageRef.current && document.body.contains(dragImageRef.current)) {
      document.body.removeChild(dragImageRef.current);
      dragImageRef.current = null;
    }

    const success = event.nativeEvent.dataTransfer?.dropEffect !== 'none';
    dragManager.endDrag(event.nativeEvent, success);
  };

  return (
    <div 
      className={`bitfun-file-explorer__node-content ${isSelected ? 'bitfun-file-explorer__node-content--selected' : ''} ${node.isDirectory ? 'bitfun-file-explorer__node-content--directory' : ''} ${isCompressed ? 'bitfun-file-explorer__node-content--compressed' : ''} ${className}`}
      style={{ paddingLeft: `${indentPx}px` }}
      onClick={handleClick}
      title={tooltip}
      draggable={true}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      data-file-path={node.path}
      data-file={!node.isDirectory}
      data-is-directory={node.isDirectory}
      data-is-expanded={node.isDirectory ? isExpanded : undefined}
      tabIndex={0}
      role="treeitem"
      aria-selected={isSelected}
    >
      {node.isDirectory ? (
        <span className={`bitfun-file-explorer__expand-icon ${isExpanded ? 'bitfun-file-explorer__expand-icon--expanded' : ''}`} onClick={handleExpandClick}>
          {isLoading ? (
            <Loader2 size={16} className="bitfun-file-explorer__loading-icon" />
          ) : isExpanded ? (
            <ChevronDown size={16} />
          ) : (
            <ChevronRight size={16} />
          )}
        </span>
      ) : (
        <span className={getFileIconClass(node, isExpanded)}>
          {getFileIcon(node, isExpanded)}
        </span>
      )}

      {isRenaming ? (
        <RenameInput
          node={node}
          onRename={(newName) => onRename?.(node.path, newName)}
          onCancel={onCancelRename}
        />
      ) : renderContent ? (
        renderContent(node, level)
      ) : (
        <span className={`bitfun-file-explorer__node-name ${isCompressed ? 'bitfun-file-explorer__compressed-path' : ''}`}>
          {node.name}
        </span>
      )}

      {renderActions ? (
        <div className="bitfun-file-explorer__node-actions" onClick={(event) => event.stopPropagation()}>
          {renderActions(node)}
        </div>
      ) : null}
    </div>
  );
};

export default FileTreeItem;

