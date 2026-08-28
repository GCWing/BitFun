import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { flowChatStore } from '../store/FlowChatStore';
import type { Session, ToolCardProps, ToolCardDisplayContext } from '../types/flow-chat';
import { isAcpFlowSession } from '../utils/acpSession';
import { AgentWaitToolCard as AgentWaitToolCardView } from '@bitfun/ui/flow-chat';

const RUNNING_STATUSES = new Set(['pending', 'preparing', 'running', 'streaming', 'receiving']);

interface AgentWaitResult {
  status?: string;
  results?: unknown[];
  pending_bg_task_ids?: string[];
}

export function shouldShowAgentWaitSteeringHint(
  status: ToolCardProps['toolItem']['status'],
  displayContext: ToolCardDisplayContext | undefined,
  session: Session | null | undefined,
): boolean {
  if (!RUNNING_STATUSES.has(status) || displayContext === 'subagent-projection' || !session) {
    return false;
  }

  return session.sessionKind !== 'subagent'
    && !session.parentToolCallId
    && !session.isHistorical
    && !isAcpFlowSession(session);
}

export const AgentWaitToolCard: React.FC<ToolCardProps> = ({
  toolItem,
  sessionId,
  displayContext,
}) => {
  const { t } = useTranslation('flow-chat');
  const { status, toolCall, toolResult } = toolItem;
  const toolId = toolItem.id ?? toolCall?.id;
  const result = toolResult?.result as AgentWaitResult | undefined;
  const session = sessionId ? flowChatStore.getState().sessions.get(sessionId) : undefined;
  const showSteeringHint = shouldShowAgentWaitSteeringHint(status, displayContext, session);
  const steeringHint = t('toolCards.agentWait.steeringHint');

  const summary = useMemo(() => {
    if (RUNNING_STATUSES.has(status)) {
      return null;
    }
    if (status === 'error') {
      const error = toolResult?.error?.trim();
      return error
        ? t('toolCards.agentWait.failedWithError', { error })
        : t('toolCards.agentWait.failed');
    }
    if (result?.status === 'steered') {
      return t('toolCards.agentWait.steered');
    }
    if (result?.status === 'timed_out') {
      return t('toolCards.agentWait.timedOut', {
        count: result.pending_bg_task_ids?.length ?? 0,
      });
    }
    if (status === 'completed') {
      return t('toolCards.agentWait.completed', { count: result?.results?.length ?? 0 });
    }
    return t('toolCards.agentWait.title');
  }, [result, status, t, toolResult?.error]);

  return (
    <AgentWaitToolCardView
      action={t('toolCards.agentWait.title')}
      data-tool-card-id={toolId ?? ''}
      status={status}
      summary={showSteeringHint ? steeringHint : summary}
      summaryTitle={showSteeringHint ? steeringHint : undefined}
    />
  );
};
