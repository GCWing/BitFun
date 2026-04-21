/**
 * AgentDispatch tool card.
 *
 * Visually similar to TaskToolDisplay but represents an independent Standard
 * session (not a SubAgent). Clicking the card jumps to the created session.
 */

import React, { useCallback, useMemo, useState } from 'react';
import { Bot, Check, X, ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { DotMatrixLoader } from '@/component-library';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard } from './BaseToolCard';
import { openMainSession } from '../services/openBtwSession';
import { flowChatStore } from '../store/FlowChatStore';
import { useSessionsExecutionRunning } from '../hooks/useSessionsExecutionRunning';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { sessionAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AgentDispatchCard.scss';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const log = createLogger('AgentDispatchCard');

function parseData<T>(value: unknown): T | null {
  if (!value) return null;
  try {
    return typeof value === 'string' ? (JSON.parse(value) as T) : (value as T);
  } catch {
    return null;
  }
}

interface AgentDispatchInput {
  action?: 'create' | 'list' | 'status';
  workspace?: string;
  agent_type?: string;
  session_name?: string;
  task_briefing?: string;
}

interface WorkspaceEntry {
  name?: string;
  path?: string;
  kind?: 'global' | 'project';
  session_count?: number;
  sessions?: Array<{
    session_id?: string;
    session_name?: string;
    agent_type?: string;
  }>;
}

interface DispatcherSession {
  session_id?: string;
  session_name?: string;
  agent_type?: string;
  workspace?: string;
  workspace_kind?: 'global' | 'project';
}

interface AgentDispatchResult {
  action?: 'create' | 'list' | 'status';
  success?: boolean;
  session_id?: string;
  session_name?: string;
  agent_type?: string;
  workspace?: string;
  workspace_count?: number;
  workspaces?: WorkspaceEntry[];
  dispatcher_session_count?: number;
  sessions?: DispatcherSession[];
}

async function ensureSessionAvailable(sessionId: string, workspace?: string): Promise<boolean> {
  if (flowChatStore.getState().sessions.has(sessionId)) {
    return true;
  }

  const workspacePath = workspace?.trim();
  if (!workspacePath || workspacePath === 'global') {
    return false;
  }

  try {
    const metadata = await sessionAPI.loadSessionMetadata(sessionId, workspacePath);
    if (!metadata) {
      return false;
    }

    await flowChatStore.hydrateWorkspaceSessionsMetadata(
      [metadata],
      metadata.workspacePath || workspacePath,
      metadata.remoteConnectionId,
      metadata.remoteSshHost,
      metadata.storageScope
    );

    return flowChatStore.getState().sessions.has(sessionId);
  } catch (error) {
    log.warn('Failed to hydrate dispatched session before navigation', {
      sessionId,
      workspacePath,
      error,
    });
    return false;
  }
}

// ---------------------------------------------------------------------------
// Agent type badge color map
// ---------------------------------------------------------------------------

const AGENT_TYPE_COLORS: Record<string, string> = {
  agentic: '#3b82f6',
  Plan: '#f59e0b',
  Cowork: '#10b981',
  debug: '#ef4444',
  Claw: '#8b5cf6',
};

function AgentBadge({ agentType }: { agentType: string }) {
  const color = AGENT_TYPE_COLORS[agentType] ?? '#6366f1';
  return (
    <span
      className="agent-dispatch-badge"
      style={{ '--agent-badge-color': color } as React.CSSProperties}
    >
      {agentType}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export const AgentDispatchCard: React.FC<ToolCardProps> = React.memo(
  ({ toolItem }) => {
    const { t } = useTranslation('flow-chat');
    const { toolCall, toolResult, status } = toolItem;
    const toolId = toolItem.id ?? toolCall?.id;

    const [isExpanded, setIsExpanded] = useState(false);
    const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
      toolId,
      toolName: toolItem.toolName,
    });

    const inputData = useMemo(
      () => parseData<AgentDispatchInput>(toolCall?.input) ?? {},
      [toolCall?.input]
    );
    const resultData = useMemo(
      () => parseData<AgentDispatchResult>(toolResult?.result),
      [toolResult?.result]
    );

    const action = resultData?.action ?? inputData.action ?? 'create';
    const agentType = resultData?.agent_type ?? inputData.agent_type ?? '';
    const sessionName = resultData?.session_name ?? inputData.session_name ?? '';
    const workspace = resultData?.workspace ?? inputData.workspace ?? '';
    const createdSessionId = resultData?.session_id;

    const trackedSessionIds = useMemo(() => {
      const ids: string[] = [];
      if (createdSessionId) ids.push(createdSessionId);
      if (action === 'status') {
        for (const s of resultData?.sessions ?? []) {
          if (s.session_id) ids.push(s.session_id);
        }
      }
      return ids;
    }, [action, createdSessionId, resultData?.sessions]);

    const runningSessionIds = useSessionsExecutionRunning(trackedSessionIds);

    // Status icon
    const statusIcon = useMemo(() => {
      switch (status) {
        case 'running':
        case 'streaming':
          return (
            <span className="agent-dispatch-dot-matrix-wrap agent-dispatch-dot-matrix-wrap--rail">
              <DotMatrixLoader size="small" className="agent-dispatch-dot-matrix" />
            </span>
          );
        case 'completed':
          return <Check size={14} className="agent-dispatch-status-icon agent-dispatch-status-icon--done" />;
        case 'error':
        case 'cancelled':
          return <X size={14} className="agent-dispatch-status-icon agent-dispatch-status-icon--error" />;
        default:
          return null;
      }
    }, [status]);

    /** After create completes, the rail reflects the child session execution state (live). */
    const headerRailIcon = useMemo(() => {
      if (action === 'create' && status === 'completed' && createdSessionId) {
        if (runningSessionIds.has(createdSessionId)) {
          const runLabel = t('toolCards.agentDispatch.sessionRunning');
          return (
            <span className="agent-dispatch-dot-matrix-wrap agent-dispatch-dot-matrix-wrap--rail" title={runLabel}>
              <DotMatrixLoader size="small" className="agent-dispatch-dot-matrix" />
            </span>
          );
        }
        return <Check size={13} className="agent-dispatch-status-icon agent-dispatch-status-icon--done" />;
      }
      return statusIcon;
    }, [action, createdSessionId, runningSessionIds, status, statusIcon, t]);

    // Header text
    const headerLine = useMemo(() => {
      if (action === 'list') {
        if (status === 'completed') {
          const count = resultData?.workspace_count ?? 0;
          return t('toolCards.agentDispatch.foundWorkspaces', { count });
        }
        return t('toolCards.agentDispatch.listingWorkspaces');
      }
      if (action === 'status') {
        if (status === 'completed') {
          const count = resultData?.dispatcher_session_count ?? 0;
          return t('toolCards.agentDispatch.statusSessions', { count });
        }
        return t('toolCards.agentDispatch.checkingStatus');
      }
      // create — format mirrors TaskToolDisplay: "AgentType agent: SessionName"
      const agentTypeLabel = agentType || t('toolCards.agentDispatch.agent');
      const sessionLabel = sessionName || t('toolCards.agentDispatch.agent');
      if (status === 'error' || status === 'cancelled') {
        return t('toolCards.agentDispatch.actionFailed');
      }
      return t('toolCards.agentDispatch.headerLine', {
        agentType: agentTypeLabel,
        session: sessionLabel,
      });
    }, [action, agentType, resultData, sessionName, status, t]);

    // Jump to the created session when the card is clicked (create action, completed)
    const canNavigate =
      action === 'create' && status === 'completed' && !!createdSessionId;

    const openDispatchedSession = useCallback(
      async (sessionId: string, sessionWorkspace?: string) => {
        const available = await ensureSessionAvailable(sessionId, sessionWorkspace);
        if (!available) {
          notificationService.warning(t('toolCards.agentDispatch.sessionUnavailable'), {
            duration: 4000,
          });
          return;
        }

        try {
          await openMainSession(sessionId);
        } catch (error) {
          log.warn('Failed to open dispatched session', { sessionId, error });
          notificationService.warning(t('toolCards.agentDispatch.openSessionFailed'), {
            duration: 4000,
          });
        }
      },
      [t]
    );

    const handleCardClick = useCallback(
      () => {
        if (action === 'create' && status === 'completed' && createdSessionId) {
          void openDispatchedSession(createdSessionId, workspace);
          return;
        }
        if (action !== 'create' || !createdSessionId) {
          applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
        }
      },
      [action, applyExpandedState, createdSessionId, isExpanded, openDispatchedSession, status, workspace]
    );

    // Expanded content (for list/status or create details)
    const expandedContent = useMemo(() => {
      if (action === 'create') {
        if (!workspace && !createdSessionId) return null;
        return (
          <div className="agent-dispatch-details">
            {createdSessionId && (
              <div className="agent-dispatch-detail-row">
                <span className="agent-dispatch-detail-label">{t('toolCards.agentDispatch.sessionId')}</span>
                <span className="agent-dispatch-detail-value agent-dispatch-detail-value--mono">{createdSessionId}</span>
              </div>
            )}
            {workspace && (
              <div className="agent-dispatch-detail-row">
                <span className="agent-dispatch-detail-label">{t('toolCards.agentDispatch.workspace')}</span>
                <span className="agent-dispatch-detail-value">{workspace}</span>
              </div>
            )}
          </div>
        );
      }

      if (action === 'list') {
        const workspaces = resultData?.workspaces ?? [];
        if (!workspaces.length) return <div className="agent-dispatch-empty">{t('toolCards.agentDispatch.noWorkspaces')}</div>;
        return (
          <div className="agent-dispatch-workspace-list">
            {workspaces.map((ws, i) => (
              <div
                key={i}
                className={`agent-dispatch-workspace-item${ws.kind === 'global' ? ' agent-dispatch-workspace-item--global' : ''}${!ws.path ? ' agent-dispatch-workspace-item--no-path' : ''}`}
              >
                <div className="agent-dispatch-workspace-header">
                  <span className="agent-dispatch-workspace-name">{ws.name ?? ws.path}</span>
                  {ws.kind === 'global' && (
                    <span className="agent-dispatch-global-tag">{t('toolCards.agentDispatch.globalTag')}</span>
                  )}
                </div>
                {ws.path ? (
                  <span className="agent-dispatch-workspace-path" title={ws.path}>
                    {ws.path}
                  </span>
                ) : null}
                <span className="agent-dispatch-workspace-count">
                  {t('toolCards.agentDispatch.sessionCount', { count: ws.session_count ?? 0 })}
                </span>
              </div>
            ))}
          </div>
        );
      }

      if (action === 'status') {
        const sessions = resultData?.sessions ?? [];
        if (!sessions.length) return <div className="agent-dispatch-empty">{t('toolCards.agentDispatch.noSessions')}</div>;
        return (
          <div className="agent-dispatch-session-list">
            {sessions.map((s, i) => {
              const sid = s.session_id;
              const isRunning = sid ? runningSessionIds.has(sid) : false;
              return (
                <div
                  key={sid ?? `row-${i}`}
                  className={`agent-dispatch-session-item${sid ? ' agent-dispatch-session-item--clickable' : ''}`}
                  onClick={sid ? () => void openDispatchedSession(sid, s.workspace) : undefined}
                >
                  <div className="agent-dispatch-session-item-main">
                    <span className="agent-dispatch-session-name">{s.session_name ?? s.session_id}</span>
                    {s.agent_type && <AgentBadge agentType={s.agent_type} />}
                  </div>
                  {s.workspace ? (
                    <span className="agent-dispatch-session-path" title={s.workspace}>
                      {s.workspace}
                    </span>
                  ) : null}
                  <div className="agent-dispatch-session-item-rail">
                    {sid ? (
                      isRunning ? (
                        <span
                          className="agent-dispatch-dot-matrix-wrap"
                          title={t('toolCards.agentDispatch.sessionRunning')}
                        >
                          <DotMatrixLoader size="small" className="agent-dispatch-dot-matrix" />
                        </span>
                      ) : (
                        <span
                          className="agent-dispatch-session-idle-dot"
                          title={t('toolCards.agentDispatch.sessionIdle')}
                          aria-label={t('toolCards.agentDispatch.sessionIdle')}
                        />
                      )
                    ) : null}
                    {sid ? (
                      <ExternalLink size={11} className="agent-dispatch-session-link-icon" aria-hidden />
                    ) : null}
                  </div>
                </div>
              );
            })}
          </div>
        );
      }

      return null;
    }, [action, createdSessionId, openDispatchedSession, resultData, runningSessionIds, t, workspace]);

    const hasExpandedContent = !!expandedContent;

    // Header content rendered inside BaseToolCard's header slot
    const headerContent = (
      <div className="agent-dispatch-header-wrapper">
        {/* Left icon column */}
        <div className="agent-dispatch-icon-col">
          <div className="agent-dispatch-icon-marks">
            <Bot className="agent-dispatch-icon" />
          </div>
        </div>

        {/* Main content */}
        <div className="agent-dispatch-body">
          <span className="agent-dispatch-label">{headerLine}</span>
          {workspace && action === 'create' && (
            workspace === 'global'
              ? <span className="agent-dispatch-global-tag agent-dispatch-global-tag--body">{t('toolCards.agentDispatch.globalTag')}</span>
              : <span className="agent-dispatch-body-path" title={workspace}>{workspace}</span>
          )}
        </div>

        {/* Right rail: status icon only */}
        <div className="agent-dispatch-rail">
          {headerRailIcon}
        </div>
      </div>
    );

    return (
      <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
        <BaseToolCard
          status={status}
          isExpanded={isExpanded}
          onClick={handleCardClick}
          className="agent-dispatch-card"
          expandedContent={expandedContent}
          headerExpandAffordance={hasExpandedContent && !canNavigate}
          header={headerContent}
        />
      </div>
    );
  }
);

AgentDispatchCard.displayName = 'AgentDispatchCard';
