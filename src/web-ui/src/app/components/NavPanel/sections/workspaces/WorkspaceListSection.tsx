import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { notificationService } from '@/shared/notification-system';
import type { WorkspaceInfo } from '@/shared/types';
import WorkspaceItem from './WorkspaceItem';
import { isLinkedWorktreeWorkspace } from '@/shared/types';
import { isSamePath } from '@/shared/utils/pathUtils';
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
const WORKSPACE_DROP_SETTLE_MS = 160;
const WORKSPACE_DROP_EASING = 'cubic-bezier(0.22, 1, 0.36, 1)';


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
  const workspaceRowRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const pendingDropRectsRef = useRef<Map<string, DOMRect> | null>(null);
  const dropAnimationsRef = useRef<Map<string, Animation>>(new Map());

  // Drag-state safety nets (window dragend/mousedown + reset timeout) to clear
  // stuck is-drag-active state when dragend doesn't fire normally.
  const dragSafetyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const windowDragEndHandlerRef = useRef<(() => void) | null>(null);
  const windowMouseDownHandlerRef = useRef<(() => void) | null>(null);
  const documentMouseMoveHandlerRef = useRef<(() => void) | null>(null);

  const sectionWorkspaces = variant === 'assistants'
    ? assistantWorkspacesList
    : normalWorkspacesList;
  const projectRoots = variant === 'projects'
    ? sectionWorkspaces
        .filter(workspace => !isLinkedWorktreeWorkspace(workspace))
        .map(workspace => workspace.rootPath)
    : [];
  const workspaces = variant === 'projects'
    ? sectionWorkspaces.filter(workspace => (
        !isLinkedWorktreeWorkspace(workspace)
        || !projectRoots.some(projectRoot => (
          isSamePath(projectRoot, workspace.worktree?.mainRepoPath || '')
        ))
      ))
    : sectionWorkspaces;
  const activeWorkspace = openedWorkspacesList.find(workspace => workspace.id === activeWorkspaceId);
  const activeProjectPath = activeWorkspace?.worktree && !activeWorkspace.worktree.isMain
    ? activeWorkspace.worktree.mainRepoPath
    : activeWorkspace?.rootPath;
  const emptyLabel = variant === 'assistants'
    ? t('nav.workspaces.emptyAssistants')
    : t('nav.workspaces.emptyProjects');
  const workspaceSignature = workspaces.map(workspace => workspace.id).join(':');

  useEffect(() => () => {
    dropAnimationsRef.current.forEach(animation => animation.cancel());
    dropAnimationsRef.current.clear();
  }, []);

  useLayoutEffect(() => {
    const previousRects = pendingDropRectsRef.current;
    if (!previousRects) return;
    pendingDropRectsRef.current = null;
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return;

    for (const [workspaceId, element] of workspaceRowRefs.current) {
      const previousRect = previousRects.get(workspaceId);
      if (!previousRect) continue;
      const currentRect = element.getBoundingClientRect();
      const deltaY = previousRect.top - currentRect.top;
      if (Math.abs(deltaY) < 0.5) continue;

      dropAnimationsRef.current.get(workspaceId)?.cancel();
      const animation = element.animate(
        [
          { transform: `translateY(${deltaY}px)` },
          { transform: 'translateY(0)' },
        ],
        { duration: WORKSPACE_DROP_SETTLE_MS, easing: WORKSPACE_DROP_EASING },
      );
      dropAnimationsRef.current.set(workspaceId, animation);
      animation.addEventListener('finish', () => {
        if (dropAnimationsRef.current.get(workspaceId) === animation) {
          dropAnimationsRef.current.delete(workspaceId);
        }
      }, { once: true });
    }
  }, [workspaceSignature]);

  const removeDragArtifacts = useCallback(() => {
    if (dragSafetyTimeoutRef.current !== null) {
      clearTimeout(dragSafetyTimeoutRef.current);
      dragSafetyTimeoutRef.current = null;
    }
    if (windowDragEndHandlerRef.current !== null) {
      window.removeEventListener('dragend', windowDragEndHandlerRef.current);
      windowDragEndHandlerRef.current = null;
    }
    if (windowMouseDownHandlerRef.current !== null) {
      window.removeEventListener('mousedown', windowMouseDownHandlerRef.current);
      windowMouseDownHandlerRef.current = null;
    }
    if (documentMouseMoveHandlerRef.current !== null) {
      document.removeEventListener('mousemove', documentMouseMoveHandlerRef.current);
      documentMouseMoveHandlerRef.current = null;
    }
  }, []);

  const clearDragState = useCallback(() => {
    draggedWorkspaceIdRef.current = null;
    dropTargetRef.current = null;
    setDraggedWorkspaceId(null);
    setDropTarget(null);
  }, []);

  const cleanupDrag = useCallback(() => {
    removeDragArtifacts();
    clearDragState();
  }, [removeDragArtifacts, clearDragState]);

  useEffect(() => {
    return () => {
      if (dragSafetyTimeoutRef.current !== null) {
        clearTimeout(dragSafetyTimeoutRef.current);
      }
      if (windowDragEndHandlerRef.current !== null) {
        window.removeEventListener('dragend', windowDragEndHandlerRef.current);
      }
      if (windowMouseDownHandlerRef.current !== null) {
        window.removeEventListener('mousedown', windowMouseDownHandlerRef.current);
      }
      if (documentMouseMoveHandlerRef.current !== null) {
        document.removeEventListener('mousemove', documentMouseMoveHandlerRef.current);
      }
    };
  }, []);

  const handleDragStart = useCallback((workspace: WorkspaceInfo) => (event: React.DragEvent<HTMLDivElement>) => {
    const payload: WorkspaceDragPayload = { workspaceId: workspace.id, variant };
    const serializedPayload = JSON.stringify(payload);
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData(WORKSPACE_DRAG_MIME_TYPE, serializedPayload);
    event.dataTransfer.setData('text/plain', serializedPayload);
    draggedWorkspaceIdRef.current = workspace.id;
    setDraggedWorkspaceId(workspace.id);

    // Native drag ghost is used (no setDragImage / no custom preview). The
    // drag-state safety nets below clear stuck is-drag-active state if dragend
    // doesn't fire normally.
    const windowDragEndHandler = () => { cleanupDrag(); };
    windowDragEndHandlerRef.current = windowDragEndHandler;
    window.addEventListener('dragend', windowDragEndHandler, { once: true });

    const windowMouseDownHandler = () => { cleanupDrag(); };
    windowMouseDownHandlerRef.current = windowMouseDownHandler;
    window.addEventListener('mousedown', windowMouseDownHandler, { once: true });

    // mousemove is suppressed during an active HTML5 drag (per spec) and resumes
    // when the drag ends — even if dragend is swallowed. So it's a safe instant
    // cleanup signal that never interrupts a normal drag (no matter how long the
    // user pauses mid-drag).
    const documentMouseMoveHandler = () => { cleanupDrag(); };
    documentMouseMoveHandlerRef.current = documentMouseMoveHandler;
    document.addEventListener('mousemove', documentMouseMoveHandler, { once: true });

    dragSafetyTimeoutRef.current = setTimeout(() => {
      cleanupDrag();
    }, 1500);
  }, [variant, cleanupDrag]);

  const handleDrag = useCallback(() => {
    // Refresh the stuck-state safety timeout on each drag event so a long drag
    // doesn't trip the fallback. (Native ghost is used; no custom preview.)
    if (dragSafetyTimeoutRef.current !== null) {
      clearTimeout(dragSafetyTimeoutRef.current);
      dragSafetyTimeoutRef.current = setTimeout(() => {
        cleanupDrag();
      }, 1500);
    }
  }, [cleanupDrag]);

  const handleDragEnd = useCallback(() => {
    cleanupDrag();
  }, [cleanupDrag]);

  const handleDragOver = useCallback((workspaceId: string) => (event: React.DragEvent<HTMLDivElement>) => {
    // Browsers block reading dataTransfer data during dragover for security.
    // Check event.dataTransfer.types instead 閳?it IS readable during dragover.
    const isWorkspaceDrag = event.dataTransfer.types.includes(WORKSPACE_DRAG_MIME_TYPE);
    const currentDraggedId = draggedWorkspaceIdRef.current;

    if (!isWorkspaceDrag || !currentDraggedId || currentDraggedId === workspaceId) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = 'move';

    if (dragSafetyTimeoutRef.current !== null) {
      clearTimeout(dragSafetyTimeoutRef.current);
      dragSafetyTimeoutRef.current = setTimeout(() => {
        cleanupDrag();
      }, 1500);
    }

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
  }, [cleanupDrag]); // cleanupDrag is stable; reads refs for the rest

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

    if (!payloadText) {
      cleanupDrag();
      return;
    }

    let payload: WorkspaceDragPayload;
    try {
      payload = JSON.parse(payloadText) as WorkspaceDragPayload;
    } catch {
      cleanupDrag();
      return;
    }

    if (!payload.workspaceId || payload.variant !== variant) {
      cleanupDrag();
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    // Reuse the position already determined by dragover 閳?avoid recalculating
    // on the wrapper whose height may have changed due to the drop-line element.
    const position =
      dropTargetRef.current?.workspaceId === workspaceId
        ? dropTargetRef.current.position
        : 'after';

    removeDragArtifacts();
    draggedWorkspaceIdRef.current = null;
    dropTargetRef.current = null;
    setDropTarget(null);
    pendingDropRectsRef.current = new Map(
      Array.from(workspaceRowRefs.current, ([id, element]) => (
        [id, element.getBoundingClientRect()] as const
      )),
    );

    try {
      await reorderOpenedWorkspacesInSection(variant, payload.workspaceId, workspaceId, position);
    } catch (error) {
      pendingDropRectsRef.current = null;
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.reorderFailed'),
        { duration: 4000 }
      );
    } finally {
      setDraggedWorkspaceId(null);
    }
  }, [reorderOpenedWorkspacesInSection, t, variant, cleanupDrag, removeDragArtifacts]);

  return (
    <div data-bf-component="workspace-list-section" data-bf-part="root" data-bf-state={draggedWorkspaceId ? 'dragging' : undefined}
      className={`bitfun-nav-panel__workspace-list${draggedWorkspaceId ? ' is-dragging' : ''}`}
      data-testid="nav-workspace-list"
      data-workspace-list={variant}
    >
      {workspaces.length === 0 ? (
        <div
          data-bf-component="workspace-list-section"
          data-bf-part="empty"
          className="bitfun-nav-panel__workspace-list-empty"
          data-testid="nav-workspace-list-empty"
          data-workspace-list={variant}
        >
          {emptyLabel}
        </div>
      ) : (
        workspaces.map(workspace => (
          <div
            data-bf-component="workspace-list-section"
            data-bf-part="item"
            data-bf-state={workspace.id === activeWorkspaceId ? 'selected' : undefined}
            key={workspace.id}
            ref={(element) => {
              if (element) workspaceRowRefs.current.set(workspace.id, element);
              else workspaceRowRefs.current.delete(workspace.id);
            }}
            className={[
              'bitfun-nav-panel__workspace-drop-target',
              draggedWorkspaceId && draggedWorkspaceId !== workspace.id && 'is-drag-active',
              dropTarget?.workspaceId === workspace.id && 'is-drop-target',
              dropTarget?.workspaceId === workspace.id && dropTarget.position === 'before' && 'is-before',
              dropTarget?.workspaceId === workspace.id && dropTarget.position === 'after' && 'is-after',
            ].filter(Boolean).join(' ')}
            data-testid="nav-workspace-drop-target"
            data-workspace-id={workspace.id}
            data-workspace-list={variant}
            onDragOver={handleDragOver(workspace.id)}
            onDragLeave={handleDragLeave(workspace.id)}
            onDrop={(event) => { void handleDrop(workspace.id)(event); }}
          >
            <WorkspaceItem
              workspace={workspace}
              isActive={
                workspace.id === activeWorkspaceId ||
                Boolean(activeProjectPath && isSamePath(workspace.rootPath, activeProjectPath))
              }
              isSingle={workspaces.length === 1}
              draggable={workspaces.length > 1}
              isDragging={draggedWorkspaceId === workspace.id}
              onDragStart={handleDragStart(workspace)}
              onDrag={handleDrag}
              onDragEnd={handleDragEnd}
            />
            {dropTarget?.workspaceId === workspace.id ? (
              <div data-bf-component="workspace-list-section" data-bf-part="dropLine" className="bitfun-nav-panel__workspace-drop-line" aria-hidden="true" />
            ) : null}
          </div>
        ))
      )}
    </div>
  );
};

export default WorkspaceListSection;
