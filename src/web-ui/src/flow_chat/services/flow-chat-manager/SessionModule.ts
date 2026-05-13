/**
 * Session management module
 * Handles session creation, switching, deletion, and other operations
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { notificationService } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';
import type { FlowChatContext, SessionConfig } from './types';
import { touchSessionActivity, cleanupSaveState } from './PersistenceModule';
import {
  createTextSessionTitleDescriptor,
  createDefaultSessionTitleDescriptor,
  getNextDefaultSessionTitleCount,
  resolveSessionTitle,
} from '../../utils/sessionTitle';

const log = createLogger('SessionModule');
const pendingSessionCreations = new Map<string, Promise<string>>();

const normalizeOptional = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
};

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
  notifyOnError: boolean
): Promise<void> {
  const existing = context.pendingHistoryLoads.get(sessionId);
  if (existing) {
    await existing;
    return;
  }

  const loadPromise = (async () => {
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    if (!session?.isHistorical) {
      return;
    }

    const workspacePath = requireSessionWorkspacePath(session.workspacePath, sessionId);

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
      effectiveSshHost
    );
  })();

  context.pendingHistoryLoads.set(sessionId, loadPromise);

  try {
    await loadPromise;
  } catch (error) {
    log.error('Failed to load session history', { sessionId, error });
    if (notifyOnError) {
      notificationService.warning('Failed to load session history, showing empty session', {
        duration: 3000
      });
    }
    throw error;
  } finally {
    if (context.pendingHistoryLoads.get(sessionId) === loadPromise) {
      context.pendingHistoryLoads.delete(sessionId);
    }
  }
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

const resolveAgentType = (
  requestedMode: string | undefined,
  workspace: WorkspaceInfo | null
): string => {
  if (isAssistantWorkspace(workspace)) {
    return 'Claw';
  }
  return requestedMode || 'agentic';
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
export async function getModelMaxTokens(modelName?: string): Promise<number> {
  try {
    const configManager = await import('@/infrastructure/config/services/ConfigManager').then(m => m.configManager);
    const models = await configManager.getConfig<any[]>('ai.models') || [];
    
    if (modelName) {
      const model = models.find(m => m.name === modelName || m.id === modelName);
      if (model?.context_window) {
        return model.context_window;
      }
    }
    
    const defaultModels = await configManager.getConfig<Record<string, string>>('ai.default_models');
    const primaryModelId = defaultModels?.primary;
    
    if (primaryModelId) {
      const primaryModel = models.find(m => m.id === primaryModelId);
      if (primaryModel?.context_window) {
        return primaryModel.context_window;
      }
    }
    
    log.debug('Model context_window config not found, using default', { modelName });
    return 128128;
  } catch (error) {
    log.warn('Failed to get model max tokens', { modelName, error });
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
    const agentType = resolveAgentType(mode, workspace);
    const sessionMode = normalizeSessionDisplayMode(agentType, workspace);
    const creationKey =
      workspace?.id?.trim()
        ? workspace.id
        : remoteConnectionId != null && remoteConnectionId !== ''
          ? `${remoteConnectionId}\n${workspacePath}`
          : workspacePath;

    const pendingCreation = pendingSessionCreations.get(creationKey);
    if (pendingCreation) {
      return pendingCreation;
    }

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
    
    const maxContextTokens = await getModelMaxTokens(config.modelName);

    const mergedConfig: SessionConfig = {
      ...config,
      workspaceId: workspace?.id ?? config.workspaceId,
    };

    const createPromise = (async () => {
      const response = await agentAPI.createSession({
        sessionName,
        agentType,
        workspacePath,
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
    })();

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
    const session = context.flowChatStore.getState().sessions.get(sessionId);

    // Switch UI immediately so the user sees the new session without waiting for history load.
    context.flowChatStore.switchSession(sessionId);

    touchSessionActivity(
      sessionId,
      session?.workspacePath,
      session?.remoteConnectionId,
      session?.remoteSshHost
    ).catch(error => {
      log.debug('Failed to touch session activity', { sessionId, error });
    });

    if (session?.isHistorical) {
      // Load history in the background — do not block the UI.
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
    const removedSessionIds = context.flowChatStore.getCascadeSessionIds(sessionId);
    await context.flowChatStore.deleteSession(sessionId);
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
    sourceSession.remoteSshHost
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
  const needsBackendSetup = isHistoricalSession || isFirstTurn;
  /** Avoid createSession when historical data is already loaded but backend files are missing (e.g. new SSH connection id). */
  const allowRecreateOnCoordinatorFailure =
    needsBackendSetup && !(isHistoricalSession && session.dialogTurns.length > 1);

  const clearHistoricalFlag = () => {
    if (!isHistoricalSession) return;
    context.flowChatStore.setState(prev => {
      const newSessions = new Map(prev.sessions);
      const sess = newSessions.get(sessionId);
      if (sess) {
        newSessions.set(sessionId, { ...sess, isHistorical: false });
      }
      return { ...prev, sessions: newSessions };
    });
  };

  try {
    await agentAPI.ensureCoordinatorSession({
      sessionId,
      workspacePath,
      remoteConnectionId: effectiveConnectionId,
      remoteSshHost: effectiveSshHost,
    });
    clearHistoricalFlag();
  } catch (e: any) {
    if (!allowRecreateOnCoordinatorFailure) {
      const raw = typeof e?.message === 'string' ? e.message : String(e);
      const hint =
        raw.includes('Session metadata not found') || raw.includes('Not found')
          ? '在后端找不到该会话数据。若刚重新连接过 SSH 远程工作区，请关闭并重新打开该远程项目，或新建会话后再试。'
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
      remoteConnectionId: effectiveConnectionId,
      remoteSshHost: effectiveSshHost,
      config: {
        modelName: latestSession.config.modelName || 'auto',
        enableTools: true,
        safeMode: true,
        remoteConnectionId: effectiveConnectionId,
        remoteSshHost: effectiveSshHost,
      }
    });
    clearHistoricalFlag();
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
    remoteConnectionId: session.remoteConnectionId,
    remoteSshHost: session.remoteSshHost,
    config: {
      modelName: session.config.modelName || 'auto',
      enableTools: true,
      safeMode: true,
      remoteConnectionId: session.remoteConnectionId,
      remoteSshHost: session.remoteSshHost,
    }
  });
}
