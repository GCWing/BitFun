import React, { useEffect, useMemo, useReducer } from 'react';
import { Check, Loader2, Wrench, Brain } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type {
  DialogTurn,
  FlowTextItem,
  FlowThinkingItem,
  FlowToolItem,
  ModelRound,
  Session,
} from '../../types/flow-chat';
import { FlowChatStore } from '../../store/FlowChatStore';
import {
  useModelBrainstormStore,
  type ModelBrainstormBatch,
  type ModelBrainstormCandidate,
} from '../../store/modelBrainstormStore';
import { getEffectiveToolName } from '../../utils/toolInvocationIdentity';
import { Tooltip } from '@/component-library';
import { FlowChatContext, useFlowChatContext } from './FlowChatContext';
import { ModelRoundItem } from './ModelRoundItem';
import './ModelBrainstormGroup.scss';

const READONLY_TOOL_NAMES = new Set([
  'Read',
  'LS',
  'Grep',
  'Glob',
  'WebSearch',
  'WebFetch',
  'GetFileDiff',
  'GetToolSpec',
  'ListModels',
  'Skill',
  'view_image',
  'analyze_image',
  'ReadCanvas',
  'SessionHistory',
]);

interface ModelBrainstormGroupProps {
  batch: ModelBrainstormBatch;
}

interface CandidateProjection {
  candidate: ModelBrainstormCandidate;
  session?: Session;
  turn?: DialogTurn;
  status: 'pending' | 'starting' | 'running' | 'completed' | 'failed' | 'cancelled' | 'error';
  readonlyToolNames: string[];
  readonlyToolCount: number;
  thinkingChars: number;
}

function getCandidateSessionSignature(batch: ModelBrainstormBatch): string {
  const store = FlowChatStore.getInstance();
  const state = store.getState();
  return batch.candidates
    .map(candidate => {
      const session = candidate.sessionId ? state.sessions.get(candidate.sessionId) : undefined;
      const turn = session?.dialogTurns[session.dialogTurns.length - 1];
      const rounds = turn?.modelRounds ?? [];
      const itemCounts = rounds.map(round => `${round.id}:${round.status}:${round.items.length}:${round.isStreaming ? '1' : '0'}`).join(',');
      return [
        candidate.id,
        candidate.status,
        candidate.error ?? '',
        candidate.sessionId ?? '',
        turn?.id ?? '',
        turn?.status ?? '',
        itemCounts,
      ].join(':');
    })
    .join('|');
}

function useCandidateSessionUpdates(batch: ModelBrainstormBatch): void {
  const [, forceRender] = useReducer((value: number) => value + 1, 0);
  const signature = useMemo(
    () => batch.candidates.map(candidate => candidate.id).join('|'),
    [batch.candidates],
  );

  useEffect(() => {
    forceRender();
    return FlowChatStore.getInstance().subscribeSelector(
      () => getCandidateSessionSignature(batch),
      () => forceRender(),
      { isEqual: Object.is },
    );
  }, [batch, signature]);
}

function flattenRoundItems(rounds: ModelRound[]) {
  return rounds.flatMap(round => round.items ?? []);
}

function projectCandidate(batch: ModelBrainstormBatch, candidate: ModelBrainstormCandidate): CandidateProjection {
  const session = candidate.sessionId
    ? FlowChatStore.getInstance().getState().sessions.get(candidate.sessionId)
    : undefined;
  const turn = session?.dialogTurns.find(dialogTurn =>
    dialogTurn.userMessage?.metadata?.brainstormBatchId === batch.id &&
    dialogTurn.userMessage?.metadata?.brainstormCandidateId === candidate.id
  ) ?? session?.dialogTurns[session.dialogTurns.length - 1];
  const items = flattenRoundItems(turn?.modelRounds ?? []);
  const readonlyTools = items
    .filter((item): item is FlowToolItem => item.type === 'tool')
    .map(item => getEffectiveToolName(item))
    .filter(toolName => READONLY_TOOL_NAMES.has(toolName));
  const readonlyToolNames = Array.from(new Set(readonlyTools));
  const thinkingChars = items
    .filter((item): item is FlowThinkingItem => item.type === 'thinking')
    .reduce((total, item) => total + (item.content?.length ?? 0), 0);

  const status = (() => {
    if (candidate.status === 'failed') {
      return 'failed';
    }
    if (!turn) {
      return candidate.status;
    }
    if (turn.status === 'completed') {
      return 'completed';
    }
    if (turn.status === 'cancelled') {
      return 'cancelled';
    }
    if (turn.status === 'error') {
      return 'error';
    }
    return 'running';
  })();

  return {
    candidate,
    session,
    turn,
    status,
    readonlyToolNames,
    readonlyToolCount: readonlyTools.length,
    thinkingChars,
  };
}

function formatThinkingChars(chars: number): string {
  if (chars >= 1000) {
    return `${(chars / 1000).toFixed(chars >= 10_000 ? 0 : 1)}k`;
  }
  return String(chars);
}

function extractAssistantAnswer(turn: DialogTurn | undefined): string {
  if (!turn) {
    return '';
  }

  const roundsFromLatestFirst = [...turn.modelRounds].reverse();
  for (const round of roundsFromLatestFirst) {
    const text = round.items
      .filter((item): item is FlowTextItem => item.type === 'text')
      .filter(item => !item.runtimeStatus)
      .map(item => item.content.trim())
      .filter(Boolean)
      .join('\n\n')
      .trim();

    if (text) {
      return text;
    }
  }

  return '';
}

export const ModelBrainstormGroup: React.FC<ModelBrainstormGroupProps> = ({ batch }) => {
  const { t } = useTranslation('flow-chat');
  const parentContext = useFlowChatContext();
  useCandidateSessionUpdates(batch);

  const projections = batch.candidates.map(candidate => projectCandidate(batch, candidate));
  const completedCount = projections.filter(projection => projection.status === 'completed').length;
  const selectedCandidateIds =
    batch.selectedCandidateIds ??
    (batch.selectedCandidateId ? [batch.selectedCandidateId] : []);
  const statusLabels: Record<CandidateProjection['status'], string> = {
    pending: t('shared:statuses.loading'),
    starting: t('shared:statuses.loading'),
    running: t('shared:statuses.running'),
    completed: t('shared:statuses.done'),
    failed: t('shared:statuses.failed'),
    cancelled: t('shared:statuses.cancelled'),
    error: t('shared:statuses.failed'),
  };

  const handleSelect = (projection: CandidateProjection) => {
    if (!projection.candidate.sessionId || projection.status !== 'completed') {
      return;
    }

    const answer = extractAssistantAnswer(projection.turn);
    if (!answer) {
      return;
    }
    useModelBrainstormStore
      .getState()
      .toggleCandidatePublicSelection(batch.id, projection.candidate.id, answer);
  };

  return (
    <section
      className="model-brainstorm-group"
      data-testid="model-brainstorm-group"
      data-batch-id={batch.id}
    >
      <header className="model-brainstorm-group__header">
        <div className="model-brainstorm-group__title-block">
          <span className="model-brainstorm-group__eyebrow">{t('modelBrainstorm.groupEyebrow')}</span>
          <h3 className="model-brainstorm-group__title">{batch.displayQuestion}</h3>
        </div>
        <div className="model-brainstorm-group__progress">
          {t('modelBrainstorm.progress', {
            completed: completedCount,
            total: projections.length,
          })}
        </div>
      </header>

      <div className="model-brainstorm-group__grid">
        {projections.map(projection => {
          const selected = selectedCandidateIds.includes(projection.candidate.id);
          const statusLabel = statusLabels[projection.status];
          const readonlyTooltip = projection.readonlyToolNames.length > 0
            ? projection.readonlyToolNames.join(', ')
            : t('modelBrainstorm.noReadonlyTools');
          const canSelect = Boolean(projection.candidate.sessionId) && projection.status === 'completed';

          return (
            <article
              key={projection.candidate.id}
              className={[
                'model-brainstorm-group__candidate',
                selected ? 'model-brainstorm-group__candidate--selected' : '',
              ].filter(Boolean).join(' ')}
              data-testid="model-brainstorm-candidate"
              data-model-id={projection.candidate.modelId}
              data-status={projection.status}
            >
              <div className="model-brainstorm-group__candidate-header">
                <div className="model-brainstorm-group__model">
                  <span className="model-brainstorm-group__model-name">
                    {projection.candidate.modelLabel}
                  </span>
                  {projection.candidate.providerName && (
                    <span className="model-brainstorm-group__model-provider">
                      {projection.candidate.providerName}
                    </span>
                  )}
                </div>
                <span className={`model-brainstorm-group__status model-brainstorm-group__status--${projection.status}`}>
                  {(projection.status === 'running' || projection.status === 'starting' || projection.status === 'pending') && (
                    <Loader2 size={12} className="model-brainstorm-group__status-spinner" aria-hidden />
                  )}
                  {statusLabel}
                </span>
              </div>

              <div className="model-brainstorm-group__metrics">
                <Tooltip content={readonlyTooltip}>
                  <span className="model-brainstorm-group__metric">
                    <Wrench size={12} aria-hidden />
                    {t('modelBrainstorm.readonlyToolMetric', {
                      count: projection.readonlyToolCount,
                    })}
                  </span>
                </Tooltip>
                <span className="model-brainstorm-group__metric">
                  <Brain size={12} aria-hidden />
                  {t('modelBrainstorm.thinkingMetric', {
                    value: formatThinkingChars(projection.thinkingChars),
                  })}
                </span>
              </div>

              <div className="model-brainstorm-group__body">
                {projection.candidate.error ? (
                  <div className="model-brainstorm-group__error">
                    {projection.candidate.error}
                  </div>
                ) : projection.turn && projection.turn.modelRounds.length > 0 && projection.session ? (
                  <FlowChatContext.Provider
                    value={{
                      ...parentContext,
                      sessionId: projection.session.sessionId,
                      activeSessionOverride: projection.session,
                      allowUserMessageRollback: false,
                      allowUserMessageEdit: false,
                    }}
                  >
                    {projection.turn.modelRounds.map((round, index) => (
                      <ModelRoundItem
                        key={round.id}
                        round={round}
                        turnId={projection.turn!.id}
                        isLastRound={index === projection.turn!.modelRounds.length - 1}
                        isTurnComplete={
                          projection.turn!.status === 'completed' ||
                          projection.turn!.status === 'cancelled' ||
                          projection.turn!.status === 'error'
                        }
                        turnStartedAt={projection.turn!.startTime}
                        turnEndedAt={projection.turn!.endTime}
                        turnDurationMs={
                          typeof projection.turn!.endTime === 'number'
                            ? Math.max(0, projection.turn!.endTime - projection.turn!.startTime)
                            : undefined
                        }
                        turnTokenUsage={projection.turn!.tokenUsage}
                      />
                    ))}
                  </FlowChatContext.Provider>
                ) : (
                  <div className="model-brainstorm-group__empty">
                    <Loader2 size={16} className="model-brainstorm-group__empty-spinner" aria-hidden />
                    {statusLabel}
                  </div>
                )}
              </div>

              <button
                type="button"
                className="model-brainstorm-group__select"
                disabled={!canSelect}
                data-testid="model-brainstorm-select"
                onClick={() => {
                  handleSelect(projection);
                }}
              >
                {selected && <Check size={14} aria-hidden />}
                {selected ? t('modelBrainstorm.selectedCandidate') : t('modelBrainstorm.selectCandidate')}
              </button>
            </article>
          );
        })}
      </div>
    </section>
  );
};
