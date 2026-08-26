import React, {
  useCallback,
  useLayoutEffect,
  useState,
  useSyncExternalStore,
} from 'react';
import { Bot, ChevronDown, ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Markdown } from '@/component-library/components/Markdown/Markdown';
import { flowChatStore } from '../store/FlowChatStore';
import {
  SubagentAvatar,
} from '../subagent-identity';
import type { FlowToolItem, ToolCardProps } from '../types/flow-chat';
import {
  sessionLineageLifecycleForSession,
  type SessionLineageLifecycle,
} from '../utils/sessionLineage';
import { openBtwSessionInAuxPane } from '../services/btwSessionPane';
import { BaseToolCard } from './BaseToolCard';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './AgentControlToolCard.scss';

const PARAMETER_STREAMING_STATUSES = new Set<FlowToolItem['status']>([
  'preparing',
  'streaming',
  'receiving',
]);

function readString(source: unknown, ...keys: string[]): string {
  if (!source || typeof source !== 'object') {
    return '';
  }

  const record = source as Record<string, unknown>;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === 'string' && value.trim()) {
      return value.trim();
    }
  }
  return '';
}

function fallbackLifecycle(status: FlowToolItem['status']): SessionLineageLifecycle {
  switch (status) {
    case 'completed':
      return 'completed';
    case 'cancelled':
    case 'rejected':
      return 'cancelled';
    case 'error':
      return 'error';
    case 'waiting':
      return 'waiting';
    case 'pending':
    case 'queued':
      return 'idle';
    default:
      return 'running';
  }
}

function subscribeToFlowChatStore(listener: () => void): () => void {
  return flowChatStore.subscribe(() => listener());
}

function readLinkedAgentSnapshot(sessionId: string): string {
  if (!sessionId) {
    return '';
  }

  const session = flowChatStore.getState().sessions.get(sessionId);
  const latestTurn = session?.dialogTurns?.[session.dialogTurns.length - 1];
  return JSON.stringify([
    session?.title ?? '',
    session?.mode ?? '',
    session?.subagentType ?? '',
    session?.config?.agentType ?? '',
    session?.needsUserAttention ?? false,
    session?.status ?? '',
    session?.persistedStatus ?? '',
    session?.hasUnreadCompletion ?? '',
    latestTurn?.id ?? '',
    latestTurn?.status ?? '',
    latestTurn?.modelRounds?.some(round => round.isStreaming) ?? false,
  ]);
}

export const AgentControlToolCard: React.FC<ToolCardProps> = ({
  toolItem,
  sessionId,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, status } = toolItem;
  const toolId = toolItem.id ?? toolCall?.id;
  const params = toolItem.partialParams ?? toolCall?.input;
  const prompt = readString(params, 'prompt');
  const inputAgentType = readString(params, 'agent_type', 'agentType');
  const agentId = readString(params, 'agent_id', 'agentId')
    || readString(toolItem.toolResult?.result, 'agent_id', 'agentId');
  const linkedSubagentSessionId = toolItem.subagentSessionId ?? '';
  const readSnapshot = useCallback(
    () => readLinkedAgentSnapshot(linkedSubagentSessionId),
    [linkedSubagentSessionId],
  );

  useSyncExternalStore(
    subscribeToFlowChatStore,
    readSnapshot,
    readSnapshot,
  );

  const linkedSession = linkedSubagentSessionId
    ? flowChatStore.getState().sessions.get(linkedSubagentSessionId)
    : undefined;
  const lifecycle = linkedSession
    ? sessionLineageLifecycleForSession(linkedSession)
    : fallbackLifecycle(status);
  const agentName = agentId
    || linkedSession?.subagentType?.trim()
    || linkedSession?.mode?.trim()
    || inputAgentType
    || t('toolCards.taskTool.defaultAgentKind');
  const stableAgentType = linkedSession?.mode?.trim()
    || linkedSession?.config?.agentType?.trim()
    || inputAgentType;
  const stableSubagentType = linkedSession?.subagentType?.trim() || inputAgentType;
  const secondaryAgentType = stableSubagentType || stableAgentType;
  const isParameterStreaming = Boolean(toolItem.isParamsStreaming)
    || PARAMETER_STREAMING_STATUSES.has(status);
  const canExpand = Boolean(prompt) && !isParameterStreaming;
  const canOpenSession = Boolean(linkedSubagentSessionId && sessionId);
  const [isExpanded, setIsExpanded] = useState(false);
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const updateExpandedState = useCallback((nextExpanded: boolean) => {
    applyExpandedState(isExpanded, nextExpanded, setIsExpanded);
  }, [applyExpandedState, isExpanded]);

  useLayoutEffect(() => {
    if (isParameterStreaming && isExpanded) {
      updateExpandedState(false);
    }
  }, [isExpanded, isParameterStreaming, updateExpandedState]);

  const handleToggle = useCallback(() => {
    if (!canExpand) {
      return;
    }
    updateExpandedState(!isExpanded);
  }, [canExpand, isExpanded, updateExpandedState]);

  const handleOpenSession = useCallback((event: React.MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    if (!linkedSubagentSessionId || !sessionId) {
      return;
    }

    const parentSession = flowChatStore.getState().sessions.get(sessionId);
    openBtwSessionInAuxPane({
      childSessionId: linkedSubagentSessionId,
      parentSessionId: sessionId,
      workspacePath: parentSession?.workspacePath,
      sessionKind: 'subagent',
      sessionTitle: agentName,
      agentType: stableAgentType || undefined,
      parentToolCallId: toolCall?.id || toolItem.id,
      subagentType: stableSubagentType || undefined,
      remoteConnectionId: parentSession?.remoteConnectionId,
      remoteSshHost: parentSession?.remoteSshHost,
      includeInternal: true,
    });
  }, [
    agentName,
    linkedSubagentSessionId,
    sessionId,
    stableAgentType,
    stableSubagentType,
    toolCall?.id,
    toolItem.id,
  ]);

  const agentPillContent = (
    <>
      <span
        className="agent-control-tool-card__avatar"
        data-bf-component="agent-control-tool-card"
        data-bf-part="avatar"
      >
        {linkedSubagentSessionId ? (
          <SubagentAvatar
            sessionId={linkedSubagentSessionId}
            name={agentName}
            size={22}
            status={lifecycle}
          />
        ) : (
          <span className="agent-control-tool-card__fallback-avatar" aria-hidden="true">
            <Bot size={14} strokeWidth={2} />
          </span>
        )}
      </span>
      <span
        className="agent-control-tool-card__name"
        data-bf-component="agent-control-tool-card"
        data-bf-part="name"
      >
        {agentName}
      </span>
    </>
  );

  const header = (
    <div
      className="agent-control-tool-card__header"
      data-bf-component="agent-control-tool-card"
      data-bf-part="header"
    >
      {canOpenSession ? (
        <button
          type="button"
          className="agent-control-tool-card__agent-pill"
          data-bf-component="agent-control-tool-card"
          data-bf-part="agentPill"
          onClick={handleOpenSession}
          aria-label={t('toolCards.taskTool.openInPanel')}
          title={t('toolCards.taskTool.openInPanel')}
        >
          {agentPillContent}
        </button>
      ) : (
        <span
          className="agent-control-tool-card__agent-pill agent-control-tool-card__agent-pill--static"
          data-bf-component="agent-control-tool-card"
          data-bf-part="agentPill"
        >
          {agentPillContent}
        </span>
      )}
      {secondaryAgentType && secondaryAgentType !== agentName ? (
        <span
          className="agent-control-tool-card__type"
          data-bf-component="agent-control-tool-card"
          data-bf-part="type"
        >
          {secondaryAgentType}
        </span>
      ) : null}
      <span
        className={`agent-control-tool-card__status agent-control-tool-card__status--${lifecycle}`}
        data-bf-component="agent-control-tool-card"
        data-bf-part="status"
        data-bf-status={lifecycle}
      >
        {t(`flowChatHeader.agentTreeStatus.${lifecycle}`)}
      </span>
      {prompt ? (
        <span
          className="agent-control-tool-card__expand-indicator"
          data-bf-component="agent-control-tool-card"
          data-bf-part="expandIndicator"
          aria-hidden="true"
        >
          {isExpanded ? (
            <ChevronDown size={15} strokeWidth={2} />
          ) : (
            <ChevronRight size={15} strokeWidth={2} />
          )}
        </span>
      ) : null}
    </div>
  );

  return (
    <div
      ref={cardRootRef}
      className="agent-control-tool-card"
      data-bf-component="agent-control-tool-card"
      data-bf-part="root"
      data-bf-status={lifecycle}
      data-bf-state={[
        isExpanded && 'expanded',
        isParameterStreaming && 'streaming',
        canOpenSession && 'openable',
      ].filter(Boolean).join(' ') || undefined}
      data-tool-card-id={toolId ?? ''}
    >
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={canExpand ? handleToggle : undefined}
        toggleTestId="agent-control-tool-card-toggle"
        className="agent-control-tool-card__base"
        header={header}
        expandedContent={prompt ? (
          <div
            className="agent-control-tool-card__prompt"
            data-bf-component="agent-control-tool-card"
            data-bf-part="prompt"
          >
            <Markdown
              content={prompt}
              isStreaming={false}
              className="agent-control-tool-card__prompt-markdown"
            />
          </div>
        ) : null}
        headerExpandAffordance={false}
      />
    </div>
  );
};
