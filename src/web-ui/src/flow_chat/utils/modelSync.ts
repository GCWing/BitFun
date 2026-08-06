/**
 * Session model-selection synchronization before a local dialog turn.
 *
 * Leaf module: shared by the local session driver and re-exported by
 * MessageModule for existing callers.
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { AIModelConfig, AgentModelDefaultsConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';
import type { FlowChatContext } from '../services/flow-chat-manager/types';
import { getModelMaxTokens } from './modelResolution';
import { sessionProjectWorkspacePath } from './sessionWorkspace';

const log = createLogger('ModelSync');

function normalizeModelSelection(
  modelId: string | undefined,
  models: AIModelConfig[],
  defaultModels: DefaultModelsConfig,
): string {
  const value = modelId?.trim();
  if (!value || value === 'auto') return 'auto';

  if (value === 'primary' || value === 'fast') {
    const resolvedDefaultId = value === 'primary' ? defaultModels.primary : defaultModels.fast;
    const matchedModel = models.find(model => model.id === resolvedDefaultId);
    return matchedModel ? value : 'auto';
  }

  const matchedModel = models.find(model =>
    model.id === value || model.name === value || model.model_name === value,
  );
  return matchedModel ? value : 'auto';
}

export async function syncSessionModelSelection(
  context: FlowChatContext,
  sessionId: string,
  agentType: string,
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }

  const sessionModelId = session.config.modelName?.trim();

  // Any stored selector, including "auto", belongs to the session. Still sync
  // it to the backend in case the restored runtime session lost that state.
  if (sessionModelId) {
    const desiredMaxContextTokens = await getModelMaxTokens(sessionModelId, agentType);
    if (session.maxContextTokens !== desiredMaxContextTokens) {
      context.flowChatStore.updateSessionMaxContextTokens(sessionId, desiredMaxContextTokens);
    }
    await agentAPI.updateSessionModel({
      sessionId,
      modelName: sessionModelId,
      reasoningPreset: session.config.reasoningPreset ?? null,
      workspacePath: sessionProjectWorkspacePath(session),
      remoteConnectionId: session.remoteConnectionId,
      remoteSshHost: session.remoteSshHost,
      includeInternal: session.sessionKind === 'subagent',
    });
    return;
  }

  const configData = await configManager.getConfigs([
    'ai.agent_model_defaults',
    'ai.models',
    'ai.default_models',
  ]);
  const agentModelDefaults = configData['ai.agent_model_defaults'] as AgentModelDefaultsConfig | undefined;
  const allModels = (configData['ai.models'] as AIModelConfig[] | undefined) || [];
  const defaultModels = (configData['ai.default_models'] as DefaultModelsConfig | undefined) || {};

  const desiredModelId = normalizeModelSelection(agentModelDefaults?.mode, allModels, defaultModels);
  const shouldForceAutoSync = desiredModelId === 'auto';
  const desiredMaxContextTokens = await getModelMaxTokens(desiredModelId, agentType);
  const shouldSyncContextWindow = session.maxContextTokens !== desiredMaxContextTokens;

  context.flowChatStore.updateSessionModelName(sessionId, desiredModelId);
  if (shouldSyncContextWindow) {
    context.flowChatStore.updateSessionMaxContextTokens(sessionId, desiredMaxContextTokens);
  }
  await agentAPI.updateSessionModel({
    sessionId,
    modelName: desiredModelId,
    reasoningPreset: session.config.reasoningPreset ?? null,
    workspacePath: sessionProjectWorkspacePath(session),
    remoteConnectionId: session.remoteConnectionId,
    remoteSshHost: session.remoteSshHost,
    includeInternal: session.sessionKind === 'subagent',
  });

  log.info('Session model synchronized before send', {
    sessionId,
    agentType,
    previousModelId: null,
    nextModelId: desiredModelId,
    forcedAutoSync: shouldForceAutoSync,
  });
}
