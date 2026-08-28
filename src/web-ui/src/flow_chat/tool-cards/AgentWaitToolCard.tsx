import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Hourglass } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ActivityItem } from '@bitfun/ui';

import { Tooltip } from '@/component-library';
import { flowChatStore } from '../store/FlowChatStore';
import type { Session, ToolCardProps, ToolCardDisplayContext } from '../types/flow-chat';
import { isAcpFlowSession } from '../utils/acpSession';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import './AgentWaitToolCard.scss';

const RUNNING_STATUSES = new Set(['pending', 'preparing', 'running', 'streaming', 'receiving']);

interface AgentWaitResult {
  status?: string;
  results?: unknown[];
  pending_bg_task_ids?: string[];
}

const TruncatedSteeringHint: React.FC<{ text: string }> = ({ text }) => {
  const hintRef = useRef<HTMLSpanElement>(null);
  const [isTruncated, setIsTruncated] = useState(false);

  useEffect(() => {
    const hint = hintRef.current;
    if (!hint) return undefined;

    const measure = () => setIsTruncated(hint.scrollWidth > hint.clientWidth + 1);
    measure();

    if (typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(measure);
    observer.observe(hint);
    return () => observer.disconnect();
  }, [text]);

  return (
    <Tooltip content={text} placement="top" delay={300} disabled={!isTruncated}>
      <span ref={hintRef} className="agent-wait-tool-card__steering-hint-inline">
        {text}
      </span>
    </Tooltip>
  );
};

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

  const state = status === 'error' ? 'failed' : undefined;

  return (
    <div
      data-bf-component="agent-wait-tool-card"
      data-bf-part="root"
      data-bf-state={state}
      data-tool-card-id={toolId ?? ''}
      className="agent-wait-tool-card"
    >
      <ActivityItem
        appearance="inline"
        className="agent-wait-tool-card__activity"
        data-bf-status={status}
        label={t('toolCards.agentWait.title')}
        leading={<ToolCardStatusSlot status={status} toolIcon={<Hourglass size={16} />} />}
      >
        {showSteeringHint
          ? <TruncatedSteeringHint text={steeringHint} />
          : summary}
      </ActivityItem>
    </div>
  );
};
