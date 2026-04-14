/**
 * AgentDispatch tool card.
 *
 * Visually similar to TaskToolDisplay but represents an independent Standard
 * session (not a SubAgent). Clicking the card jumps to the created session.
 */

import React, { useCallback, useMemo, useState } from 'react';
import { Bot, Check, ChevronRight, Loader2, X, ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { BaseToolCard } from './BaseToolCard';
import { openMainSession } from '../services/openBtwSession';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './AgentDispatchCard.scss';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
  ({ toolItem, sessionId }) => {
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

    // Status icon
    const statusIcon = useMemo(() => {
      switch (status) {
        case 'running':
        case 'streaming':
          return <Loader2 className="animate-spin agent-dispatch-status-icon" size={13} />;
        case 'completed':
          return <Check size={13} className="agent-dispatch-status-icon agent-dispatch-status-icon--done" />;
        case 'error':
        case 'cancelled':
          return <X size={13} className="agent-dispatch-status-icon agent-dispatch-status-icon--error" />;
        default:
          return null;
      }
    }, [status]);

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
      // create
      const label = sessionName || agentType || t('toolCards.agentDispatch.agent');
      if (status === 'completed') {
        return t('toolCards.agentDispatch.createdAgent', { agent: label });
      }
      if (status === 'running' || status === 'streaming') {
        return t('toolCards.agentDispatch.creatingAgent', { agent: label });
      }
      if (status === 'error' || status === 'cancelled') {
        return t('toolCards.agentDispatch.actionFailed');
      }
      return t('toolCards.agentDispatch.preparingCreate', { agent: label });
    }, [action, agentType, resultData, sessionName, status, t]);

    // Jump to the created session when the card is clicked (create action, completed)
    const canNavigate =
      action === 'create' && status === 'completed' && !!createdSessionId;

    const handleNavigate = useCallback(
      async (e: React.MouseEvent) => {
        e.stopPropagation();
        if (!createdSessionId) return;
        await openMainSession(createdSessionId);
      },
      [createdSessionId]
    );

    const handleCardClick = useCallback(
      (e: React.MouseEvent) => {
        if ((e.target as HTMLElement).closest('.agent-dispatch-nav-btn')) return;
        if (action === 'create' && status === 'completed' && createdSessionId) {
          void openMainSession(createdSessionId);
          return;
        }
        if (action !== 'create' || !createdSessionId) {
          applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
        }
      },
      [action, applyExpandedState, createdSessionId, isExpanded, status]
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
              <div key={i} className={`agent-dispatch-workspace-item${ws.kind === 'global' ? ' agent-dispatch-workspace-item--global' : ''}`}>
                <div className="agent-dispatch-workspace-header">
                  <span className="agent-dispatch-workspace-name">{ws.name ?? ws.path}</span>
                  {ws.kind === 'global' && (
                    <span className="agent-dispatch-global-tag">{t('toolCards.agentDispatch.globalTag')}</span>
                  )}
                </div>
                <span className="agent-dispatch-workspace-path">{ws.path}</span>
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
            {sessions.map((s, i) => (
              <div
                key={i}
                className={`agent-dispatch-session-item${s.session_id ? ' agent-dispatch-session-item--clickable' : ''}`}
                onClick={s.session_id ? () => openMainSession(s.session_id!) : undefined}
              >
                <span className="agent-dispatch-session-name">{s.session_name ?? s.session_id}</span>
                {s.agent_type && <AgentBadge agentType={s.agent_type} />}
                {s.session_id && <ExternalLink size={11} className="agent-dispatch-session-link-icon" />}
              </div>
            ))}
          </div>
        );
      }

      return null;
    }, [action, createdSessionId, resultData, t, workspace]);

    const hasExpandedContent = !!expandedContent;

    // Header content rendered inside BaseToolCard's header slot
    const headerContent = (
      <div className="agent-dispatch-header-wrapper">
        {/* Left icon column */}
        <div className="agent-dispatch-icon-col">
          <Bot size={16} className="agent-dispatch-icon" />
        </div>

        {/* Main content */}
        <div className="agent-dispatch-body">
          <div className="agent-dispatch-header-main">
            <span className="agent-dispatch-label">{headerLine}</span>
            {agentType && action === 'create' && <AgentBadge agentType={agentType} />}
          </div>
          {workspace && action === 'create' && (
            <div className="agent-dispatch-workspace-hint">
              {workspace === 'global'
                ? <span className="agent-dispatch-global-tag">{t('toolCards.agentDispatch.globalTag')}</span>
                : workspace}
            </div>
          )}
        </div>

        {/* Right rail: status + optional navigate button */}
        <div className="agent-dispatch-rail">
          {statusIcon}
          {canNavigate && (
            <button
              type="button"
              className="agent-dispatch-nav-btn"
              onClick={handleNavigate}
              title={t('toolCards.agentDispatch.clickToSwitch')}
            >
              <ChevronRight size={14} />
            </button>
          )}
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
