import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { SessionControlToolCard as SessionControlToolCardView } from '@openbitfun/ui/flow-chat';
import { useToolCardHeightContract } from './useToolCardHeightContract';

interface SessionSummary {
  session_id?: string;
  session_name?: string;
  agent_type?: string;
}

interface SessionControlInput {
  action?: 'create' | 'cancel' | 'delete' | 'rename' | 'list';
  workspace?: string;
  session_id?: string;
  session_name?: string;
  agent_type?: string;
}

interface SessionControlResult {
  success?: boolean;
  action?: 'create' | 'cancel' | 'delete' | 'rename' | 'list';
  workspace?: string;
  count?: number;
  session_id?: string;
  session_name?: string;
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
  const sessionName = session?.session_name ?? resultData?.session_name ?? inputData.session_name;
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

  const getActionLabel = () => {
    switch (action) {
      case 'create':
        return sessionName || t('toolCards.sessionControl.defaultSessionName');
      case 'cancel':
      case 'delete':
      case 'rename':
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
        case 'rename':
          return <>{t('toolCards.sessionControl.renamedSession', {
            session: label,
            name: sessionName || t('toolCards.sessionControl.defaultSessionName'),
          })}</>;
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
        case 'rename':
          return <>{t('toolCards.sessionControl.renamingSession', {
            session: label,
            name: sessionName || t('toolCards.sessionControl.defaultSessionName'),
          })}...</>;
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
      case 'rename':
        return <>{t('toolCards.sessionControl.preparingRename', {
          session: label,
          name: sessionName || t('toolCards.sessionControl.defaultSessionName'),
        })}</>;
      case 'list':
      default:
        return <>{t('toolCards.sessionControl.preparingList')}</>;
    }
  };

  const fields = [
    workspace ? { label: `${t('shared:features.workspace')}:`, value: workspace } : null,
    sessionId ? { label: `${t('toolCards.sessionControl.sessionId')}:`, value: sessionId } : null,
    sessionName ? { label: `${t('toolCards.sessionControl.sessionName')}:`, value: sessionName } : null,
    agentType ? { label: `${t('toolCards.sessionControl.agentType')}:`, value: agentType } : null,
    action === 'cancel' && cancelStatus ? {
      label: `${t('toolCards.sessionControl.cancelStatus')}:`,
      value: cancelStatus === 'no_active_turn'
        ? t('toolCards.sessionControl.noActiveTurnStatus')
        : t('toolCards.sessionControl.cancelRequestedStatus'),
    } : null,
    action === 'cancel' && cancelledTurnId
      ? { label: `${t('toolCards.sessionControl.cancelledTurnId')}:`, value: cancelledTurnId }
      : null,
    action === 'cancel' && hadActiveTurn !== undefined ? {
      label: `${t('toolCards.sessionControl.hadActiveTurn')}:`,
      value: hadActiveTurn
        ? t('toolCards.sessionControl.booleanYes')
        : t('toolCards.sessionControl.booleanNo'),
    } : null,
    action === 'list'
      ? { label: `${t('toolCards.sessionControl.sessionCount')}:`, value: sessionCount }
      : null,
  ].filter((field): field is NonNullable<typeof field> => Boolean(field));

  return (
    <div ref={cardRootRef} data-openbitfun-adapter="session-control" data-tool-card-id={toolId ?? ''}>
      <SessionControlToolCardView
        status={status}
        isExpanded={isExpanded}
        onToggle={hasDetails
          ? () => applyExpandedState(isExpanded, !isExpanded, setIsExpanded)
          : undefined}
        action={`${t('toolCards.sessionControl.title')}:`}
        summary={renderContent()}
        fields={fields}
        sessions={action === 'list' ? sessions.map((item, index) => ({
          agentType: item.agent_type || '-',
          id: item.session_id || t('toolCards.sessionControl.unknownSession'),
          key: `${item.session_id ?? 'session'}-${index}`,
          name: item.session_name || t('toolCards.sessionControl.defaultSessionName'),
        })) : undefined}
        emptyState={action === 'list' && sessions.length === 0 && status === 'completed'
          ? t('toolCards.sessionControl.noSessions')
          : undefined}
        error={toolResult?.error}
      />
    </div>
  );
});
