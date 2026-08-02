import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Bot,
  ChevronDown,
  ChevronRight,
  ListTree,
  MessageSquare,
  RefreshCw,
} from 'lucide-react';
import { DotMatrixLoader, IconButton } from '@/component-library';
import { sessionAPI, type SessionLineageSnapshot } from '@/infrastructure/api/service-api/SessionAPI';
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
  onSelectSession?: (selection: SessionTreeSelection) => void;
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
  onSelectSession,
  t,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [snapshot, setSnapshot] = useState<SessionLineageSnapshot | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [liveRevision, setLiveRevision] = useState(0);
  const [expandedSessionIds, setExpandedSessionIds] = useState<Set<string>>(new Set());
  const containerRef = useRef<HTMLDivElement | null>(null);
  const requestGenerationRef = useRef(0);

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
      if (!containerRef.current?.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setIsOpen(false);
    };
    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen]);

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

  useEffect(() => {
    if (!tree) return;
    const defaults = collectExpandedRunningBranches(tree);
    setExpandedSessionIds(previous => {
      const next = new Set(previous);
      defaults.forEach(sessionId => next.add(sessionId));
      return next.size === previous.size ? previous : next;
    });
  }, [tree]);

  const toggleExpanded = useCallback((targetSessionId: string) => {
    setExpandedSessionIds(previous => {
      const next = new Set(previous);
      if (next.has(targetSessionId)) next.delete(targetSessionId);
      else next.add(targetSessionId);
      return next;
    });
  }, []);

  const handleSelect = useCallback((node: SessionLineageNode) => {
    onSelectSession?.({
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
    });
    setIsOpen(false);
  }, [onSelectSession]);

  const renderNode = (node: SessionLineageNode, depth: number): React.ReactNode => {
    const hasChildren = node.children.length > 0;
    const isExpanded = expandedSessionIds.has(node.sessionId);
    const statusLabel = lifecycleLabel(node.lifecycle, t);
    const secondaryLabel = node.subagentType || node.agentType;

    return (
      <React.Fragment key={node.sessionId}>
        <div
          className={[
            'session-tree-popover__node',
            node.isRoot && 'session-tree-popover__node--root',
            nodeHasActiveWork(node) && 'session-tree-popover__node--active',
          ].filter(Boolean).join(' ')}
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
            <span
              className={`session-tree-popover__status session-tree-popover__status--${node.lifecycle}`}
              title={statusLabel}
              aria-label={statusLabel}
            />
          </button>
        </div>
        {hasChildren && isExpanded ? node.children.map(child => renderNode(child, depth + 1)) : null}
      </React.Fragment>
    );
  };

  const panelLabel = t('flowChatHeader.agentTree');

  return (
    <div className="session-tree-popover" ref={containerRef}>
      <IconButton
        className={[
          'session-tree-popover__trigger',
          isOpen && 'session-tree-popover__trigger--active',
        ].filter(Boolean).join(' ')}
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
        <ListTree size={14} />
      </IconButton>

      {isOpen ? (
        <div className="session-tree-popover__panel" role="dialog" aria-label={panelLabel}>
          <div className="session-tree-popover__header">
            <span>{panelLabel}</span>
            <span>{descendantCount + (tree ? 1 : 0)}</span>
          </div>
          <div className="session-tree-popover__body">
            {tree ? <div role="tree">{renderNode(tree, 0)}</div> : null}
            {isLoading && !tree ? (
              <div className="session-tree-popover__state" aria-live="polite">
                <DotMatrixLoader size="small" />
                <span>{t('flowChatHeader.agentTreeLoading')}</span>
              </div>
            ) : null}
            {!isLoading && !loadFailed && tree && descendantCount === 0 ? (
              <div className="session-tree-popover__state">
                {t('flowChatHeader.agentTreeEmpty')}
              </div>
            ) : null}
            {loadFailed ? (
              <div className="session-tree-popover__state session-tree-popover__state--error">
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
        </div>
      ) : null}
    </div>
  );
};

SessionTreePopover.displayName = 'SessionTreePopover';
