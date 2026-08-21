/**
 * Session management module
 * Handles session creation, switching, deletion, and other operations
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { isSessionInUseError } from '@/infrastructure/api/errors/TauriCommandError';
import { notificationService } from '../../../shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { isRemoteTraceContext, startupTrace } from '@/shared/utils/startupTrace';
import { elapsedMs, nowMs } from '@/shared/utils/timing';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceManager } from '@/infrastructure/services/business/workspaceManager';
import { isPeerDeviceModeActive } from '@/infrastructure/peer-device/peerModeFlag';
import {
  getActiveSurfaceScope,
  isSurfaceChangedError,
  type SurfaceScope,
} from '@/infrastructure/peer-device/deviceSurface';
import { normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';
import { isRemoteWorkspace, WorkspaceKind, type WorkspaceInfo } from '@/shared/types';
import type {
  FlowChatContext,
  SessionConfig,
  SessionHistoryHydrationLocation,
} from './types';
import type { Session } from '../../types/flow-chat';
import { touchSessionActivity } from './PersistenceModule';
import {
  createTextSessionTitleDescriptor,
  createDefaultSessionTitleDescriptor,
  deriveSessionTitleStateFromMetadata,
  getNextDefaultSessionTitleCount,
  resolveSessionTitle,
} from '../../utils/sessionTitle';
import { buildCreateSessionRelationship } from '../../utils/sessionMetadata';
import {
  clearHistorySessionOpenTransition,
  clearRecentHistorySessionOpenIntent,
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
import {
  requireSessionProjectWorkspacePath,
  sessionProjectWorkspacePath,
} from '../../utils/sessionWorkspace';
import { driverForCreation, driverForSession } from '../../session-drivers/registry';
import {
  isProjectedFirstRuntimeTurn,
  isProjectedSessionEmpty,
} from '../../utils/flowChatTurnIdentity';

const log = createLogger('SessionModule');
const pendingSessionCreations = new Map<string, Promise<string>>();

const getHydrationLocationKey = (
  location: SessionHistoryHydrationLocation | undefined,
): string => location?.workspacePath
  ? JSON.stringify([
      location.workspacePath,
      location.remoteConnectionId ?? '',
      location.remoteSshHost ?? '',
    ])
  : '';
export const SESSION_ACTIVITY_TOUCH_DELAY_MS = 350;
let latestSwitchRequestId = 0;
let pendingActivityTouchTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleSessionActivityTouch(scope: SurfaceScope, task: () => void): void {
  if (pendingActivityTouchTimer !== null) {
    clearTimeout(pendingActivityTouchTimer);
  }
  pendingActivityTouchTimer = setTimeout(() => {
    pendingActivityTouchTimer = null;
    if (!scope.isCurrent()) {
      return;
    }
    task();
  }, SESSION_ACTIVITY_TOUCH_DELAY_MS);
}

const normalizeOptional = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
};

export function pendingHistoryLoadKey(
  sessionId: string,
  scope: SurfaceScope = getActiveSurfaceScope(),
): string {
  return scope.key('history-load', scope.epoch, sessionId);
}

function pendingHistoryLoadSessionId(
  key: string,
  scope: SurfaceScope,
): string | null {
  try {
    const parsed = JSON.parse(key) as unknown;
    if (
      Array.isArray(parsed)
      && parsed[0] === scope.surfaceId
      && parsed[1] === 'history-load'
      && parsed[2] === scope.epoch
      && typeof parsed[3] === 'string'
    ) {
      return parsed[3];
    }
  } catch {
    // In-flight keys are internal and non-persistent; unknown keys are ignored.
  }
  return null;
}

function hasCompetingHistoryLoad(
  context: FlowChatContext,
  sessionId: string,
  scope: SurfaceScope = getActiveSurfaceScope(),
): boolean {
  return Array.from(context.pendingHistoryLoads.keys()).some(key => {
    const pendingSessionId = pendingHistoryLoadSessionId(key, scope);
    return pendingSessionId !== null && pendingSessionId !== sessionId;
  });
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
    allowNonHistorical?: boolean;
    includeInternal?: boolean;
    deferFullHistoryUntilActive?: boolean;
    location?: SessionHistoryHydrationLocation;
  },
): Promise<void> {
  const surfaceScope = getActiveSurfaceScope();
  const pendingKey = pendingHistoryLoadKey(sessionId, surfaceScope);
  const existing = context.pendingHistoryLoads.get(pendingKey);
  if (existing) {
    const existingCapabilities = context.pendingHistoryLoadCapabilities?.get(pendingKey);
    const requestedLocationKey = getHydrationLocationKey(options?.location);
    const requiresStrongerHydrate =
      (options?.includeInternal === true && existingCapabilities?.includeInternal !== true) ||
      (options?.deferFullHistoryUntilActive === false &&
        existingCapabilities?.deferFullHistoryUntilActive !== false) ||
      (Boolean(requestedLocationKey) && existingCapabilities?.locationKey !== requestedLocationKey);
    startupTrace.markPhase('historical_session_hydrate_reused');
    recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_reused_pending', {
      notifyOnError,
      retryActiveStaleReuse: options?.retryActiveStaleReuse === true,
    });
    let existingFailed = false;
    let existingError: unknown;
    try {
      await existing;
      surfaceScope.assertCurrent('reuse historical session hydration');
    } catch (error) {
      existingFailed = true;
      existingError = error;
    }
    if (requiresStrongerHydrate) {
      surfaceScope.assertCurrent('upgrade historical session hydration');
      if (context.pendingHistoryLoads.get(pendingKey) === existing) {
        context.pendingHistoryLoads.delete(pendingKey);
      }
      if (context.pendingHistoryLoadCapabilities?.get(pendingKey)?.promise === existing) {
        context.pendingHistoryLoadCapabilities.delete(pendingKey);
      }
      await hydrateHistoricalSession(context, sessionId, notifyOnError, options);
      return;
    }
    if (existingFailed) {
      throw existingError;
    }
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
      if (context.pendingHistoryLoads.get(pendingKey) === existing) {
        context.pendingHistoryLoads.delete(pendingKey);
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
    surfaceScope.assertCurrent('start historical session hydration');
    const session = context.flowChatStore.getState().sessions.get(sessionId);
    if (!session || (!session.isHistorical && options?.allowNonHistorical !== true)) {
      recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_request_skipped', {
        reason: session ? 'not_historical' : 'missing_session',
      });
      return;
    }

    const workspacePath = requireSessionWorkspacePath(
      session.workspacePath || options?.location?.workspacePath,
      sessionId,
    );
    const storedConnectionId = options?.location?.remoteConnectionId || session.remoteConnectionId;
    const storedSshHost = options?.location?.remoteSshHost || session.remoteSshHost;
    const remote = isRemoteTraceContext(storedConnectionId, storedSshHost);
    const deferFullHistoryUntilActive = options?.deferFullHistoryUntilActive ?? true;
    markHistorySessionHydratePending(sessionId, {
      notifyOnError,
      remote,
      deferFullHistoryUntilActive,
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
      storedConnectionId,
      storedSshHost,
      workspacePath
    );
    const effectiveSshHost = resolveEffectiveSshHost(
      storedSshHost,
      storedConnectionId,
      workspacePath
    );

    await context.flowChatStore.loadSessionHistory(
      sessionId,
      workspacePath,
      undefined,
      effectiveConnectionId,
      effectiveSshHost,
      {
        includeInternal: options?.includeInternal,
        deferFullHistoryUntilActive,
      },
    );
    surfaceScope.assertCurrent('finish historical session hydration');
  })();

  context.pendingHistoryLoads.set(pendingKey, loadPromise);
  const pendingHistoryLoadCapabilities =
    context.pendingHistoryLoadCapabilities ??= new Map();
  pendingHistoryLoadCapabilities.set(pendingKey, {
    promise: loadPromise,
    includeInternal: options?.includeInternal === true,
    deferFullHistoryUntilActive: options?.deferFullHistoryUntilActive ?? true,
    locationKey: getHydrationLocationKey(options?.location),
  });

  try {
    await loadPromise;
    recordHistorySessionDiagnosticEvent(sessionId, 'hydrate_request_finished');
    startupTrace.markPhase('historical_session_hydrate_request_end', {
      durationMs: elapsedMs(traceStartedAt),
    });
  } catch (error) {
    if (isSurfaceChangedError(error)) {
      throw error;
    }
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
    if (surfaceScope.isCurrent()) {
      clearHistorySessionHydratePending(sessionId, 'settled', {
        pendingStillCurrent: context.pendingHistoryLoads.get(pendingKey) === loadPromise,
      });
    }
    if (context.pendingHistoryLoads.get(pendingKey) === loadPromise) {
      context.pendingHistoryLoads.delete(pendingKey);
    }
    if (context.pendingHistoryLoadCapabilities?.get(pendingKey)?.promise === loadPromise) {
      context.pendingHistoryLoadCapabilities.delete(pendingKey);
    }
  }
}

export async function hydrateSessionHistoryForDetail(
  context: FlowChatContext,
  sessionId: string,
  location?: SessionHistoryHydrationLocation,
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  await hydrateHistoricalSession(context, sessionId, false, {
    allowNonHistorical: true,
    includeInternal: session?.sessionKind === 'subagent',
    deferFullHistoryUntilActive: false,
    location,
  });
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
    if (isSurfaceChangedError(error)) {
      return;
    }
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
  // Peer Device Mode: always prefer the live peer workspace over any stale
  // controller-local path left in FlowChat context.
  if (isPeerDeviceModeActive()) {
    const peerWorkspace = workspaceManager.getState().currentWorkspace;
    const peerRoot = peerWorkspace?.rootPath?.trim();
    if (peerRoot) {
      return peerWorkspace?.workspaceKind === WorkspaceKind.Remote
        ? normalizeRemoteWorkspacePath(peerRoot)
        : peerRoot;
    }
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

    const availableModes = await agentAPI.getAvailableModes({
      workspacePath: workspace?.rootPath,
      remoteConnectionId: isRemoteWorkspace(workspace) ? workspace?.connectionId : undefined,
      remoteSshHost: isRemoteWorkspace(workspace) ? workspace?.sshHost : undefined,
    });
    if (availableModes.some(mode => mode.id === configuredDefaultMode)) {
      return configuredDefaultMode;
    }

    log.warn('Ignoring unavailable default chat input mode preference during session creation', {
      modeId: configuredDefaultMode,
    });
  } catch (error) {
    if (isSurfaceChangedError(error)) {
      throw error;
    }
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

export { getModelMaxTokens } from '../../utils/modelResolution';

/**
 * Create new chat session (managed by backend)
 */
export async function createChatSession(
  context: FlowChatContext,
  config: SessionConfig,
  mode?: string
): Promise<string> {
  const surfaceScope = getActiveSurfaceScope();
  try {
    const workspacePath = resolveSessionWorkspacePath(context, config);
    const workspace = resolveSessionWorkspace(context, config);

    if (!workspacePath) {
      throw new Error('Workspace path is required to create a session');
    }
    const projectWorkspacePath = config.projectWorkspacePath || workspacePath;
    const remoteConnectionId =
      workspace?.workspaceKind === WorkspaceKind.Remote ? workspace.connectionId : undefined;
    const remoteSshHost =
      workspace?.workspaceKind === WorkspaceKind.Remote
        ? workspace.sshHost?.trim() || undefined
        : undefined;
    const agentType = await resolveAgentTypeForSessionCreation(mode, workspace);
    surfaceScope.assertCurrent('resolve session creation mode');
    const sessionMode = normalizeSessionDisplayMode(agentType, workspace);
    const workspaceCreationKey =
      workspace?.id?.trim()
        ? workspace.id
        : remoteConnectionId != null && remoteConnectionId !== ''
          ? `${remoteConnectionId}\n${workspacePath}`
          : workspacePath;
    const creationKey = surfaceScope.key(
      'session-create',
      surfaceScope.epoch,
      workspaceCreationKey,
      agentType,
      JSON.stringify(config.executionTargetRequest ?? { kind: 'local' }),
      JSON.stringify(config.dispatchTargetRequest ?? { kind: 'local' }),
    );

    const pendingCreation = pendingSessionCreations.get(creationKey);
    if (pendingCreation) {
      return pendingCreation;
    }

    // Register the pending promise before any async work below. Remote workspace
    // activation can rerun initialization while model config is still loading.
    const createPromise = Promise.resolve().then(async () => {
      surfaceScope.assertCurrent('start session creation');
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

      const sessionId = await driverForCreation(config).createSession(context, {
        surfaceScope,
        config,
        agentType,
        sessionName,
        titleDescriptor,
        workspacePath,
        projectWorkspacePath,
        workspaceId: workspace?.id,
        remoteConnectionId,
        remoteSshHost,
      });
      surfaceScope.assertCurrent('finish session creation');
      return sessionId;
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
    if (isSurfaceChangedError(error)) {
      throw error;
    }
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
  const surfaceScope = getActiveSurfaceScope();
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
      if (driverForSession(sessionId, session).id === 'dispatch') {
        return;
      }
      scheduleSessionActivityTouch(surfaceScope, () => {
        const latestState = context.flowChatStore.getState();
        const latestSession = latestState.sessions.get(sessionId);
        if (switchRequestId !== latestSwitchRequestId || latestState.activeSessionId !== sessionId || !latestSession) {
          return;
        }
        touchSessionActivity(
          sessionId,
          sessionProjectWorkspacePath(latestSession),
          latestSession.remoteConnectionId,
          latestSession.remoteSshHost
        ).catch(error => {
          if (isSurfaceChangedError(error)) {
            return;
          }
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
          isRetryStillRelevant: () => (
            surfaceScope.isCurrent() && switchRequestId === latestSwitchRequestId
          ),
          retryActiveStaleReuse: shouldActivateBeforeHydrate,
        });
        surfaceScope.assertCurrent('hydrate session before switch');
      } catch (error) {
        if (isSurfaceChangedError(error)) {
          throw error;
        }
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
      surfaceScope.assertCurrent('activate switched session');
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
      void hydrateHistoricalSession(context, sessionId, true).catch(error => {
        if (!isSurfaceChangedError(error)) {
          log.debug('Historical session background hydrate failed', { sessionId, error });
        }
      });
    }
  } catch (error) {
    if (isSurfaceChangedError(error)) {
      throw error;
    }
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
    // Deletion cancels any speculative pointer-down transition before awaiting
    // the backend. Otherwise a target that is never activated can leave the
    // history-open shield visible until its safety timeout.
    removedSessionIds.forEach(removedSessionId => {
      clearRecentHistorySessionOpenIntent(removedSessionId);
      clearHistorySessionOpenTransition(removedSessionId);
    });
    const session = stateBeforeDelete.sessions.get(sessionId);
    await driverForSession(sessionId, session).deleteSession(context, sessionId, {
      removedSessionIds,
      removedActiveSession,
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

    await driverForSession(sessionId, session).archiveSession(context, sessionId, {
      removedSessionIds,
      removedActiveSession,
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
  return driverForSession(sessionId, session).renameSession(context, sessionId, trimmedTitle);
}

export async function reloadSessionTitle(
  context: FlowChatContext,
  sessionId: string,
): Promise<void> {
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) return;

  const metadata = await sessionAPI.loadSessionMetadata(
    sessionId,
    requireSessionProjectWorkspacePath(session, sessionId),
    session.remoteConnectionId,
    session.remoteSshHost,
  );
  if (!metadata) return;

  const titleState = deriveSessionTitleStateFromMetadata(metadata);
  context.flowChatStore.setState(previous => {
    const current = previous.sessions.get(sessionId);
    if (!current) return previous;

    const sessions = new Map(previous.sessions);
    sessions.set(sessionId, {
      ...current,
      ...titleState,
      titleStatus: 'generated',
    });
    return { ...previous, sessions };
  });
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
  if (driverForSession(sourceSessionId, sourceSession).id === 'dispatch') {
    throw new Error('Forking a detached dispatch session is not supported');
  }

  const executionWorkspacePath = requireSessionWorkspacePath(
    sourceSession.workspacePath,
    sourceSessionId
  );
  const projectWorkspacePath = requireSessionProjectWorkspacePath(
    sourceSession,
    sourceSessionId,
  );

  const response = await sessionAPI.forkSession(
    sourceSessionId,
    sourceTurnId,
    projectWorkspacePath,
    sourceSession.remoteConnectionId,
    sourceSession.remoteSshHost
  );

  const currentState = context.flowChatStore.getState();
  if (!currentState.sessions.has(response.sessionId)) {
    context.flowChatStore.createSession(
      response.sessionId,
      {
        ...sourceSession.config,
        workspacePath: executionWorkspacePath,
        projectWorkspacePath,
        workspaceId: sourceSession.workspaceId,
        remoteConnectionId: sourceSession.remoteConnectionId,
        remoteSshHost: sourceSession.remoteSshHost,
      },
      undefined,
      response.sessionName,
      sourceSession.maxContextTokens,
      sourceSession.mode,
      executionWorkspacePath,
      sourceSession.remoteConnectionId,
      sourceSession.remoteSshHost,
      createTextSessionTitleDescriptor(response.sessionName),
    );
  } else {
    context.flowChatStore.switchSession(response.sessionId);
  }

  await context.flowChatStore.loadSessionHistory(
    response.sessionId,
    projectWorkspacePath,
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
  const surfaceScope = getActiveSurfaceScope();
  const session = context.flowChatStore.getState().sessions.get(sessionId);
  if (!session) {
    throw new Error(`Session does not exist: ${sessionId}`);
  }
  if (session.isTransient) {
    return;
  }
  if (driverForSession(sessionId, session).id === 'dispatch') {
    return;
  }

  if (session.isHistorical) {
    await hydrateHistoricalSession(context, sessionId, false);
    surfaceScope.assertCurrent('hydrate session before backend readiness');
  }

  const latestSession = context.flowChatStore.getState().sessions.get(sessionId) ?? session;
  const workspacePath = requireSessionWorkspacePath(latestSession.workspacePath, sessionId);
  const projectWorkspacePath = requireSessionProjectWorkspacePath(latestSession, sessionId);

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
  const isFirstTurn = isProjectedFirstRuntimeTurn(latestSession);
  const requiresContextRestore =
    latestSession.contextRestoreState === 'pending' ||
    latestSession.contextRestoreState === 'failed';
  const needsBackendSetup = isHistoricalSession || isFirstTurn || requiresContextRestore;
  const hasProjectedTurns = !isProjectedSessionEmpty(latestSession);
  /** Avoid createSession when historical data is already loaded but backend files are missing (e.g. new SSH connection id). */
  const allowRecreateOnCoordinatorFailure =
    needsBackendSetup &&
    !(requiresContextRestore && hasProjectedTurns) &&
    !(isHistoricalSession && hasProjectedTurns);

  const markBackendContextReady = () => {
    if (!isHistoricalSession && !requiresContextRestore) return;
    if (!surfaceScope.isCurrent()) return;
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
    if (!surfaceScope.isCurrent()) return;
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
    surfaceScope.assertCurrent('ensure coordinator session');
    await agentAPI.ensureCoordinatorSession({
      sessionId,
      workspacePath: projectWorkspacePath,
      remoteConnectionId: effectiveConnectionId,
      remoteSshHost: effectiveSshHost,
      includeInternal: latestSession.sessionKind === 'subagent',
    });
    surfaceScope.assertCurrent('ensure coordinator session');
    markBackendContextReady();
  };

  const restorePendingBackendContext = async () => {
    if (!context.pendingContextRestores) {
      context.pendingContextRestores = new Map();
    }
    const restoreKey = surfaceScope.key(
      'context-restore',
      surfaceScope.epoch,
      sessionId,
      projectWorkspacePath,
      effectiveConnectionId,
      effectiveSshHost,
    );
    const existingRestore = context.pendingContextRestores.get(restoreKey);
    if (existingRestore) {
      await existingRestore;
      surfaceScope.assertCurrent('reuse backend context restore');
      return;
    }

    const restorePromise = ensureCoordinator().catch(error => {
      if (!isSurfaceChangedError(error)) {
        markBackendContextFailed();
      }
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
    if (isSurfaceChangedError(e)) {
      throw e;
    }
    surfaceScope.assertCurrent('handle coordinator session failure');
    if (isSessionInUseError(e)) {
      throw e;
    }
    if (!allowRecreateOnCoordinatorFailure) {
      const raw = typeof e?.message === 'string' ? e.message : String(e);
      const hint =
        raw.includes('Session metadata not found') || raw.includes('Not found')
          ? i18nService.t('flow-chat:historyState.remoteSessionMissing')
          : raw;
      throw new Error(hint);
    }

    log.debug('Coordinator session missing, creating backend session', { sessionId, error: e });
    surfaceScope.assertCurrent('recreate backend session');
    await agentAPI.createSession({
      sessionId: sessionId,
      sessionName:
        resolveSessionTitle(latestSession, (key, options) => i18nService.t(key, options)) ||
        `Session ${sessionId.slice(0, 8)}`,
      agentType: latestSession.mode || 'agentic',
      workspacePath,
      projectWorkspacePath,
      executionTarget:
        latestSession.config.executionTarget?.worktreeId
          ? {
              kind: 'existingWorktree',
              worktreeId: latestSession.config.executionTarget.worktreeId,
            }
          : { kind: 'local' },
      workspaceId: latestSession.workspaceId,
      remoteConnectionId: effectiveConnectionId,
      remoteSshHost: effectiveSshHost,
      relationship: buildCreateSessionRelationship(latestSession),
      deepReviewRunManifest: latestSession.deepReviewRunManifest,
      reviewTargetEvidence: latestSession.reviewTargetEvidence,
      config: {
        modelName: latestSession.config.modelName || 'auto',
        enableTools: true,
        safeMode: true,
        remoteConnectionId: effectiveConnectionId,
        remoteSshHost: effectiveSshHost,
      }
    });
    surfaceScope.assertCurrent('recreate backend session');
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
  const projectWorkspacePath = requireSessionProjectWorkspacePath(session, sessionId);
  
  await agentAPI.createSession({
    sessionId: sessionId,
    sessionName:
      resolveSessionTitle(session, (key, options) => i18nService.t(key, options)) ||
      `Session ${sessionId.slice(0, 8)}`,
    agentType: session.mode || 'agentic',
    workspacePath,
    projectWorkspacePath,
    executionTarget:
      session.config.executionTarget?.worktreeId
        ? {
            kind: 'existingWorktree',
            worktreeId: session.config.executionTarget.worktreeId,
          }
        : { kind: 'local' },
    workspaceId: session.workspaceId,
    remoteConnectionId: session.remoteConnectionId,
    remoteSshHost: session.remoteSshHost,
    relationship: buildCreateSessionRelationship(session),
    deepReviewRunManifest: session.deepReviewRunManifest,
    reviewTargetEvidence: session.reviewTargetEvidence,
    config: {
      modelName: session.config.modelName || 'auto',
      enableTools: true,
      safeMode: true,
      remoteConnectionId: session.remoteConnectionId,
      remoteSshHost: session.remoteSshHost,
    }
  });
}
