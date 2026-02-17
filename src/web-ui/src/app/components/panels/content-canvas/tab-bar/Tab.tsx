/**
 * Tab component.
 * Supports preview/active/pinned tab states.
 */

import React, { useCallback, useState } from 'react';
import { X, Pin } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import type { CanvasTab, EditorGroupId, TabState } from '../types';
import { createLogger } from '@/shared/utils/logger';
import './Tab.scss';

const log = createLogger('Tab');

export interface TabProps {
  /** Tab data */
  tab: CanvasTab;
  /** Editor group ID */
  groupId: EditorGroupId;
  /** Whether active tab */
  isActive: boolean;
  /** Click callback */
  onClick: () => void;
  /** Double-click callback */
  onDoubleClick: () => void;
  /** Close callback */
  onClose: () => Promise<void> | void;
  /** Pin/unpin callback */
  onPin: () => void;
  /** Drag start callback */
  onDragStart: (e: React.DragEvent) => void;
  /** Drag end callback */
  onDragEnd: () => void;
  /** Whether being dragged */
  isDragging?: boolean;
}

/**
 * Get class name for tab state.
 */
const getStateClassName = (state: TabState): string => {
  switch (state) {
    case 'preview':
      return 'is-preview';
    case 'pinned':
      return 'is-pinned';
    default:
      return '';
  }
};

export const Tab: React.FC<TabProps> = ({
  tab,
  groupId,
  isActive,
  onClick,
  onDoubleClick,
  onClose,
  onPin,
  onDragStart,
  onDragEnd,
  isDragging = false,
}) => {
  const { t } = useTranslation('components');
  const [isHovered, setIsHovered] = useState(false);

  // Build tooltip text
  const unsavedSuffix = tab.isDirty ? ` (${t('tabs.unsaved')})` : '';
  const tooltipText = tab.content.data?.filePath
    ? `${tab.content.data.filePath}${unsavedSuffix}`
    : `${tab.title}${unsavedSuffix}`;

  // Handle single click - respond immediately
  const handleClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onClick();
  }, [onClick]);

  // Handle double click - rely on native onDoubleClick
  const handleDoubleClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onDoubleClick();
  }, [onDoubleClick]);

  // Handle close click
  const handleCloseClick = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation();
    await onClose();
  }, [onClose]);

  // Handle pin click
  const handlePinClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onPin();
  }, [onPin]);

  // Handle drag start
  const handleDragStart = useCallback((e: React.DragEvent) => {
    e.dataTransfer.setData('application/json', JSON.stringify({
      tabId: tab.id,
      sourceGroupId: groupId,
    }));
    e.dataTransfer.effectAllowed = 'move';
    onDragStart(e);
  }, [tab.id, groupId, onDragStart]);

  // Handle context menu
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
  }, []);

  // Build class names
  const classNames = [
    'canvas-tab',
    isActive && 'is-active',
    tab.isDirty && 'is-dirty',
    isDragging && 'is-dragging',
    getStateClassName(tab.state),
  ].filter(Boolean).join(' ');

  // Show close button only while hovering to avoid reserving layout space.
  const showCloseButton = isHovered;

  return (
    <Tooltip content={tooltipText} placement="bottom">
      <div
        className={classNames}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
        draggable
        onDragStart={handleDragStart}
        onDragEnd={onDragEnd}
      >
        {/* Pin icon */}
        {tab.state === 'pinned' && (
          <Tooltip content={t('tabs.unpin')}>
            <button
              className="canvas-tab__pin-icon"
              onClick={handlePinClick}
            >
              <Pin size={12} />
            </button>
          </Tooltip>
        )}

        {/* Title */}
        <span className="canvas-tab__title">
          {tab.title}
        </span>

        {/* Dirty state indicator */}
        {tab.isDirty && (
          <span className="canvas-tab__dirty-indicator" title={t('tabs.unsaved')}>
            ●
          </span>
        )}

        {/* Close button */}
        {showCloseButton && (
          <Tooltip content={t('tabs.close')}>
            <button
              className="canvas-tab__close-btn"
              onClick={handleCloseClick}
            >
              <X size={12} />
            </button>
          </Tooltip>
        )}

      </div>
    </Tooltip>
  );
};

Tab.displayName = 'Tab';

export default Tab;
