/**
 * Tab component.
 * Supports preview/active/pinned tab states.
 */

import React, { useCallback } from 'react';
import { X, Pin, Split } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { commandExecutor } from '@/shared/context-menu-system/commands/CommandExecutor';
import { useContextMenuStore } from '@/shared/context-menu-system/store/ContextMenuStore';
import { ContextType, type TabContext } from '@/shared/context-menu-system/types/context.types';
import type { MenuItem } from '@/shared/context-menu-system/types/menu.types';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { isRemoteWorkspace } from '@/shared/types';
import { hasNonFileUriScheme } from '@/shared/utils/pathUtils';
import { isHtmlFilePath } from '@/shared/utils/htmlFilePreview';
import type { CanvasTab, EditorGroupId, TabState } from '../types';
import './Tab.scss';
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
  /** Pop out as independent scene */
  onPopOut?: () => void;
  /** Close all other tabs in the same group */
  onCloseOthers?: () => Promise<void> | void;
  /** Close all tabs in the same group */
  onCloseAll?: () => Promise<void> | void;
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
  onPopOut,
  onCloseOthers,
  onCloseAll,
}) => {
  const { t } = useTranslation(['components', 'common']);
  const showMenu = useContextMenuStore(state => state.showMenu);
  const tabData = tab.content.data as { filePath?: string; workspacePath?: string } | undefined;
  const filePath = typeof tabData?.filePath === 'string' ? tabData.filePath : undefined;
  const workspacePath = typeof tabData?.workspacePath === 'string' ? tabData.workspacePath : undefined;
  const isRemote = isRemoteWorkspace(workspaceManager.getState().currentWorkspace);
  const canUseLocalFileActions = Boolean(filePath) && !isRemote && !hasNonFileUriScheme(filePath || '');
  const isPinned = tab.state === 'pinned';

  // Build tooltip text
  const unsavedSuffix = tab.isDirty ? ` (${t('tabs.unsaved')})` : '';
  const deletedSuffix = tab.fileDeletedFromDisk ? ` - ${t('tabs.fileDeleted')}` : '';
  const titleDisplay = `${tab.title}${deletedSuffix}`;
  const tooltipText = tab.content.data?.filePath
    ? `${tab.content.data.filePath}${deletedSuffix}${unsavedSuffix}`
    : `${titleDisplay}${unsavedSuffix}`;

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

  const runCommand = useCallback((commandId: string, context: TabContext) => {
    void commandExecutor.execute(commandId, context);
  }, []);

  // Handle context menu
  const handleContextMenu = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    e.nativeEvent.stopImmediatePropagation?.();

    const context: TabContext = {
      type: ContextType.TAB,
      event: e,
      targetElement: e.currentTarget,
      position: { x: e.clientX, y: e.clientY },
      timestamp: Date.now(),
      metadata: {
        groupId,
        tabState: tab.state,
        isDirty: tab.isDirty,
      },
      tabId: tab.id,
      tabTitle: tab.title,
      tabType: tab.content.type,
      filePath,
      workspacePath,
      isActive,
      isClosable: true,
    };

    const items: MenuItem[] = [
      {
        id: 'tab-close',
        label: t('tabs.close'),
        icon: 'X',
        onClick: () => {
          void onClose();
        },
      },
      {
        id: 'tab-close-others',
        label: t('tabs.closeOthers'),
        icon: 'X',
        disabled: !onCloseOthers,
        onClick: () => {
          void onCloseOthers?.();
        },
      },
      {
        id: 'tab-close-all',
        label: t('tabs.closeAll'),
        icon: 'X',
        disabled: !onCloseAll,
        onClick: () => {
          void onCloseAll?.();
        },
      },
      {
        id: 'tab-separator-actions',
        label: '',
        separator: true,
      },
      {
        id: 'tab-toggle-pin',
        label: isPinned ? t('tabs.unpin') : t('tabs.pin'),
        icon: 'Pin',
        onClick: onPin,
      },
    ];

    if (onPopOut) {
      items.push({
        id: 'tab-pop-out',
        label: t('tabs.popOut'),
        icon: 'ExternalLink',
        onClick: onPopOut,
      });
    }

    if (filePath) {
      items.push(
        {
          id: 'tab-separator-file',
          label: '',
          separator: true,
        },
        {
          id: 'tab-copy-path',
          label: t('common:file.copyPath'),
          icon: 'Copy',
          onClick: () => runCommand('file.copy-path', context),
        },
        {
          id: 'tab-reveal-file',
          label: t('common:file.reveal'),
          icon: 'FolderOpen',
          disabled: !canUseLocalFileActions,
          onClick: () => runCommand('file.reveal-in-explorer', context),
        },
      );

      if (isHtmlFilePath(filePath)) {
        items.push({
          id: 'tab-open-html-in-browser',
          label: t('common:file.openInBrowser'),
          icon: 'ExternalLink',
          disabled: !canUseLocalFileActions,
          onClick: () => runCommand('file.open-html-in-browser', context),
        });
      }
    }

    showMenu({ x: e.clientX, y: e.clientY }, items, context);
  }, [
    canUseLocalFileActions,
    filePath,
    groupId,
    isActive,
    isPinned,
    onClose,
    onCloseAll,
    onCloseOthers,
    onPin,
    onPopOut,
    runCommand,
    showMenu,
    t,
    tab.content.type,
    tab.id,
    tab.isDirty,
    tab.state,
    tab.title,
    workspacePath,
  ]);

  /** Middle-click closes (same as SceneBar session tabs); skip pinned and pin/popout controls. */
  const handleMiddleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 1) return;
    if (isPinned) return;
    const target = e.target as HTMLElement;
    if (target.closest('.canvas-tab__action-btn') || target.closest('.canvas-tab__popout-btn')) return;
    e.preventDefault();
  }, [isPinned]);

  const handleAuxClick = useCallback((e: React.MouseEvent) => {
    if (e.button !== 1) return;
    if (isPinned) return;
    const target = e.target as HTMLElement;
    if (target.closest('.canvas-tab__action-btn') || target.closest('.canvas-tab__popout-btn')) return;
    e.preventDefault();
    e.stopPropagation();
    void onClose();
  }, [isPinned, onClose]);

  const isTaskDetail = tab.content.type === 'task-detail';

  // Build class names
  const classNames = [
    'canvas-tab',
    isActive && 'is-active',
    tab.isDirty && 'is-dirty',
    tab.fileDeletedFromDisk && 'is-file-deleted',
    isDragging && 'is-dragging',
    getStateClassName(tab.state),
    isTaskDetail && 'is-task-detail',
  ].filter(Boolean).join(' ');

  return (
    <Tooltip content={tooltipText} placement="bottom">
      <div
        className={classNames}
        data-tab-id={tab.id}
        data-tab-title={tab.title}
        data-tab-type={tab.content.type}
        data-active={isActive}
        data-closable="true"
        data-file-path={filePath}
        data-workspace-path={workspacePath}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
        onMouseDown={handleMiddleMouseDown}
        onAuxClick={handleAuxClick}
        draggable
        onDragStart={handleDragStart}
        onDragEnd={onDragEnd}
      >
        {/* Task-detail type icon */}
        {isTaskDetail && (
          <Split size={12} className="canvas-tab__type-icon" aria-hidden />
        )}

        {/* Title */}
        <span className="canvas-tab__title">
          {titleDisplay}
        </span>

        {/* Dirty state indicator */}
        {tab.isDirty && (
          <span className="canvas-tab__dirty-indicator" title={t('tabs.unsaved')}>
            ●
          </span>
        )}

        {/* Close / pinned action */}
        <Tooltip content={isPinned ? t('tabs.unpin') : t('tabs.close')}>
          <button
            className={`canvas-tab__action-btn canvas-tab__close-btn ${isPinned ? 'canvas-tab__close-btn--pin' : ''}`}
            onClick={isPinned ? handlePinClick : handleCloseClick}
            tabIndex={-1}
          >
            {isPinned ? <Pin size={12} /> : <X size={12} />}
          </button>
        </Tooltip>

      </div>
    </Tooltip>
  );
};

Tab.displayName = 'Tab';

export default Tab;
