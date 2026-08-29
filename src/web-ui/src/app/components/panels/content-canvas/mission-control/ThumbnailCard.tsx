/**
 * ThumbnailCard component.
 * File thumbnail card in mission control.
 */

import React, { useCallback, useMemo } from 'react';
import { FileCode, FileText } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { CanvasTab, EditorGroupId } from '../types';
import { isFileViewerType } from '../types';
import './ThumbnailCard.scss';
import { Icon, Tooltip } from '@bitfun/ui';

export interface ThumbnailCardProps {
  /** Tab data */
  tab: CanvasTab;
  /** Editor group */
  groupId: EditorGroupId;
  /** Whether active tab */
  isActive: boolean;
  /** Click callback */
  onClick: () => void;
  /** Close callback */
  onClose: () => void;
  /** Pin callback */
  onPin: () => void;
  /** Drag start */
  onDragStart: (e: React.DragEvent) => void;
  /** Drag end */
  onDragEnd: () => void;
}

/**
 * Get icon for content type.
 */
const getContentIcon = (type: string) => {
  if (type.includes('code') || type.includes('editor')) {
    return <FileCode size={16} />;
  }
  if (type.includes('markdown') || type.includes('text')) {
    return <FileText size={16} />;
  }
  if (type.includes('image')) {
    return <Icon name="image" size="md" />;
  }
  if (type === 'terminal') {
    return <Icon name="terminal" size="md" />;
  }
  if (type.includes('git')) {
    return <Icon name="git" size="md" />;
  }
  return <FileCode size={16} />;
};

/**
 * Get preview content (first few lines).
 */
const getPreviewContent = (tab: CanvasTab, noPreviewText: string): string[] => {
  const data = tab.content.data;
  
  // Try to resolve content
  let content = '';
  if (typeof data === 'string') {
    content = data;
  } else if (data?.content) {
    content = data.content;
  } else if (data?.sourceCode) {
    content = data.sourceCode;
  } else if (data?.initialContent) {
    content = data.initialContent;
  }
  
  if (!content) {
    return [noPreviewText];
  }
  
  // Take first 5 lines
  return content.split('\n').slice(0, 5);
};

export const ThumbnailCard: React.FC<ThumbnailCardProps> = ({
  tab,
  groupId,
  isActive,
  onClick,
  onClose,
  onPin,
  onDragStart,
  onDragEnd,
}) => {
  const { t } = useTranslation('components');

  // Preview content
  const previewLines = useMemo(() => getPreviewContent(tab, t('canvas.noPreview')), [tab, t]);
  
  // Whether file type
  const isFileType = useMemo(() => isFileViewerType(tab.content.type), [tab.content.type]);
  
  // Group label text
  const groupLabel = useMemo(() => {
    if (groupId === 'primary') return t('canvas.groupPrimary');
    if (groupId === 'secondary') return t('canvas.groupSecondary');
    return t('canvas.groupTertiary');
  }, [groupId, t]);

  const titleWithDeleted = useMemo(() => {
    const suffix = tab.fileDeletedFromDisk ? ` - ${t('tabs.fileDeleted')}` : '';
    return `${tab.title}${suffix}`;
  }, [tab.fileDeletedFromDisk, tab.title, t]);

  // Handle close
  const handleClose = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onClose();
  }, [onClose]);

  // Handle pin
  const handlePin = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onPin();
  }, [onPin]);

  // Handle context menu
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
  }, []);

  // Handle drag start
  const handleDragStart = useCallback((e: React.DragEvent) => {
    e.dataTransfer.setData('application/json', JSON.stringify({
      tabId: tab.id,
      sourceGroupId: groupId,
    }));
    e.dataTransfer.effectAllowed = 'move';
    onDragStart(e);
  }, [tab.id, groupId, onDragStart]);

  // State class
  const stateClass = tab.state === 'pinned' ? 'is-pinned' : tab.state === 'preview' ? 'is-preview' : '';

  return (
    <div data-bf-component="canvas-thumbnail" data-bf-part="root" data-bf-group={groupId}
      data-bf-state={[
        isActive && 'active',
        tab.isDirty && 'dirty',
        tab.fileDeletedFromDisk && 'deleted',
        tab.state === 'pinned' && 'pinned',
        tab.state === 'preview' && 'preview',
      ].filter(Boolean).join(' ')}
      className={`canvas-thumbnail-card ${isActive ? 'is-active' : ''} ${stateClass} ${tab.isDirty ? 'is-dirty' : ''} ${tab.fileDeletedFromDisk ? 'is-file-deleted' : ''}`}
      onClick={onClick}
      onContextMenu={handleContextMenu}
      draggable
      onDragStart={handleDragStart}
      onDragEnd={onDragEnd}
    >
      {/* Header */}
      <div data-bf-component="canvas-thumbnail" data-bf-part="header" className="canvas-thumbnail-card__header">
        <div data-bf-component="canvas-thumbnail" data-bf-part="icon" className="canvas-thumbnail-card__icon">
          {getContentIcon(tab.content.type)}
        </div>
        <div data-bf-component="canvas-thumbnail" data-bf-part="title" className="canvas-thumbnail-card__title">
          {tab.state === 'pinned' && <Icon name="pin" size="2xs" className="canvas-thumbnail-card__pin-icon" />}
          <span className={tab.state === 'preview' ? 'is-preview' : ''}>
            {titleWithDeleted}
          </span>
          {tab.isDirty && <span className="canvas-thumbnail-card__dirty">●</span>}
        </div>
        <div data-bf-component="canvas-thumbnail" data-bf-part="actions" className="canvas-thumbnail-card__actions">
          <Tooltip content={tab.state === 'pinned' ? t('tabs.unpin') : t('tabs.pin')}>
            <button
              data-bf-component="canvas-thumbnail"
              data-bf-part="action"
              className={`canvas-thumbnail-card__action-btn ${tab.state === 'pinned' ? 'is-active' : ''}`}
              onClick={handlePin}
            >
              <Icon name="pin" size="xs" />
            </button>
          </Tooltip>
          <Tooltip content={t('tabs.close')}>
            <button
              data-bf-component="canvas-thumbnail"
              data-bf-part="action"
              className="canvas-thumbnail-card__action-btn canvas-thumbnail-card__close-btn"
              onClick={handleClose}
            >
              <Icon name="xmark" size="xs" />
            </button>
          </Tooltip>
        </div>
      </div>

      {/* Preview area */}
      <div data-bf-component="canvas-thumbnail" data-bf-part="preview" className="canvas-thumbnail-card__preview">
        {isFileType ? (
          <pre data-bf-component="canvas-thumbnail" data-bf-part="code" className="canvas-thumbnail-card__code">
            {previewLines.map((line, index) => (
              <div key={index} className="canvas-thumbnail-card__code-line">
                {line || ' '}
              </div>
            ))}
          </pre>
        ) : (
          <div data-bf-component="canvas-thumbnail" data-bf-part="placeholder" className="canvas-thumbnail-card__placeholder">
            {getContentIcon(tab.content.type)}
            <span>{tab.content.type}</span>
          </div>
        )}
      </div>

      {/* Group badge */}
      <div 
        data-bf-component="canvas-thumbnail"
        data-bf-part="groupBadge"
        className={`canvas-thumbnail-card__group-badge canvas-thumbnail-card__group-badge--${groupId}`}
      >
        {groupLabel}
      </div>
    </div>
  );
};

ThumbnailCard.displayName = 'ThumbnailCard';

export default ThumbnailCard;
