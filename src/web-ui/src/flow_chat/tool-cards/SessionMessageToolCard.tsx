import React, { useMemo, useState } from 'react';
import { Check, Clock, Loader2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';

interface SessionMessageInput {
  workspace?: string;
  session_id?: string;
  message?: string;
  agent_type?: string;
}

interface SessionMessageResult {
  success?: boolean;
  target_workspace?: string;
  target_session_id?: string;
  target_agent_type?: string;
}

function parseData<T>(value: unknown): T | null {
  if (!value) return null;

  try {
    return typeof value === 'string' ? JSON.parse(value) as T : value as T;
  } catch {
    return null;
  }
}

export const SessionMessageToolCard: React.FC<ToolCardProps> = React.memo(({
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
    () => parseData<SessionMessageInput>(toolCall?.input) ?? {},
    [toolCall?.input]
  );

  const resultData = useMemo(
    () => parseData<SessionMessageResult>(toolResult?.result),
    [toolResult?.result]
  );

  const targetSessionId = resultData?.target_session_id ?? inputData.session_id;
  const workspace = resultData?.target_workspace ?? inputData.workspace;
  const agentType = resultData?.target_agent_type ?? inputData.agent_type;
  const message = inputData.message ?? '';
  const hasDetails = Boolean(targetSessionId || workspace || agentType || message || toolResult?.error);

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

  const targetLabel = targetSessionId || t('toolCards.sessionMessage.unknownSession');

  const renderContent = () => {
    if (status === 'completed') {
      return <>{t('toolCards.sessionMessage.messageAccepted', { session: targetLabel })}</>;
    }

    if (status === 'running' || status === 'streaming') {
      return <>{t('toolCards.sessionMessage.sendingMessage', { session: targetLabel })}...</>;
    }

    if (status === 'error' || status === 'cancelled') {
      return <>{t('toolCards.sessionMessage.sendFailed', { session: targetLabel })}</>;
    }

    return <>{t('toolCards.sessionMessage.preparingSend', { session: targetLabel })}</>;
  };

  const expandedContent = hasDetails ? (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
      {targetSessionId && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionMessage.targetSession')}:</span>
          <span className="detail-value">{targetSessionId}</span>
        </div>
      )}

      {workspace && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionMessage.workspace')}:</span>
          <span className="detail-value">{workspace}</span>
        </div>
      )}

      {agentType && (
        <div className="detail-item">
          <span className="detail-label">{t('toolCards.sessionMessage.agentType')}:</span>
          <span className="detail-value">{agentType}</span>
        </div>
      )}

      {message && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <span className="detail-label">{t('toolCards.sessionMessage.message')}:</span>
          <pre
            style={{
              margin: 0,
              padding: '10px 12px',
              borderRadius: 8,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
              background: 'var(--color-bg-subtle, rgba(255,255,255,0.04))',
              fontFamily: 'var(--tool-card-font-mono)'
            }}
          >
            {message}
          </pre>
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
        className="session-message-card"
        clickable={hasDetails}
        header={(
          <CompactToolCardHeader
            icon={getStatusIcon()}
            action={`${t('toolCards.sessionMessage.title')}:`}
            content={renderContent()}
            extra={agentType ? agentType : undefined}
          />
        )}
        expandedContent={expandedContent}
      />
    </div>
  );
});
