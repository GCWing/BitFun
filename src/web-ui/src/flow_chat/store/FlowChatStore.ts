/**
 * Flow Chat global state store
 * Prevents state loss when components remount
 */

import {
  FlowChatState,
  Session,
  DialogTurn,
  ModelRound,
  FlowItem,
  FlowToolItem,
  FlowImageAnalysisItem,
  ImageAnalysisResult,
  AnyFlowItem,
  AcpContextUsage,
  SessionConfig,
  SessionContextRestoreState,
  SessionHistoryState,
} from '../types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import {
  isRemoteTraceContext,
  markPhaseAfterAnimationFrames,
  startupTrace,
} from '@/shared/utils/startupTrace';
import { elapsedMs, nowMs } from '@/shared/utils/timing';
import { i18nService } from '@/infrastructure/i18n/core/I18nService';
import type { DialogTurnData, LocalCommandMetadata, SessionKind } from '@/shared/types/session-history';
import type {
  SessionInfo as AgentSessionInfo,
  SessionViewRestoreTiming,
} from '@/infrastructure/api/service-api/AgentAPI';
import type { SessionMetadataPage } from '@/infrastructure/api/service-api/SessionAPI';
import {
  deriveLastFinishedAtFromMetadata,
  deriveSessionRelationshipFromMetadata,
  isLegacyPersistedBtwSession,
  normalizeSessionRelationship,
} from '../utils/sessionMetadata';
import type { SessionTitleDescriptor } from '../utils/sessionTitle';
import {
  deriveSessionTitleState,
  deriveSessionTitleStateFromMetadata,
  freezeSessionTitleState,
} from '../utils/sessionTitle';
import {
  isTransientToolStatus,
  normalizeRecoveredRoundStatus,
  normalizeRecoveredTextStatus,
  normalizeRecoveredThinkingStatus,
  normalizeRecoveredToolStatus,
  normalizeRecoveredTurnStatus,
  settleInterruptedDialogTurn,
} from '../utils/dialogTurnStability';
import type { WorkspaceInfo } from '@/shared/types';
import { sessionBelongsToWorkspaceNavRow } from '../utils/sessionOrdering';
import { sessionMatchesWorkspace } from '../utils/workspaceScope';
import { resolveThreadGoalUserMessageDisplay } from '../utils/threadGoalDisplay';

const log = createLogger('FlowChatStore');
const VALID_AGENT_TYPES = new Set([
  'agentic',
  'Multitask',
  'debug',
  'Plan',
  'Cowork',
  'Claw',
  'Team',
  'DeepResearch',
]);
const METADATA_LIST_RECENT_DEDUPE_TTL_MS = 1000;
const HISTORICAL_SESSION_INITIAL_REMOTE_TAIL_TURN_COUNT = 3;
const HISTORICAL_SESSION_INITIAL_LOCAL_TAIL_TURN_COUNT = 8;
const HISTORICAL_SESSION_FULL_HISTORY_IDLE_TIMEOUT_MS = 1500;
const HISTORICAL_SESSION_FULL_HISTORY_FIRST_PAINT_TIMEOUT_MS = 2500;
const HISTORICAL_SESSION_FULL_HISTORY_STABLE_VIEWPORT_DELAY_MS = 250;
const MAX_DEFERRED_FULL_HISTORY_PROJECTIONS = 3;

interface FullHistoryHydrationReleaseOptions {
  immediate?: boolean;
  reason?: string;
}

interface MetadataListRequest {
  promise: Promise<void>;
  completedAtMs?: number;
  cleanupTimer?: ReturnType<typeof setTimeout>;
}

interface MetadataPageRequest {
  promise: Promise<SessionMetadataPage>;
  completedAtMs?: number;
  cleanupTimer?: ReturnType<typeof setTimeout>;
}

interface FullHistoryHydrationRequest {
  sessionId: string;
  remote: boolean;
  requireActiveSession: boolean;
  sessionTraceId: string;
  promise: Promise<void>;
  cancel?: () => void;
  releaseAfterInitialPaint?: (options?: FullHistoryHydrationReleaseOptions) => void;
}

interface CompleteSessionHistoryLoadRequest {
  sessionId: string;
  workspacePath: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  includeInternal?: boolean;
  requireActiveSession?: boolean;
  initialSessionTraceId: string;
  expectedDialogTurnIds: string[];
}

interface DeferredFullHistoryProjection {
  remote: boolean;
  requireActiveSession: boolean;
  expectedDialogTurnIds: string[];
  dialogTurns: DialogTurn[];
  contextRestoreState: SessionContextRestoreState;
  restoredSessionInfo?: AgentSessionInfo;
  restoredLastUserDialogMode?: string;
}

function areStringArraysEqual(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function startsWithStringArray(values: string[], prefix: string[]): boolean {
  return values.length >= prefix.length && prefix.every((value, index) => values[index] === value);
}

function scheduleHistoricalSessionFullHydrate(callback: () => void): () => void {
  let cancelled = false;
  const run = () => {
    if (cancelled) {
      return;
    }
    callback();
  };

  const requestIdleCallback = (globalThis as {
    requestIdleCallback?: (callback: () => void, options?: { timeout?: number }) => number;
  }).requestIdleCallback;
  const cancelIdleCallback = (globalThis as {
    cancelIdleCallback?: (handle: number) => void;
  }).cancelIdleCallback;

  if (typeof requestIdleCallback === 'function') {
    const handle = requestIdleCallback(run, {
      timeout: HISTORICAL_SESSION_FULL_HISTORY_IDLE_TIMEOUT_MS,
    });
    return () => {
      cancelled = true;
      cancelIdleCallback?.(handle);
    };
  }

  const timer = globalThis.setTimeout(run, HISTORICAL_SESSION_FULL_HISTORY_IDLE_TIMEOUT_MS);
  return () => {
    cancelled = true;
    globalThis.clearTimeout(timer);
  };
}

function scheduleLocalHistoricalSessionFullHydrate(
  callback: (reason: 'initial_paint' | 'timeout' | 'explicit') => void,
): {
  cancel: () => void;
  releaseAfterInitialPaint: (options?: FullHistoryHydrationReleaseOptions) => void;
} {
  let cancelled = false;
  let started = false;
  let cancelIdle: (() => void) | undefined;
  let stableViewportTimer: ReturnType<typeof setTimeout> | undefined;
  const timeout = globalThis.setTimeout(
    () => start('timeout'),
    HISTORICAL_SESSION_FULL_HISTORY_FIRST_PAINT_TIMEOUT_MS,
  );

  function start(reason: 'initial_paint' | 'timeout' | 'explicit') {
    if (cancelled || started) {
      return;
    }

    started = true;
    globalThis.clearTimeout(timeout);
    if (stableViewportTimer) {
      globalThis.clearTimeout(stableViewportTimer);
      stableViewportTimer = undefined;
    }
    cancelIdle = scheduleHistoricalSessionFullHydrate(() => callback(reason));
  }

  return {
    cancel: () => {
      cancelled = true;
      globalThis.clearTimeout(timeout);
      if (stableViewportTimer) {
        globalThis.clearTimeout(stableViewportTimer);
      }
      cancelIdle?.();
    },
    releaseAfterInitialPaint: (options?: FullHistoryHydrationReleaseOptions) => {
      if (options?.immediate === true) {
        start('explicit');
        return;
      }
      if (stableViewportTimer || started || cancelled) {
        return;
      }
      globalThis.clearTimeout(timeout);
      stableViewportTimer = globalThis.setTimeout(
        () => start('initial_paint'),
        HISTORICAL_SESSION_FULL_HISTORY_STABLE_VIEWPORT_DELAY_MS,
      );
    },
  };
}

function historicalSessionInitialTailTurnCount(remote: boolean): number {
  return remote
    ? HISTORICAL_SESSION_INITIAL_REMOTE_TAIL_TURN_COUNT
    : HISTORICAL_SESSION_INITIAL_LOCAL_TAIL_TURN_COUNT;
}

function sessionViewRestoreTimingTraceFields(
  timing: SessionViewRestoreTiming | undefined,
): Record<string, unknown> {
  if (!timing) {
    return {};
  }

  return {
    restoreResolveStorageDurationMs: timing.resolveStoragePathDurationMs,
    restoreVisibilityMetadataDurationMs: timing.visibilityMetadataDurationMs,
    restoreLoadSessionWithTurnsDurationMs: timing.loadSessionWithTurnsDurationMs,
    restoreNormalizeTurnIdsDurationMs: timing.normalizeTurnIdsDurationMs,
    restoreTotalDurationMs: timing.totalDurationMs,
    restoreTurnTailCount: timing.turnLoad?.requestedTailTurnCount,
    restoreTurnLoadedCount: timing.turnLoad?.loadedTurnCount,
    restoreTurnTotalCount: timing.turnLoad?.totalTurnCount,
    restoreTurnFileCount: timing.turnLoad?.turnFileCount,
    restoreTurnMissingFileCount: timing.turnLoad?.missingTurnFileCount,
    restoreTurnFastPath: timing.turnLoad?.fastPath,
    restoreTurnMetadataDurationMs: timing.turnLoad?.metadataDurationMs,
    restoreTurnStateDurationMs: timing.turnLoad?.stateDurationMs,
    restoreTurnScanDurationMs: timing.turnLoad?.scanDurationMs,
    restoreTurnReadDurationMs: timing.turnLoad?.readDurationMs,
    restoreTurnMaxReadDurationMs: timing.turnLoad?.maxTurnReadDurationMs,
    restoreTurnBuildSessionDurationMs: timing.turnLoad?.buildSessionDurationMs,
    restoreTurnTotalDurationMs: timing.turnLoad?.totalDurationMs,
  };
}

function isUnsupportedTauriCommandError(error: unknown, command: string): boolean {
  const anyError = error as any;
  const originalError = anyError?.context?.originalError;
  const messageParts = [
    anyError?.message,
    typeof originalError === 'string' ? originalError : originalError?.message,
  ].filter((part): part is string => typeof part === 'string');
  const normalizedMessage = messageParts.join(' ').toLowerCase();
  const normalizedCommand = command.toLowerCase();
  const contextCommand =
    typeof anyError?.context?.command === 'string'
      ? anyError.context.command.toLowerCase()
      : '';
  const mentionsCommand =
    contextCommand === normalizedCommand ||
    normalizedMessage.includes(normalizedCommand);

  if (!mentionsCommand) {
    return false;
  }

  return normalizedMessage.includes('unknown command') ||
    normalizedMessage.includes('command not found') ||
    (normalizedMessage.includes('command') && normalizedMessage.includes('not found')) ||
    normalizedMessage.includes('not registered') ||
    normalizedMessage.includes('is not a function');
}

function restoreCommandSupportKey(
  command: string,
  remoteConnectionId?: string,
  remoteSshHost?: string
): string {
  return JSON.stringify([
    command,
    remoteConnectionId?.trim() || 'local',
    remoteSshHost?.trim().toLowerCase() || '',
  ]);
}

function isValidPersistedAgentType(agentType: string): boolean {
  return VALID_AGENT_TYPES.has(agentType) || agentType.startsWith('acp:');
}

interface SelectorListener<T = any> {
  selector: (state: FlowChatState) => T;
  callback: (selected: T) => void;
  isEqual: (a: T, b: T) => boolean;
  lastValue: T | undefined;
  hasLastValue: boolean;
}

export class FlowChatStore {
  private static instance: FlowChatStore;
  private state: FlowChatState;
  private listeners: Set<(state: FlowChatState) => void> = new Set();
  private selectorListeners: Set<SelectorListener> = new Set();
  private silentMode = false;
  private metadataListRequests = new Map<string, MetadataListRequest>();
  private metadataPageRequests = new Map<string, MetadataPageRequest>();
  private fullHistoryHydrationRequests = new Map<string, FullHistoryHydrationRequest>();
  private deferredFullHistoryProjections = new Map<string, DeferredFullHistoryProjection>();
  private fullHistoryProjectionApplyRequests = new Set<string>();
  private unsupportedRestoreCommands = new Set<string>();
  private onPersistUnreadCompletion?: (sessionId: string, value: 'completed' | 'error' | 'interrupted' | undefined) => void;

  private constructor() {
    this.clearOldStorage();
    this.state = {
      sessions: new Map(),
      activeSessionId: null
    };
  }

  private clearOldStorage(): void {
    try {
      const keysToRemove = [
        'bitfun-flow-chat-state',
        'bitfun-flow-chat-global',
        'bitfun-session-ids'
      ];
      
      keysToRemove.forEach(key => {
        if (localStorage.getItem(key)) {
          localStorage.removeItem(key);
        }
      });

      Object.keys(localStorage).forEach(key => {
        if (key.startsWith('bitfun-session-')) {
          localStorage.removeItem(key);
        }
      });
    } catch (error) {
      log.warn('Failed to clear old storage data', error);
    }
  }


  public static getInstance(): FlowChatStore {
    if (!FlowChatStore.instance) {
      FlowChatStore.instance = new FlowChatStore();
    }
    return FlowChatStore.instance;
  }

  public getState(): FlowChatState {
    return this.state;
  }

  private getMetadataListRequestKey(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
  ): string {
    return JSON.stringify([
      workspacePath,
      remoteConnectionId || '',
      remoteSshHost || '',
    ]);
  }

  private getMetadataPageRequestKey(
    workspacePath: string,
    limit: number,
    cursor?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
  ): string {
    return JSON.stringify([
      workspacePath,
      remoteConnectionId || '',
      remoteSshHost || '',
      cursor || '',
      limit,
    ]);
  }

  private getFullHistoryHydrationKey(
    sessionId: string,
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    includeInternal?: boolean,
  ): string {
    return JSON.stringify([
      sessionId,
      workspacePath,
      remoteConnectionId || '',
      remoteSshHost || '',
      includeInternal === true,
    ]);
  }

  private scheduleCompleteSessionHistoryLoad(request: CompleteSessionHistoryLoadRequest): void {
    const requestKey = this.getFullHistoryHydrationKey(
      request.sessionId,
      request.workspacePath,
      request.remoteConnectionId,
      request.remoteSshHost,
      request.includeInternal,
    );
    if (this.fullHistoryHydrationRequests.has(requestKey)) {
      return;
    }

    const remote = isRemoteTraceContext(request.remoteConnectionId, request.remoteSshHost);
    const requireActiveSession = request.requireActiveSession === true;
    startupTrace.markPhase('historical_session_full_hydrate_scheduled', {
      remote,
      sessionId: request.sessionId,
      sessionTraceId: request.initialSessionTraceId,
      loadedTurnCount: request.expectedDialogTurnIds.length,
      requireActiveSession,
      scheduler: remote ? 'idle' : 'after_initial_paint_idle',
    });

    let cancelScheduled: (() => void) | undefined;
    let releaseAfterInitialPaint: ((options?: FullHistoryHydrationReleaseOptions) => void) | undefined;
    let resolveRequest: (() => void) | undefined;
    const promise = new Promise<void>(resolve => {
      resolveRequest = resolve;
      const startFullHydrate = (trigger: 'idle' | 'initial_paint' | 'timeout' | 'explicit') => {
        startupTrace.markPhase('historical_session_full_hydrate_released', {
          remote,
          sessionId: request.sessionId,
          sessionTraceId: request.initialSessionTraceId,
          trigger,
        });
        void this.completeSessionHistoryLoad(request)
          .catch(error => {
            startupTrace.markPhase('historical_session_full_hydrate_failed', {
              remote,
              sessionId: request.sessionId,
              sessionTraceId: `${request.initialSessionTraceId}-full`,
            });
            log.warn('Failed to complete partial session history restore', {
              sessionId: request.sessionId,
              error,
            });
          })
          .finally(resolve);
      };

      if (remote) {
        cancelScheduled = scheduleHistoricalSessionFullHydrate(() => startFullHydrate('idle'));
        return;
      }

      const scheduled = scheduleLocalHistoricalSessionFullHydrate(startFullHydrate);
      cancelScheduled = scheduled.cancel;
      releaseAfterInitialPaint = scheduled.releaseAfterInitialPaint;
    }).finally(() => {
      const currentRequest = this.fullHistoryHydrationRequests.get(requestKey);
      if (currentRequest?.promise === promise) {
        this.fullHistoryHydrationRequests.delete(requestKey);
      }
    });

    this.fullHistoryHydrationRequests.set(requestKey, {
      sessionId: request.sessionId,
      remote,
      requireActiveSession,
      sessionTraceId: request.initialSessionTraceId,
      promise,
      cancel: () => {
        cancelScheduled?.();
        resolveRequest?.();
      },
      releaseAfterInitialPaint: (options?: FullHistoryHydrationReleaseOptions) => {
        releaseAfterInitialPaint?.(options);
      },
    });
  }

  private cancelLocalSessionHistoryCompletion(sessionId: string, reason: string): boolean {
    let cancelled = false;
    for (const [requestKey, request] of this.fullHistoryHydrationRequests) {
      if (request.sessionId !== sessionId || request.remote || !request.requireActiveSession) {
        continue;
      }
      request.cancel?.();
      this.fullHistoryHydrationRequests.delete(requestKey);
      startupTrace.markPhase('historical_session_full_hydrate_cancelled', {
        remote: false,
        sessionId,
        sessionTraceId: request.sessionTraceId,
        reason,
      });
      cancelled = true;
    }
    return cancelled;
  }

  private clearRemovedSessionHistoryState(sessionIds: Iterable<string>, reason: string): void {
    const removedSessionIds = new Set(sessionIds);
    if (removedSessionIds.size === 0) {
      return;
    }

    for (const [requestKey, request] of this.fullHistoryHydrationRequests) {
      if (!removedSessionIds.has(request.sessionId)) {
        continue;
      }

      request.cancel?.();
      this.fullHistoryHydrationRequests.delete(requestKey);
      startupTrace.markPhase('historical_session_full_hydrate_cancelled', {
        remote: request.remote,
        sessionId: request.sessionId,
        sessionTraceId: request.sessionTraceId,
        reason,
      });
    }

    for (const sessionId of removedSessionIds) {
      this.deferredFullHistoryProjections.delete(sessionId);
      this.fullHistoryProjectionApplyRequests.delete(sessionId);
    }
  }

  private scheduleActiveLocalPartialSessionHistoryCompletion(
    sessionId: string,
    reason: string
  ): boolean {
    const session = this.state.sessions.get(sessionId);
    if (
      this.state.activeSessionId !== sessionId ||
      !session ||
      session.historyState !== 'ready' ||
      session.isPartial !== true ||
      isRemoteTraceContext(session.remoteConnectionId, session.remoteSshHost) ||
      this.hasPendingSessionHistoryCompletion(sessionId) ||
      this.hasDeferredSessionHistoryProjection(sessionId)
    ) {
      return false;
    }

    const workspacePath = session.workspacePath || session.config.workspacePath;
    if (!workspacePath || session.dialogTurns.length === 0) {
      return false;
    }

    const sessionTraceId = `${sessionId.slice(0, 8)}-${Math.random().toString(36).slice(2, 8)}`;
    startupTrace.markPhase('historical_session_full_hydrate_rescheduled', {
      remote: false,
      sessionId,
      sessionTraceId,
      reason,
      loadedTurnCount: session.dialogTurns.length,
      totalTurnCount: session.totalTurnCount,
    });
    this.scheduleCompleteSessionHistoryLoad({
      sessionId,
      workspacePath,
      initialSessionTraceId: sessionTraceId,
      requireActiveSession: true,
      expectedDialogTurnIds: session.dialogTurns.map(turn => turn.id),
    });
    return true;
  }

  public hasPendingSessionHistoryCompletion(sessionId: string): boolean {
    for (const request of this.fullHistoryHydrationRequests.values()) {
      if (request.sessionId === sessionId) {
        return true;
      }
    }
    return false;
  }

  public hasDeferredSessionHistoryProjection(sessionId: string): boolean {
    return this.deferredFullHistoryProjections.has(sessionId);
  }

  public requestSessionFullHistoryProjection(sessionId: string, reason: string): boolean {
    this.fullHistoryProjectionApplyRequests.add(sessionId);
    const applied = this.applyDeferredSessionHistoryProjection(sessionId, reason);
    const released = this.releaseSessionHistoryCompletionAfterInitialPaint(sessionId, {
      immediate: true,
      reason,
    });

    if (!applied && !released) {
      this.fullHistoryProjectionApplyRequests.delete(sessionId);
    }

    if (applied || released) {
      startupTrace.markPhase('historical_session_full_hydrate_projection_requested', {
        sessionId,
        reason,
        applied,
        released,
      });
    }

    return applied || released;
  }

  public releaseSessionHistoryCompletionAfterInitialPaint(
    sessionId: string,
    options?: FullHistoryHydrationReleaseOptions
  ): boolean {
    let released = false;
    for (const request of this.fullHistoryHydrationRequests.values()) {
      if (request.sessionId !== sessionId) {
        continue;
      }
      request.releaseAfterInitialPaint?.(options);
      released = true;
    }
    return released;
  }

  private shouldDeferFullHistoryProjection(sessionId: string, remote: boolean, _requireActiveSession: boolean): boolean {
    return (
      !remote &&
      this.state.activeSessionId === sessionId &&
      !this.fullHistoryProjectionApplyRequests.has(sessionId)
    );
  }

  private setDeferredFullHistoryProjection(
    sessionId: string,
    projection: DeferredFullHistoryProjection
  ): void {
    this.deferredFullHistoryProjections.delete(sessionId);
    this.deferredFullHistoryProjections.set(sessionId, projection);

    while (this.deferredFullHistoryProjections.size > MAX_DEFERRED_FULL_HISTORY_PROJECTIONS) {
      const oldestSessionId = this.deferredFullHistoryProjections.keys().next().value;
      if (!oldestSessionId) {
        break;
      }

      this.deferredFullHistoryProjections.delete(oldestSessionId);
      this.fullHistoryProjectionApplyRequests.delete(oldestSessionId);
      startupTrace.markPhase('historical_session_full_hydrate_deferred_projection_evicted', {
        sessionId: oldestSessionId,
        reason: 'cache-limit',
      });
    }
  }

  private applyDeferredSessionHistoryProjection(sessionId: string, reason: string): boolean {
    const projection = this.deferredFullHistoryProjections.get(sessionId);
    if (!projection) {
      return false;
    }

    const result = this.applyCompletedSessionHistoryProjection(sessionId, projection);
    if (result.applied) {
      this.deferredFullHistoryProjections.delete(sessionId);
      this.fullHistoryProjectionApplyRequests.delete(sessionId);
      startupTrace.markPhase('historical_session_full_hydrate_deferred_projection_applied', {
        remote: projection.remote,
        sessionId,
        reason,
        turnCount: projection.dialogTurns.length,
        preservedTurnCount: result.preservedTurnCount,
      });
    }

    return result.applied;
  }

  private applyCompletedSessionHistoryProjection(
    sessionId: string,
    projection: DeferredFullHistoryProjection
  ): { applied: boolean; preservedTurnCount: number } {
    let applied = false;
    let preservedTurnCount = 0;

    this.setState(prev => {
      if (projection.requireActiveSession && !projection.remote && prev.activeSessionId !== sessionId) {
        return prev;
      }

      const session = prev.sessions.get(sessionId);
      if (!session || session.historyState !== 'ready') {
        return prev;
      }

      const currentDialogTurns = session.dialogTurns;
      const currentDialogTurnIds = currentDialogTurns.map(turn => turn.id);
      const canMergeCurrentTurns =
        areStringArraysEqual(currentDialogTurnIds, projection.expectedDialogTurnIds) ||
        startsWithStringArray(currentDialogTurnIds, projection.expectedDialogTurnIds);
      if (!canMergeCurrentTurns) {
        return prev;
      }

      const currentDialogTurnsById = new Map(currentDialogTurns.map(turn => [turn.id, turn]));
      const restoredDialogTurnIds = new Set(projection.dialogTurns.map(turn => turn.id));
      const appendedCurrentDialogTurns = currentDialogTurns
        .slice(projection.expectedDialogTurnIds.length)
        .filter(turn => !restoredDialogTurnIds.has(turn.id));
      const mergedDialogTurns = [
        ...projection.dialogTurns.map(turn => currentDialogTurnsById.get(turn.id) ?? turn),
        ...appendedCurrentDialogTurns,
      ];
      preservedTurnCount = mergedDialogTurns.reduce(
        (count, turn) => count + (currentDialogTurnsById.get(turn.id) === turn ? 1 : 0),
        0,
      );
      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, {
        ...session,
        dialogTurns: mergedDialogTurns,
        isPartial: false,
        loadedTurnCount: mergedDialogTurns.length,
        totalTurnCount: mergedDialogTurns.length,
        contextRestoreState:
          session.contextRestoreState === 'ready' ? 'ready' : projection.contextRestoreState,
        mode: projection.restoredSessionInfo?.agentType || session.mode,
        lastUserDialogMode: projection.restoredLastUserDialogMode,
        lastSubmittedMode:
          projection.restoredSessionInfo?.lastSubmittedAgentType ?? session.lastSubmittedMode,
      });
      applied = true;

      return {
        ...prev,
        sessions: newSessions,
      };
    });

    return { applied, preservedTurnCount };
  }

  private async completeSessionHistoryLoad(
    request: CompleteSessionHistoryLoadRequest
  ): Promise<void> {
    const fullTraceId = `${request.initialSessionTraceId}-full`;
    const startedAt = nowMs();
    const remote = isRemoteTraceContext(request.remoteConnectionId, request.remoteSshHost);
    if (request.requireActiveSession === true && !remote && this.state.activeSessionId !== request.sessionId) {
      startupTrace.markPhase('historical_session_full_hydrate_skipped', {
        remote,
        sessionId: request.sessionId,
        sessionTraceId: fullTraceId,
        reason: 'inactive-before-start',
      });
      return;
    }
    startupTrace.markPhase('historical_session_full_hydrate_start', {
      remote,
      sessionId: request.sessionId,
      sessionTraceId: fullTraceId,
      loadedTurnCount: request.expectedDialogTurnIds.length,
    });

    const { agentAPI } = await import('@/infrastructure/api');
    const restored = await agentAPI.restoreSessionView(
      request.sessionId,
      request.workspacePath,
      request.remoteConnectionId,
      request.remoteSshHost,
      fullTraceId,
      request.includeInternal,
      undefined,
    );

    if (request.requireActiveSession === true && !remote && this.state.activeSessionId !== request.sessionId) {
      startupTrace.markPhase('historical_session_full_hydrate_skipped', {
        remote,
        sessionId: request.sessionId,
        sessionTraceId: fullTraceId,
        reason: 'inactive-after-restore',
        ...sessionViewRestoreTimingTraceFields(restored.timings),
        durationMs: elapsedMs(startedAt),
      });
      return;
    }

    const convertStartedAt = nowMs();
    const dialogTurns = this.convertToDialogTurns(restored.turns);
    const restoredLastUserDialogMode =
      restored.session.lastUserDialogAgentType || this.deriveLastUserDialogMode(dialogTurns);
    const contextRestoreState: SessionContextRestoreState =
      restored.contextRestoreState === 'ready' ? 'ready' : 'pending';
    startupTrace.markPhase('historical_session_full_hydrate_convert_end', {
      remote,
      sessionId: request.sessionId,
      sessionTraceId: fullTraceId,
      turnCount: dialogTurns.length,
      durationMs: elapsedMs(convertStartedAt),
    });

    const projection: DeferredFullHistoryProjection = {
      remote,
      requireActiveSession: request.requireActiveSession === true,
      expectedDialogTurnIds: request.expectedDialogTurnIds,
      dialogTurns,
      contextRestoreState,
      restoredSessionInfo: restored.session,
      restoredLastUserDialogMode,
    };
    let applied = false;
    let preservedTurnCount = 0;
    if (this.shouldDeferFullHistoryProjection(request.sessionId, remote, request.requireActiveSession === true)) {
      this.setDeferredFullHistoryProjection(request.sessionId, projection);
      startupTrace.markPhase('historical_session_full_hydrate_deferred_projection', {
        remote,
        sessionId: request.sessionId,
        sessionTraceId: fullTraceId,
        turnCount: dialogTurns.length,
      });
    } else {
      const result = this.applyCompletedSessionHistoryProjection(request.sessionId, projection);
      applied = result.applied;
      preservedTurnCount = result.preservedTurnCount;
      if (applied) {
        this.deferredFullHistoryProjections.delete(request.sessionId);
        this.fullHistoryProjectionApplyRequests.delete(request.sessionId);
      }
    }

    startupTrace.markPhase('historical_session_full_hydrate_end', {
      remote,
      sessionId: request.sessionId,
      sessionTraceId: fullTraceId,
      turnCount: dialogTurns.length,
      applied,
      preservedTurnCount,
      ...sessionViewRestoreTimingTraceFields(restored.timings),
      durationMs: elapsedMs(startedAt),
    });
    if (applied) {
      markPhaseAfterAnimationFrames(startupTrace, 'historical_session_full_hydrate_after_state_commit_frame', {
        remote,
        sessionId: request.sessionId,
        sessionTraceId: fullTraceId,
        turnCount: dialogTurns.length,
        durationMs: elapsedMs(startedAt),
      }, {
        frameCount: 2,
      });
    }
  }

  public setState(updater: (prevState: FlowChatState) => FlowChatState): void {
    const newState = updater(this.state);
    this.state = newState;
    
    if (!this.silentMode) {
      // Notify plain listeners (backward compat)
      this.listeners.forEach(listener => {
        try {
          listener(newState);
        } catch (error) {
          console.error('[FlowChatStore] Listener threw an error, skipping:', error);
        }
      });

      // Notify selector listeners
      this.selectorListeners.forEach(entry => {
        try {
          const nextValue = entry.selector(newState);
          if (!entry.hasLastValue || !entry.isEqual(entry.lastValue, nextValue)) {
            entry.lastValue = nextValue;
            entry.hasLastValue = true;
            entry.callback(nextValue);
          }
        } catch (error) {
          console.error('[FlowChatStore] Selector listener threw an error, skipping:', error);
        }
      });
    }
  }
  
  /**
   * Silent state update (does not trigger listeners)
   * Used for batch updates, call notifyListeners() after completion
   */
  public setStateSilent(updater: (prevState: FlowChatState) => FlowChatState): void {
    const prevSilentMode = this.silentMode;
    this.silentMode = true;
    try {
      this.setState(updater);
    } finally {
      this.silentMode = prevSilentMode;
    }
  }
  
  /**
   * Manually notify all listeners (call after batch updates complete)
   */
  public notifyListeners(): void {
    this.listeners.forEach(listener => {
      try {
        listener(this.state);
      } catch (error) {
        console.error('[FlowChatStore] Listener threw an error during notifyListeners, skipping:', error);
      }
    });
    this.selectorListeners.forEach(entry => {
      try {
        const nextValue = entry.selector(this.state);
        if (!entry.hasLastValue || !entry.isEqual(entry.lastValue, nextValue)) {
          entry.lastValue = nextValue;
          entry.hasLastValue = true;
          entry.callback(nextValue);
        }
      } catch (error) {
        console.error('[FlowChatStore] Selector listener threw an error during notifyListeners, skipping:', error);
      }
    });
  }
  
  public beginSilentMode(): void {
    this.silentMode = true;
  }
  
  public endSilentMode(): void {
    this.silentMode = false;
    this.notifyListeners();
  }

  private collectCascadeSessionIds(
    rootSessionId: string,
    sessions: Map<string, Session>
  ): string[] {
    if (!sessions.has(rootSessionId)) {
      return [];
    }

    const childSessionIdsByParent = new Map<string, string[]>();
    sessions.forEach(session => {
      const parentSessionId = session.parentSessionId;
      if (!parentSessionId) {
        return;
      }

      const existing = childSessionIdsByParent.get(parentSessionId) || [];
      existing.push(session.sessionId);
      childSessionIdsByParent.set(parentSessionId, existing);
    });

    const visited = new Set<string>();
    const orderedSessionIds: string[] = [];

    const visit = (sessionId: string): void => {
      if (visited.has(sessionId)) {
        return;
      }

      visited.add(sessionId);
      const childSessionIds = childSessionIdsByParent.get(sessionId) || [];
      childSessionIds.forEach(childSessionId => {
        visit(childSessionId);
      });
      orderedSessionIds.push(sessionId);
    };

    visit(rootSessionId);
    return orderedSessionIds;
  }

  public getCascadeSessionIds(sessionId: string): string[] {
    return this.collectCascadeSessionIds(sessionId, this.state.sessions);
  }

  public subscribe(listener: (state: FlowChatState) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  public subscribeSelector<T>(
    selector: (state: FlowChatState) => T,
    callback: (selected: T) => void,
    options?: { isEqual?: (a: T, b: T) => boolean },
  ): () => void {
    const entry: SelectorListener<T> = {
      selector,
      callback,
      isEqual: options?.isEqual ?? Object.is,
      lastValue: undefined,
      hasLastValue: false,
    };
    this.selectorListeners.add(entry);
    return () => {
      this.selectorListeners.delete(entry);
    };
  }

  /**
   * Register a callback to persist unread completion changes.
   * Called by FlowChatManager during initialization.
   */
  public registerPersistUnreadCompletionCallback(
    callback: (sessionId: string, value: 'completed' | 'error' | 'interrupted' | undefined) => void
  ): void {
    this.onPersistUnreadCompletion = callback;
  }

  private deriveLastUserDialogMode(dialogTurns: DialogTurn[]): string | undefined {
    for (let index = dialogTurns.length - 1; index >= 0; index -= 1) {
      const turn = dialogTurns[index];
      const kind = turn.kind || 'user_dialog';
      const agentType = turn.agentType?.trim();
      if (kind === 'user_dialog' && agentType) {
        return agentType;
      }
    }

    return undefined;
  }

  public createSession(
    sessionId: string,
    config: SessionConfig,
    _unused?: undefined,
    title?: string,
    maxContextTokens?: number,
    mode?: string,
    workspacePath?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    titleDescriptor?: SessionTitleDescriptor,
  ): void {
    import('../state-machine').then(({ stateMachineManager }) => {
      stateMachineManager.getOrCreate(sessionId);
    });
    
    this.setState(prev => {
      const relationship = normalizeSessionRelationship({ sessionKind: 'normal' });
      const titleState = deriveSessionTitleState(titleDescriptor);
      const session: Session = {
        sessionId,
        title:
          titleState.title ||
          title ||
          i18nService.t('flow-chat:session.new'),
        titleSource: titleState.titleSource,
        titleI18nKey: titleState.titleI18nKey,
        titleI18nParams: titleState.titleI18nParams,
        titleStatus: undefined,
        dialogTurns: [],
        status: 'idle',
        config,
        createdAt: Date.now(),
        lastActiveAt: Date.now(),
        lastFinishedAt: undefined,
        error: null,
        historyState: 'new',
        maxContextTokens: maxContextTokens || 128128,
        mode: mode || 'agentic',
        lastUserDialogMode: undefined,
        lastSubmittedMode: undefined,
        workspacePath,
        workspaceId: config.workspaceId,
        remoteConnectionId,
        remoteSshHost,
        parentSessionId: relationship.parentSessionId,
        sessionKind: relationship.sessionKind,
        parentToolCallId: relationship.parentToolCallId,
        subagentType: relationship.subagentType,
        btwThreads: [],
        btwOrigin: relationship.btwOrigin,
        isTransient: false,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, session);

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId: sessionId
      };
    });
  }

  /**
   * Add a session created externally (e.g., from mobile remote) without switching the active session.
   * workspacePath is stored on the session so the sidebar can filter by current workspace.
   */
  public addExternalSession(
    sessionId: string,
    title: string,
    mode: string,
    workspacePath?: string,
    meta?: {
      parentSessionId?: string;
      sessionKind?: SessionKind;
      btwOrigin?: Session['btwOrigin'];
      parentToolCallId?: string;
      subagentType?: string;
      isTransient?: boolean;
      agentBackedTransient?: boolean;
      deepReviewRunManifest?: Session['deepReviewRunManifest'];
    },
    remoteConnectionId?: string,
    remoteSshHost?: string
  ): void {
    import('../state-machine').then(({ stateMachineManager }) => {
      stateMachineManager.getOrCreate(sessionId);
    });

    this.setState(prev => {
      if (prev.sessions.has(sessionId)) {
        return prev;
      }

      const relationship = normalizeSessionRelationship(meta);
      const session: Session = {
        sessionId,
        title: title || i18nService.t('flow-chat:session.new'),
        titleSource: 'text',
        titleI18nKey: undefined,
        titleI18nParams: undefined,
        titleStatus: 'generated',
        dialogTurns: [],
        status: 'idle',
        config: { maxContextTokens: 128128, autoCompact: true, enableTools: true } as any,
        createdAt: Date.now(),
        lastActiveAt: Date.now(),
        lastFinishedAt: undefined,
        error: null,
        maxContextTokens: 128128,
        mode: mode || 'agentic',
        lastUserDialogMode: undefined,
        lastSubmittedMode: undefined,
        isHistorical: false,
        historyState: 'new',
        workspacePath,
        remoteConnectionId,
        remoteSshHost,
        parentSessionId: relationship.parentSessionId,
        sessionKind: relationship.sessionKind,
        parentToolCallId: relationship.parentToolCallId,
        subagentType: relationship.subagentType,
        btwThreads: [],
        btwOrigin: relationship.btwOrigin,
        deepReviewRunManifest: meta?.deepReviewRunManifest,
        isTransient: meta?.isTransient ?? false,
        agentBackedTransient: meta?.agentBackedTransient ?? false,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, session);

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public switchSession(sessionId: string): void {
    const previousSessionId = this.state.activeSessionId;
    const targetSessionExists = this.state.sessions.has(sessionId);
    if (targetSessionExists && previousSessionId && previousSessionId !== sessionId) {
      this.cancelLocalSessionHistoryCompletion(previousSessionId, 'session-switch');
    }

    let sessionMode: string | undefined;
    
    this.setState(prev => {
      if (!prev.sessions.has(sessionId)) return prev;
      
      const session = prev.sessions.get(sessionId)!;
      sessionMode = session.mode;
      
      const updatedSession = {
        ...session,
        lastActiveAt: Date.now()
      };
      
      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId: sessionId
      };
    });
    
    window.dispatchEvent(new CustomEvent('bitfun:session-switched', {
      detail: { sessionId, mode: sessionMode || 'agentic' }
    }));

    if (targetSessionExists && previousSessionId !== sessionId) {
      this.scheduleActiveLocalPartialSessionHistoryCompletion(sessionId, 'session-switch');
    }
  }

  /**
   * Update session mode
   * @param sessionId Session ID
   * @param mode Mode ID (e.g., 'agentic', 'Plan')
   */
  public updateSessionMode(sessionId: string, mode: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      if (session.mode === mode) return prev;

      const updatedSession = {
        ...session,
        mode,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Record the mode used by the most recent user submission accepted by the runtime.
   * Unlike `lastUserDialogMode`, this does not rewind when history is rolled back.
   */
  public updateSessionLastSubmittedMode(sessionId: string, mode: string): void {
    const normalizedMode = mode.trim();
    if (!normalizedMode) {
      return;
    }

    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || session.lastSubmittedMode === normalizedMode) {
        return prev;
      }

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, {
        ...session,
        lastSubmittedMode: normalizedMode,
        lastActiveAt: Date.now(),
      });

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public setGoalModeActive(sessionId: string, active: boolean): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      if (Boolean(session.goalModeActive) === active) return prev;

      const updatedSession = {
        ...session,
        goalModeActive: active,
        lastActiveAt: Date.now(),
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public setThreadGoal(
    sessionId: string,
    goal: Session['threadGoal'] | null
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const active =
        Boolean(goal) &&
        (goal!.status === 'active' || goal!.status === 'budgetLimited');

      const prevGoal = session.threadGoal;
      const sameGoal =
        (prevGoal == null && goal == null) ||
        (prevGoal != null &&
          goal != null &&
          prevGoal.goalId === goal.goalId &&
          prevGoal.status === goal.status &&
          prevGoal.objective === goal.objective &&
          prevGoal.updatedAt === goal.updatedAt &&
          prevGoal.tokensUsed === goal.tokensUsed &&
          prevGoal.tokenBudget === goal.tokenBudget &&
          prevGoal.timeUsedSeconds === goal.timeUsedSeconds &&
          (prevGoal.autoContinuationCount ?? 0) === (goal.autoContinuationCount ?? 0));

      if (sameGoal && Boolean(session.goalModeActive) === active) {
        return prev;
      }

      const updatedSession = {
        ...session,
        threadGoal: goal ?? undefined,
        goalModeActive: active,
        lastActiveAt: Date.now(),
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public updateSessionModelName(sessionId: string, modelName: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const normalizedModelName = modelName.trim() || 'auto';
      if ((session.config.modelName || 'auto') === normalizedModelName) {
        return prev;
      }

      const updatedSession = {
        ...session,
        config: {
          ...session.config,
          modelName: normalizedModelName,
        },
        lastActiveAt: Date.now(),
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  /**
   * Update session relationship metadata (parent/child grouping, kind, etc.).
   * This is UI-only and does not affect backend behavior directly.
   */
  public updateSessionRelationship(
    sessionId: string,
    updates: {
      parentSessionId?: string;
      sessionKind?: SessionKind;
      parentToolCallId?: string;
      subagentType?: string;
    }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const relationship = normalizeSessionRelationship({
        sessionKind: updates.sessionKind ?? session.sessionKind,
        parentSessionId: updates.parentSessionId ?? session.parentSessionId,
        btwOrigin: session.btwOrigin,
        parentToolCallId:
          updates.parentToolCallId !== undefined
            ? updates.parentToolCallId
            : session.parentToolCallId,
        subagentType:
          updates.subagentType !== undefined
            ? updates.subagentType
            : session.subagentType,
      });
      const next: Session = {
        ...session,
        parentSessionId: relationship.parentSessionId,
        sessionKind: relationship.sessionKind,
        parentToolCallId: relationship.parentToolCallId,
        subagentType: relationship.subagentType,
        btwOrigin: relationship.btwOrigin,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, next);

      return { ...prev, sessions: newSessions };
    });
  }

  public updateSessionBtwOrigin(
    sessionId: string,
    origin: Session['btwOrigin'],
    sessionKind: SessionKind = 'btw'
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const relationship = normalizeSessionRelationship({
        sessionKind,
        parentSessionId: origin?.parentSessionId ?? session.parentSessionId,
        btwOrigin: { ...(session.btwOrigin || {}), ...(origin || {}) },
      });
      const next: Session = {
        ...session,
        parentSessionId: relationship.parentSessionId,
        sessionKind: relationship.sessionKind,
        btwOrigin: relationship.btwOrigin,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, next);
      return { ...prev, sessions: newSessions };
    });
  }

  public addBtwThreadMarker(
    parentSessionId: string,
    marker: {
      requestId: string;
      childSessionId: string;
      title: string;
      status: 'running' | 'done' | 'error';
      createdAt: number;
      parentDialogTurnId?: string;
      parentTurnIndex?: number;
      error?: string;
    }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(parentSessionId);
      if (!session) return prev;

      const existing = session.btwThreads || [];
      if (existing.some(t => t.requestId === marker.requestId)) {
        return prev;
      }

      const nextSession: Session = {
        ...session,
        btwThreads: [marker, ...existing].slice(0, 20),
        lastActiveAt: Date.now(),
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(parentSessionId, nextSession);
      return { ...prev, sessions: newSessions };
    });
  }

  public updateBtwThreadMarker(
    parentSessionId: string,
    requestId: string,
    updates: Partial<{
      status: 'running' | 'done' | 'error';
      error?: string;
      title: string;
    }>
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(parentSessionId);
      if (!session) return prev;

      const existing = session.btwThreads || [];
      if (existing.length === 0) return prev;

      const nextThreads = existing.map(t => {
        if (t.requestId !== requestId) return t;
        return { ...t, ...updates };
      });

      const newSessions = new Map(prev.sessions);
      newSessions.set(parentSessionId, { ...session, btwThreads: nextThreads });
      return { ...prev, sessions: newSessions };
    });
  }

  public removeBtwThreadMarker(parentSessionId: string, requestId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(parentSessionId);
      if (!session) return prev;
      const existing = session.btwThreads || [];
      const nextThreads = existing.filter(t => t.requestId !== requestId);
      const newSessions = new Map(prev.sessions);
      newSessions.set(parentSessionId, { ...session, btwThreads: nextThreads });
      return { ...prev, sessions: newSessions };
    });
  }

  /**
   * Move session to front by updating createdAt timestamp
   */
  public moveSessionToFront(sessionId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession = {
        ...session,
        createdAt: Date.now(),
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public async deleteSession(sessionId: string): Promise<void> {
    const sessionIdsToDelete = this.getCascadeSessionIds(sessionId);
    if (sessionIdsToDelete.length === 0) {
      return;
    }

    const { stateMachineManager } = await import('../state-machine');
    sessionIdsToDelete.forEach(id => {
      stateMachineManager.delete(id);
    });

    try {
      const { agentAPI } = await import('@/infrastructure/api');
      const deleteResults = await Promise.allSettled(
        sessionIdsToDelete.map(async id => {
          const sess = this.state.sessions.get(id);
          const workspacePath = sess?.workspacePath;
          if (!workspacePath) {
            throw new Error(`Workspace path not found for session ${id}`);
          }

          await agentAPI.deleteSession(
            id,
            workspacePath,
            sess?.remoteConnectionId,
            sess?.remoteSshHost
          );
        })
      );

      deleteResults.forEach((result, index) => {
        if (result.status === 'rejected') {
          log.error('Failed to delete session on backend', {
            sessionId: sessionIdsToDelete[index],
            error: result.reason,
          });
        }
      });
    } catch (error) {
      log.error('Failed to delete session on backend', { sessionId, error });
    }

    this.removeSession(sessionId);
  }

  public removeSession(sessionId: string): string[] {
    const removedSessionIds = this.getCascadeSessionIds(sessionId);
    if (removedSessionIds.length === 0) {
      return [];
    }
    this.clearRemovedSessionHistoryState(removedSessionIds, 'session-removed');

    this.setState(prev => {
      const removedSessionIdSet = new Set(removedSessionIds);
      const newSessions = new Map(prev.sessions);
      const removedSessions = removedSessionIds
        .map(id => prev.sessions.get(id))
        .filter((session): session is Session => Boolean(session));

      removedSessionIds.forEach(id => {
        newSessions.delete(id);
      });

      removedSessions.forEach(session => {
        const parentSessionId = session.btwOrigin?.parentSessionId ?? session.parentSessionId;
        if (!parentSessionId || removedSessionIdSet.has(parentSessionId)) {
          return;
        }

        const parentSession = newSessions.get(parentSessionId);
        if (!parentSession?.btwThreads?.length) {
          return;
        }

        const requestId = session.btwOrigin?.requestId;
        const nextThreads = parentSession.btwThreads.filter(thread => {
          if (thread.childSessionId === session.sessionId) {
            return false;
          }

          if (requestId && thread.requestId === requestId) {
            return false;
          }

          return true;
        });

        if (nextThreads.length !== parentSession.btwThreads.length) {
          newSessions.set(parentSessionId, {
            ...parentSession,
            btwThreads: nextThreads,
          });
        }
      });

      let newActiveSessionId = prev.activeSessionId;
      if (prev.activeSessionId && removedSessionIdSet.has(prev.activeSessionId)) {
        const remainingSessions = Array.from(newSessions.keys());
        newActiveSessionId = remainingSessions.length > 0 ? remainingSessions[0] : null;
      }

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId: newActiveSessionId
      };
    });

    return removedSessionIds;
  }

  public clearSession(sessionId?: string): void {
    const targetSessionId = sessionId || this.state.activeSessionId;
    if (!targetSessionId) return;

    this.setState(prev => {
      const session = prev.sessions.get(targetSessionId);
      if (!session) return prev;

      const clearedSession = {
        ...session,
        dialogTurns: [],
        error: null,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(targetSessionId, clearedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Remove sessions bound to a workspace using stable id + host/path scope (never path-only).
   */
  public removeSessionsForWorkspace(
    workspace: Pick<WorkspaceInfo, 'id' | 'rootPath' | 'connectionId' | 'sshHost'>
  ): string[] {
    const removedSessionIds = Array.from(this.state.sessions.values())
      .filter(session => sessionMatchesWorkspace(session, workspace))
      .map(session => session.sessionId);

    return this.removeSessionsByIds(removedSessionIds);
  }

  public async cancelRunningSessionsForWorkspace(
    workspace: Pick<WorkspaceInfo, 'id' | 'rootPath' | 'connectionId' | 'sshHost'>
  ): Promise<string[]> {
    const runningSessionIds = Array.from(this.state.sessions.values())
      .filter(session => sessionMatchesWorkspace(session, workspace))
      .filter(session => {
        const lastTurn = session.dialogTurns[session.dialogTurns.length - 1];
        return Boolean(
          lastTurn &&
          !['completed', 'cancelled', 'error'].includes(lastTurn.status)
        );
      })
      .map(session => session.sessionId);

    if (runningSessionIds.length === 0) {
      return [];
    }

    const { agentAPI } = await import('@/infrastructure/api');
    await Promise.allSettled(
      runningSessionIds.map(async sessionId => {
        try {
          await agentAPI.cancelSession(sessionId);
        } catch (error) {
          log.warn('Failed to cancel running session before closing workspace', {
            sessionId,
            workspaceId: workspace.id,
            error,
          });
        } finally {
          this.cancelSessionTask(sessionId);
        }
      })
    );

    return runningSessionIds;
  }

  /** @deprecated Prefer `removeSessionsForWorkspace` with full `WorkspaceInfo`. */
  public removeSessionsByWorkspace(
    workspacePath: string,
    remoteConnectionId?: string | null,
    remoteSshHost?: string | null
  ): string[] {
    const removedSessionIds = Array.from(this.state.sessions.values())
      .filter(session =>
        sessionBelongsToWorkspaceNavRow(session, workspacePath, remoteConnectionId, remoteSshHost)
      )
      .map(session => session.sessionId);

    return this.removeSessionsByIds(removedSessionIds);
  }

  private removeSessionsByIds(removedSessionIds: string[]): string[] {

    if (removedSessionIds.length === 0) {
      return [];
    }
    this.clearRemovedSessionHistoryState(removedSessionIds, 'sessions-removed');

    const removedSessionIdSet = new Set(removedSessionIds);

    this.setState(prev => {
      const newSessions = new Map(prev.sessions);
      removedSessionIdSet.forEach(sessionId => {
        newSessions.delete(sessionId);
      });

      return {
        ...prev,
        sessions: newSessions,
        activeSessionId:
          prev.activeSessionId && removedSessionIdSet.has(prev.activeSessionId)
            ? null
            : prev.activeSessionId
      };
    });

    return removedSessionIds;
  }

  public getActiveSession(): Session | null {
    if (!this.state.activeSessionId) {
      return null;
    }
    return this.state.sessions.get(this.state.activeSessionId) || null;
  }

  public addDialogTurn(sessionId: string, dialogTurn: DialogTurn): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      if (session.dialogTurns.some(turn => turn.id === dialogTurn.id)) {
        return prev;
      }

      const updatedDialogTurns = [...session.dialogTurns, dialogTurn];
      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        lastUserDialogMode: this.deriveLastUserDialogMode(updatedDialogTurns),
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public addLocalUsageReportTurn(params: {
    sessionId: string;
    markdown: string;
    reportId: string;
    schemaVersion: number;
    generatedAt: number;
    report?: Record<string, any>;
    status?: 'loading' | 'completed';
  }): DialogTurn | null {
    const session = this.state.sessions.get(params.sessionId);
    if (!session) {
      log.warn('Session not found, cannot add local usage report', { sessionId: params.sessionId });
      return null;
    }

    const metadata: LocalCommandMetadata = {
      localCommandKind: 'usage_report',
      reportId: params.reportId,
      schemaVersion: params.schemaVersion,
      generatedAt: params.generatedAt,
      modelVisible: false,
      usageReport: params.report,
      usageReportStatus: params.status ?? 'completed',
    };
    const turnIndex = session.dialogTurns.length;
    const dialogTurn: DialogTurn = {
      id: `local-usage-${params.reportId}`,
      sessionId: params.sessionId,
      kind: 'local_command',
      userMessage: {
        id: `local-usage-user-${params.reportId}`,
        content: params.markdown,
        timestamp: params.generatedAt,
        metadata,
      },
      modelRounds: [],
      status: params.status === 'loading' ? 'processing' : 'completed',
      startTime: params.generatedAt,
      endTime: params.generatedAt,
      backendTurnIndex: turnIndex,
    };

    this.setState(prev => {
      const currentSession = prev.sessions.get(params.sessionId);
      if (!currentSession) return prev;

      if (currentSession.dialogTurns.some(turn => turn.id === dialogTurn.id)) {
        return prev;
      }

      const newSessions = new Map(prev.sessions);
      newSessions.set(params.sessionId, {
        ...currentSession,
        dialogTurns: [...currentSession.dialogTurns, dialogTurn],
      });

      return {
        ...prev,
        sessions: newSessions,
      };
    });
    return dialogTurn;
  }

  public deleteDialogTurn(sessionId: string, dialogTurnId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedDialogTurns = session.dialogTurns.filter(turn => turn.id !== dialogTurnId);

      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        lastUserDialogMode: this.deriveLastUserDialogMode(updatedDialogTurns),
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Delete all dialog turns from turnIndex (inclusive)
   * Used for turn rollback: revert to before this turn and remove this turn and all subsequent history
   */
  public truncateDialogTurnsFrom(sessionId: string, turnIndex: number): void {

    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const clampedIndex = Math.max(0, Math.min(turnIndex, session.dialogTurns.length));
      const updatedDialogTurns = session.dialogTurns.slice(0, clampedIndex);
      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        lastUserDialogMode: this.deriveLastUserDialogMode(updatedDialogTurns),
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public updateDialogTurn(
    sessionId: string,
    dialogTurnId: string,
    updater: (turn: DialogTurn) => DialogTurn,
    options?: { touchActivity?: boolean }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedDialogTurns = session.dialogTurns.map(turn => 
        turn.id === dialogTurnId ? updater(turn) : turn
      );

      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        lastActiveAt: options?.touchActivity === false
          ? session.lastActiveAt
          : Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Add image analysis phase to dialog turn
   */
  public addImageAnalysisPhase(
    sessionId: string, 
    dialogTurnId: string, 
    imageContexts: import('@/shared/types/context').ImageContext[]
  ): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      const imageAnalysisItems: FlowImageAnalysisItem[] = imageContexts.map((ctx, index) => ({
        id: `img-analysis-${ctx.id}`,
        type: 'image-analysis',
        imageContext: ctx,
        result: null,
        status: 'analyzing',
        timestamp: Date.now() + index,
      }));

      return {
        ...turn,
        imageAnalysisPhase: {
          items: imageAnalysisItems,
          status: 'analyzing',
          startTime: Date.now(),
        },
        status: 'image_analyzing',
      };
    });
  }

  /**
   * Update image analysis results
   */
  public updateImageAnalysisResults(
    sessionId: string,
    dialogTurnId: string,
    results: ImageAnalysisResult[]
  ): void {
      this.updateDialogTurn(sessionId, dialogTurnId, turn => {
        if (!turn.imageAnalysisPhase) {
          log.warn('Attempting to update non-existent image analysis phase', { sessionId, dialogTurnId });
          return turn;
        }

      const updatedItems: FlowImageAnalysisItem[] = turn.imageAnalysisPhase.items.map(item => {
        const result = results.find(r => r.image_id === item.imageContext.id);
        if (result) {
          return {
            ...item,
            result,
            status: 'completed' as const,
          };
        }
        return item;
      });

      const allCompleted = updatedItems.every(item => item.status === 'completed');

      return {
        ...turn,
        imageAnalysisPhase: {
          ...turn.imageAnalysisPhase,
          items: updatedItems,
          status: allCompleted ? 'completed' : 'analyzing',
          endTime: allCompleted ? Date.now() : undefined,
        },
        status: allCompleted ? 'pending' : 'image_analyzing',
      };
    });
  }

  /**
   * Update single image analysis item status (for error handling)
   */
  public updateImageAnalysisItem(
    sessionId: string,
    dialogTurnId: string,
    imageId: string,
    updates: { status?: 'analyzing' | 'completed' | 'error'; error?: string; result?: any }
  ): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      if (!turn.imageAnalysisPhase) return turn;

      const updatedItems = turn.imageAnalysisPhase.items.map(item => {
        if (item.imageContext.id === imageId) {
          return { ...item, ...updates };
        }
        return item;
      });

      return {
        ...turn,
        imageAnalysisPhase: {
          ...turn.imageAnalysisPhase,
          items: updatedItems,
        },
      };
    });
  }

  public addModelRound(sessionId: string, dialogTurnId: string, modelRound: ModelRound): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => ({
      ...turn,
      modelRounds: [...turn.modelRounds, modelRound],
      status: 'processing'
    }));
  }

  public updateModelRound(sessionId: string, dialogTurnId: string, modelRoundId: string, updater: (round: ModelRound) => ModelRound): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => ({
      ...turn,
      modelRounds: turn.modelRounds.map(round => 
        round.id === modelRoundId ? updater(round) : round
      )
    }));
  }

  /**
   * Batch update multiple model round items (reduces store update frequency)
   */
  public batchUpdateModelRoundItems(
    sessionId: string, 
    dialogTurnId: string, 
    updates: Array<{ itemId: string; changes: Partial<FlowItem> }>
  ): void {
    if (updates.length === 0) return;
    
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      const updatedModelRounds = turn.modelRounds.map(round => ({
        ...round,
        items: round.items.map(item => {
          const update = updates.find(u => u.itemId === item.id);
          return update ? ({ ...item, ...update.changes } as AnyFlowItem) : item;
        })
      }));
      
      return {
        ...turn,
        modelRounds: updatedModelRounds
      };
    });
  }

  public addModelRoundItem(sessionId: string, dialogTurnId: string, item: AnyFlowItem, modelRoundId?: string): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      let targetModelRoundIndex = turn.modelRounds.length - 1;
        if (modelRoundId) {
          targetModelRoundIndex = turn.modelRounds.findIndex(round => round.id === modelRoundId);
          if (targetModelRoundIndex === -1) {
            log.warn('Model round not found', { sessionId, dialogTurnId, modelRoundId });
            return turn;
          }
        }
        
        if (targetModelRoundIndex === -1) {
          log.warn('No available model rounds', { sessionId, dialogTurnId });
          return turn;
        }

      const targetModelRound = turn.modelRounds[targetModelRoundIndex];

      const existingItem = targetModelRound.items.find(existingItem => existingItem.id === item.id);
      if (existingItem) {
        return turn;
      }

      const updatedModelRounds = [...turn.modelRounds];
      
      updatedModelRounds[targetModelRoundIndex] = {
        ...targetModelRound,
        items: [...targetModelRound.items, item]
      };

      return {
        ...turn,
        modelRounds: updatedModelRounds
      };
    });
  }

  /**
   * Silent add ModelRound item (does not trigger listeners)
   * Used for batch update scenarios
   */
  public addModelRoundItemSilent(sessionId: string, dialogTurnId: string, item: AnyFlowItem, modelRoundId?: string): void {
    const prevSilentMode = this.silentMode;
    this.silentMode = true;
    try {
      this.addModelRoundItem(sessionId, dialogTurnId, item, modelRoundId);
    } finally {
      this.silentMode = prevSilentMode;
    }
  }

  public updateModelRoundItem(sessionId: string, dialogTurnId: string, itemId: string, updates: Partial<FlowItem>): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => {
      let updated = false;
      
      const updatedModelRounds = turn.modelRounds.map(modelRound => {
        if (updated) return modelRound;
        
        const updatedItems = modelRound.items.map((item: any) => {
          const toolCallId =
            item.type === 'tool' ? ((item as FlowToolItem).toolCall?.id as string | undefined) : undefined;
          const idMatches = item.id === itemId || (toolCallId !== undefined && toolCallId === itemId);
          if (idMatches) {
            const updatedItem = { ...item, ...updates };
            return updatedItem;
          }
          return item;
        });
        
        if (updatedItems.some((item: any) => {
          const tc = item.type === 'tool' ? (item as FlowToolItem).toolCall?.id : undefined;
          return item.id === itemId || tc === itemId;
        })) {
          updated = true;
          return { ...modelRound, items: updatedItems };
        }
        
        return modelRound;
      });
      
      if (!updated) {
        log.warn('Item not found for update', { sessionId, dialogTurnId, itemId });
        return turn;
      }

      return {
        ...turn,
        modelRounds: updatedModelRounds
      };
    });
  }

  /**
   * Silent update ModelRound item (does not trigger listeners)
   * Used for batch update scenarios
   */
  public updateModelRoundItemSilent(sessionId: string, dialogTurnId: string, itemId: string, updates: Partial<FlowItem>): void {
    const prevSilentMode = this.silentMode;
    this.silentMode = true;
    try {
      this.updateModelRoundItem(sessionId, dialogTurnId, itemId, updates);
    } finally {
      this.silentMode = prevSilentMode;
    }
  }

  /**
   * Find tool item (for early detection updates)
   */
  public findToolItem(sessionId: string, dialogTurnId: string, toolUseId: string): FlowItem | null {
    const session = this.state.sessions.get(sessionId);
    if (!session) return null;

    const dialogTurn = session.dialogTurns.find(turn => turn.id === dialogTurnId);
    if (!dialogTurn) return null;

    for (const modelRound of dialogTurn.modelRounds) {
      const item = modelRound.items.find((item: any) => {
        if (item.id === toolUseId) return true;
        if (item.type === 'tool') {
          const ti = item as FlowToolItem;
          return ti.toolCall?.id === toolUseId;
        }
        return false;
      });
      if (item) {
        return item;
      }
    }

    return null;
  }

  public updateTokenUsage(
    sessionId: string, 
    tokenUsage: { inputTokens: number; outputTokens?: number; totalTokens: number }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession = {
        ...session,
        currentTokenUsage: {
          inputTokens: tokenUsage.inputTokens,
          outputTokens: tokenUsage.outputTokens,
          totalTokens: tokenUsage.totalTokens,
          timestamp: Date.now()
        }
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public updateAcpContextUsage(
    sessionId: string,
    contextUsage: { used: number; size: number; cost?: { amount: number; currency: string } }
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const nextUsage: AcpContextUsage = {
        used: contextUsage.used,
        size: contextUsage.size,
        timestamp: Date.now(),
      };
      if (contextUsage.cost) {
        nextUsage.cost = contextUsage.cost;
      }

      const currentUsage = session.currentAcpContextUsage;
      if (
        currentUsage &&
        currentUsage.used === nextUsage.used &&
        currentUsage.size === nextUsage.size &&
        currentUsage.cost?.amount === nextUsage.cost?.amount &&
        currentUsage.cost?.currency === nextUsage.cost?.currency
      ) {
        return prev;
      }

      const updatedSession = {
        ...session,
        currentAcpContextUsage: nextUsage,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public rollbackTokenUsage(): void {
  }

  public updateSessionMaxContextTokens(sessionId: string, maxContextTokens: number): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      if (session.maxContextTokens === maxContextTokens) return prev;

      const updatedSession = {
        ...session,
        maxContextTokens
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public setError(sessionId: string, error: string | null): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession: Session = {
        ...session,
        error,
        status: error ? 'error' as const : 'idle' as const,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public markSessionFinished(sessionId: string, timestamp: number = Date.now()): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession: Session = {
        ...session,
        lastActiveAt: timestamp,
        lastFinishedAt: timestamp,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public markSessionUnreadCompletion(
    sessionId: string,
    completionKind: 'completed' | 'error' | 'interrupted'
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession: Session = {
        ...session,
        hasUnreadCompletion: completionKind,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return { ...prev, sessions: newSessions };
    });
    this.onPersistUnreadCompletion?.(sessionId, completionKind);
  }

  public clearSessionUnreadCompletion(sessionId: string): void {
    let didClear = false;
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || !session.hasUnreadCompletion) return prev;

      const updatedSession: Session = {
        ...session,
        hasUnreadCompletion: undefined,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      didClear = true;
      return { ...prev, sessions: newSessions };
    });
    if (didClear) {
      this.onPersistUnreadCompletion?.(sessionId, undefined);
    }
  }

  public setSessionNeedsAttention(
    sessionId: string,
    attentionKind: 'ask_user' | 'tool_confirm'
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const updatedSession: Session = {
        ...session,
        needsUserAttention: attentionKind,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return { ...prev, sessions: newSessions };
    });
    this.onPersistUnreadCompletion?.(sessionId, undefined);
  }

  public clearSessionNeedsAttention(sessionId: string): void {
    let didClear = false;
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || !session.needsUserAttention) return prev;

      const updatedSession: Session = {
        ...session,
        needsUserAttention: undefined,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      didClear = true;
      return { ...prev, sessions: newSessions };
    });
    if (didClear) {
      this.onPersistUnreadCompletion?.(sessionId, undefined);
    }
  }

  public async updateSessionTitle(
    sessionId: string,
    title: string,
    status: 'generating' | 'generated' | 'failed'
  ): Promise<void> {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      // As soon as the user meaningfully interacts with the session we freeze the
      // title to text, so later locale changes do not rewrite real conversation names.
      const nextTitleState = freezeSessionTitleState(title);
      const updatedSession = {
        ...session,
        ...nextTitleState,
        titleStatus: status,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Cancel current session task (UI state update)
   * Called by SessionStateMachine side effects, updates all related states to cancelled
   */
  public cancelSessionTask(sessionId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found', { sessionId });
        return prev;
      }

      const lastDialogTurn = session.dialogTurns[session.dialogTurns.length - 1];
      if (!lastDialogTurn) {
        log.warn('No dialog turns found', { sessionId });
        return prev;
      }

      if (lastDialogTurn.status === 'completed' || lastDialogTurn.status === 'cancelled') {
        return prev;
      }

      const settledAt = Date.now();
      const updatedDialogTurns = session.dialogTurns.map((turn, index) =>
        index === session.dialogTurns.length - 1
          ? settleInterruptedDialogTurn(turn, settledAt)
          : turn
      );

      const updatedSession = {
        ...session,
        dialogTurns: updatedDialogTurns,
        status: 'idle' as const,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      window.dispatchEvent(new CustomEvent('bitfun:dialog-cancelled', {
        detail: { sessionId }
      }));

      const lastTurn = updatedDialogTurns[updatedDialogTurns.length - 1];
      if (lastTurn && lastTurn.status === 'cancelled') {
        this.saveCancelledDialogTurn(sessionId, lastTurn.id).catch(error => {
          log.error('Failed to save cancelled dialog turn', { sessionId, turnId: lastTurn.id, error });
        });
      }

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Save cancelled dialog turn to disk
   */
  private async saveCancelledDialogTurn(sessionId: string, turnId: string): Promise<void> {
    try {
      const { sessionAPI } = await import('@/infrastructure/api/service-api/SessionAPI');
      const session = this.state.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found, skipping save', { sessionId, turnId });
        return;
      }
      if (session.isTransient) {
        return;
      }

      const workspacePath = session.workspacePath;
      if (!workspacePath) {
        log.warn('Workspace path not available, skipping save', { sessionId, turnId });
        return;
      }

      const dialogTurn = session.dialogTurns.find(turn => turn.id === turnId);
      if (!dialogTurn) {
        log.warn('Dialog turn not found, skipping save', { sessionId, turnId });
        return;
      }

      const turnIndex = session.dialogTurns.findIndex(t => t.id === turnId);
      
      const turnData = {
        turnId,
        turnIndex,
        sessionId,
        timestamp: dialogTurn.startTime,
        kind: dialogTurn.kind || 'user_dialog',
        userMessage: {
          id: dialogTurn.userMessage.id,
          content: dialogTurn.userMessage.content,
          timestamp: dialogTurn.userMessage.timestamp,
          metadata: dialogTurn.userMessage.metadata,
        },
        modelRounds: dialogTurn.modelRounds.map((round, roundIndex) => {
          const textItems = round.items
            .filter(item => item.type === 'text' && !(item as any).runtimeStatus)
            .map(item => ({
              id: item.id,
              content: (item as any).content || '',
              isStreaming: false,
              timestamp: item.timestamp,
              status: item.status,
            }));
          
          const toolItems = round.items
            .filter(item => item.type === 'tool')
            .map(item => ({
              id: item.id,
              toolName: (item as any).toolName || '',
              interruptionReason: (item as any).interruptionReason,
              toolCall: (item as any).toolCall || { input: {}, id: item.id },
              toolResult: (item as any).toolResult,
              aiIntent: (item as any).aiIntent,
              startTime: (item as any).startTime || item.timestamp,
              endTime: (item as any).endTime,
              status: item.status,
              durationMs: (item as any).durationMs ?? ((item as any).endTime
                ? (item as any).endTime - (item as any).startTime
                : undefined),
              queueWaitMs: (item as any).queueWaitMs,
              preflightMs: (item as any).preflightMs,
              confirmationWaitMs: (item as any).confirmationWaitMs,
              executionMs: (item as any).executionMs,
            }));
          
          const thinkingItems = round.items
            .filter(item => item.type === 'thinking')
            .map(item => ({
              id: item.id,
              content: (item as any).content || '',
              isStreaming: false,
              isCollapsed: (item as any).isCollapsed || false,
              timestamp: item.timestamp,
              status: item.status,
            }));
          
          return {
            id: round.id,
            turnId,
            roundIndex,
            timestamp: round.startTime,
            renderHints: round.renderHints,
            textItems,
            toolItems,
            thinkingItems,
            startTime: round.startTime,
            endTime: round.endTime || Date.now(),
            durationMs: round.durationMs,
            providerId: round.providerId,
            modelId: round.modelId,
            modelAlias: round.modelAlias,
            firstChunkMs: round.firstChunkMs,
            firstVisibleOutputMs: round.firstVisibleOutputMs,
            streamDurationMs: round.streamDurationMs,
            attemptCount: round.attemptCount,
            failureCategory: round.failureCategory,
            tokenDetails: round.tokenDetails,
            status: round.status
          };
        }),
        startTime: dialogTurn.startTime,
        endTime: dialogTurn.endTime || Date.now(),
        durationMs: (dialogTurn.endTime || Date.now()) - dialogTurn.startTime,
        status: 'cancelled' as const
      };

      await sessionAPI.saveSessionTurn(
        turnData,
        workspacePath,
        session.remoteConnectionId,
        session.remoteSshHost
      );
    } catch (error) {
      log.error('Failed to save cancelled dialog turn', { sessionId, turnId, error });
    }
  }


  /**
   * Initialize by loading persisted session metadata from disk
   * Clears sessions from other workspaces, then loads sessions for the target workspace.
   */
  public async refreshWorkspaceFromDisk(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    traceSource = 'refresh'
  ): Promise<void> {
    const requestKey = this.getMetadataListRequestKey(workspacePath, remoteConnectionId, remoteSshHost);
    this.metadataListRequests.delete(requestKey);
    await this.initializeFromDisk(workspacePath, remoteConnectionId, remoteSshHost, traceSource);
  }

  public async initializeFromDisk(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    traceSource = 'unknown'
  ): Promise<void> {
    const requestKey = this.getMetadataListRequestKey(workspacePath, remoteConnectionId, remoteSshHost);
    const existingRequest = this.metadataListRequests.get(requestKey);
    const remote = isRemoteTraceContext(remoteConnectionId, remoteSshHost);
    if (existingRequest) {
      const completedAtMs = existingRequest.completedAtMs;
      const isRecentCompletedRequest =
        completedAtMs !== undefined &&
        elapsedMs(completedAtMs) <= METADATA_LIST_RECENT_DEDUPE_TTL_MS;

      if (completedAtMs === undefined || isRecentCompletedRequest) {
        startupTrace.markPhase('session_metadata_list_deduped', {
          remote,
          source: traceSource,
          dedupeState: completedAtMs === undefined ? 'in-flight' : 'recent',
        });
        return existingRequest.promise;
      }

      if (existingRequest.cleanupTimer) {
        clearTimeout(existingRequest.cleanupTimer);
      }
      this.metadataListRequests.delete(requestKey);
    }

    let succeeded = false;
    const loadPromise = this.initializeFromDiskUncached(
      workspacePath,
      remoteConnectionId,
      remoteSshHost,
      traceSource,
    ).then(result => {
      succeeded = result;
    });

    const request: MetadataListRequest = { promise: loadPromise };
    this.metadataListRequests.set(requestKey, request);

    loadPromise.finally(() => {
      const currentRequest = this.metadataListRequests.get(requestKey);
      if (currentRequest !== request) {
        return;
      }

      if (!succeeded) {
        this.metadataListRequests.delete(requestKey);
        return;
      }

      request.completedAtMs = nowMs();
      request.cleanupTimer = setTimeout(() => {
        if (this.metadataListRequests.get(requestKey) === request) {
          this.metadataListRequests.delete(requestKey);
        }
      }, METADATA_LIST_RECENT_DEDUPE_TTL_MS);
    });

    return loadPromise;
  }

  private async loadSessionMetadataModelConfig(): Promise<{
    models: any[];
    defaultModels: Record<string, string>;
  }> {
    let models: any[] = [];
    let defaultModels: Record<string, string> = {};
    try {
      const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
      const [modelsResult, defaultModelsResult] = await Promise.allSettled([
        configManager.getConfig<any[]>('ai.models'),
        configManager.getConfig<Record<string, string>>('ai.default_models'),
      ]);

      if (modelsResult.status === 'fulfilled' && Array.isArray(modelsResult.value)) {
        models = modelsResult.value;
      }
      if (
        defaultModelsResult.status === 'fulfilled' &&
        defaultModelsResult.value &&
        typeof defaultModelsResult.value === 'object'
      ) {
        defaultModels = defaultModelsResult.value;
      }
    } catch (error) {
      log.warn('Failed to load model config for session metadata, using defaults', { error });
    }

    return { models, defaultModels };
  }

  private async processPersistedSessionMetadataList(
    sessions: any[],
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
  ): Promise<void> {
    const { stateMachineManager } = await import('../state-machine');
    const { models, defaultModels } = await this.loadSessionMetadataModelConfig();

    const processSession = async (metadata: any) => {
      try {
        const existingSession = this.state.sessions.get(metadata.sessionId);
        if (existingSession) {
          return;
        }
        if (isLegacyPersistedBtwSession(metadata)) {
          return;
        }
        // Skip archived sessions - they are managed in the settings page.
        if (metadata.status === 'archived') {
          return;
        }

        stateMachineManager.getOrCreate(metadata.sessionId);

        let maxContextTokens = 128128;
        if (metadata.modelName) {
          const model = models.find((m: any) => m.name === metadata.modelName || m.id === metadata.modelName);
          if (model?.context_window) {
            maxContextTokens = model.context_window;
          }
        }

        if (maxContextTokens === 128128) {
          const primaryModelId = defaultModels?.primary;

          if (primaryModelId) {
            const primaryModel = models.find((m: any) => m.id === primaryModelId);
            if (primaryModel?.context_window) {
              maxContextTokens = primaryModel.context_window;
            }
          }
        }

        const relationship = deriveSessionRelationshipFromMetadata(metadata);
        const lastFinishedAt = deriveLastFinishedAtFromMetadata(metadata);
        const titleState = deriveSessionTitleStateFromMetadata(metadata);
        const hasDynamicDefaultTitle = titleState.titleSource === 'i18n';

        this.setState(prev => {
          if (prev.sessions.has(metadata.sessionId)) {
            return prev;
          }

          const rawAgentType = metadata.agentType || 'agentic';
          const validatedAgentType = isValidPersistedAgentType(rawAgentType) ? rawAgentType : 'agentic';

          if (rawAgentType !== validatedAgentType) {
            log.warn('Invalid agentType, falling back to agentic', { sessionId: metadata.sessionId, rawAgentType, validatedAgentType });
          }

          const session: Session = {
            sessionId: metadata.sessionId,
            title: titleState.title,
            titleSource: titleState.titleSource,
            titleI18nKey: titleState.titleI18nKey,
            titleI18nParams: titleState.titleI18nParams,
            titleStatus: hasDynamicDefaultTitle ? undefined : 'generated',
            dialogTurns: [],
            status: 'idle',
            config: {
              agentType: validatedAgentType,
              modelName: metadata.modelName,
            },
            createdAt: metadata.createdAt,
            lastActiveAt: metadata.lastActiveAt,
            lastFinishedAt,
            error: null,
            isHistorical: true,
            historyState: 'metadata-only',
            todos: (metadata as any).todos || [],
            maxContextTokens,
            mode: validatedAgentType,
            lastUserDialogMode: metadata.lastUserDialogAgentType,
            lastSubmittedMode: metadata.lastSubmittedAgentType,
            workspacePath: (metadata as any).workspacePath || workspacePath,
            remoteConnectionId: metadata.remoteConnectionId || remoteConnectionId,
            remoteSshHost:
              metadata.remoteSshHost || metadata.workspaceHostname || remoteSshHost,
            parentSessionId: relationship.parentSessionId,
            sessionKind: relationship.sessionKind,
            parentToolCallId: relationship.parentToolCallId,
            subagentType: relationship.subagentType,
            btwThreads: [],
            btwOrigin: relationship.btwOrigin,
            hasUnreadCompletion: metadata.unreadCompletion,
            needsUserAttention: metadata.needsUserAttention,
            deepReviewRunManifest: metadata.deepReviewRunManifest,
            isTransient: false,
          };

          const newSessions = new Map(prev.sessions);
          newSessions.set(metadata.sessionId, session);

          return {
            ...prev,
            sessions: newSessions,
          };
        });
      } catch (error) {
        log.warn('Failed to process persisted session metadata', {
          sessionId: metadata?.sessionId,
          error,
        });
      }
    };

    await Promise.all(sessions.map(processSession));
  }

  public async loadSessionMetadataPage(
    workspacePath: string,
    limit: number,
    cursor?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    traceSource = 'unknown'
  ): Promise<SessionMetadataPage> {
    const requestKey = this.getMetadataPageRequestKey(
      workspacePath,
      limit,
      cursor,
      remoteConnectionId,
      remoteSshHost,
    );
    const existingRequest = this.metadataPageRequests.get(requestKey);
    const remote = isRemoteTraceContext(remoteConnectionId, remoteSshHost);
    if (existingRequest) {
      const completedAtMs = existingRequest.completedAtMs;
      const isRecentCompletedRequest =
        completedAtMs !== undefined &&
        elapsedMs(completedAtMs) <= METADATA_LIST_RECENT_DEDUPE_TTL_MS;

      if (completedAtMs === undefined || isRecentCompletedRequest) {
        startupTrace.markPhase('session_metadata_page_deduped', {
          remote,
          source: traceSource,
          cursor: cursor || null,
          limit,
          dedupeState: completedAtMs === undefined ? 'in-flight' : 'recent',
        });
        return existingRequest.promise;
      }

      if (existingRequest.cleanupTimer) {
        clearTimeout(existingRequest.cleanupTimer);
      }
      this.metadataPageRequests.delete(requestKey);
    }

    const loadPromise = this.loadSessionMetadataPageUncached(
      workspacePath,
      limit,
      cursor,
      remoteConnectionId,
      remoteSshHost,
      traceSource,
    );

    const request: MetadataPageRequest = { promise: loadPromise };
    this.metadataPageRequests.set(requestKey, request);

    loadPromise
      .then(() => {
        const currentRequest = this.metadataPageRequests.get(requestKey);
        if (currentRequest !== request) {
          return;
        }

        request.completedAtMs = nowMs();
        request.cleanupTimer = setTimeout(() => {
          if (this.metadataPageRequests.get(requestKey) === request) {
            this.metadataPageRequests.delete(requestKey);
          }
        }, METADATA_LIST_RECENT_DEDUPE_TTL_MS);
      })
      .catch(() => {
        if (this.metadataPageRequests.get(requestKey) === request) {
          this.metadataPageRequests.delete(requestKey);
        }
      });

    return loadPromise;
  }

  private async loadSessionMetadataPageUncached(
    workspacePath: string,
    limit: number,
    cursor?: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    traceSource = 'unknown'
  ): Promise<SessionMetadataPage> {
    const traceStartedAt = nowMs();
    const remote = isRemoteTraceContext(remoteConnectionId, remoteSshHost);
    const metadataListTraceId = `metadata-page-${Math.random().toString(36).slice(2, 8)}`;
    startupTrace.markPhase('session_metadata_page_start', {
      remote,
      source: traceSource,
      metadataListTraceId,
      cursor: cursor || null,
      limit,
    });

    try {
      const importStartedAt = nowMs();
      startupTrace.markPhase('session_metadata_api_import_start', {
        remote,
        source: traceSource,
        metadataListTraceId,
      });
      const { sessionAPI } = await import('@/infrastructure/api/service-api/SessionAPI');
      startupTrace.markPhase('session_metadata_api_import_end', {
        remote,
        source: traceSource,
        metadataListTraceId,
        durationMs: elapsedMs(importStartedAt),
      });
      let page: SessionMetadataPage;
      const pageRequestStartedAt = nowMs();
      try {
        startupTrace.markPhase('session_metadata_page_request_start', {
          remote,
          source: traceSource,
          metadataListTraceId,
          command: 'list_persisted_sessions_page',
        });
        page = await sessionAPI.listSessionsPage({
          workspacePath,
          limit,
          cursor,
          remoteConnectionId,
          remoteSshHost,
        });
        startupTrace.markPhase('session_metadata_page_request_end', {
          remote,
          source: traceSource,
          metadataListTraceId,
          command: 'list_persisted_sessions_page',
          durationMs: elapsedMs(pageRequestStartedAt),
        });
      } catch (error) {
        if (!isUnsupportedTauriCommandError(error, 'list_persisted_sessions_page')) {
          startupTrace.markPhase('session_metadata_page_request_failed', {
            remote,
            source: traceSource,
            metadataListTraceId,
            command: 'list_persisted_sessions_page',
            durationMs: elapsedMs(pageRequestStartedAt),
          });
          throw error;
        }

        const fallbackStartedAt = nowMs();
        startupTrace.markPhase('session_metadata_page_request_start', {
          remote,
          source: traceSource,
          metadataListTraceId,
          command: 'list_persisted_sessions',
          fallback: true,
        });
        const sessions = await sessionAPI.listSessions(workspacePath, remoteConnectionId, remoteSshHost);
        startupTrace.markPhase('session_metadata_page_request_end', {
          remote,
          source: traceSource,
          metadataListTraceId,
          command: 'list_persisted_sessions',
          fallback: true,
          durationMs: elapsedMs(fallbackStartedAt),
        });
        page = {
          sessions,
          totalTopLevelCount: sessions.length,
          loadedTopLevelCount: sessions.length,
          nextCursor: undefined,
          hasMore: false,
        };
      }

      await this.processPersistedSessionMetadataList(
        page.sessions,
        workspacePath,
        remoteConnectionId,
        remoteSshHost,
      );
      startupTrace.markPhase('session_metadata_page_end', {
        remote,
        source: traceSource,
        metadataListTraceId,
        sessionCount: page.sessions.length,
        totalTopLevelCount: page.totalTopLevelCount,
        loadedTopLevelCount: page.loadedTopLevelCount,
        hasMore: page.hasMore,
        durationMs: elapsedMs(traceStartedAt),
      });
      return page;
    } catch (error) {
      startupTrace.markPhase('session_metadata_page_failed', {
        remote,
        source: traceSource,
        metadataListTraceId,
        durationMs: elapsedMs(traceStartedAt),
      });
      log.error('Failed to load persisted session metadata page', error);
      throw error;
    }
  }

  private async initializeFromDiskUncached(
    workspacePath: string,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    traceSource = 'unknown'
  ): Promise<boolean> {
    const traceStartedAt = nowMs();
    const remote = isRemoteTraceContext(remoteConnectionId, remoteSshHost);
    const metadataListTraceId = `metadata-${Math.random().toString(36).slice(2, 8)}`;
    let sessionCount = 0;
    startupTrace.markPhase('session_metadata_list_start', {
      remote,
      source: traceSource,
      metadataListTraceId,
    });
    try {
      const { sessionAPI } = await import('@/infrastructure/api/service-api/SessionAPI');
      const sessions = await sessionAPI.listSessions(workspacePath, remoteConnectionId, remoteSshHost);
      sessionCount = sessions.length;
      startupTrace.markPhase('session_metadata_list_loaded', {
        remote,
        source: traceSource,
        metadataListTraceId,
        sessionCount,
      });

      const { stateMachineManager } = await import('../state-machine');

      let models: any[] = [];
      let defaultModels: Record<string, string> = {};
      try {
        const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
        const [modelsResult, defaultModelsResult] = await Promise.allSettled([
          configManager.getConfig<any[]>('ai.models'),
          configManager.getConfig<Record<string, string>>('ai.default_models'),
        ]);

        if (modelsResult.status === 'fulfilled' && Array.isArray(modelsResult.value)) {
          models = modelsResult.value;
        }
        if (
          defaultModelsResult.status === 'fulfilled' &&
          defaultModelsResult.value &&
          typeof defaultModelsResult.value === 'object'
        ) {
          defaultModels = defaultModelsResult.value;
        }
      } catch (error) {
        log.warn('Failed to load model config for session metadata, using defaults', { error });
      }

      const processSession = async (metadata: any) => {
        try {
          const existingSession = this.state.sessions.get(metadata.sessionId);
          if (existingSession) {
            return;
          }
          if (isLegacyPersistedBtwSession(metadata)) {
            return;
          }
          // Skip archived sessions - they are managed in the settings page
          if (metadata.status === 'archived') {
            return;
          }

          stateMachineManager.getOrCreate(metadata.sessionId);

          let maxContextTokens = 128128;
          if (metadata.modelName) {
            const model = models.find((m: any) => m.name === metadata.modelName || m.id === metadata.modelName);
            if (model?.context_window) {
              maxContextTokens = model.context_window;
            }
          }

          if (maxContextTokens === 128128) {
            const primaryModelId = defaultModels?.primary;

            if (primaryModelId) {
              const primaryModel = models.find((m: any) => m.id === primaryModelId);
              if (primaryModel?.context_window) {
                maxContextTokens = primaryModel.context_window;
              }
            }
          }

          const relationship = deriveSessionRelationshipFromMetadata(metadata);
          const lastFinishedAt = deriveLastFinishedAtFromMetadata(metadata);
          const titleState = deriveSessionTitleStateFromMetadata(metadata);
          const hasDynamicDefaultTitle = titleState.titleSource === 'i18n';

          this.setState(prev => {
            if (prev.sessions.has(metadata.sessionId)) {
              return prev;
            }

            const rawAgentType = metadata.agentType || 'agentic';
            const validatedAgentType = isValidPersistedAgentType(rawAgentType) ? rawAgentType : 'agentic';

            if (rawAgentType !== validatedAgentType) {
              log.warn('Invalid agentType, falling back to agentic', { sessionId: metadata.sessionId, rawAgentType, validatedAgentType });
            }

            const session: Session = {
              sessionId: metadata.sessionId,
              title: titleState.title,
              titleSource: titleState.titleSource,
              titleI18nKey: titleState.titleI18nKey,
              titleI18nParams: titleState.titleI18nParams,
              titleStatus: hasDynamicDefaultTitle ? undefined : 'generated',
              dialogTurns: [],
              status: 'idle',
              config: {
                agentType: validatedAgentType,
                modelName: metadata.modelName,
              },
              createdAt: metadata.createdAt,
              lastActiveAt: metadata.lastActiveAt,
              lastFinishedAt,
              error: null,
              isHistorical: true,
              historyState: 'metadata-only',
              todos: (metadata as any).todos || [],
              maxContextTokens,
              mode: validatedAgentType,
              lastUserDialogMode: metadata.lastUserDialogAgentType,
              lastSubmittedMode: metadata.lastSubmittedAgentType,
              workspacePath: (metadata as any).workspacePath || workspacePath,
              remoteConnectionId: metadata.remoteConnectionId || remoteConnectionId,
              remoteSshHost:
                metadata.remoteSshHost || metadata.workspaceHostname || remoteSshHost,
              parentSessionId: relationship.parentSessionId,
              sessionKind: relationship.sessionKind,
              parentToolCallId: relationship.parentToolCallId,
              subagentType: relationship.subagentType,
              btwThreads: [],
              btwOrigin: relationship.btwOrigin,
              hasUnreadCompletion: metadata.unreadCompletion,
              needsUserAttention: metadata.needsUserAttention,
              deepReviewRunManifest: metadata.deepReviewRunManifest,
              isTransient: false,
            };

            const newSessions = new Map(prev.sessions);
            newSessions.set(metadata.sessionId, session);

            return {
              ...prev,
              sessions: newSessions,
            };
          });
        } catch (error) {
          log.warn('Failed to process persisted session metadata', {
            sessionId: metadata?.sessionId,
            error,
          });
        }
      };
      
      await Promise.all(sessions.map(processSession));
      startupTrace.markPhase('session_metadata_list_end', {
        remote,
        source: traceSource,
        metadataListTraceId,
        sessionCount,
        durationMs: elapsedMs(traceStartedAt),
      });
      return true;
    } catch (error) {
      startupTrace.markPhase('session_metadata_list_failed', {
        remote,
        source: traceSource,
        metadataListTraceId,
        sessionCount,
        durationMs: elapsedMs(traceStartedAt),
      });
      log.error('Failed to load persisted sessions', error);
      return false;
    }
  }

  public setSessionHistoryState(sessionId: string, historyState: SessionHistoryState): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || session.historyState === historyState) {
        return prev;
      }

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, {
        ...session,
        historyState,
      });

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  public setSessionContextRestoreState(
    sessionId: string,
    contextRestoreState: SessionContextRestoreState
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session || session.contextRestoreState === contextRestoreState) {
        return prev;
      }

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, {
        ...session,
        contextRestoreState,
      });

      return {
        ...prev,
        sessions: newSessions,
      };
    });
  }

  /**
   * Lazy load session history (convert historical data to FlowChat format)
   */
  public async loadSessionHistory(
    sessionId: string,
    workspacePath: string,
    limit?: number,
    remoteConnectionId?: string,
    remoteSshHost?: string,
    options?: {
      includeInternal?: boolean;
      deferFullHistoryUntilActive?: boolean;
    }
  ): Promise<void> {
    const traceStartedAt = nowMs();
    const remote = isRemoteTraceContext(remoteConnectionId, remoteSshHost);
    const sessionTraceId = `${sessionId.slice(0, 8)}-${Math.random().toString(36).slice(2, 8)}`;
    startupTrace.markPhase('historical_session_hydrate_start', {
      remote,
      sessionId,
      sessionTraceId,
    });
    this.setSessionHistoryState(sessionId, 'hydrating');

    try {
      const { stateMachineManager } = await import('../state-machine');
      stateMachineManager.getOrCreate(sessionId);
      
      const existingSession = this.state.sessions.get(sessionId);
      const isAcpSession = existingSession?.mode?.startsWith('acp:') ||
        existingSession?.config.agentType?.startsWith('acp:');
      let turns: DialogTurnData[] | undefined;
      let restoredSessionInfo: AgentSessionInfo | undefined;
      let contextRestoreState: SessionContextRestoreState = 'ready';
      let restoredHistoryPartial = false;
      let restoredLoadedTurnCount: number | undefined;
      let restoredTotalTurnCount: number | undefined;
      let restoredTiming: SessionViewRestoreTiming | undefined;
      if (!isAcpSession) {
        const restoreStartedAt = nowMs();
        startupTrace.markPhase('historical_session_restore_start', {
          remote,
          sessionId,
          sessionTraceId,
        });
        try {
          const { agentAPI } = await import('@/infrastructure/api');
          const restoreSessionViewSupportKey = restoreCommandSupportKey(
            'restore_session_view',
            remoteConnectionId,
            remoteSshHost
          );
          const restoreSessionWithTurnsSupportKey = restoreCommandSupportKey(
            'restore_session_with_turns',
            remoteConnectionId,
            remoteSshHost
          );
          const restoreWithTurnsOrSession = async (): Promise<void> => {
            if (
              typeof agentAPI.restoreSessionWithTurns === 'function' &&
              !this.unsupportedRestoreCommands.has(restoreSessionWithTurnsSupportKey)
            ) {
              try {
                const restored = await agentAPI.restoreSessionWithTurns(
                  sessionId,
                  workspacePath,
                  remoteConnectionId,
                  remoteSshHost,
                  sessionTraceId,
                  options?.includeInternal,
                );
                restoredSessionInfo = restored.session;
                turns = restored.turns;
                contextRestoreState = 'ready';
                return;
              } catch (error) {
                if (!isUnsupportedTauriCommandError(error, 'restore_session_with_turns')) {
                  throw error;
                }
                this.unsupportedRestoreCommands.add(restoreSessionWithTurnsSupportKey);
                startupTrace.markPhase('historical_session_restore_fallback', {
                  remote,
                  sessionId,
                  sessionTraceId,
                  from: 'restore_session_with_turns',
                  to: 'restore_session',
                  reason: 'unsupported-command',
                });
              }
            }

            restoredSessionInfo = await agentAPI.restoreSession(
              sessionId,
              workspacePath,
              remoteConnectionId,
              remoteSshHost,
              sessionTraceId,
              options?.includeInternal,
            );
            contextRestoreState = 'ready';
          };

          if (
            typeof agentAPI.restoreSessionView === 'function' &&
            !this.unsupportedRestoreCommands.has(restoreSessionViewSupportKey)
          ) {
            try {
              const restored = await agentAPI.restoreSessionView(
                sessionId,
                workspacePath,
                remoteConnectionId,
                remoteSshHost,
                sessionTraceId,
                options?.includeInternal,
                historicalSessionInitialTailTurnCount(remote),
              );
              restoredSessionInfo = restored.session;
              turns = restored.turns;
              contextRestoreState =
                restored.contextRestoreState === 'ready' ? 'ready' : 'pending';
              restoredHistoryPartial = restored.isPartial === true;
              restoredLoadedTurnCount = restored.loadedTurnCount;
              restoredTotalTurnCount = restored.totalTurnCount;
              restoredTiming = restored.timings;
            } catch (error) {
              if (!isUnsupportedTauriCommandError(error, 'restore_session_view')) {
                throw error;
              }
              this.unsupportedRestoreCommands.add(restoreSessionViewSupportKey);
              startupTrace.markPhase('historical_session_restore_fallback', {
                remote,
                sessionId,
                sessionTraceId,
                from: 'restore_session_view',
                to: 'restore_session_with_turns',
                reason: 'unsupported-command',
              });
              await restoreWithTurnsOrSession();
            }
          } else {
            await restoreWithTurnsOrSession();
          }
          startupTrace.markPhase('historical_session_restore_end', {
            remote,
            sessionId,
            sessionTraceId,
            turnCount: Array.isArray(turns) ? turns.length : 0,
            loadedTurnCount: restoredLoadedTurnCount,
            totalTurnCount: restoredTotalTurnCount,
            isPartial: restoredHistoryPartial,
            contextRestoreState,
            ...sessionViewRestoreTimingTraceFields(restoredTiming),
            durationMs: elapsedMs(restoreStartedAt),
          });
        } catch (error) {
          contextRestoreState = 'pending';
          startupTrace.markPhase('historical_session_restore_failed', {
            remote,
            sessionId,
            sessionTraceId,
            durationMs: elapsedMs(restoreStartedAt),
          });
          log.warn('Backend session restore failed (may be new session)', { sessionId, error });
        }
      }
      
      if (!turns) {
        const turnsLoadStartedAt = nowMs();
        startupTrace.markPhase('historical_session_turns_load_start', {
          remote,
          sessionId,
          sessionTraceId,
        });
        const { sessionAPI } = await import('@/infrastructure/api/service-api/SessionAPI');
        turns = await sessionAPI.loadSessionTurns(
          sessionId,
          workspacePath,
          limit,
          remoteConnectionId,
          remoteSshHost
        );
        startupTrace.markPhase('historical_session_turns_load_end', {
          remote,
          sessionId,
          sessionTraceId,
          turnCount: Array.isArray(turns) ? turns.length : 0,
          durationMs: elapsedMs(turnsLoadStartedAt),
        });
      }
      startupTrace.markPhase('historical_session_turns_loaded', {
        remote,
        sessionId,
        sessionTraceId,
        turnCount: Array.isArray(turns) ? turns.length : 0,
      });
      
      const convertStartedAt = nowMs();
      const dialogTurns = this.convertToDialogTurns(turns);
      const restoredLastUserDialogMode =
        restoredSessionInfo?.lastUserDialogAgentType || this.deriveLastUserDialogMode(dialogTurns);
      startupTrace.markPhase('historical_session_convert_end', {
        remote,
        sessionId,
        sessionTraceId,
        turnCount: dialogTurns.length,
        durationMs: elapsedMs(convertStartedAt),
      });
      
      const stateCommitStartedAt = nowMs();
      this.setState(prev => {
        const session = prev.sessions.get(sessionId);
        if (!session) return prev;
        
        const updatedSession = {
          ...session,
          dialogTurns,
          isHistorical: false,
          historyState: 'ready' as const,
          contextRestoreState,
          isPartial: restoredHistoryPartial,
          loadedTurnCount: restoredLoadedTurnCount ?? dialogTurns.length,
          totalTurnCount: restoredTotalTurnCount ?? dialogTurns.length,
          error: null,
          mode: restoredSessionInfo?.agentType || session.mode,
          lastUserDialogMode: restoredLastUserDialogMode,
          lastSubmittedMode:
            restoredSessionInfo?.lastSubmittedAgentType ?? session.lastSubmittedMode,
        };
        
        const newSessions = new Map(prev.sessions);
        newSessions.set(sessionId, updatedSession);
        
        return {
          ...prev,
          sessions: newSessions,
        };
      });
      startupTrace.markPhase('historical_session_state_commit_end', {
        remote,
        sessionId,
        sessionTraceId,
        turnCount: dialogTurns.length,
        totalTurnCount: restoredTotalTurnCount,
        isPartial: restoredHistoryPartial,
        durationMs: elapsedMs(stateCommitStartedAt),
      });
      markPhaseAfterAnimationFrames(startupTrace, 'historical_session_after_state_commit_frame', {
        remote,
        sessionId,
        sessionTraceId,
        turnCount: dialogTurns.length,
        totalTurnCount: restoredTotalTurnCount,
        isPartial: restoredHistoryPartial,
        durationMs: elapsedMs(traceStartedAt),
      }, {
        frameCount: 2,
      });
      
      // Reset state machine to IDLE after loading history
      // This handles the case where restoreSession triggered events that left the state machine in PROCESSING
      stateMachineManager.reset(sessionId);
      startupTrace.markPhase('historical_session_hydrate_end', {
        remote,
        sessionId,
        sessionTraceId,
        turnCount: dialogTurns.length,
        totalTurnCount: restoredTotalTurnCount,
        isPartial: restoredHistoryPartial,
        durationMs: elapsedMs(traceStartedAt),
      });
      if (restoredHistoryPartial) {
        const deferFullHistoryUntilActive =
          !remote &&
          options?.deferFullHistoryUntilActive === true &&
          this.state.activeSessionId !== sessionId;
        if (!deferFullHistoryUntilActive) {
          this.scheduleCompleteSessionHistoryLoad({
            sessionId,
            workspacePath,
            remoteConnectionId,
            remoteSshHost,
            includeInternal: options?.includeInternal,
            requireActiveSession: options?.deferFullHistoryUntilActive === true,
            initialSessionTraceId: sessionTraceId,
            expectedDialogTurnIds: dialogTurns.map(turn => turn.id),
          });
        } else {
          startupTrace.markPhase('historical_session_full_hydrate_deferred', {
            remote,
            sessionId,
            sessionTraceId,
            reason: 'inactive-after-tail-restore',
            loadedTurnCount: dialogTurns.length,
            totalTurnCount: restoredTotalTurnCount,
          });
        }
      }
    } catch (error) {
      this.setState(prev => {
        const session = prev.sessions.get(sessionId);
        if (!session) return prev;

        const newSessions = new Map(prev.sessions);
        newSessions.set(sessionId, {
          ...session,
          isHistorical: true,
          historyState: 'failed',
        });

        return {
          ...prev,
          sessions: newSessions,
        };
      });
      startupTrace.markPhase('historical_session_hydrate_failed', {
        remote,
        sessionId,
        sessionTraceId,
        durationMs: elapsedMs(traceStartedAt),
      });
      log.error('Failed to load session history', { sessionId, error });
      throw error;
    }
  }

  /**
   * Strip agent-internal XML wrapper tags from persisted user inputs.
   */
  private cleanRemoteUserInput(raw: string): string {
    const s = raw.trim();
    const userQueryMatch = s.match(/<user_query>\s*([\s\S]*?)\s*<\/user_query>/);
    if (userQueryMatch) {
      return userQueryMatch[1].trim();
    }

    return s
      .replace(/<system(?:_|-)reminder>[\s\S]*?<\/system(?:_|-)reminder>/g, '')
      .trim();
  }

  /**
   * Convert DialogTurnData to FlowChat DialogTurn format
   */
  private convertToDialogTurns(turns: any[]): DialogTurn[] {
    return turns.map(turn => {
      const metadata = turn.userMessage.metadata;
      const metaImages = metadata?.images;
      const hasImages = Array.isArray(metaImages) && metaImages.length > 0;
      const images = hasImages
        ? metaImages.map((img: any) => ({
            id: img.id || img.name || `img-${Date.now()}`,
            name: img.name || 'image',
            dataUrl: img.data_url,
            imagePath: img.image_path,
            mimeType: img.mime_type,
          }))
        : undefined;

      const rawDisplay =
        metadata?.localCommandKind === 'usage_report'
          || metadata?.threadGoalKickoff
          || metadata?.threadGoalObjectiveUpdated
          || metadata?.threadGoalContinuation
          ? turn.userMessage.content
          : metadata?.original_text || this.cleanRemoteUserInput(turn.userMessage.content);
      const displayContent = resolveThreadGoalUserMessageDisplay(
        rawDisplay,
        metadata as Record<string, unknown> | undefined
      );
      const normalizedTurnStatus = normalizeRecoveredTurnStatus(turn.status, { error: undefined });

      return {
      id: turn.turnId,
      sessionId: turn.sessionId,
      kind: turn.kind || 'user_dialog',
      agentType: turn.agentType,
      userMessage: {
        id: turn.userMessage.id,
        type: 'user' as const,
        content: displayContent,
        timestamp: turn.userMessage.timestamp,
        hasImages,
        metadata,
        images,
      },
      modelRounds: turn.modelRounds.map((round: any) => {
        const normalizedRoundStatus = normalizeRecoveredRoundStatus(round.status, normalizedTurnStatus);

        return {
          id: round.id,
          turnId: round.turnId,
          index: round.roundIndex ?? 0,
          renderHints: round.renderHints,
          items: [
            ...round.textItems.map((text: any) => ({
              id: text.id,
              type: 'text' as const,
              content: text.content,
              isStreaming: false,
              isMarkdown: text.isMarkdown !== undefined ? text.isMarkdown : true,
              timestamp: text.timestamp,
              status: normalizeRecoveredTextStatus(text.status, normalizedTurnStatus),
              orderIndex: text.orderIndex,
              subagentSessionId: text.subagentSessionId,
            })),
            ...round.toolItems.map((tool: any) => ({
              id: tool.id,
              type: 'tool' as const,
              toolName: tool.toolName,
              interruptionReason:
                tool.interruptionReason === 'app_restart'
                  ? 'app_restart'
                  : isTransientToolStatus(tool.status)
                    ? 'app_restart'
                    : undefined,
              toolCall: tool.toolCall,
              toolResult: tool.toolResult,
              aiIntent: tool.aiIntent,
              requiresConfirmation: tool.requiresConfirmation,
              userConfirmed: tool.userConfirmed,
              acpPermission: tool.acpPermission,
              startTime: tool.startTime,
              endTime: tool.endTime,
              durationMs: tool.durationMs,
              queueWaitMs: tool.queueWaitMs,
              preflightMs: tool.preflightMs,
              confirmationWaitMs: tool.confirmationWaitMs,
              executionMs: tool.executionMs,
              timestamp: tool.startTime,
              status: normalizeRecoveredToolStatus(
                tool.status,
                normalizedTurnStatus,
                tool.toolResult,
                { preservePendingConfirmation: true },
              ),
              orderIndex: tool.orderIndex,
              subagentSessionId: tool.subagentSessionId,
              subagentModelId: tool.subagentModelId,
              subagentModelAlias: tool.subagentModelAlias,
            })),
            ...(round.thinkingItems || []).map((thinking: any) => ({
              id: thinking.id,
              type: 'thinking' as const,
              content: thinking.content,
              isStreaming: false,
              isCollapsed: thinking.isCollapsed ?? true,
              timestamp: thinking.timestamp,
              status: normalizeRecoveredThinkingStatus(thinking.status, normalizedTurnStatus),
              orderIndex: thinking.orderIndex,
              subagentSessionId: thinking.subagentSessionId,
            })),
          ].sort((a: any, b: any) => {
            const aIndex = a.orderIndex !== undefined ? a.orderIndex : a.timestamp || 0;
            const bIndex = b.orderIndex !== undefined ? b.orderIndex : b.timestamp || 0;
            
            return aIndex - bIndex;
          }),
          isStreaming: false,
          isComplete: normalizedRoundStatus !== 'pending' && normalizedRoundStatus !== 'streaming',
          status: normalizedRoundStatus,
          startTime: round.startTime ?? round.timestamp,
          endTime: round.endTime,
          durationMs: round.durationMs,
          providerId: round.providerId,
          modelId: round.modelId,
          modelAlias: round.modelAlias,
          firstChunkMs: round.firstChunkMs,
          firstVisibleOutputMs: round.firstVisibleOutputMs,
          streamDurationMs: round.streamDurationMs,
          attemptCount: round.attemptCount,
          failureCategory: round.failureCategory,
          tokenDetails: round.tokenDetails,
          timestamp: round.timestamp,
        };
      }),
      timestamp: turn.timestamp,
      status: normalizedTurnStatus,
      startTime: turn.startTime,
      endTime: turn.endTime,
      backendTurnIndex: turn.turnIndex,
    };
    });
  }

  public setDialogTurnTodos(sessionId: string, turnId: string, todos: import('../types/flow-chat').TodoItem[]): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found, cannot set turn todos', { sessionId, turnId });
        return prev;
      }

      const turnIndex = session.dialogTurns.findIndex(turn => turn.id === turnId);
      if (turnIndex === -1) {
        log.warn('Dialog turn not found, cannot set turn todos', { sessionId, turnId });
        return prev;
      }

      const updatedTurns = [...session.dialogTurns];
      updatedTurns[turnIndex] = {
        ...updatedTurns[turnIndex],
        todos: [...todos]
      };

      const updatedSession = {
        ...session,
        dialogTurns: updatedTurns,
        lastActiveAt: Date.now()
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  public getDialogTurnTodos(sessionId: string, turnId: string): import('../types/flow-chat').TodoItem[] {
    const session = this.state.sessions.get(sessionId);
    if (!session) return [];

    const turn = session.dialogTurns.find(t => t.id === turnId);
    return turn?.todos || [];
  }
  
  public deleteTodo(sessionId: string, todoId: string): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) {
        log.warn('Session not found, cannot delete todo', { sessionId, todoId });
        return prev;
      }

      const todos = session.todos || [];
      const updatedTodos = todos.filter(t => t.id !== todoId);

      const updatedSession = {
        ...session,
        todos: updatedTodos,
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }

  /**
   * Get all todo items for session (aggregates todos from all DialogTurns)
   * Mainly used by PlannerPanel to display overall progress
   */
  public getTodos(sessionId: string): import('../types/flow-chat').TodoItem[] {
    const session = this.state.sessions.get(sessionId);
    if (!session) return [];
    
    const allTodos: import('../types/flow-chat').TodoItem[] = [];
    session.dialogTurns.forEach(turn => {
      if (turn.todos && turn.todos.length > 0) {
        allTodos.push(...turn.todos);
      }
    });
    
    if (session.todos && session.todos.length > 0) {
      allTodos.push(...session.todos);
    }
    
    return allTodos;
  }

  public setTodos(sessionId: string, todos: import('../types/flow-chat').TodoItem[]): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) {
        return prev;
      }

      const updatedSession = {
        ...session,
        todos: [...todos],
      };

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, updatedSession);

      return {
        ...prev,
        sessions: newSessions
      };
    });
  }
}

export const flowChatStore = FlowChatStore.getInstance();
