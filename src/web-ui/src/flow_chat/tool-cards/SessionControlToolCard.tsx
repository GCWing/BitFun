import React, { useMemo, useState } from 'react';
import { Check, Clock, Loader2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';

interface SessionSummary {
  session_id?: string;
  session_name?: string;
  agent_type?: string;
}

interface SessionControlInput {
  action?: 'create' | 'cancel' | 'delete' | 'list';
  workspace?: string;
  session_id?: string;
  session_name?: string;
  agent_type?: string;
}

interface SessionControlResult {
  success?: boolean;
  action?: 'create' | 'cancel' | 'delete' | 'list';
  workspace?: string;
  count?: number;
  session_id?: string;
  had_active_turn?: boolean;
  cancelled_turn_id?: string;
  status?: 'cancel_requested' | 'no_active_turn';
  session?: SessionSummary;
  sessions?: SessionSummary[];
}

function parseData<T>(value: unknown): T | null {
  if (!value) return null;

  try {
    return typeof value === 'string' ? JSON.parse(value) as T : value as T;
  } catch {
    return null;
  }
}

export const SessionControlToolCard: React.FC<ToolCardProps> = React.memo(({
  toolItem,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, toolResult, status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const inputData = useMemo(
    () => parseData<SessionControlInput>(toolCall?.input) ?? {},
    [toolCall?.input]
  );

  const resultData = useMemo(
    () => parseData<SessionControlResult>(toolResult?.result),
    [toolResult?.result]
  );

  const action = resultData?.action ?? inputData.action ?? 'list';
  const workspace = resultData?.workspace ?? inputData.workspace;
  const session = resultData?.session;
  const sessionId = session?.session_id ?? resultData?.session_id ?? inputData.session_id;
  const sessionName = session?.session_name ?? inputData.session_name;
  const agentType = session?.agent_type ?? inputData.agent_type;
  const sessions = Array.isArray(resultData?.sessions) ? resultData.sessions : [];
  const sessionCount = resultData?.count ?? sessions.length;
  const cancelStatus = resultData?.status;
  const hadActiveTurn = resultData?.had_active_turn;
  const cancelledTurnId = resultData?.cancelled_turn_id;
  const hasDetails = Boolean(
    workspace ||
    sessionId ||
    sessionName ||
    agentType ||
    sessions.length ||
    cancelStatus ||
    hadActiveTurn !== undefined ||
    cancelledTurnId ||
    toolResult?.error
  );

  const getStatusIcon = () => {
    switch (status) {
      case 'running':
      case 'streaming':
        return <Loader2 className="animate-spin" size={16} />;
      case 'completed':
        return <Check size={16} className="icon-check-done" />;
      case 'error':
      case 'cancelled':
        return <X size={16} />;
      case 'pending':
      case 'preparing':
      default:
        return <Clock size={16} />;
    }
  };

  const getActionLabel = () => {
    switch (action) {
      case 'create':
        return sessionName || t('toolCards.sessionControl.defaultSessionName');
      case 'cancel':
      case 'delete':
        return sessionId || t('toolCards.sessionControl.unknownSession');
      case 'list':
      default:
        return workspace || t('toolCards.sessionControl.currentWorkspace');
    }
  };

  const renderContent = () => {
    const label = getActionLabel();

    if (status === 'completed') {
      switch (action) {
        case 'create':
          return <>{t('toolCards.sessionControl.createdSession', { session: label })}</>;
        case 'cancel':
          if (cancelStatus === 'no_active_turn') {
            return <>{t('toolCards.sessionControl.noActiveTurn', { session: label })}</>;
          }
          return <>{t('toolCards.sessionControl.cancelledSession', { session: label })}</>;
        case 'delete':
          return <>{t('toolCards.sessionControl.deletedSession', { session: label })}</>;
        case 'list':
        default:
          return <>{t('toolCards.sessionControl.listedSessions', { count: sessionCount })}</>;
      }
    }

    if (status === 'running' || status === 'streaming') {
      switch (action) {
        case 'create':
          return <>{t('toolCards.sessionControl.creatingSession', { session: label })}...</>;
        case 'cancel':
          return <>{t('toolCards.sessionControl.cancellingSession', { session: label })}...</>;
        case 'delete':
          return <>{t('toolCards.sessionControl.deletingSession', { session: label })}...</>;
        case 'list':
        default:
          return <>{t('toolCards.sessionControl.listingSessions')}...</>;
      }
    }

    if (status === 'error' || status === 'cancelled') {
      return <>{t('toolCards.sessionControl.actionFailed')}</>;
    }

    switch (action) {
      case 'create':
        return <>{t('toolCards.sessionControl.preparingCreate', { session: label })}</>;
      case 'cancel':
        return <>{t('toolCards.sessionControl.preparingCancel', { session: label })}</>;
      case 'delete':
        return <>{t('toolCards.sessionControl.preparingDelete', { session: label })}</>;
      case 'list':
      default:
        return <>{t('toolCards.sessionControl.preparingList')}</>;
    }
  };

  const expandedContent = hasDetails ? (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
      {workspace && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionControl.workspace')}:</span>
          <span className="detail-value">{workspace}</span>
        </div>
      )}

      {sessionId && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionControl.sessionId')}:</span>
          <span className="detail-value">{sessionId}</span>
        </div>
      )}

      {sessionName && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionControl.sessionName')}:</span>
          <span className="detail-value">{sessionName}</span>
        </div>
      )}

      {agentType && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionControl.agentType')}:</span>
          <span className="detail-value">{agentType}</span>
        </div>
      )}

      {action === 'cancel' && cancelStatus && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionControl.cancelStatus')}:</span>
          <span className="detail-value">
            {cancelStatus === 'no_active_turn'
              ? t('toolCards.sessionControl.noActiveTurnStatus')
              : t('toolCards.sessionControl.cancelRequestedStatus')}
          </span>
        </div>
      )}

      {action === 'cancel' && cancelledTurnId && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionControl.cancelledTurnId')}:</span>
          <span className="detail-value">{cancelledTurnId}</span>
        </div>
      )}

      {action === 'cancel' && hadActiveTurn !== undefined && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionControl.hadActiveTurn')}:</span>
          <span className="detail-value">
            {hadActiveTurn
              ? t('toolCards.sessionControl.booleanYes')
              : t('toolCards.sessionControl.booleanNo')}
          </span>
        </div>
      )}

      {action === 'list' && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionControl.sessionCount')}:</span>
          <span className="detail-value">{sessionCount}</span>
        </div>
      )}

      {action === 'list' && sessions.length > 0 && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {sessions.map((item, index) => (
            <div
              key={`${item.session_id ?? 'session'}-${index}`}
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 2,
                padding: '8px 10px',
                borderRadius: 8,
                background: 'var(--color-bg-subtle, rgba(255,255,255,0.04))'
              }}
            >
              <span style={{ fontFamily: 'var(--tool-card-font-mono)', wordBreak: 'break-all' }}>
                {item.session_id || t('toolCards.sessionControl.unknownSession')}
              </span>
              <span>{item.session_name || t('toolCards.sessionControl.defaultSessionName')}</span>
              <span style={{ opacity: 0.7 }}>{item.agent_type || '-'}</span>
            </div>
          ))}
        </div>
      )}

      {action === 'list' && sessions.length === 0 && status === 'completed' && (
        <div style={{ opacity: 0.7 }}>
          {t('toolCards.sessionControl.noSessions')}
        </div>
      )}

      {toolResult?.error && (
        <div style={{ color: 'var(--color-danger-text, #f87171)', whiteSpace: 'pre-wrap' }}>
          {toolResult.error}
        </div>
      )}
    </div>
  ) : null;

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <CompactToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={() => {
          if (hasDetails) {
            applyExpandedState(isExpanded, !isExpanded, setIsExpanded);
          }
        }}
        className="session-control-card"
        clickable={hasDetails}
        header={(
          <CompactToolCardHeader
            icon={getStatusIcon()}
            action={`${t('toolCards.sessionControl.title')}:`}
            content={renderContent()}
            extra={action === 'list' && status === 'completed' ? `${sessionCount}` : undefined}
          />
        )}
        expandedContent={expandedContent}
      />
    </div>
  );
});
