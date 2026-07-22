import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { getProviderDisplayName } from '@/infrastructure/config/services/modelConfigs';
import type { AIModelConfig } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';
import type { ContextItem, ImageContext } from '@/shared/types/context';
import type { SessionConfig } from '../types/flow-chat';
import { FlowChatManager } from './FlowChatManager';
import { useModelBrainstormStore, type ModelBrainstormCandidate } from '../store/modelBrainstormStore';
import { buildImagePayload } from '../utils/imagePayload';
import { buildPromptMessage, stripInlineImageTags } from '../utils/messagePrompt';

const log = createLogger('ModelBrainstormService');

export const MODEL_BRAINSTORM_MAX_CANDIDATES = 4;
export const MODEL_BRAINSTORM_MIN_CANDIDATES = 2;

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
  const promptMessage = buildPromptMessage(stripInlineImageTags(request.message), request.contexts);
  const selectedModels = await resolveSelectedModels(request.modelIds);

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
    sourceSessionId,
    question: promptMessage,
    displayQuestion: displayMessage,
    createdAt: Date.now(),
    candidates,
  });

  const launches = candidates.map(async (candidate) => {
    const updateCandidate = (updates: Partial<ModelBrainstormCandidate>) => {
      useModelBrainstormStore.getState().updateCandidate(batchId, candidate.id, updates);
    };

    try {
      updateCandidate({ status: 'starting' });
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
