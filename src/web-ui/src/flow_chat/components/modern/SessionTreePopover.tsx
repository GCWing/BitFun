import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Bot,
  ChevronDown,
  ChevronRight,
  MessageSquare,
  MoreHorizontal,
  RefreshCw,
  Square,
} from 'lucide-react';
import { DotMatrixLoader, IconButton } from '@/component-library';
import { sessionAPI, type SessionLineageSnapshot } from '@/infrastructure/api/service-api/SessionAPI';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance';
import { computeFixedPopoverPosition } from '@/shared/utils/fixedPopoverViewport';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { flowChatStore } from '../../store/FlowChatStore';
import {
  buildSessionLineageTree,
  collectExpandedRunningBranches,
  countSessionLineageDescendants,
  type SessionLineageLifecycle,
  type SessionLineageNode,
} from '../../utils/sessionLineage';
import './SessionTreePopover.scss';

export interface SessionTreeSelection {
  sessionId: string;
  parentSessionId?: string;
  parentToolCallId?: string;
  title: string;
  agentType?: string;
  subagentType?: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  isRoot: boolean;
}

interface SessionTreePopoverProps {
  sessionId?: string;
  fallbackWorkspacePath?: string;
  hasActiveDescendants?: boolean;
  onSelectSession?: (selection: SessionTreeSelection) => void;
  onCancelSession?: (selection: SessionTreeSelection) => Promise<boolean>;
  t: (key: string, options?: Record<string, unknown>) => string;
}

function lifecycleLabel(
  lifecycle: SessionLineageLifecycle,
  t: SessionTreePopoverProps['t'],
): string {
  return t(`flowChatHeader.agentTreeStatus.${lifecycle}`);
}

function nodeHasActiveWork(node: SessionLineageNode): boolean {
  return node.lifecycle === 'running' || node.lifecycle === 'finishing';
}

export const SessionTreePopover: React.FC<SessionTreePopoverProps> = ({
  sessionId,
  fallbackWorkspacePath,
  hasActiveDescendants = false,
  onSelectSession,
  onCancelSession,
  t,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [snapshot, setSnapshot] = useState<SessionLineageSnapshot | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [liveRevision, setLiveRevision] = useState(0);
  const [expandedSessionIds, setExpandedSessionIds] = useState<Set<string>>(new Set());
  const [collapsedSessionIds, setCollapsedSessionIds] = useState<Set<string>>(new Set());
  const [openActionSessionId, setOpenActionSessionId] = useState<string | null>(null);
  const [cancellingSessionIds, setCancellingSessionIds] = useState<Set<string>>(new Set());
  const containerRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const actionMenuAnchorRef = useRef<HTMLButtonElement | null>(null);
  const actionMenuRef = useRef<HTMLDivElement | null>(null);
  const requestGenerationRef = useRef(0);
  const [actionMenuPosition, setActionMenuPosition] = useState<{ top: number; left: number } | null>(null);

  const refreshSnapshot = useCallback(async () => {
    if (!sessionId) return;
    const requestGeneration = requestGenerationRef.current + 1;
    requestGenerationRef.current = requestGeneration;
    const session = flowChatStore.getState().sessions.get(sessionId);
    const workspacePath = session?.workspacePath || fallbackWorkspacePath;
    if (!workspacePath) {
      if (requestGeneration === requestGenerationRef.current) setLoadFailed(true);
      return;
    }

    setIsLoading(true);
    setLoadFailed(false);
    try {
      const nextSnapshot = await sessionAPI.getSessionLineage({
        sessionId,
        workspacePath,
        remoteConnectionId: session?.remoteConnectionId,
        remoteSshHost: session?.remoteSshHost,
      });
      if (requestGeneration === requestGenerationRef.current) {
        setSnapshot(nextSnapshot);
      }
    } catch {
      if (requestGeneration === requestGenerationRef.current) {
        setLoadFailed(true);
      }
    } finally {
      if (requestGeneration === requestGenerationRef.current) {
        setIsLoading(false);
      }
    }
  }, [fallbackWorkspacePath, sessionId]);

  useEffect(() => {
    requestGenerationRef.current += 1;
    setIsOpen(false);
    setSnapshot(null);
    setLoadFailed(false);
    setExpandedSessionIds(new Set());
    setCollapsedSessionIds(new Set());
    setOpenActionSessionId(null);
    setCancellingSessionIds(new Set());
    setActionMenuPosition(null);
  }, [sessionId]);

  useEffect(() => {
    if (!isOpen) return;
    void refreshSnapshot();

    let frameId: number | null = null;
    const unsubscribe = flowChatStore.subscribe(() => {
      if (frameId !== null) return;
      frameId = requestAnimationFrame(() => {
        frameId = null;
        setLiveRevision(revision => revision + 1);
      });
    });
    return () => {
      unsubscribe();
      if (frameId !== null) cancelAnimationFrame(frameId);
    };
  }, [isOpen, refreshSnapshot]);

  useEffect(() => {
    if (!isOpen) return;
    const handlePointerDown = (event: MouseEvent) => {
      if (
        !containerRef.current?.contains(event.target as Node) &&
        !panelRef.current?.contains(event.target as Node) &&
        !actionMenuRef.current?.contains(event.target as Node)
      ) {
        setIsOpen(false);
        setOpenActionSessionId(null);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsOpen(false);
        setOpenActionSessionId(null);
      }
    };
    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen]);

  const updateActionMenuPosition = useCallback(() => {
    const anchor = actionMenuAnchorRef.current;
    if (!anchor) return;

    const menu = actionMenuRef.current;
    const position = computeFixedPopoverPosition(
      anchor.getBoundingClientRect(),
      menu?.offsetWidth ?? 150,
      menu?.offsetHeight ?? 40,
      4,
      8,
    );
    setActionMenuPosition(position);
  }, []);

  useLayoutEffect(() => {
    if (!openActionSessionId) {
      setActionMenuPosition(null);
      return;
    }

    updateActionMenuPosition();
    window.addEventListener('resize', updateActionMenuPosition);
    window.addEventListener('scroll', updateActionMenuPosition, true);
    return () => {
      window.removeEventListener('resize', updateActionMenuPosition);
      window.removeEventListener('scroll', updateActionMenuPosition, true);
    };
  }, [openActionSessionId, updateActionMenuPosition]);

  const tree = useMemo(() => {
    void liveRevision;
    if (!sessionId) return null;
    return buildSessionLineageTree(
      sessionId,
      snapshot,
      flowChatStore.getState().sessions,
    );
  }, [liveRevision, sessionId, snapshot]);
  const descendantCount = countSessionLineageDescendants(tree);
  const panelLayout = useAnchoredPopoverPosition({
    open: isOpen,
    anchorRef: triggerRef,
    popoverRef: panelRef,
    preferredPlacement: 'bottom',
    alignment: 'end',
    gap: 8,
    layoutRevision: `${descendantCount}:${isLoading}:${loadFailed}`,
  });

  useEffect(() => {
    if (!tree) return;
    const defaults = collectExpandedRunningBranches(tree);
    setExpandedSessionIds(previous => {
      const next = new Set(previous);
      defaults.forEach(sessionId => {
        if (!collapsedSessionIds.has(sessionId)) {
          next.add(sessionId);
        }
      });
      return next.size === previous.size ? previous : next;
    });
  }, [collapsedSessionIds, tree]);

  const toggleExpanded = useCallback((targetSessionId: string) => {
    const isExpanded = expandedSessionIds.has(targetSessionId);
    setExpandedSessionIds(previous => {
      const next = new Set(previous);
      if (isExpanded) next.delete(targetSessionId);
      else next.add(targetSessionId);
      return next;
    });
    setCollapsedSessionIds(previous => {
      const next = new Set(previous);
      if (isExpanded) next.add(targetSessionId);
      else next.delete(targetSessionId);
      return next;
    });
  }, [expandedSessionIds]);

  const handleSelect = useCallback((node: SessionLineageNode) => {
    const selection = {
      sessionId: node.sessionId,
      parentSessionId: node.parentSessionId,
      parentToolCallId: node.parentToolCallId,
      title: node.title,
      agentType: node.agentType,
      subagentType: node.subagentType,
      workspacePath: node.workspacePath,
      remoteConnectionId: node.remoteConnectionId,
      remoteSshHost: node.remoteSshHost,
      isRoot: node.isRoot,
    } satisfies SessionTreeSelection;
    onSelectSession?.(selection);
    setIsOpen(false);
    setOpenActionSessionId(null);
  }, [onSelectSession]);

  const handleActionMenuToggle = useCallback((
    event: React.MouseEvent<HTMLButtonElement>,
    node: SessionLineageNode,
  ) => {
    event.preventDefault();
    event.stopPropagation();
    if (!onCancelSession || node.isRoot || !nodeHasActiveWork(node)) return;

    if (openActionSessionId === node.sessionId) {
      setOpenActionSessionId(null);
      setActionMenuPosition(null);
      return;
    }

    actionMenuAnchorRef.current = event.currentTarget;
    setOpenActionSessionId(node.sessionId);
    updateActionMenuPosition();
  }, [onCancelSession, openActionSessionId, updateActionMenuPosition]);

  const handleCancel = useCallback(async (node: SessionLineageNode) => {
    if (!onCancelSession || node.isRoot || !nodeHasActiveWork(node)) return;
    if (cancellingSessionIds.has(node.sessionId)) return;

    const selection = {
      sessionId: node.sessionId,
      parentSessionId: node.parentSessionId,
      parentToolCallId: node.parentToolCallId,
      title: node.title,
      agentType: node.agentType,
      subagentType: node.subagentType,
      workspacePath: node.workspacePath,
      remoteConnectionId: node.remoteConnectionId,
      remoteSshHost: node.remoteSshHost,
      isRoot: node.isRoot,
    } satisfies SessionTreeSelection;

    setOpenActionSessionId(null);
    setActionMenuPosition(null);
    setCancellingSessionIds(previous => new Set(previous).add(node.sessionId));
    try {
      await onCancelSession(selection);
    } finally {
      setCancellingSessionIds(previous => {
        const next = new Set(previous);
        next.delete(node.sessionId);
        return next;
      });
    }
  }, [cancellingSessionIds, onCancelSession]);

  const renderNode = (node: SessionLineageNode, depth: number): React.ReactNode => {
    const hasChildren = node.children.length > 0;
    const isExpanded = expandedSessionIds.has(node.sessionId);
    const isCancelling = cancellingSessionIds.has(node.sessionId);
    const statusLabel = isCancelling
      ? t('flowChatHeader.agentTreeCancelling')
      : lifecycleLabel(node.lifecycle, t);
    const secondaryLabel = node.subagentType || node.agentType;
    const canCancel = !!onCancelSession && !node.isRoot && nodeHasActiveWork(node);

    return (
      <React.Fragment key={node.sessionId}>
        <div
          className={[
            'session-tree-popover__node',
            node.isRoot && 'session-tree-popover__node--root',
            nodeHasActiveWork(node) && 'session-tree-popover__node--active',
            isCancelling && 'session-tree-popover__node--cancelling',
          ].filter(Boolean).join(' ')}
          data-bf-component="flow-chat-header"
          data-bf-part="sessionTreeNode"
          data-bf-status={node.lifecycle}
          data-bf-state={isCancelling ? 'cancelling' : undefined}
          role="treeitem"
          aria-level={depth + 1}
          aria-expanded={hasChildren ? isExpanded : undefined}
          style={{ paddingLeft: `${6 + Math.min(depth, 10) * 14}px` }}
        >
          {hasChildren ? (
            <button
              type="button"
              className="session-tree-popover__expand"
              onClick={() => toggleExpanded(node.sessionId)}
              aria-label={isExpanded
                ? t('flowChatHeader.agentTreeCollapse')
                : t('flowChatHeader.agentTreeExpand')}
            >
              {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            </button>
          ) : (
            <span className="session-tree-popover__expand-spacer" aria-hidden="true" />
          )}
          <button
            type="button"
            className="session-tree-popover__node-main"
            onClick={() => handleSelect(node)}
          >
            {node.isRoot
              ? <MessageSquare size={13} aria-hidden="true" />
              : <Bot size={13} aria-hidden="true" />}
            <span className="session-tree-popover__node-copy">
              <span className="session-tree-popover__node-title">{node.title}</span>
              {secondaryLabel ? (
                <span className="session-tree-popover__node-meta">{secondaryLabel}</span>
              ) : null}
            </span>
          </button>
          {canCancel ? (
            <div className="session-tree-popover__node-actions">
              <IconButton
                className="session-tree-popover__action-menu-button"
                variant="ghost"
                size="xs"
                onClick={(event) => handleActionMenuToggle(event, node)}
                tooltip={t('flowChatHeader.agentTreeActions')}
                aria-label={t('flowChatHeader.agentTreeActions')}
                aria-haspopup="menu"
                aria-expanded={openActionSessionId === node.sessionId}
                disabled={isCancelling}
              >
                <MoreHorizontal size={13} aria-hidden="true" />
              </IconButton>
              {openActionSessionId === node.sessionId && actionMenuPosition ? createPortal(
                <div
                  ref={actionMenuRef}
                  className="session-tree-popover__action-menu"
                  data-bf-component="flow-chat-header"
                  data-bf-part="sessionTreeMenu"
                  role="menu"
                  aria-label={t('flowChatHeader.agentTreeActions')}
                  style={actionMenuPosition}
                  data-testid="flowchat-header-session-tree-menu"
                >
                  <button
                    type="button"
                    role="menuitem"
                    className="session-tree-popover__action-menu-item session-tree-popover__action-menu-item--danger"
                    data-bf-component="flow-chat-header"
                    data-bf-part="sessionTreeMenuItem"
                    onClick={() => void handleCancel(node)}
                    disabled={isCancelling}
                  >
                    <Square size={12} aria-hidden="true" />
                    <span>{isCancelling
                      ? t('flowChatHeader.agentTreeCancelling')
                      : t('flowChatHeader.agentTreeCancel')}</span>
                  </button>
                </div>,
                getAppearanceOverlayHost(),
              ) : null}
            </div>
          ) : null}
          <span
            className={`session-tree-popover__status session-tree-popover__status--${node.lifecycle}`}
            title={statusLabel}
            aria-label={statusLabel}
          />
        </div>
        {hasChildren && isExpanded ? node.children.map(child => renderNode(child, depth + 1)) : null}
      </React.Fragment>
    );
  };

  const panelLabel = t('flowChatHeader.agentTree');

  return (
    <div
      className="session-tree-popover"
      ref={containerRef}
      data-bf-component="flow-chat-header"
      data-bf-part="sessionTree"
    >
      <IconButton
        ref={triggerRef}
        className={[
          'session-tree-popover__trigger',
          isOpen && 'session-tree-popover__trigger--active',
          hasActiveDescendants && 'session-tree-popover__trigger--has-activity',
        ].filter(Boolean).join(' ')}
        data-bf-component="flow-chat-header"
        data-bf-part="sessionTreeTrigger"
        data-bf-state={[
          isOpen ? 'open' : null,
          hasActiveDescendants ? 'active' : null,
        ].filter(Boolean).join(' ') || undefined}
        variant="ghost"
        size="xs"
        onClick={() => setIsOpen(open => !open)}
        tooltip={panelLabel}
        aria-label={panelLabel}
        aria-expanded={isOpen}
        aria-haspopup="dialog"
        disabled={!sessionId}
        data-testid="flowchat-header-session-tree"
      >
        <span className="session-tree-popover__trigger-inner">
          <Bot size={14} />
          {hasActiveDescendants ? (
            <span className="session-tree-popover__status-dot" aria-hidden="true" />
          ) : null}
        </span>
      </IconButton>

      {isOpen ? createPortal(
        <div
          ref={panelRef}
          className="session-tree-popover__panel"
          data-bf-component="flow-chat-header"
          data-bf-part="sessionTreePanel"
          data-bf-placement={panelLayout?.placement ?? 'bottom'}
          role="dialog"
          aria-label={panelLabel}
          style={{
            top: `${panelLayout?.top ?? 0}px`,
            left: `${panelLayout?.left ?? 0}px`,
            visibility: panelLayout ? 'visible' : 'hidden',
          }}
        >
          <div
            className="session-tree-popover__header"
            data-bf-component="flow-chat-header"
            data-bf-part="sessionTreeHeader"
          >
            <span>{panelLabel}</span>
            <span>{descendantCount + (tree ? 1 : 0)}</span>
          </div>
          <div
            className="session-tree-popover__body"
            data-bf-component="flow-chat-header"
            data-bf-part="sessionTreeBody"
          >
            {tree ? <div role="tree">{renderNode(tree, 0)}</div> : null}
            {isLoading && !tree ? (
              <div
                className="session-tree-popover__state"
                data-bf-component="flow-chat-header"
                data-bf-part="sessionTreeState"
                data-bf-state="loading"
                aria-live="polite"
              >
                <DotMatrixLoader size="small" />
                <span>{t('flowChatHeader.agentTreeLoading')}</span>
              </div>
            ) : null}
            {!isLoading && !loadFailed && tree && descendantCount === 0 ? (
              <div
                className="session-tree-popover__state"
                data-bf-component="flow-chat-header"
                data-bf-part="sessionTreeState"
                data-bf-state="empty"
              >
                {t('flowChatHeader.agentTreeEmpty')}
              </div>
            ) : null}
            {loadFailed ? (
              <div
                className="session-tree-popover__state session-tree-popover__state--error"
                data-bf-component="flow-chat-header"
                data-bf-part="sessionTreeState"
                data-bf-state="error"
              >
                <span>{t('flowChatHeader.agentTreeLoadFailed')}</span>
                <IconButton
                  variant="ghost"
                  size="xs"
                  onClick={() => void refreshSnapshot()}
                  tooltip={t('flowChatHeader.agentTreeRetry')}
                  aria-label={t('flowChatHeader.agentTreeRetry')}
                >
                  <RefreshCw size={13} />
                </IconButton>
              </div>
            ) : null}
          </div>
        </div>,
        getAppearanceOverlayHost(),
      ) : null}
    </div>
  );
};

SessionTreePopover.displayName = 'SessionTreePopover';
