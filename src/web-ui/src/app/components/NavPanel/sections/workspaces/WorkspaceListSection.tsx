import React, { useCallback, useRef, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { notificationService } from '@/shared/notification-system';
import WorkspaceItem from './WorkspaceItem';
import './WorkspaceListSection.scss';

interface WorkspaceListSectionProps {
  variant: 'assistants' | 'projects';
}

type WorkspaceDragPosition = 'before' | 'after';

interface WorkspaceDragPayload {
  workspaceId: string;
  variant: 'assistants' | 'projects';
}

const WORKSPACE_DRAG_MIME_TYPE = 'application/x-bitfun-workspace';


const WorkspaceListSection: React.FC<WorkspaceListSectionProps> = ({ variant }) => {
  const { t } = useI18n('common');
  const {
    openedWorkspacesList,
    normalWorkspacesList,
    assistantWorkspacesList,
    activeWorkspaceId,
    reorderOpenedWorkspacesInSection,
  } = useWorkspaceContext();
  const [draggedWorkspaceId, setDraggedWorkspaceId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<{
    workspaceId: string;
    position: WorkspaceDragPosition;
  } | null>(null);

  // Refs for values that must be read inside event handlers without stale closures
  const draggedWorkspaceIdRef = useRef<string | null>(null);
  const dropTargetRef = useRef<{ workspaceId: string; position: WorkspaceDragPosition } | null>(null);

  const workspaces = variant === 'assistants'
    ? assistantWorkspacesList
    : normalWorkspacesList;
  const emptyLabel = variant === 'assistants'
    ? t('nav.workspaces.emptyAssistants')
    : t('nav.workspaces.emptyProjects');

  const handleDragStart = useCallback((workspaceId: string) => (event: React.DragEvent<HTMLDivElement>) => {
    const payload: WorkspaceDragPayload = { workspaceId, variant };
    const serializedPayload = JSON.stringify(payload);
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData(WORKSPACE_DRAG_MIME_TYPE, serializedPayload);
    event.dataTransfer.setData('text/plain', serializedPayload);
    draggedWorkspaceIdRef.current = workspaceId;
    setDraggedWorkspaceId(workspaceId);
  }, [variant]);

  const handleDragEnd = useCallback(() => {
    draggedWorkspaceIdRef.current = null;
    dropTargetRef.current = null;
    setDraggedWorkspaceId(null);
    setDropTarget(null);
  }, []);

  const handleDragOver = useCallback((workspaceId: string) => (event: React.DragEvent<HTMLDivElement>) => {
    // Browsers block reading dataTransfer data during dragover for security.
    // Check event.dataTransfer.types instead — it IS readable during dragover.
    const isWorkspaceDrag = event.dataTransfer.types.includes(WORKSPACE_DRAG_MIME_TYPE);
    const currentDraggedId = draggedWorkspaceIdRef.current;

    if (!isWorkspaceDrag || !currentDraggedId || currentDraggedId === workspaceId) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = 'move';

    // Measure only the workspace card, not the wrapper that includes the drop-line.
    const itemEl = event.currentTarget.querySelector<HTMLElement>(
      '.bitfun-nav-panel__workspace-item'
    );
    const rect = itemEl
      ? itemEl.getBoundingClientRect()
      : event.currentTarget.getBoundingClientRect();

    const position: WorkspaceDragPosition = event.clientY >= rect.top + rect.height / 2
      ? 'after'
      : 'before';

    setDropTarget(current => {
      if (current?.workspaceId === workspaceId && current.position === position) {
        return current;
      }
      const next = { workspaceId, position };
      dropTargetRef.current = next;
      return next;
    });
  }, []); // Intentionally empty: reads from refs, not closed-over state

  const handleDragLeave = useCallback((workspaceId: string) => (event: React.DragEvent<HTMLDivElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
      setDropTarget(current => {
        if (current?.workspaceId !== workspaceId) return current;
        dropTargetRef.current = null;
        return null;
      });
    }
  }, []);

  const handleDrop = useCallback((workspaceId: string) => async (event: React.DragEvent<HTMLDivElement>) => {
    // On drop, reading dataTransfer data IS allowed.
    const payloadText =
      event.dataTransfer.getData(WORKSPACE_DRAG_MIME_TYPE) ||
      event.dataTransfer.getData('text/plain');

    if (!payloadText) return;

    let payload: WorkspaceDragPayload;
    try {
      payload = JSON.parse(payloadText) as WorkspaceDragPayload;
    } catch {
      return;
    }

    if (!payload.workspaceId || payload.variant !== variant) return;

    event.preventDefault();
    event.stopPropagation();

    // Reuse the position already determined by dragover — avoid recalculating
    // on the wrapper whose height may have changed due to the drop-line element.
    const position =
      dropTargetRef.current?.workspaceId === workspaceId
        ? dropTargetRef.current.position
        : 'after';

    draggedWorkspaceIdRef.current = null;
    dropTargetRef.current = null;
    setDropTarget(null);

    try {
      await reorderOpenedWorkspacesInSection(variant, payload.workspaceId, workspaceId, position);
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.reorderFailed'),
        { duration: 4000 }
      );
    } finally {
      setDraggedWorkspaceId(null);
    }
  }, [reorderOpenedWorkspacesInSection, t, variant]);

  return (
    <div className={`bitfun-nav-panel__workspace-list${draggedWorkspaceId ? ' is-dragging' : ''}`}>
      {workspaces.length === 0 ? (
        <div className="bitfun-nav-panel__workspace-list-empty">
          {emptyLabel}
        </div>
      ) : (
        workspaces.map(workspace => (
          <div
            key={workspace.id}
            className={[
              'bitfun-nav-panel__workspace-drop-target',
              draggedWorkspaceId && draggedWorkspaceId !== workspace.id && 'is-drag-active',
              dropTarget?.workspaceId === workspace.id && 'is-drop-target',
              dropTarget?.workspaceId === workspace.id && dropTarget.position === 'before' && 'is-before',
              dropTarget?.workspaceId === workspace.id && dropTarget.position === 'after' && 'is-after',
            ].filter(Boolean).join(' ')}
            onDragOver={handleDragOver(workspace.id)}
            onDragLeave={handleDragLeave(workspace.id)}
            onDrop={(event) => { void handleDrop(workspace.id)(event); }}
          >
            {dropTarget?.workspaceId === workspace.id && dropTarget.position === 'before' ? (
              <div className="bitfun-nav-panel__workspace-drop-line" aria-hidden="true" />
            ) : null}
            <WorkspaceItem
              workspace={workspace}
              isActive={workspace.id === activeWorkspaceId}
              isSingle={openedWorkspacesList.length === 1}
              draggable={workspaces.length > 1}
              isDragging={draggedWorkspaceId === workspace.id}
              onDragStart={handleDragStart(workspace.id)}
              onDragEnd={handleDragEnd}
            />
            {dropTarget?.workspaceId === workspace.id && dropTarget.position === 'after' ? (
              <div className="bitfun-nav-panel__workspace-drop-line" aria-hidden="true" />
            ) : null}
          </div>
        ))
      )}
    </div>
  );
};

export default WorkspaceListSection;
