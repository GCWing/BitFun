import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { getProviderDisplayName } from '@/infrastructure/config/services/modelConfigs';
import type { AIModelConfig } from '@/infrastructure/config/types';
import {
  areSensitiveDiagnosticsEnabled,
  createLogger,
  logger,
  LogLevel,
} from '@/shared/utils/logger';
import type { ContextItem, ImageContext } from '@/shared/types/context';
import type { DialogTurn, FlowTextItem, SessionConfig } from '../types/flow-chat';
import { FlowChatManager } from './FlowChatManager';
import {
  useModelBrainstormStore,
  type ModelBrainstormBatch,
  type ModelBrainstormCandidate,
  type ModelBrainstormContextMode,
  type ModelBrainstormPublicSelection,
} from '../store/modelBrainstormStore';
import { buildImagePayload } from '../utils/imagePayload';
import { buildPromptMessage, stripInlineImageTags } from '../utils/messagePrompt';

const log = createLogger('ModelBrainstormService');

export const MODEL_BRAINSTORM_MAX_CANDIDATES = 4;
export const MODEL_BRAINSTORM_MIN_CANDIDATES = 2;
const LEDGER_LOG_PREVIEW_MAX_CHARS = 360;

interface BrainstormModelInfo {
  id: string;
  label: string;
  providerName: string;
}

export interface LaunchModelBrainstormRequest {
  message: string;
  displayMessage?: string;
  contexts: ContextItem[];
  sourceSessionId?: string;
  workspaceConfig: SessionConfig;
  agentType: string;
  modelIds: string[];
  contextMode: ModelBrainstormContextMode;
}

export interface LaunchModelBrainstormResult {
  batchId: string;
  sourceSessionId: string;
}

function isTextChatModel(model: AIModelConfig): model is AIModelConfig & { id: string } {
  if (!model.enabled || !model.id) {
    return false;
  }

  const capabilities = Array.isArray(model.capabilities) ? model.capabilities : [];
  return capabilities.includes('text_chat');
}

function uniqueModelIds(modelIds: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const rawModelId of modelIds) {
    const modelId = rawModelId.trim();
    if (!modelId || seen.has(modelId)) {
      continue;
    }
    seen.add(modelId);
    result.push(modelId);
  }
  return result.slice(0, MODEL_BRAINSTORM_MAX_CANDIDATES);
}

async function resolveSelectedModels(modelIds: string[]): Promise<BrainstormModelInfo[]> {
  const configData = await configManager.getConfigs(['ai.models']);
  const allModels = (configData['ai.models'] as AIModelConfig[] | undefined) || [];
  const availableModels = allModels.filter(isTextChatModel);
  const byId = new Map(availableModels.map(model => [model.id, model]));

  return uniqueModelIds(modelIds)
    .map(modelId => byId.get(modelId))
    .filter((model): model is AIModelConfig & { id: string } => Boolean(model))
    .map(model => ({
      id: model.id,
      label: model.model_name || model.name || model.id,
      providerName: getProviderDisplayName(model),
    }));
}

function buildCandidateId(batchId: string, modelId: string, index: number): string {
  const normalizedModelId = modelId.replace(/[^a-zA-Z0-9_-]/g, '-');
  return `${batchId}_${index}_${normalizedModelId}`;
}

function buildBatchId(): string {
  return `brainstorm_${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;
}

function buildCandidateSessionTitle(model: BrainstormModelInfo): string {
  return `Brainstorm · ${model.label}`;
}

function normalizeAnswerKey(answer: string): string {
  return answer.replace(/\s+/g, ' ').trim().toLowerCase();
}

function shouldLogLedgerDiagnostics(): boolean {
  return logger.getLevel() <= LogLevel.DEBUG;
}

function previewForLog(value: string | undefined, maxChars = LEDGER_LOG_PREVIEW_MAX_CHARS): string | undefined {
  if (!value || !areSensitiveDiagnosticsEnabled()) {
    return undefined;
  }

  const normalized = value.replace(/\s+/g, ' ').trim();
  if (normalized.length <= maxChars) {
    return normalized;
  }

  return `${normalized.slice(0, maxChars)}...`;
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

function getCandidateTurn(
  batch: ModelBrainstormBatch,
  candidate: ModelBrainstormCandidate,
  manager: FlowChatManager,
): DialogTurn | undefined {
  const session = candidate.sessionId
    ? manager.getFlowChatState().sessions.get(candidate.sessionId)
    : undefined;
  return session?.dialogTurns.find(dialogTurn =>
    dialogTurn.userMessage?.metadata?.brainstormBatchId === batch.id &&
    dialogTurn.userMessage?.metadata?.brainstormCandidateId === candidate.id
  ) ?? session?.dialogTurns[session.dialogTurns.length - 1];
}

function hydrateCompletedCandidateAnswers(
  sourceSessionId: string,
  manager: FlowChatManager,
): ModelBrainstormBatch[] {
  const store = useModelBrainstormStore.getState();
  const batches = store.getBatchesForSession(sourceSessionId);

  for (const batch of batches) {
    for (const candidate of batch.candidates) {
      if (candidate.answer?.trim()) {
        continue;
      }

      const turn = getCandidateTurn(batch, candidate, manager);
      if (turn?.status !== 'completed') {
        continue;
      }

      const answer = extractAssistantAnswer(turn);
      if (answer) {
        store.setCandidateAnswer(batch.id, candidate.id, answer, turn.endTime);
      }
    }
  }

  return useModelBrainstormStore.getState().getBatchesForSession(sourceSessionId);
}

function appendPublicSelections(
  lines: string[],
  publicSelections: ModelBrainstormPublicSelection[],
  seenAnswerKeys: Set<string>,
): void {
  const uniqueSelections = publicSelections.filter(selection => {
    const key = normalizeAnswerKey(selection.answer);
    if (!key || seenAnswerKeys.has(key)) {
      return false;
    }
    seenAnswerKeys.add(key);
    return true;
  });

  if (uniqueSelections.length === 0) {
    return;
  }

  lines.push('Public selections from this round:');
  uniqueSelections.forEach(selection => {
    lines.push(`${selection.modelLabel}:`);
    lines.push(selection.answer.trim());
    lines.push('');
  });
}

function buildLedgerPrompt(
  promptMessage: string,
  targetModel: BrainstormModelInfo,
  contextMode: ModelBrainstormContextMode,
  previousBatches: ModelBrainstormBatch[],
): string {
  if (previousBatches.length === 0) {
    return promptMessage;
  }

  const lines = [
    'You are participating in a multi-model brainstorm.',
    contextMode === 'independent'
      ? 'Context mode: independent. Use your own previous answers plus the user-selected public answers.'
      : 'Context mode: shared. Use only user questions and user-selected public answers; ignore private answers that were not selected.',
    'The ledger below is the authoritative context for this brainstorm. Public selections are user-approved shared context.',
    'Do not claim you cannot see selected public answers that appear in the ledger.',
    '',
  ];

  previousBatches.forEach((batch, index) => {
    const seenAnswerKeys = new Set<string>();
    const publicSelections = batch.publicSelections ?? [];
    lines.push(`Round ${index + 1} user question:`);
    lines.push(batch.question || batch.displayQuestion);
    lines.push('');

    if (contextMode === 'independent') {
      const ownCandidate = batch.candidates.find(candidate =>
        candidate.modelId === targetModel.id && candidate.answer?.trim()
      );
      if (ownCandidate?.answer) {
        const wasPublic = publicSelections.some(selection => selection.candidateId === ownCandidate.id);
        lines.push(`${ownCandidate.modelLabel} previous answer${wasPublic ? ' (selected public answer)' : ''}:`);
        lines.push(ownCandidate.answer.trim());
        lines.push('');
        seenAnswerKeys.add(normalizeAnswerKey(ownCandidate.answer));
      }
    }

    appendPublicSelections(lines, publicSelections, seenAnswerKeys);
  });

  return [
    ...lines,
    'Current user question:',
    promptMessage,
  ].join('\n');
}

function buildLedgerLogSummary(
  targetModel: BrainstormModelInfo,
  contextMode: ModelBrainstormContextMode,
  previousBatches: ModelBrainstormBatch[],
) {
  return previousBatches.map((batch, index) => {
    const publicSelections = batch.publicSelections ?? [];
    const seenAnswerKeys = new Set<string>();
    const ownCandidate = contextMode === 'independent'
      ? batch.candidates.find(candidate =>
          candidate.modelId === targetModel.id && candidate.answer?.trim()
        )
      : undefined;
    const ownAnswer = ownCandidate?.answer?.trim();
    const includedOwnAnswer = ownCandidate && ownAnswer
      ? {
          candidateId: ownCandidate.id,
          modelId: ownCandidate.modelId,
          modelLabel: ownCandidate.modelLabel,
          answerChars: ownAnswer.length,
          answerPreview: previewForLog(ownAnswer),
          alsoPublic: publicSelections.some(selection => selection.candidateId === ownCandidate.id),
        }
      : undefined;

    if (ownAnswer) {
      seenAnswerKeys.add(normalizeAnswerKey(ownAnswer));
    }

    const includedPublicSelections: Array<Record<string, unknown>> = [];
    const skippedDuplicatePublicSelections: Array<Record<string, unknown>> = [];
    for (const selection of publicSelections) {
      const answer = selection.answer.trim();
      const key = normalizeAnswerKey(answer);
      const summary = {
        candidateId: selection.candidateId,
        modelId: selection.modelId,
        modelLabel: selection.modelLabel,
        answerChars: answer.length,
        answerPreview: previewForLog(answer),
      };

      if (!key || seenAnswerKeys.has(key)) {
        skippedDuplicatePublicSelections.push(summary);
        continue;
      }

      seenAnswerKeys.add(key);
      includedPublicSelections.push(summary);
    }

    const privateAnswersNotIncluded = batch.candidates
      .filter(candidate => {
        const hasAnswer = Boolean(candidate.answer?.trim());
        if (!hasAnswer) {
          return false;
        }
        if (includedOwnAnswer?.candidateId === candidate.id) {
          return false;
        }
        return !publicSelections.some(selection => selection.candidateId === candidate.id);
      })
      .map(candidate => ({
        candidateId: candidate.id,
        modelId: candidate.modelId,
        modelLabel: candidate.modelLabel,
        answerChars: candidate.answer?.trim().length ?? 0,
      }));

    const question = batch.question || batch.displayQuestion;
    return {
      round: index + 1,
      batchId: batch.id,
      sourceSessionId: batch.sourceSessionId,
      questionChars: question.length,
      questionPreview: previewForLog(question),
      includedOwnAnswer,
      includedPublicSelections,
      skippedDuplicatePublicSelections,
      privateAnswersNotIncluded,
    };
  });
}

function buildDisplayPromptWithLedgerNote(promptMessage: string, contextMode: ModelBrainstormContextMode): string {
  return [
    contextMode === 'independent'
      ? 'Use the independent brainstorm ledger supplied above the current question.'
      : 'Use the shared brainstorm ledger supplied above the current question.',
    '',
    promptMessage,
  ].join('\n');
}

async function ensureSourceSession(
  manager: FlowChatManager,
  sourceSessionId: string | undefined,
  workspaceConfig: SessionConfig,
  agentType: string,
): Promise<string> {
  if (sourceSessionId) {
    return sourceSessionId;
  }

  return manager.createChatSession(workspaceConfig, agentType, {
    title: 'Multi-model Brainstorm',
  });
}

export async function launchModelBrainstorm(
  request: LaunchModelBrainstormRequest,
): Promise<LaunchModelBrainstormResult> {
  const displayMessage = request.displayMessage?.trim() || request.message.trim();
  const basePromptMessage = buildPromptMessage(stripInlineImageTags(request.message), request.contexts);
  const selectedModels = await resolveSelectedModels(request.modelIds);
  const contextMode = request.contextMode;

  if (selectedModels.length < MODEL_BRAINSTORM_MIN_CANDIDATES) {
    throw new Error('Select at least two enabled chat models for brainstorm mode.');
  }

  const imageContexts = request.contexts.filter((ctx): ctx is ImageContext => ctx.type === 'image');
  const imagePayload = await buildImagePayload(imageContexts);
  const manager = FlowChatManager.getInstance();
  const sourceSessionId = await ensureSourceSession(
    manager,
    request.sourceSessionId,
    request.workspaceConfig,
    request.agentType,
  );
  const previousBatches = hydrateCompletedCandidateAnswers(sourceSessionId, manager);
  const batchId = buildBatchId();
  const candidates: ModelBrainstormCandidate[] = selectedModels.map((model, index) => ({
    id: buildCandidateId(batchId, model.id, index),
    modelId: model.id,
    modelLabel: model.label,
    providerName: model.providerName,
    status: 'pending',
  }));

  useModelBrainstormStore.getState().createBatch({
    id: batchId,
    roomId: sourceSessionId,
    sourceSessionId,
    contextMode,
    question: basePromptMessage,
    displayQuestion: displayMessage,
    createdAt: Date.now(),
    selectedCandidateIds: [],
    publicSelections: [],
    candidates,
  });

  const launches = candidates.map(async (candidate) => {
    const updateCandidate = (updates: Partial<ModelBrainstormCandidate>) => {
      useModelBrainstormStore.getState().updateCandidate(batchId, candidate.id, updates);
    };

    try {
      updateCandidate({ status: 'starting' });
      const promptMessage = buildLedgerPrompt(
        basePromptMessage,
        {
          id: candidate.modelId,
          label: candidate.modelLabel,
          providerName: candidate.providerName || '',
        },
        contextMode,
        previousBatches,
      );
      if (shouldLogLedgerDiagnostics()) {
        log.debug('Prepared model brainstorm candidate context', {
          batchId,
          sourceSessionId,
          candidateId: candidate.id,
          modelId: candidate.modelId,
          modelLabel: candidate.modelLabel,
          contextMode,
          previousRoundCount: previousBatches.length,
          promptChars: promptMessage.length,
          promptPreview: previewForLog(promptMessage, 800),
          ledger: buildLedgerLogSummary(
            {
              id: candidate.modelId,
              label: candidate.modelLabel,
              providerName: candidate.providerName || '',
            },
            contextMode,
            previousBatches,
          ),
        });
      }
      const sessionId = await manager.createChatSession(
        {
          ...request.workspaceConfig,
          modelName: candidate.modelId,
        },
        request.agentType,
        {
          activate: false,
          title: buildCandidateSessionTitle({
            id: candidate.modelId,
            label: candidate.modelLabel,
            providerName: candidate.providerName || '',
          }),
        },
      );

      updateCandidate({ sessionId, status: 'running' });
      await manager.sendMessage(
        promptMessage,
        sessionId,
        displayMessage,
        request.agentType,
        undefined,
        {
          imageContexts: imagePayload?.imageContexts,
          imageDisplayData: imagePayload?.imageDisplayData,
          userMessageMetadata: {
            brainstormBatchId: batchId,
            brainstormCandidateId: candidate.id,
            brainstormModelId: candidate.modelId,
            brainstormContextMode: contextMode,
            brainstormLedgerDisplayPrompt: buildDisplayPromptWithLedgerNote(basePromptMessage, contextMode),
          },
        },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to launch candidate';
      log.error('Failed to launch model brainstorm candidate', {
        batchId,
        candidateId: candidate.id,
        modelId: candidate.modelId,
        error,
      });
      updateCandidate({
        status: 'failed',
        error: message,
      });
    }
  });

  await Promise.all(launches);

  const latestBatch = useModelBrainstormStore.getState().batches[batchId];
  const launchedCount = latestBatch?.candidates.filter(candidate => candidate.sessionId).length ?? 0;
  if (launchedCount === 0) {
    throw new Error('Failed to launch any brainstorm candidates.');
  }

  return {
    batchId,
    sourceSessionId,
  };
}
