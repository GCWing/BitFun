import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { ChevronRight, ChevronDown, FolderOpen, FileText, Loader2 } from 'lucide-react';
import { Input } from '../../../component-library/components/Input';
import { dragManager } from '../../../shared/services/DragManager';
import { fileTreeDragSource } from '../../../shared/context-system/drag-drop/FileTreeDragSource';
import { useI18n } from '@/infrastructure/i18n';
import { pathsEquivalentFs } from '@/shared/utils/pathUtils';
import { FileSystemNode } from '../types';
import { getFileIcon, getFileIconClass } from '../utils/fileIcons';
import { getCompressionTooltip } from '../utils/pathCompression';
import { validateFileName } from '../utils/validateFileName';

interface RenameInputProps {
  node: FileSystemNode;
  /** 同级已存在的名称列表（应已排除当前节点原名）。用于重名检测。 */
  siblings: string[];
  /** 当前是否为远程工作区，决定非法字符集与是否检查 Windows 保留名。 */
  isRemote: boolean;
  onRename: (newName: string) => void;
  onCancel?: () => void;
}

/** 报错浮层与输入框之间的垂直间距。 */
const ERROR_OFFSET = 4;

/**
 * 重命名输入框。
 *
 * 输入过程中实时校验（空名/非法字符/Windows保留名/同级重名/名称过长），
 * 命中即报错，Enter/blur 时若仍有错误则阻断提交、保持聚焦（与 VSCode 一致）。
 *
 * 报错气泡用 portal 挂到 `document.body`：树容器有 `overflow:hidden`，若直接画在
 * 节点下方/上方会被裁剪。气泡默认位于输入框下方，下方空间不足时翻到上方。
 */
const RenameInput: React.FC<RenameInputProps> = ({ node, siblings, isRemote, onRename, onCancel }) => {
  const { t } = useI18n('panels/files');
  const [value, setValue] = useState(node.name);
  const [error, setError] = useState<string | null>(null);
  const submittedRef = React.useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const errorRef = useRef<HTMLDivElement>(null);
  const [errorPos, setErrorPos] = useState<{ top: number; left: number } | null>(null);

  // 聚焦并选中名称主体（文件去掉扩展名，目录全选）。
  useEffect(() => {
    const timer = setTimeout(() => {
      const input = inputRef.current;
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

  const validate = (nextValue: string): string | null => {
    const errorKey = validateFileName(nextValue, {
      isRemote,
      isDirectory: node.isDirectory,
      siblings,
    });
    return errorKey ? t(errorKey, { name: nextValue.trim() }) : null;
  };

  const handleChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const nextValue = event.target.value;
    setValue(nextValue);
    setError(validate(nextValue));
  };

  const commitRename = (nextValue: string, source: 'enter' | 'blur' = 'enter') => {
    if (submittedRef.current) {
      return;
    }

    const newName = nextValue.trim();

    // 空名或与原名一致：取消（与原行为一致）。
    if (!newName || newName === node.name) {
      submittedRef.current = true;
      onCancel?.();
      return;
    }

    // 有校验错误：
    // - 按 Enter：阻断提交，保持输入框与聚焦并提示（VSCode 行为），让用户继续修正。
    // - 失焦（点击别处）：不再阻断，直接取消重命名并清除错误，避免卡死。
    const errorMsg = validate(nextValue);
    if (errorMsg) {
      if (source === 'blur') {
        submittedRef.current = true;
        onCancel?.();
        return;
      }
      setError(errorMsg);
      inputRef.current?.focus();
      return;
    }

    submittedRef.current = true;
    onRename(newName);
  };

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      commitRename(value);
      return;
    }

    if (event.key === 'Escape') {
      event.preventDefault();
      if (!submittedRef.current) {
        submittedRef.current = true;
        onCancel?.();
      }
    }
  };

  const handleBlur = (event: React.FocusEvent<HTMLInputElement>) => {
    // 焦点若移到输入框容器内部（如点击 prefix 图标等），视为未离开，不处理。
    const next = event.relatedTarget as Node | null;
    if (next && wrapperRef.current?.contains(next)) {
      return;
    }
    commitRename(value, 'blur');
  };

  // 计算报错气泡位置：默认输入框下方，下方空间不足则翻到上方。
  useLayoutEffect(() => {
    if (!error || !inputRef.current) {
      setErrorPos(null);
      return;
    }

    const rect = inputRef.current.getBoundingClientRect();
    const bubbleRect = errorRef.current?.getBoundingClientRect();
    const bubbleHeight = bubbleRect?.height ?? 28;
    const bubbleWidth = bubbleRect?.width ?? Math.min(280, window.innerWidth - 16);
    const spaceBelow = window.innerHeight - rect.bottom;
    const flip = spaceBelow < bubbleHeight + ERROR_OFFSET;
    const nextPos = {
      top: Math.max(8, flip ? rect.top - bubbleHeight - ERROR_OFFSET : rect.bottom + ERROR_OFFSET),
      left: Math.max(8, Math.min(rect.left, window.innerWidth - bubbleWidth - 8)),
    };

    if (errorPos?.top !== nextPos.top || errorPos.left !== nextPos.left) {
      setErrorPos(nextPos);
    }
  }, [error, value, errorPos]);

  // 窗口尺寸变化时重算气泡位置。
  useEffect(() => {
    if (!error) {
      return;
    }
    const handleResize = () => setErrorPos((prev) => prev);
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [error]);

  const errorBubble = error && errorPos
    ? createPortal(
        <div
          className="bitfun-file-explorer__rename-error"
          ref={errorRef}
          style={{ top: errorPos.top, left: errorPos.left }}
          role="alert"
        >
          {error}
        </div>,
        getAppearanceOverlayHost(),
      )
    : null;

  return (
    <>
      <div ref={wrapperRef} className="bitfun-file-explorer__rename-input-wrapper" onClick={(event) => event.stopPropagation()}>
        <Input
          ref={inputRef}
          type="text"
          variant="filled"
          inputSize="small"
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          prefix={node.isDirectory ? <FolderOpen size={14} /> : <FileText size={14} />}
          error={!!error}
          autoFocus
        />
      </div>
      {errorBubble}
    </>
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
  /** 重命名时用于重名检测的同级名称列表（按父目录分组，已排除当前节点）。 */
  renameSiblings?: string[];
  /** 当前是否为远程工作区，传入重命名校验上下文。 */
  isRemoteWorkspace?: boolean;
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
  renameSiblings,
  isRemoteWorkspace = false,
}) => {
  const { t } = useI18n('tools');
  const dragImageRef = React.useRef<HTMLDivElement | null>(null);

  const isCompressed = node.isCompressed;
  const tooltip = isCompressed ? getCompressionTooltip(node as any) : node.path;
  const isRenaming = renamingPath ? pathsEquivalentFs(renamingPath, node.path) : false;

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
    dragImage.style.background = 'var(--bf-appearance-token-color-overlay-black-80)';
    dragImage.style.color = 'var(--bf-appearance-token-color-static-white)';
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
      draggable={!isRenaming}
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
          siblings={renameSiblings ?? []}
          isRemote={isRemoteWorkspace}
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
