/**
 * Session management module
 * Handles session creation, switching, deletion, and other operations
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { notificationService } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { isRemoteTraceContext, startupTrace } from '@/shared/utils/startupTrace';
import { elapsedMs, nowMs } from '@/shared/utils/timing';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';
import type { AIModelConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import type { FlowChatContext, SessionConfig } from './types';
import type { Session } from '../../types/flow-chat';
import { touchSessionActivity, cleanupSaveState } from './PersistenceModule';
import { cleanupSessionBuffers } from './TextChunkModule';
import {
  createTextSessionTitleDescriptor,
  createDefaultSessionTitleDescriptor,
  getNextDefaultSessionTitleCount,
  resolveSessionTitle,
} from '../../utils/sessionTitle';
import { buildCreateSessionRelationship } from '../../utils/sessionMetadata';
import {
  consumeRecentHistorySessionOpenIntent,
  hasRenderableSessionContent,
} from '../sessionOpenIntent';
import {
  clearHistorySessionHydratePending,
  markHistorySessionHydratePending,
  recordHistorySessionDiagnosticEvent,
} from '../historySessionDiagnostics';
import {
  DEFAULT_CHAT_INPUT_MODE_CONFIG_PATH,
  normalizeUserDefaultChatInputModeId,
} from '../../utils/chatInputMode';

const log = createLogger('SessionModule');
const pendingSessionCreations = new Map<string, Promise<string>>();
export const SESSION_ACTIVITY_TOUCH_DELAY_MS = 350;
let latestSwitchRequestId = 0;
let pendingActivityTouchTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleSessionActivityTouch(task: () => void): void {
  if (pendingActivityTouchTimer !== null) {
    clearTimeout(pendingActivityTouchTimer);
  }
  pendingActivityTouchTimer = setTimeout(() => {
    pendingActivityTouchTimer = null;
    task();
  }, SESSION_ACTIVITY_TOUCH_DELAY_MS);
}

const normalizeOptional = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
};

function hasCompetingHistoryLoad(context: FlowChatContext, sessionId: string): boolean {
  return Array.from(context.pendingHistoryLoads.keys())
    .some(pendingSessionId => pendingSessionId !== sessionId);
}

const hostFromSshConnectionId = (connectionId: string | undefined): string | undefined => {
  const trimmed = connectionId?.trim();
  if (!trimmed) return undefined;
  const match = trimmed.match(/^ssh-[^@]+@(.+?)(?::\d+)?$/);
  return match?.[1]?.trim().toLowerCase() || undefined;
};

const remotePathsMatch = (left: string | undefined, right: string | undefined): boolean => {
  const leftNorm = normalizeOptional(left);
  const rightNorm = normalizeOptional(right);
  if (!leftNorm || !rightNorm) return false;
  return normalizeRemoteWorkspacePath(leftNorm) === normalizeRemoteWorkspacePath(rightNorm);
};

const currentWorkspaceMatchesSessionScope = (
  current: WorkspaceInfo | null | undefined,
  storedConnectionId: string | undefined,
  storedSshHost: string | undefined,
  workspacePath: string
): current is WorkspaceInfo => {
  if (current?.workspaceKind !== WorkspaceKind.Remote || !current.connectionId) {
    return false;
  }
  if (!remotePathsMatch(current.rootPath, workspacePath)) {
    return false;
  }

  const currentHost = normalizeOptional(current.sshHost)?.toLowerCase()
    || hostFromSshConnectionId(current.connectionId);
  const storedHost = normalizeOptional(storedSshHost)?.toLowerCase()
    || hostFromSshConnectionId(storedConnectionId);
  if (currentHost && storedHost) {
    return currentHost === storedHost;
  }

  const storedConnection = normalizeOptional(storedConnectionId);
  return !storedConnection || storedConnection === current.connectionId;
};

/// Resolve the effective connection_id for a session, preferring the
/// current workspace's connection when the stored ID may be stale
/// (e.g. after the user changed the SSH port).
const resolveEffectiveConnectionId = (
  storedConnectionId: string | undefined,
  storedSshHost: string | undefined,
  workspacePath: string
): string | undefined => {
  const current = workspaceManager.getState().currentWorkspace;
  if (currentWorkspaceMatchesSessionScope(current, storedConnectionId, storedSshHost, workspacePath)) {
    return current.connectionId;
  }
  return storedConnectionId;
};

const resolveEffectiveSshHost = (
  storedSshHost: string | undefined,
  storedConnectionId: string | undefined,
  workspacePath: string
): string | undefined => {
  const current = workspaceManager.getState().currentWorkspace;
  if (
    currentWorkspaceMatchesSessionScope(current, storedConnectionId, storedSshHost, workspacePath)
    && current.sshHost?.trim()
  ) {
    return current.sshHost.trim() || undefined;
  }
  return storedSshHost;
};

async function hydrateHistoricalSession(
  context: FlowChatContext,
  sessionId: string,
  notifyOnError: boolean,
  options?: {
    isRetryStillRelevant?: () => boolean;
    retryActiveStaleReuse?: boolean;
  },
): Promise<void> {
  const existing = context.pendingHistoryLoads.get(sessionId);
  if (existing) {
    startupTrace.markPhase('historical_session_hydrate_reused');
    recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_reused_pending', {
      notifyOnError,
      retryActiveStaleReuse: options?.retryActiveStaleReuse === true,
    });
    await existing;
    const retryStillRelevant = options?.isRetryStillRelevant?.() !== false;
    const shouldRetryActiveStale = shouldRetryActiveStaleHydrate(context, sessionId);
    recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_reused_settled', {
      retryActiveStaleReuse: options?.retryActiveStaleReuse === true,
      retryStillRelevant,
      shouldRetryActiveStale,
    });
    if (
      options?.retryActiveStaleReuse === true &&
      retryStillRelevant &&
      shouldRetryActiveStale
    ) {
      if (context.pendingHistoryLoads.get(sessionId) === existing) {
        context.pendingHistoryLoads.delete(sessionId);
      }
      startupTrace.markPhase('historical_session_hydrate_retry_active_stale_reuse', {
        sessionId,
      });
      recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_retry_active_stale_reuse_started');
      await hydrateHistoricalSession(context, sessionId, notifyOnError);
    } else if (options?.retryActiveStaleReuse === true) {
      recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_retry_active_stale_reuse_skipped', {
        reason: retryStillRelevant ? 'active_stale_condition_not_met' : 'switch_superseded',
      });
    }
    return;
  }
  const traceStartedAt = nowMs();

  const loadPromise = (async () => {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    if (!session?.isHistorical) {
      recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_request_skipped', {
        reason: session ? 'not_historical' : 'missing_session',
      });
      return;
    }

    const workspacePath = requireSessionWorkspacePath(session.workspacePath, sessionId);
    const remote = isRemoteTraceContext(session.remoteConnectionId, session.remoteSshHost);
    markHistorySessionHydratePending(sessionId, {
      notifyOnError,
      remote,
      deferFullHistoryUntilActive: true,
    });
    startupTrace.markPhase('historical_session_hydrate_request', { remote });
    recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_request_started', {
      remote,
      historyState: session.historyState,
      hasRenderableContent: hasRenderableSessionContent(session),
    });

    // Prefer the current workspace's connection info over the session's
    // stored values.  When the user changes the SSH port the session's
    // remoteConnectionId becomes stale; the active workspace always
    // carries the up-to-date connection_id.
    const effectiveConnectionId = resolveEffectiveConnectionId(
      session.remoteConnectionId,
      session.remoteSshHost,
      workspacePath
    );
    const effectiveSshHost = resolveEffectiveSshHost(
      session.remoteSshHost,
      session.remoteConnectionId,
      workspacePath
    );

    await context.flowChatStore.loadSessionHistory(
      sessionId,
      workspacePath,
      undefined,
      effectiveConnectionId,
      effectiveSshHost,
      { deferFullHistoryUntilActive: true },
    );
  })();

  context.pendingHistoryLoads.set(sessionId, loadPromise);

  try {
    await loadPromise;
    recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_request_finished');
    startupTrace.markPhase('historical_session_hydrate_request_end', {
      durationMs: elapsedMs(traceStartedAt),
    });
  } catch (error) {
    recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_request_failed');
    startupTrace.markPhase('historical_session_hydrate_request_failed', {
      durationMs: elapsedMs(traceStartedAt),
    });
    log.error('Failed to load session history', { sessionId, error });
    if (notifyOnError) {
      notificationService.warning('Failed to load session history, showing empty session', {
        duration: 3000
      });
    }
    throw error;
  } finally {
    clearHistorySessionHydratePending(sessionId, 'settled', {
      pendingStillCurrent: context.pendingHistoryLoads.get(sessionId) === loadPromise,
    });
    if (context.pendingHistoryLoads.get(sessionId) === loadPromise) {
      context.pendingHistoryLoads.delete(sessionId);
    }
  }
}

function shouldHydrateHistoricalSessionBeforeSwitch(session: Session | undefined): session is Session {
  if (session?.isHistorical !== true) {
    return false;
  }
  if (isRemoteTraceContext(session.remoteConnectionId, session.remoteSshHost)) {
    return false;
  }
  return !hasRenderableSessionContent(session);
}

function shouldRetryActiveStaleHydrate(context: FlowChatContext, sessionId: string): boolean {
  const state = context.flowChatStore.getState();
  if (state.activeSessionId !== sessionId) {
    return false;
  }

  const session = state.sessions.get(sessionId);
  if (!session || session.historyState !== 'metadata-only') {
    return false;
  }

  return shouldHydrateHistoricalSessionBeforeSwitch(session);
}

export function preloadHistoricalSessionForOpen(
  context: FlowChatContext,
  sessionId: string
): void {
  if (!hasCompetingHistoryLoad(context, sessionId)) {
    return;
  }

  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!shouldHydrateHistoricalSessionBeforeSwitch(session)) {
    return;
  }

  void hydrateHistoricalSession(context, sessionId, false).catch(error => {
    log.debug('Historical session preload failed', { sessionId, error });
  });
}

type SessionDisplayMode = 'code' | 'cowork' | 'claw';

const isAssistantWorkspace = (workspace?: WorkspaceInfo | null): boolean => {
  return workspace?.workspaceKind === WorkspaceKind.Assistant;
};

const normalizeSessionDisplayMode = (
  mode?: string,
  workspace?: WorkspaceInfo | null
): SessionDisplayMode => {
  if (isAssistantWorkspace(workspace)) return 'claw';
  if (!mode) return 'code';
  const normalizedMode = mode.toLowerCase();
  if (normalizedMode === 'cowork') return 'cowork';
  if (normalizedMode === 'claw') return 'claw';
  return 'code';
};

const resolveSessionWorkspacePath = (
  context: FlowChatContext,
  config?: SessionConfig
): string | null => {
  const explicitWorkspacePath = config?.workspacePath?.trim();
  if (explicitWorkspacePath) {
    return explicitWorkspacePath;
  }
  const fromFlowChat = context.currentWorkspacePath?.trim();
  if (fromFlowChat) {
    return fromFlowChat;
  }
  // Remote restore: AppLayout may skip FlowChat.initialize until SSH connects, so
  // currentWorkspacePath stays null while global workspace already has rootPath.
  const current = workspaceManager.getState().currentWorkspace;
  const root = current?.rootPath?.trim();
  if (!root) {
    return null;
  }
  return current?.workspaceKind === WorkspaceKind.Remote
    ? normalizeRemoteWorkspacePath(root)
    : root;
};

const resolveSessionWorkspace = (
  context: FlowChatContext,
  config?: SessionConfig
): WorkspaceInfo | null => {
  const state = workspaceManager.getState();
  const configWorkspaceId = config?.workspaceId?.trim();
  if (configWorkspaceId) {
    const byId = state.openedWorkspaces.get(configWorkspaceId);
    if (byId) return byId;
  }

  const workspacePath = resolveSessionWorkspacePath(context, config);
  if (!workspacePath) return null;
  const pathMatches = Array.from(state.openedWorkspaces.values()).filter(workspace => {
    if (workspace.rootPath !== workspacePath) return false;
    if (workspace.workspaceKind !== WorkspaceKind.Remote) return true;
    const cid = config?.remoteConnectionId?.trim();
    const host = config?.remoteSshHost?.trim();
    if (cid && workspace.connectionId !== cid) return false;
    if (host && (workspace.sshHost?.trim() ?? '') !== host) return false;
    return true;
  });
  if (pathMatches.length === 0) {
    return state.currentWorkspace;
  }
  if (pathMatches.length === 1) {
    return pathMatches[0];
  }
  const configCid = config?.remoteConnectionId?.trim();
  if (configCid) {
    const byConn = pathMatches.find(w => w.connectionId === configCid);
    if (byConn) return byConn;
  }
  const configHost = config?.remoteSshHost?.trim();
  if (configHost) {
    const byHost = pathMatches.find(w => (w.sshHost?.trim() ?? '') === configHost);
    if (byHost) return byHost;
  }
  const cur = state.currentWorkspace;
  if (cur && pathMatches.some(w => w.id === cur.id)) {
    return cur;
  }
  return pathMatches[0];
};

export const resolveAgentTypeForSessionCreation = async (
  requestedMode: string | undefined,
  workspace: WorkspaceInfo | null
): Promise<string> => {
  if (isAssistantWorkspace(workspace)) {
    return 'Claw';
  }

  const normalizedRequestedMode = requestedMode?.trim();
  if (normalizedRequestedMode && normalizedRequestedMode !== 'agentic') {
    return normalizedRequestedMode;
  }

  try {
    const configuredDefaultMode = normalizeUserDefaultChatInputModeId(
      await configAPI.getConfig(DEFAULT_CHAT_INPUT_MODE_CONFIG_PATH, {
        skipRetryOnNotFound: true,
      }),
    );
    if (!configuredDefaultMode) {
      return normalizedRequestedMode || 'agentic';
    }

    const availableModes = await agentAPI.getAvailableModes();
    if (availableModes.some(mode => mode.id === configuredDefaultMode)) {
      return configuredDefaultMode;
    }

    log.warn('Ignoring unavailable default chat input mode preference during session creation', {
      modeId: configuredDefaultMode,
    });
  } catch (error) {
    log.warn('Failed to resolve default chat input mode preference during session creation', {
      error,
    });
  }

  return normalizedRequestedMode || 'agentic';
};

function requireSessionWorkspacePath(
  workspacePath: string | undefined,
  sessionId: string
): string {
  if (!workspacePath) {
    throw new Error(`Workspace path is required for session: ${sessionId}`);
  }
  return workspacePath;
}

/**
 * Get model's maximum token count
 */
function findEnabledModel(models: AIModelConfig[], modelRef: string | null | undefined): AIModelConfig | null {
  const value = modelRef?.trim();
  if (!value) return null;
  return models.find(model =>
    model.enabled !== false
    && (model.id === value || model.name === value || model.model_name === value)
  ) ?? null;
}

function resolveModelForContextWindow(
  modelRef: string | null | undefined,
  models: AIModelConfig[],
  defaultModels: DefaultModelsConfig,
): AIModelConfig | null {
  const value = modelRef?.trim();
  if (!value) return null;

  if (value === 'primary') {
    return findEnabledModel(models, defaultModels.primary);
  }

  if (value === 'fast') {
    return findEnabledModel(models, defaultModels.fast) ?? findEnabledModel(models, defaultModels.primary);
  }

  if (value === 'auto' || value === 'default') {
    return null;
  }

  return findEnabledModel(models, value);
}

export async function getModelMaxTokens(modelName?: string, agentType?: string): Promise<number> {
  try {
    const configManager = await import('@/infrastructure/config/services/ConfigManager').then(m => m.configManager);
    const configData = await configManager.getConfigs([
      'ai.models',
      'ai.default_models',
      'ai.agent_models',
    ]);
    const models = (configData['ai.models'] as AIModelConfig[] | undefined) || [];
    const defaultModels = (configData['ai.default_models'] as DefaultModelsConfig | undefined) || {};
    const agentModels = (configData['ai.agent_models'] as Record<string, string> | undefined) || {};
    
    const explicitModel = resolveModelForContextWindow(modelName, models, defaultModels);
    if (explicitModel?.context_window) {
      return explicitModel.context_window;
    }

    const agentModel = resolveModelForContextWindow(
      agentType ? agentModels[agentType] : undefined,
      models,
      defaultModels,
    );
    if (agentModel?.context_window) {
      return agentModel.context_window;
    }

    const primaryModel = resolveModelForContextWindow('primary', models, defaultModels);
    if (primaryModel?.context_window) {
      return primaryModel.context_window;
    }
    
    log.debug('Model context_window config not found, using default', { modelName, agentType });
    return 128128;
  } catch (error) {
    log.warn('Failed to get model max tokens', { modelName, agentType, error });
    return 128128;
  }
}

/**
 * Create new chat session (managed by backend)
 */
export async function createChatSession(
  context: FlowChatContext,
  config: SessionConfig,
  mode?: string
): Promise<string> {
  try {
    const workspacePath = resolveSessionWorkspacePath(context, config);
    const workspace = resolveSessionWorkspace(context, config);

    if (!workspacePath) {
      throw new Error('Workspace path is required to create a session');
    }
    const remoteConnectionId =
      workspace?.workspaceKind === WorkspaceKind.Remote ? workspace.connectionId : undefined;
    const remoteSshHost =
      workspace?.workspaceKind === WorkspaceKind.Remote
        ? workspace.sshHost?.trim() || undefined
        : undefined;
    const agentType = await resolveAgentTypeForSessionCreation(mode, workspace);
    const sessionMode = normalizeSessionDisplayMode(agentType, workspace);
    const workspaceCreationKey =
      workspace?.id?.trim()
        ? workspace.id
        : remoteConnectionId != null && remoteConnectionId !== ''
          ? `${remoteConnectionId}\n${workspacePath}`
          : workspacePath;
    const creationKey = JSON.stringify([workspaceCreationKey, agentType]);

    const pendingCreation = pendingSessionCreations.get(creationKey);
    if (pendingCreation) {
      return pendingCreation;
    }

    // Register the pending promise before any async work below. Remote workspace
    // activation can rerun initialization while model config is still loading.
    const createPromise = Promise.resolve().then(async () => {
      const sameModeCount = getNextDefaultSessionTitleCount(
        context.flowChatStore.getState().sessions.values(),
        {
          mode: sessionMode,
          workspaceId: workspace?.id,
          workspacePath,
          remoteConnectionId,
          remoteSshHost,
        },
      );
      const titleDescriptor = createDefaultSessionTitleDescriptor(
        sessionMode,
        sameModeCount,
        (key, options) => i18nService.t(key, options),
      );
      const sessionName = titleDescriptor.text;
      
      const maxContextTokens = await getModelMaxTokens(config.modelName, agentType);

      const mergedConfig: SessionConfig = {
        ...config,
        workspaceId: workspace?.id ?? config.workspaceId,
      };

      const response = await agentAPI.createSession({
        sessionName,
        agentType,
        workspacePath,
        workspaceId: mergedConfig.workspaceId,
        remoteConnectionId,
        remoteSshHost,
        config: {
          modelName: config.modelName || 'auto',
          enableTools: true,
          safeMode: true,
          autoCompact: true,
          maxContextTokens: maxContextTokens,
          enableContextCompression: true,
          remoteConnectionId,
          remoteSshHost,
        }
      });

      context.flowChatStore.createSession(
        response.sessionId, 
        mergedConfig, 
        undefined,
        sessionName,
        maxContextTokens,
        agentType,
        workspacePath,
        remoteConnectionId,
        remoteSshHost,
        titleDescriptor,
      );

      return response.sessionId;
    });

    pendingSessionCreations.set(creationKey, createPromise);
    try {
      return await createPromise;
    } finally {
      if (pendingSessionCreations.get(creationKey) === createPromise) {
        pendingSessionCreations.delete(creationKey);
      }
    }
  } catch (error) {
    log.error('Failed to create chat session', { config, error });
    
    notificationService.error('Failed to create chat session', {
      duration: 3000
    });
    throw error;
  }
}

/**
 * Switch to specified session
 */
export async function switchChatSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  try {
    const switchRequestId = ++latestSwitchRequestId;
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    const isRemoteSession = isRemoteTraceContext(session?.remoteConnectionId, session?.remoteSshHost);
    const shouldHydrateBeforeSwitch = shouldHydrateHistoricalSessionBeforeSwitch(session);
    const shouldActivateBeforeHydrate =
      shouldHydrateBeforeSwitch &&
      consumeRecentHistorySessionOpenIntent(sessionId);
    recordHistorySessionDiagnosticEvent(sessionId, 'switch_requested', {
      switchRequestId,
      isRemoteSession,
      isHistorical: session?.isHistorical === true,
      historyState: session?.historyState,
      hasRenderableContent: session ? hasRenderableSessionContent(session) : false,
      shouldHydrateBeforeSwitch,
      shouldActivateBeforeHydrate,
    });

    const touchActiveSessionInBackground = () => {
      scheduleSessionActivityTouch(() => {
        const latestState = context.flowChatStore.getState();
        const latestSession = latestState.sessions.get(sessionId);
        if (switchRequestId !== latestSwitchRequestId || latestState.activeSessionId !== sessionId || !latestSession) {
          return;
        }
        touchSessionActivity(
          sessionId,
          latestSession.workspacePath,
          latestSession.remoteConnectionId,
          latestSession.remoteSshHost
        ).catch(error => {
          log.debug('Failed to touch session activity', { sessionId, error });
        });
      });
    };

    if (shouldActivateBeforeHydrate) {
      context.flowChatStore.switchSession(sessionId);
      recordHistorySessionDiagnosticEvent(sessionId, 'switch_activated_before_hydrate', {
        switchRequestId,
      });
      startupTrace.markPhase('historical_session_switch', {
        historical: true,
        remote: false,
        activation: 'before-hydrate',
      });
      touchActiveSessionInBackground();
    }

    if (shouldHydrateBeforeSwitch) {
      try {
        await hydrateHistoricalSession(context, sessionId, true, {
          isRetryStillRelevant: () => switchRequestId === latestSwitchRequestId,
          retryActiveStaleReuse: shouldActivateBeforeHydrate,
        });
      } catch {
        // The hydrate path already marks the session failed and notifies the user.
        // Continue with activation so the failed state is visible.
      }

      if (switchRequestId !== latestSwitchRequestId) {
        recordHistorySessionDiagnosticEvent(sessionId, 'switch_superseded', {
          switchRequestId,
          latestSwitchRequestId,
        });
        startupTrace.markPhase('historical_session_switch_superseded', {
          sessionId,
        });
        return;
      }
    }

    // Avoid showing an empty loading page between two sessions. Historical
    // sessions without a renderable tail are activated after their first
    // visible content is restored unless a fresh user open intent is present.
    // In that explicit path the intent shield keeps metadata-only content from
    // flashing while the old large session is unmounted immediately.
    if (!shouldActivateBeforeHydrate) {
      context.flowChatStore.switchSession(sessionId);
      recordHistorySessionDiagnosticEvent(sessionId, 'switch_activated_after_hydrate', {
        switchRequestId,
        shouldHydrateBeforeSwitch,
      });
      startupTrace.markPhase('historical_session_switch', {
        historical: Boolean(session?.isHistorical),
        remote: isRemoteSession,
        activation: shouldHydrateBeforeSwitch ? 'after-hydrate' : 'immediate',
      });
      touchActiveSessionInBackground();
    }

    if (session?.isHistorical && !shouldHydrateBeforeSwitch) {
      // Load history in the background — do not block the UI.
      recordHistorySessionDiagnosticEvent(sessionId, 'switch_background_hydrate_started', {
        switchRequestId,
      });
      void hydrateHistoricalSession(context, sessionId, true);
    }
  } catch (error) {
    log.error('Failed to switch chat session', { sessionId, error });
    notificationService.error('Failed to switch session', {
      duration: 3000
    });
    throw error;
  }
}

/**
 * Delete session (cascading delete Terminal)
 */
export async function deleteChatSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  try {
    const stateBeforeDelete = context.flowChatStore.getState();
    const removedSessionIds = context.flowChatStore.getCascadeSessionIds(sessionId);
    const removedSessionIdSet = new Set(removedSessionIds);
    const removedActiveSession = Boolean(
      stateBeforeDelete.activeSessionId
      && removedSessionIdSet.has(stateBeforeDelete.activeSessionId)
    );
    await context.flowChatStore.deleteSession(
      sessionId,
      removedActiveSession ? { nextActiveSessionId: null } : undefined,
    );

    removedSessionIds.forEach(id => {
      context.processingManager.clearSessionStatus(id);
      cleanupSaveState(context, id);
    });
  } catch (error) {
    log.error('Failed to delete chat session', { sessionId, error });
    notificationService.error('Failed to delete session', {
      duration: 3000
    });
    throw error;
  }
}

export async function archiveChatSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  try {
    const stateBeforeArchive = context.flowChatStore.getState();
    const session = stateBeforeArchive.sessions.get(sessionId);
    if (!session) {
      throw new Error(`Session does not exist: ${sessionId}`);
    }

    const removedSessionIds = context.flowChatStore.getCascadeSessionIds(sessionId);
    const removedSessionIdSet = new Set(removedSessionIds);
    const removedActiveSession = Boolean(
      stateBeforeArchive.activeSessionId
      && removedSessionIdSet.has(stateBeforeArchive.activeSessionId)
    );

    await sessionAPI.archiveSession(
      sessionId,
      requireSessionWorkspacePath(session.workspacePath, sessionId),
      session.remoteConnectionId,
      session.remoteSshHost,
    );

    const { stateMachineManager } = await import('../../state-machine');
    context.flowChatStore.removeSession(
      sessionId,
      removedActiveSession ? { nextActiveSessionId: null } : undefined,
    );

    removedSessionIds.forEach(id => {
      stateMachineManager.delete(id);
      context.processingManager.clearSessionStatus(id);
      cleanupSaveState(context, id);
      cleanupSessionBuffers(context, id);
    });
  } catch (error) {
    log.error('Failed to archive chat session', { sessionId, error });
    notificationService.error('Failed to archive session', {
      duration: 3000
    });
    throw error;
  }
}

export async function renameChatSessionTitle(
  context: FlowChatContext,
  sessionId: string,
  title: string
): Promise<string> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }

  const trimmedTitle = title.trim();
  if (!trimmedTitle) {
    throw new Error('Session title must not be empty');
  }
  if (session.isTransient) {
    await context.flowChatStore.updateSessionTitle(sessionId, trimmedTitle, 'generated');
    return trimmedTitle;
  }

  const updatedTitle = await agentAPI.updateSessionTitle({
    sessionId,
    title: trimmedTitle,
    workspacePath: session.workspacePath,
    remoteConnectionId: session.remoteConnectionId,
    remoteSshHost: session.remoteSshHost,
  });

  await context.flowChatStore.updateSessionTitle(sessionId, updatedTitle, 'generated');
  return updatedTitle;
}

export async function forkChatSession(
  context: FlowChatContext,
  sourceSessionId: string,
  sourceTurnId: string
): Promise<string> {
  const sourceSession = context.flowChatStore.getState().sessions.get(sourceSessionId);
  if (!sourceSession) {
    throw new Error(`Session does not exist: ${sourceSessionId}`);
  }

  const workspacePath = requireSessionWorkspacePath(
    sourceSession.workspacePath,
    sourceSessionId
  );

  const response = await sessionAPI.forkSession(
    sourceSessionId,
    sourceTurnId,
    workspacePath,
    sourceSession.remoteConnectionId,
    sourceSession.remoteSshHost
  );

  const currentState = context.flowChatStore.getState();
  if (!currentState.sessions.has(response.sessionId)) {
    context.flowChatStore.createSession(
      response.sessionId,
      {
        ...sourceSession.config,
        workspacePath,
        workspaceId: sourceSession.workspaceId,
        remoteConnectionId: sourceSession.remoteConnectionId,
        remoteSshHost: sourceSession.remoteSshHost,
      },
      undefined,
      response.sessionName,
      sourceSession.maxContextTokens,
      sourceSession.mode,
      workspacePath,
      sourceSession.remoteConnectionId,
      sourceSession.remoteSshHost,
      createTextSessionTitleDescriptor(response.sessionName),
    );
  } else {
    context.flowChatStore.switchSession(response.sessionId);
  }

  await context.flowChatStore.loadSessionHistory(
    response.sessionId,
    workspacePath,
    undefined,
    sourceSession.remoteConnectionId,
    sourceSession.remoteSshHost,
    { deferFullHistoryUntilActive: true },
  );
  context.flowChatStore.switchSession(response.sessionId);

  return response.sessionId;
}

/**
 * Ensure backend session exists (check before sending message)
 */
export async function ensureBackendSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }
  if (session.isTransient) {
    return;
  }

  if (session.isHistorical) {
    await hydrateHistoricalSession(context, sessionId, false);
  }

  const latestSession = context.flowChatStore.getState().sessions.get(sessionId) ?? session;
  const workspacePath = requireSessionWorkspacePath(latestSession.workspacePath, sessionId);

  // Resolve effective connection info: prefer the current workspace's
  // connection_id over the session's stored value.  When the user changes
  // the SSH port the session's remoteConnectionId becomes stale.
  const effectiveConnectionId = resolveEffectiveConnectionId(
    latestSession.remoteConnectionId,
    latestSession.remoteSshHost,
    workspacePath
  );
  const effectiveSshHost = resolveEffectiveSshHost(
    latestSession.remoteSshHost,
    latestSession.remoteConnectionId,
    workspacePath
  );

  const isHistoricalSession = latestSession.isHistorical === true;
  const isFirstTurn = latestSession.dialogTurns.length <= 1;
  const requiresContextRestore =
    latestSession.contextRestoreState === 'pending' ||
    latestSession.contextRestoreState === 'failed';
  const needsBackendSetup = isHistoricalSession || isFirstTurn || requiresContextRestore;
  const hasLoadedTurns = latestSession.dialogTurns.length > 0;
  /** Avoid createSession when historical data is already loaded but backend files are missing (e.g. new SSH connection id). */
  const allowRecreateOnCoordinatorFailure =
    needsBackendSetup &&
    !(requiresContextRestore && hasLoadedTurns) &&
    !(isHistoricalSession && hasLoadedTurns);

  const markBackendContextReady = () => {
    if (!isHistoricalSession && !requiresContextRestore) return;
    context.flowChatStore.setState(prev => {
      const newSessions = new Map(prev.sessions);
      const sess = newSessions.get(sessionId);
      if (sess) {
        newSessions.set(sessionId, {
          ...sess,
          isHistorical: false,
          historyState: 'ready',
          contextRestoreState: 'ready',
        });
      }
      return { ...prev, sessions: newSessions };
    });
  };

  const markBackendContextFailed = () => {
    if (!requiresContextRestore) return;
    context.flowChatStore.setState(prev => {
      const newSessions = new Map(prev.sessions);
      const sess = newSessions.get(sessionId);
      if (sess) {
        newSessions.set(sessionId, { ...sess, contextRestoreState: 'failed' });
      }
      return { ...prev, sessions: newSessions };
    });
  };

  const ensureCoordinator = async () => {
    await agentAPI.ensureCoordinatorSession({
      sessionId,
      workspacePath,
      remoteConnectionId: effectiveConnectionId,
      remoteSshHost: effectiveSshHost,
    });
    markBackendContextReady();
  };

  const restorePendingBackendContext = async () => {
    if (!context.pendingContextRestores) {
      context.pendingContextRestores = new Map();
    }
    const restoreKey = [
      sessionId,
      workspacePath,
      effectiveConnectionId ?? '',
      effectiveSshHost ?? '',
    ].join('\u001f');
    const existingRestore = context.pendingContextRestores.get(restoreKey);
    if (existingRestore) {
      await existingRestore;
      return;
    }

    const restorePromise = ensureCoordinator().catch(error => {
      markBackendContextFailed();
      throw error;
    }).finally(() => {
      if (context.pendingContextRestores?.get(restoreKey) === restorePromise) {
        context.pendingContextRestores.delete(restoreKey);
      }
    });
    context.pendingContextRestores.set(restoreKey, restorePromise);
    await restorePromise;
  };

  try {
    if (requiresContextRestore) {
      await restorePendingBackendContext();
      return;
    }

    await ensureCoordinator();
  } catch (e: any) {
    if (!allowRecreateOnCoordinatorFailure) {
      const raw = typeof e?.message === 'string' ? e.message : String(e);
      const hint =
        raw.includes('Session metadata not found') || raw.includes('Not found')
          ? i18nService.t('flow-chat:historyState.remoteSessionMissing')
          : raw;
      throw new Error(hint);
    }

    log.debug('Coordinator session missing, creating backend session', { sessionId, error: e });
    await agentAPI.createSession({
      sessionId: sessionId,
      sessionName:
        resolveSessionTitle(latestSession, (key, options) => i18nService.t(key, options)) ||
        `Session ${sessionId.slice(0, 8)}`,
      agentType: latestSession.mode || 'agentic',
      workspacePath,
      workspaceId: latestSession.workspaceId,
      remoteConnectionId: effectiveConnectionId,
      remoteSshHost: effectiveSshHost,
      relationship: buildCreateSessionRelationship(latestSession),
      deepReviewRunManifest: latestSession.deepReviewRunManifest,
      config: {
        modelName: latestSession.config.modelName || 'auto',
        enableTools: true,
        safeMode: true,
        remoteConnectionId: effectiveConnectionId,
        remoteSshHost: effectiveSshHost,
      }
    });
    markBackendContextReady();
  }
}

/**
 * Retry creating backend session (retry after message send failure)
 */
export async function retryCreateBackendSession(
  context: FlowChatContext,
  sessionId: string
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }
  if (session.isTransient) {
    return;
  }

  const workspacePath = requireSessionWorkspacePath(session.workspacePath, sessionId);
  
  await agentAPI.createSession({
    sessionId: sessionId,
    sessionName:
      resolveSessionTitle(session, (key, options) => i18nService.t(key, options)) ||
      `Session ${sessionId.slice(0, 8)}`,
    agentType: session.mode || 'agentic',
    workspacePath,
    workspaceId: session.workspaceId,
    remoteConnectionId: session.remoteConnectionId,
    remoteSshHost: session.remoteSshHost,
    relationship: buildCreateSessionRelationship(session),
    deepReviewRunManifest: session.deepReviewRunManifest,
    config: {
      modelName: session.config.modelName || 'auto',
      enableTools: true,
      safeMode: true,
      remoteConnectionId: session.remoteConnectionId,
      remoteSshHost: session.remoteSshHost,
    }
  });
}
