/**
 * Flow Chat global state store
 * Prevents state loss when components remount
 */

import {
  FlowChatState,
  Session,
  DialogTurn,
  ModelRound,
  ModelRoundAttempt,
  FlowItem,
  FlowToolItem,
  FlowImageAnalysisItem,
  ImageAnalysisResult,
  AnyFlowItem,
  AcpContextUsage,
  SessionConfig,
  SessionContextRestoreState,
  SessionHistoryState,
  TokenUsage,
} from '../types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import {
  isRemoteTraceContext,
  markPhaseAfterAnimationFrames,
  startupTrace,
} from '@/shared/utils/startupTrace';
import { elapsedMs, nowMs } from '@/shared/utils/timing';
import { isPeerDeviceModeActive } from '@/infrastructure/peer-device/peerModeFlag';
import { i18nService } from '@/infrastructure/i18n/core/I18nService';
import type { DialogTurnData, LocalCommandMetadata, SessionKind } from '@/shared/types/session-history';
import {
  agentAPI,
  type SessionInfo as AgentSessionInfo,
  type SessionViewRestoreTiming,
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
import { useBackgroundSubagentActivityStore } from './backgroundSubagentActivityStore';
import { sessionComposerStore } from './sessionComposerStore';
import { recordHistorySessionDiagnosticEvent } from '../services/historySessionDiagnostics';

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
const HISTORICAL_SESSION_INITIAL_LOCAL_TAIL_TURN_COUNT = 3;
const HISTORICAL_SESSION_FULL_HISTORY_IDLE_TIMEOUT_MS = 1500;
const HISTORICAL_SESSION_PREVIOUS_WINDOW_TURN_COUNT = 12;
const PEER_SESSION_REFRESH_TAIL_TURN_COUNT = 3;

type RemoveSessionOptions = {
  nextActiveSessionId?: string | null;
};
const HISTORICAL_SESSION_FULL_HISTORY_FIRST_PAINT_TIMEOUT_MS = 2500;
const MAX_DEFERRED_FULL_HISTORY_PROJECTIONS = 3;

export interface PeerSessionSnapshotRefreshResult {
  applied: boolean;
  backendState: string;
  latestTurnId?: string;
  latestTurnStatus?: DialogTurn['status'];
}

export function isBackendSessionActivelyProcessing(state: unknown): boolean {
  if (typeof state !== 'string') {
    return false;
  }

  const normalized = state.trim().toLowerCase();
  return normalized === 'processing' ||
    normalized.startsWith('processing ') ||
    normalized.startsWith('processing {') ||
    normalized === 'waitingfortoolresponse' ||
    normalized === 'paused';
}

function normalizeLiveTurnStatus(status: unknown): DialogTurn['status'] {
  const normalized = typeof status === 'string' ? status.trim().toLowerCase() : '';
  switch (normalized) {
    case 'pending':
      return 'pending';
    case 'image_analyzing':
      return 'image_analyzing';
    case 'finishing':
      return 'finishing';
    case 'cancelling':
      return 'cancelling';
    case 'completed':
      return 'completed';
    case 'cancelled':
      return 'cancelled';
    case 'error':
      return 'error';
    case 'inprogress':
    case 'processing':
    default:
      return 'processing';
  }
}

function normalizeLiveRoundStatus(
  status: unknown,
  parentTurnStatus: DialogTurn['status'],
): ModelRound['status'] {
  const normalized = typeof status === 'string' ? status.trim().toLowerCase() : '';
  switch (normalized) {
    case 'pending':
      return 'pending';
    case 'streaming':
    case 'inprogress':
    case 'running':
      return 'streaming';
    case 'pending_confirmation':
      return 'pending_confirmation';
    case 'completed':
      return 'completed';
    case 'cancelled':
      return 'cancelled';
    case 'rejected':
      return 'rejected';
    case 'error':
      return 'error';
    default:
      return parentTurnStatus === 'processing' || parentTurnStatus === 'pending'
        ? 'streaming'
        : normalizeRecoveredRoundStatus(status, parentTurnStatus);
  }
}

function normalizeLiveItemStatus(
  status: unknown,
  fallback: AnyFlowItem['status'],
): AnyFlowItem['status'] {
  const normalized = typeof status === 'string' ? status.trim().toLowerCase() : '';
  switch (normalized) {
    case 'pending':
    case 'queued':
    case 'waiting':
    case 'preparing':
    case 'running':
    case 'streaming':
    case 'receiving':
    case 'completed':
    case 'cancelled':
    case 'rejected':
    case 'error':
    case 'analyzing':
    case 'pending_confirmation':
    case 'confirmed':
      return normalized;
    case 'starting':
      return 'preparing';
    default:
      return fallback;
  }
}

function compareDialogTurnOrder(left: DialogTurn, right: DialogTurn): number {
  if (
    typeof left.backendTurnIndex === 'number' &&
    typeof right.backendTurnIndex === 'number' &&
    left.backendTurnIndex !== right.backendTurnIndex
  ) {
    return left.backendTurnIndex - right.backendTurnIndex;
  }
  return left.startTime - right.startTime;
}

function itemMatchesIdentity(item: AnyFlowItem, itemId: string): boolean {
  if (item.id === itemId) {
    return true;
  }

  if (item.type === 'tool') {
    return (item as FlowToolItem).toolCall?.id === itemId;
  }

  return false;
}

function withAttemptMetadata<T extends AnyFlowItem>(
  item: T,
  attempt: { id: string; index: number },
): T {
  if (item.attemptId === attempt.id && item.attemptIndex === attempt.index) {
    return item;
  }

  return {
    ...item,
    attemptId: attempt.id,
    attemptIndex: attempt.index,
  };
}

function sortAttemptEntries<T extends { index: number }>(attempts: T[]): T[] {
  return [...attempts].sort((left, right) => left.index - right.index);
}

function isFlowItemActiveStatus(status: AnyFlowItem['status']): boolean {
  return [
    'pending',
    'queued',
    'waiting',
    'preparing',
    'running',
    'streaming',
    'receiving',
    'pending_confirmation',
    'confirmed',
    'analyzing',
  ].includes(status);
}

function normalizeSupersededItem(item: AnyFlowItem, endedAt: number): AnyFlowItem {
  if (item.type === 'text') {
    return {
      ...item,
      isStreaming: false,
      status: 'completed',
      runtimeStatus: undefined,
    };
  }

  if (item.type === 'thinking') {
    return {
      ...item,
      isStreaming: false,
      isCollapsed: true,
      status: 'completed',
    };
  }

  if (item.type === 'tool') {
    const toolItem = item as FlowToolItem;
    if (!isFlowItemActiveStatus(toolItem.status)) {
      return item;
    }

    const startTime = toolItem.startTime;
    return {
      ...toolItem,
      status: 'cancelled',
      requiresConfirmation: false,
      acpPermission: undefined,
      isParamsStreaming: false,
      interruptionReason: 'retry_superseded',
      endTime: toolItem.endTime ?? endedAt,
      durationMs: toolItem.durationMs ?? (
        typeof startTime === 'number' ? Math.max(0, endedAt - startTime) : undefined
      ),
      toolResult: toolItem.toolResult ?? {
        result: null,
        success: false,
        error: 'Superseded by a newer retry in the same model round.',
      },
    };
  }

  return item;
}

function deriveAttemptStatus(
  round: ModelRound,
  attempt: ModelRoundAttempt,
  attemptIndex: number,
  attemptCount: number,
): ModelRoundAttempt['status'] {
  const isLatestAttempt = attemptIndex === attemptCount - 1;
  if (!isLatestAttempt) {
    return 'superseded';
  }

  if (round.status === 'completed') {
    return 'completed';
  }
  if (round.status === 'cancelled') {
    return 'cancelled';
  }
  if (round.status === 'error') {
    return 'failed';
  }
  if (attempt.status === 'superseded') {
    return 'superseded';
  }
  return 'streaming';
}

function normalizePersistedToolInterruptionReason(
  interruptionReason: unknown,
  status: unknown,
): FlowToolItem['interruptionReason'] {
  if (interruptionReason === 'retry_superseded') {
    return 'retry_superseded';
  }

  if (interruptionReason === 'app_restart') {
    return 'app_restart';
  }

  return isTransientToolStatus(status) ? 'app_restart' : undefined;
}

function flattenRoundAttemptItems(round: ModelRound): AnyFlowItem[] {
  const attempts = sortAttemptEntries(round.attempts ?? []);
  return attempts.flatMap(attempt => attempt.items);
}

function deriveRoundAttemptsFromItems(items: AnyFlowItem[]): ModelRound['attempts'] | undefined {
  const attempts: Array<ModelRoundAttempt> = [];
  const byKey = new Map<string, number>();
  const leadingUnassigned: AnyFlowItem[] = [];
  let currentAttemptKey: string | null = null;
  let hasAttemptedItems = false;

  const getOrCreateAttempt = (id: string, index: number) => {
    const key = `${id}::${index}`;
    const existingIndex = byKey.get(key);
    if (existingIndex !== undefined) {
      return attempts[existingIndex];
    }

    const attempt: ModelRoundAttempt = { id, index, status: 'streaming', items: [] as AnyFlowItem[] };
    byKey.set(key, attempts.length);
    attempts.push(attempt);
    return attempt;
  };

  for (const item of items) {
    const attemptId = typeof item.attemptId === 'string' && item.attemptId.length > 0
      ? item.attemptId
      : undefined;
    const attemptIndex = typeof item.attemptIndex === 'number' && Number.isFinite(item.attemptIndex)
      ? item.attemptIndex
      : undefined;

    if (attemptId || attemptIndex !== undefined) {
      hasAttemptedItems = true;
      const resolvedIndex = attemptIndex ?? attempts.length + 1;
      const resolvedId = attemptId ?? `attempt:${resolvedIndex}`;
      const attempt = getOrCreateAttempt(resolvedId, resolvedIndex);

      if (leadingUnassigned.length > 0 && attempts.length === 1 && attempt.items.length === 0) {
        attempt.items.push(...leadingUnassigned.map(unassigned => withAttemptMetadata(unassigned, attempt)));
        leadingUnassigned.length = 0;
      }

      attempt.items.push(withAttemptMetadata(item, attempt));
      currentAttemptKey = `${attempt.id}::${attempt.index}`;
      continue;
    }

    if (currentAttemptKey) {
      const attemptIndexInList = byKey.get(currentAttemptKey);
      if (attemptIndexInList !== undefined) {
        const attempt = attempts[attemptIndexInList];
        attempt.items.push(withAttemptMetadata(item, attempt));
        continue;
      }
    }

    leadingUnassigned.push(item);
  }

  if (!hasAttemptedItems) {
    return undefined;
  }

  if (leadingUnassigned.length > 0 && attempts.length > 0) {
    const firstAttempt = attempts[0];
    firstAttempt.items.unshift(...leadingUnassigned.map(item => withAttemptMetadata(item, firstAttempt)));
  }

  return sortAttemptEntries(attempts);
}

function synchronizeRoundAttempts(round: ModelRound): ModelRound {
  const attempts = round.attempts ?? deriveRoundAttemptsFromItems(round.items);
  if (!attempts || attempts.length === 0) {
    return round;
  }

  const endedAt = round.endTime ?? Date.now();
  const sortedAttempts = sortAttemptEntries(attempts).map((attempt, index, allAttempts) => {
    const status = deriveAttemptStatus(round, attempt, index, allAttempts.length);
    return {
      ...attempt,
      status,
      items: status === 'superseded'
        ? attempt.items.map(item => normalizeSupersededItem(item, endedAt))
        : attempt.items.map(item => {
            if (item.type === 'text') {
              return round.isStreaming ? item : { ...item, isStreaming: false };
            }
            if (item.type === 'thinking') {
              return round.isStreaming ? item : { ...item, isStreaming: false };
            }
            return item;
          }),
    };
  });
  const disableExploreGrouping = sortedAttempts.length > 1;

  return {
    ...round,
    attempts: sortedAttempts,
    items: flattenRoundAttemptItems({ ...round, attempts: sortedAttempts }),
    renderHints: disableExploreGrouping
      ? {
          ...(round.renderHints ?? {}),
          disableExploreGrouping: true,
        }
      : round.renderHints,
  };
}

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
    cancelIdle = scheduleHistoricalSessionFullHydrate(() => callback(reason));
  }

  return {
    cancel: () => {
      cancelled = true;
      globalThis.clearTimeout(timeout);
      cancelIdle?.();
    },
    releaseAfterInitialPaint: (options?: FullHistoryHydrationReleaseOptions) => {
      if (options?.immediate === true) {
        start('explicit');
        return;
      }
      if (started || cancelled) {
        return;
      }
      start('initial_paint');
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

/** Transport / gateway failures must fail hydrate instead of falling through to more RPCs. */
function isSessionRestoreTransportError(error: unknown): boolean {
  const anyError = error as { message?: unknown; context?: { originalError?: unknown } };
  const originalError = anyError?.context?.originalError;
  const messageParts = [
    anyError?.message,
    typeof originalError === 'string' ? originalError : (originalError as { message?: unknown })?.message,
  ].filter((part): part is string => typeof part === 'string');
  const normalizedMessage = messageParts.join(' ').toLowerCase();
  return (
    normalizedMessage.includes('504') ||
    normalizedMessage.includes('gateway timeout') ||
    normalizedMessage.includes('peer hostinvoke transport') ||
    normalizedMessage.includes('timed out') ||
    normalizedMessage.includes('timeout')
  );
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
  /** Bumped on peer mode surface reset; stale metadata loads must not write. */
  private surfaceGeneration = 0;
  private fullHistoryHydrationRequests = new Map<string, FullHistoryHydrationRequest>();
  private deferredFullHistoryProjections = new Map<string, DeferredFullHistoryProjection>();
  private fullHistoryProjectionApplyRequests = new Set<string>();
  private unsupportedRestoreCommands = new Set<string>();
  private pendingRemoveSessionOptions = new Map<string, RemoveSessionOptions>();
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

    const hydrationRequest: FullHistoryHydrationRequest = {
      sessionId: request.sessionId,
      remote,
      requireActiveSession,
      sessionTraceId: request.initialSessionTraceId,
      promise,
      cancel: () => {
        cancelScheduled?.();
        resolveRequest?.();
      },
    };

    if (releaseAfterInitialPaint) {
      hydrationRequest.releaseAfterInitialPaint = (options?: FullHistoryHydrationReleaseOptions) => {
        releaseAfterInitialPaint?.(options);
      };
    }

    this.fullHistoryHydrationRequests.set(requestKey, hydrationRequest);
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
      if (!request.releaseAfterInitialPaint) {
        continue;
      }
      request.releaseAfterInitialPaint(options);
      released = true;
    }
    return released;
  }

  private shouldDeferFullHistoryProjection(sessionId: string, remote: boolean, _requireActiveSession: boolean): boolean {
    if (remote) {
      return true;
    }

    if (this.fullHistoryProjectionApplyRequests.has(sessionId)) {
      return false;
    }

    return this.state.activeSessionId === sessionId;
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

    if (projection.remote) {
      startupTrace.markPhase('historical_session_full_hydrate_remote_projection_blocked', {
        remote: true,
        sessionId,
        reason,
        turnCount: projection.dialogTurns.length,
      });
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

  public revealPreviousSessionHistoryWindow(
    sessionId: string,
    reason: string,
    turnLimit: number = HISTORICAL_SESSION_PREVIOUS_WINDOW_TURN_COUNT
  ): boolean {
    const projection = this.deferredFullHistoryProjections.get(sessionId);
    if (!projection) {
      return false;
    }

    const boundedTurnLimit = Math.max(1, Math.floor(turnLimit));
    let revealed = false;
    let revealedTurnCount = 0;
    let loadedTurnCount = 0;
    let totalTurnCount = projection.dialogTurns.length;
    let remainingBefore = 0;
    let nextExpectedDialogTurnIds: string[] = [];

    this.setState(prev => {
      if (projection.requireActiveSession && !projection.remote && prev.activeSessionId !== sessionId) {
        return prev;
      }

      const session = prev.sessions.get(sessionId);
      if (!session || session.historyState !== 'ready' || session.dialogTurns.length === 0) {
        return prev;
      }

      const firstLoadedTurnId = session.dialogTurns[0]?.id;
      if (!firstLoadedTurnId) {
        return prev;
      }

      const firstLoadedIndex = projection.dialogTurns.findIndex(turn => turn.id === firstLoadedTurnId);
      if (firstLoadedIndex <= 0) {
        return prev;
      }

      const startIndex = Math.max(0, firstLoadedIndex - boundedTurnLimit);
      const currentTurnIds = new Set(session.dialogTurns.map(turn => turn.id));
      const previousWindow = projection.dialogTurns
        .slice(startIndex, firstLoadedIndex)
        .filter(turn => !currentTurnIds.has(turn.id));
      if (previousWindow.length === 0) {
        return prev;
      }

      const currentDialogTurnsById = new Map(session.dialogTurns.map(turn => [turn.id, turn]));
      const projectionDialogTurnIds = new Set(projection.dialogTurns.map(turn => turn.id));
      const mergedDialogTurns = [
        ...previousWindow.map(turn => currentDialogTurnsById.get(turn.id) ?? turn),
        ...session.dialogTurns,
      ];
      nextExpectedDialogTurnIds = [];
      for (const turn of mergedDialogTurns) {
        if (!projectionDialogTurnIds.has(turn.id)) {
          break;
        }
        nextExpectedDialogTurnIds.push(turn.id);
      }
      revealed = true;
      revealedTurnCount = previousWindow.length;
      loadedTurnCount = mergedDialogTurns.length;
      totalTurnCount = Math.max(
        session.totalTurnCount ?? 0,
        projection.dialogTurns.length,
        mergedDialogTurns.length,
      );
      remainingBefore = startIndex;

      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, {
        ...session,
        dialogTurns: mergedDialogTurns,
        isPartial: remainingBefore > 0,
        loadedTurnCount,
        totalTurnCount,
        contextRestoreState:
          session.contextRestoreState === 'ready' ? 'ready' : projection.contextRestoreState,
        mode: projection.restoredSessionInfo?.agentType || session.mode,
        lastUserDialogMode: projection.restoredLastUserDialogMode,
        lastSubmittedMode:
          projection.restoredSessionInfo?.lastSubmittedAgentType ?? session.lastSubmittedMode,
      });

      return {
        ...prev,
        sessions: newSessions,
      };
    });

    if (!revealed) {
      return false;
    }

    if (remainingBefore === 0) {
      this.deferredFullHistoryProjections.delete(sessionId);
      this.fullHistoryProjectionApplyRequests.delete(sessionId);
    } else if (nextExpectedDialogTurnIds.length > 0) {
      this.deferredFullHistoryProjections.set(sessionId, {
        ...projection,
        expectedDialogTurnIds: nextExpectedDialogTurnIds,
      });
    }

    startupTrace.markPhase('historical_session_deferred_window_revealed', {
      remote: projection.remote,
      sessionId,
      reason,
      turnLimit: boundedTurnLimit,
      revealedTurnCount,
      loadedTurnCount,
      totalTurnCount,
      remainingBefore,
    });
    return true;
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

    const { agentAPI } = await import('@/infrastructure/api/service-api/AgentAPI');
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
    const activeTurnId = isBackendSessionActivelyProcessing(restored.session.state)
      ? restored.turns[restored.turns.length - 1]?.turnId
      : undefined;
    const dialogTurns = this.convertToDialogTurns(restored.turns, { activeTurnId });
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
      reviewTargetEvidence?: Session['reviewTargetEvidence'];
      reviewTargetFilePaths?: Session['reviewTargetFilePaths'];
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
        reviewTargetEvidence: meta?.reviewTargetEvidence,
        reviewTargetFilePaths: meta?.reviewTargetFilePaths,
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
      if (session.config.modelName?.trim() === normalizedModelName) {
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

  public async deleteSession(sessionId: string, options?: RemoveSessionOptions): Promise<void> {
    const sessionIdsToDelete = this.getCascadeSessionIds(sessionId);
    if (sessionIdsToDelete.length === 0) {
      return;
    }
    if (options) {
      this.pendingRemoveSessionOptions.set(sessionId, options);
    }

    const { stateMachineManager } = await import('../state-machine');
    sessionIdsToDelete.forEach(id => {
      stateMachineManager.delete(id);
    });

    try {
      const { agentAPI } = await import('@/infrastructure/api/service-api/AgentAPI');
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

    const removedSessionIds = this.removeSession(sessionId, options);
    sessionComposerStore.getState().removeDrafts(removedSessionIds);
    this.pendingRemoveSessionOptions.delete(sessionId);
  }

  public removeSession(sessionId: string, options?: RemoveSessionOptions): string[] {
    const removedSessionIds = this.getCascadeSessionIds(sessionId);
    if (removedSessionIds.length === 0) {
      this.pendingRemoveSessionOptions.delete(sessionId);
      return [];
    }
    const resolvedOptions = options ?? this.pendingRemoveSessionOptions.get(sessionId);
    this.pendingRemoveSessionOptions.delete(sessionId);
    this.clearRemovedSessionHistoryState(removedSessionIds, 'session-removed');
    useBackgroundSubagentActivityStore.getState().removeSessions(removedSessionIds);

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
        if (resolvedOptions && 'nextActiveSessionId' in resolvedOptions) {
          newActiveSessionId = resolvedOptions.nextActiveSessionId ?? null;
        } else {
          const remainingSessions = Array.from(newSessions.keys());
          newActiveSessionId = remainingSessions.length > 0 ? remainingSessions[0] : null;
        }
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

    const { agentAPI } = await import('@/infrastructure/api/service-api/AgentAPI');
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

  /**
   * Drop all in-memory sessions and metadata request caches before switching
   * Peer Device Mode data plane. Prevents local sessionIds from blocking peer
   * metadata import (existingSession skip).
   */
  public clearAllSessionsForPeerSwitch(): string[] {
    this.surfaceGeneration += 1;
    const removedSessionIds = Array.from(this.state.sessions.keys());
    this.metadataListRequests.clear();
    this.metadataPageRequests.clear();
    if (removedSessionIds.length === 0) {
      this.setState(prev => ({
        ...prev,
        sessions: new Map(),
        activeSessionId: null,
      }));
      return [];
    }
    return this.removeSessionsByIds(removedSessionIds);
  }

  public getSurfaceGeneration(): number {
    return this.surfaceGeneration;
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
      modelRounds: [...turn.modelRounds, synchronizeRoundAttempts(modelRound)],
      status: 'processing'
    }));
  }

  public updateModelRound(sessionId: string, dialogTurnId: string, modelRoundId: string, updater: (round: ModelRound) => ModelRound): void {
    this.updateDialogTurn(sessionId, dialogTurnId, turn => ({
      ...turn,
      modelRounds: turn.modelRounds.map(round => 
        round.id === modelRoundId ? synchronizeRoundAttempts(updater(round)) : round
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
      const updatedModelRounds = turn.modelRounds.map(round => {
        const activeAttempts = round.attempts ?? deriveRoundAttemptsFromItems(round.items);

        if (activeAttempts && activeAttempts.length > 0) {
          let roundChanged = false;
          const nextAttempts = activeAttempts.map(attempt => {
            let attemptChanged = false;
            const nextItems = attempt.items.map(item => {
              const update = updates.find(u => itemMatchesIdentity(item, u.itemId));
              if (!update) {
                return item;
              }

              attemptChanged = true;
              roundChanged = true;
              return { ...item, ...update.changes } as AnyFlowItem;
            });

            return attemptChanged ? { ...attempt, items: nextItems } : attempt;
          });

          return roundChanged
            ? synchronizeRoundAttempts({
                ...round,
                attempts: nextAttempts,
              })
            : round;
        }

        return {
          ...round,
          items: round.items.map(item => {
            const update = updates.find(u => itemMatchesIdentity(item, u.itemId));
            return update ? ({ ...item, ...update.changes } as AnyFlowItem) : item;
          })
        };
      });
      
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
      const activeAttempts = targetModelRound.attempts ?? deriveRoundAttemptsFromItems(targetModelRound.items);
      const incomingAttemptId = typeof item.attemptId === 'string' && item.attemptId.length > 0
        ? item.attemptId
        : undefined;
      const incomingAttemptIndex = typeof item.attemptIndex === 'number' && Number.isFinite(item.attemptIndex)
        ? item.attemptIndex
        : undefined;

      if (!activeAttempts || activeAttempts.length === 0) {
        if (!incomingAttemptId && incomingAttemptIndex === undefined) {
          updatedModelRounds[targetModelRoundIndex] = {
            ...targetModelRound,
            items: [...targetModelRound.items, item]
          };
        } else {
          const initialAttempt = {
            id: incomingAttemptId ?? `attempt:${incomingAttemptIndex ?? 1}`,
            index: incomingAttemptIndex ?? 1,
          };
          const attemptItems = [
            ...targetModelRound.items.map(existing => withAttemptMetadata(existing, initialAttempt)),
            withAttemptMetadata(item, initialAttempt),
          ];
          updatedModelRounds[targetModelRoundIndex] = synchronizeRoundAttempts({
            ...targetModelRound,
            attempts: [{
              ...initialAttempt,
              status: 'streaming',
              items: attemptItems,
            }],
          });
        }
      } else {
        const latestAttempt = sortAttemptEntries(activeAttempts)[activeAttempts.length - 1];
        const targetAttempt = {
          id: incomingAttemptId ?? latestAttempt.id,
          index: incomingAttemptIndex ?? latestAttempt.index,
        };
        const normalizedItem = withAttemptMetadata(item, targetAttempt);
        const targetAttemptKey = `${targetAttempt.id}::${targetAttempt.index}`;
        let attemptFound = false;
        const nextAttempts = activeAttempts.map(attempt => {
          const attemptKey = `${attempt.id}::${attempt.index}`;
          if (attemptKey !== targetAttemptKey) {
            return attempt;
          }

          attemptFound = true;
          return {
            ...attempt,
            items: [...attempt.items, normalizedItem],
          };
        });

        updatedModelRounds[targetModelRoundIndex] = synchronizeRoundAttempts({
          ...targetModelRound,
          attempts: attemptFound
            ? nextAttempts
            : [...nextAttempts, { ...targetAttempt, status: 'streaming', items: [normalizedItem] }],
        });
      }

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

        const activeAttempts = modelRound.attempts ?? deriveRoundAttemptsFromItems(modelRound.items);
        if (activeAttempts && activeAttempts.length > 0) {
          let foundInAttempts = false;
          const nextAttempts = activeAttempts.map(attempt => {
            let attemptChanged = false;
            const nextItems = attempt.items.map(item => {
              if (!itemMatchesIdentity(item, itemId)) {
                return item;
              }

              foundInAttempts = true;
              attemptChanged = true;
              return { ...item, ...updates } as AnyFlowItem;
            });

            return attemptChanged ? { ...attempt, items: nextItems } : attempt;
          });

          if (foundInAttempts) {
            updated = true;
            return synchronizeRoundAttempts({
              ...modelRound,
              attempts: nextAttempts,
            });
          }
        }

        const updatedItems = modelRound.items.map((item: any) => {
          if (!itemMatchesIdentity(item, itemId)) {
            return item;
          }

          return { ...item, ...updates };
        });

        if (updatedItems.some((item: any) => itemMatchesIdentity(item, itemId))) {
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
    tokenUsage: { inputTokens: number; outputTokens?: number; totalTokens: number },
    dialogTurnId?: string
  ): void {
    this.setState(prev => {
      const session = prev.sessions.get(sessionId);
      if (!session) return prev;

      const nextTokenUsage = {
        inputTokens: tokenUsage.inputTokens,
        outputTokens: tokenUsage.outputTokens,
        totalTokens: tokenUsage.totalTokens,
        timestamp: Date.now()
      };
      let dialogTurns = session.dialogTurns;
      if (dialogTurnId) {
        const turnIndex = session.dialogTurns.findIndex(turn => turn.id === dialogTurnId);
        if (turnIndex !== -1) {
          const previousTurnUsage = session.dialogTurns[turnIndex].tokenUsage;
          const accumulatedOutputTokens = previousTurnUsage
            ? (
                typeof previousTurnUsage.outputTokens === 'number' &&
                typeof nextTokenUsage.outputTokens === 'number'
                  ? previousTurnUsage.outputTokens + nextTokenUsage.outputTokens
                  : undefined
              )
            : nextTokenUsage.outputTokens;
          const accumulatedTurnUsage: TokenUsage = {
            inputTokens: (previousTurnUsage?.inputTokens ?? 0) + nextTokenUsage.inputTokens,
            outputTokens: accumulatedOutputTokens,
            totalTokens: (previousTurnUsage?.totalTokens ?? 0) + nextTokenUsage.totalTokens,
            timestamp: nextTokenUsage.timestamp,
          };
          dialogTurns = [...session.dialogTurns];
          dialogTurns[turnIndex] = {
            ...dialogTurns[turnIndex],
            tokenUsage: accumulatedTurnUsage,
          };
        }
      }

      const updatedSession = {
        ...session,
        currentTokenUsage: nextTokenUsage,
        dialogTurns
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
              attemptId: item.attemptId,
              attemptIndex: item.attemptIndex,
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
              attemptId: item.attemptId,
              attemptIndex: item.attemptIndex,
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
              attemptId: item.attemptId,
              attemptIndex: item.attemptIndex,
            }));
          
          return {
            id: round.id,
            turnId,
            roundIndex,
            roundGroupId: round.roundGroupId,
            timestamp: round.startTime,
            renderHints: round.renderHints,
            textItems,
            toolItems,
            thinkingItems,
            startTime: round.startTime,
            endTime: round.endTime || Date.now(),
            durationMs: round.durationMs,
            providerId: round.providerId,
            modelConfigId: round.modelConfigId,
            effectiveModelName: round.effectiveModelName,
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
        tokenUsage: dialogTurn.tokenUsage,
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
      const configData = await configManager.getConfigs([
        'ai.models',
        'ai.default_models',
      ]);

      if (Array.isArray(configData['ai.models'])) {
        models = configData['ai.models'];
      }
      if (
        configData['ai.default_models'] &&
        typeof configData['ai.default_models'] === 'object'
      ) {
        defaultModels = configData['ai.default_models'] as Record<string, string>;
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
    modelConfigPromise?: Promise<{
      models: any[];
      defaultModels: Record<string, string>;
    }>,
  ): Promise<void> {
    const surfaceGeneration = this.surfaceGeneration;
    const [
      { stateMachineManager },
      { models, defaultModels },
    ] = await Promise.all([
      import('../state-machine'),
      modelConfigPromise ?? this.loadSessionMetadataModelConfig(),
    ]);
    if (surfaceGeneration !== this.surfaceGeneration) {
      return;
    }

    const processSession = async (metadata: any) => {
      try {
        if (surfaceGeneration !== this.surfaceGeneration) {
          return;
        }
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
          if (surfaceGeneration !== this.surfaceGeneration) {
            return prev;
          }
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
            persistedStatus: metadata.status,
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
            reviewTargetEvidence: metadata.reviewTargetEvidence,
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
      let modelConfigPromise: Promise<{
        models: any[];
        defaultModels: Record<string, string>;
      }> | undefined;
      const pageRequestStartedAt = nowMs();
      try {
        startupTrace.markPhase('session_metadata_page_request_start', {
          remote,
          source: traceSource,
          metadataListTraceId,
          command: 'list_persisted_sessions_page',
        });
        const pagePromise = sessionAPI.listSessionsPage({
          workspacePath,
          limit,
          cursor,
          remoteConnectionId,
          remoteSshHost,
        });
        modelConfigPromise = this.loadSessionMetadataModelConfig();
        page = await pagePromise;
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
        modelConfigPromise,
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
        const configData = await configManager.getConfigs([
          'ai.models',
          'ai.default_models',
        ]);

        if (Array.isArray(configData['ai.models'])) {
          models = configData['ai.models'];
        }
        if (
          configData['ai.default_models'] &&
          typeof configData['ai.default_models'] === 'object'
        ) {
          defaultModels = configData['ai.default_models'] as Record<string, string>;
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
              persistedStatus: metadata.status,
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
              reviewTargetEvidence: metadata.reviewTargetEvidence,
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
   * Reconcile the active Peer Device session with a small authoritative
   * snapshot from the host.
   *
   * Peer agentic events remain the primary low-latency path. This snapshot is
   * the recovery path for a controller that attached after lifecycle events,
   * or for a DeviceEvent gap (the relay stream has no ACK/replay contract).
   */
  public async refreshPeerSessionSnapshot(
    sessionId: string,
    workspacePath: string,
    options?: {
      replaceRunningSnapshot?: boolean;
      requireActiveSession?: boolean;
      shouldApply?: () => boolean;
    },
  ): Promise<PeerSessionSnapshotRefreshResult> {
    const initialSession = this.state.sessions.get(sessionId);
    if (!initialSession) {
      return {
        applied: false,
        backendState: 'Unknown',
      };
    }

    const restored = await agentAPI.restoreSessionView(
      sessionId,
      workspacePath,
      initialSession.remoteConnectionId,
      initialSession.remoteSshHost,
      `peer-refresh-${sessionId.slice(0, 8)}`,
      undefined,
      PEER_SESSION_REFRESH_TAIL_TURN_COUNT,
    );
    const backendActive = isBackendSessionActivelyProcessing(restored.session.state);
    const activeTurnId = backendActive
      ? restored.turns[restored.turns.length - 1]?.turnId
      : undefined;
    const snapshotTurns = this.convertToDialogTurns(restored.turns, { activeTurnId });
    const replaceExistingTurns =
      !backendActive || options?.replaceRunningSnapshot === true;
    let applied = false;

    this.setState(prev => {
      if (
        options?.shouldApply?.() === false ||
        (
          options?.requireActiveSession !== false &&
          prev.activeSessionId !== sessionId
        )
      ) {
        return prev;
      }

      const session = prev.sessions.get(sessionId);
      // A live event or another refresh won the race while HostInvoke was in
      // flight. Do not overwrite that newer local projection.
      if (!session || session !== initialSession) {
        return prev;
      }

      const mergedTurns = [...session.dialogTurns];
      let turnsChanged = false;
      for (const snapshotTurn of snapshotTurns) {
        const existingIndex = mergedTurns.findIndex(turn => turn.id === snapshotTurn.id);
        if (existingIndex === -1) {
          mergedTurns.push(snapshotTurn);
          turnsChanged = true;
        } else if (replaceExistingTurns) {
          mergedTurns[existingIndex] = snapshotTurn;
          turnsChanged = true;
        }
      }

      if (!turnsChanged) {
        return prev;
      }

      mergedTurns.sort(compareDialogTurnOrder);
      const newSessions = new Map(prev.sessions);
      newSessions.set(sessionId, {
        ...session,
        dialogTurns: mergedTurns,
        isHistorical: false,
        historyState: 'ready',
        contextRestoreState:
          session.contextRestoreState === 'ready'
            ? 'ready'
            : restored.contextRestoreState,
        isPartial: session.isPartial === true,
        loadedTurnCount: Math.max(session.loadedTurnCount ?? 0, mergedTurns.length),
        totalTurnCount: Math.max(
          session.totalTurnCount ?? 0,
          restored.totalTurnCount ?? restored.session.turnCount,
          mergedTurns.length,
        ),
        config: {
          ...session.config,
          ...(restored.session.modelName
            ? { modelName: restored.session.modelName }
            : {}),
        },
        mode: restored.session.agentType || session.mode,
        lastUserDialogMode:
          restored.session.lastUserDialogAgentType || session.lastUserDialogMode,
        lastSubmittedMode:
          restored.session.lastSubmittedAgentType ?? session.lastSubmittedMode,
      });
      applied = true;

      return {
        ...prev,
        sessions: newSessions,
      };
    });

    const latestTurn = snapshotTurns[snapshotTurns.length - 1];
    return {
      applied,
      backendState: restored.session.state,
      latestTurnId: latestTurn?.id,
      latestTurnStatus: latestTurn?.status,
    };
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
    const initialSession = this.state.sessions.get(sessionId);
    const suppressInitialHydratingState =
      !remote &&
      options?.deferFullHistoryUntilActive === true &&
      this.state.activeSessionId === sessionId &&
      initialSession?.isHistorical === true &&
      initialSession.historyState === 'metadata-only';
    let hydratingStateNotified = false;
    const notifyHydratingState = (): void => {
      if (suppressInitialHydratingState) {
        return;
      }
      if (hydratingStateNotified) {
        return;
      }
      hydratingStateNotified = true;
      this.setSessionHistoryState(sessionId, 'hydrating');
    };

    try {
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

      // Finish or resume relay history import before Core restores its model
      // context. Ordinary local sessions return after one metadata read, while
      // an incomplete relay import fails closed instead of publishing a
      // truncated UI/Core history pair.
      //
      // Peer Device Mode: cloud turn fetch is paused on the controller; session
      // history must come from the peer host via restore_session_view.
      if (!remote && workspacePath && !isPeerDeviceModeActive()) {
        const relayImportStartedAt = nowMs();
        startupTrace.markPhase('historical_session_relay_import_start', {
          remote,
          sessionId,
          sessionTraceId,
        });
        try {
          const { remoteConnectAPI } = await import(
            '@/infrastructure/api/service-api/RemoteConnectAPI'
          );
          const fetched = await remoteConnectAPI.accountFetchSessionTurns(
            sessionId,
            workspacePath
          );
          startupTrace.markPhase('historical_session_relay_import_end', {
            remote,
            sessionId,
            sessionTraceId,
            fetched,
            durationMs: elapsedMs(relayImportStartedAt),
          });
        } catch (fetchErr) {
          startupTrace.markPhase('historical_session_relay_import_failed', {
            remote,
            sessionId,
            sessionTraceId,
            durationMs: elapsedMs(relayImportStartedAt),
          });
          log.warn('Relay session history is incomplete; retry opening the session', {
            sessionId,
            error: fetchErr,
          });
          throw fetchErr;
        }
      }

      const stateMachineManagerPromise = import('../state-machine');
      if (!isAcpSession) {
        const restoreStartedAt = nowMs();
        startupTrace.markPhase('historical_session_restore_start', {
          remote,
          sessionId,
          sessionTraceId,
        });
        try {
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
                const restoredPromise = agentAPI.restoreSessionWithTurns(
                  sessionId,
                  workspacePath,
                  remoteConnectionId,
                  remoteSshHost,
                  sessionTraceId,
                  options?.includeInternal,
                );
                notifyHydratingState();
                const restored = await restoredPromise;
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

            const restoredSessionPromise = agentAPI.restoreSession(
              sessionId,
              workspacePath,
              remoteConnectionId,
              remoteSshHost,
              sessionTraceId,
              options?.includeInternal,
            );
            notifyHydratingState();
            restoredSessionInfo = await restoredSessionPromise;
            contextRestoreState = 'ready';
          };

          if (
            typeof agentAPI.restoreSessionView === 'function' &&
            !this.unsupportedRestoreCommands.has(restoreSessionViewSupportKey)
          ) {
            try {
              const restoredPromise = agentAPI.restoreSessionView(
                sessionId,
                workspacePath,
                remoteConnectionId,
                remoteSshHost,
                sessionTraceId,
                options?.includeInternal,
                historicalSessionInitialTailTurnCount(remote),
              );
              notifyHydratingState();
              const restored = await restoredPromise;
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
          if (isSessionRestoreTransportError(error)) {
            throw error;
          }
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
        notifyHydratingState();
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
      const { stateMachineManager, SessionExecutionEvent } = await stateMachineManagerPromise;
      stateMachineManager.getOrCreate(sessionId);
      startupTrace.markPhase('historical_session_turns_loaded', {
        remote,
        sessionId,
        sessionTraceId,
        turnCount: Array.isArray(turns) ? turns.length : 0,
      });

      const skipStaleLocalHydrateCommit =
        !remote &&
        options?.deferFullHistoryUntilActive === true &&
        this.state.activeSessionId !== sessionId;
      if (skipStaleLocalHydrateCommit) {
        this.setState(prev => {
          const session = prev.sessions.get(sessionId);
          if (!session || prev.activeSessionId === sessionId) {
            return prev;
          }

          const newSessions = new Map(prev.sessions);
          newSessions.set(sessionId, {
            ...session,
            historyState: session.isHistorical ? 'metadata-only' : session.historyState,
          });

          return {
            ...prev,
            sessions: newSessions,
          };
        });
        stateMachineManager.reset(sessionId);
        startupTrace.markPhase('historical_session_hydrate_stale_commit_skipped', {
          remote,
          sessionId,
          sessionTraceId,
          loadedTurnCount: restoredLoadedTurnCount,
          totalTurnCount: restoredTotalTurnCount,
          isPartial: restoredHistoryPartial,
          durationMs: elapsedMs(traceStartedAt),
        });
        recordHistorySessionDiagnosticEvent(sessionId, 'store_stale_commit_skipped', {
          remote,
          loadedTurnCount: restoredLoadedTurnCount,
          totalTurnCount: restoredTotalTurnCount,
          isPartial: restoredHistoryPartial,
        });
        startupTrace.markPhase('historical_session_hydrate_end', {
          remote,
          sessionId,
          sessionTraceId,
          skipped: true,
          loadedTurnCount: restoredLoadedTurnCount,
          totalTurnCount: restoredTotalTurnCount,
          isPartial: restoredHistoryPartial,
          durationMs: elapsedMs(traceStartedAt),
        });
        return;
      }
      
      const convertStartedAt = nowMs();
      const activeTurnId = isBackendSessionActivelyProcessing(restoredSessionInfo?.state)
        ? turns[turns.length - 1]?.turnId
        : undefined;
      const dialogTurns = this.convertToDialogTurns(turns, { activeTurnId });
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
          config: {
            ...session.config,
            ...(restoredSessionInfo?.modelName
              ? { modelName: restoredSessionInfo.modelName }
              : {}),
          },
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
      recordHistorySessionDiagnosticEvent(sessionId, 'store_state_commit_finished', {
        remote,
        dialogTurnCount: dialogTurns.length,
        totalTurnCount: restoredTotalTurnCount,
        isPartial: restoredHistoryPartial,
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
      
      // Historical views normally settle to IDLE. When the same process still
      // owns a live turn (notably a Peer Host), keep the controller state
      // machine aligned so subsequent streamed chunks are accepted even though
      // their DialogTurnStarted event happened before the controller attached.
      stateMachineManager.reset(sessionId);
      if (activeTurnId) {
        await stateMachineManager.transition(sessionId, SessionExecutionEvent.START, {
          taskId: sessionId,
          dialogTurnId: activeTurnId,
        });
      }
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
      recordHistorySessionDiagnosticEvent(sessionId, 'store_hydrate_failed', {
        remote,
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
  private convertToDialogTurns(
    turns: any[],
    options?: { activeTurnId?: string },
  ): DialogTurn[] {
    return turns.map(turn => {
      const isLiveTurn = options?.activeTurnId === turn.turnId;
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
      const normalizedTurnStatus = isLiveTurn
        ? normalizeLiveTurnStatus(turn.status)
        : normalizeRecoveredTurnStatus(turn.status, { error: undefined });
      const rawTokenUsage = turn.tokenUsage ?? turn.token_usage;

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
        const normalizedRoundStatus = isLiveTurn
          ? normalizeLiveRoundStatus(round.status, normalizedTurnStatus)
          : normalizeRecoveredRoundStatus(round.status, normalizedTurnStatus);
        const flatItems = [
          ...round.textItems.map((text: any) => ({
            id: text.id,
            type: 'text' as const,
            content: text.content,
            isStreaming: isLiveTurn ? text.isStreaming === true : false,
            isMarkdown: text.isMarkdown !== undefined ? text.isMarkdown : true,
            timestamp: text.timestamp,
            status: isLiveTurn
              ? normalizeLiveItemStatus(
                  text.status,
                  text.isStreaming === true ? 'streaming' : 'completed',
                )
              : normalizeRecoveredTextStatus(text.status, normalizedTurnStatus),
            orderIndex: text.orderIndex,
            subagentSessionId: text.subagentSessionId,
            attemptId: text.attemptId,
            attemptIndex: text.attemptIndex,
          })),
          ...round.toolItems.map((tool: any) => ({
              id: tool.id,
              type: 'tool' as const,
              toolName: tool.toolName,
              toolCall: tool.toolCall,
              interruptionReason: normalizePersistedToolInterruptionReason(
                tool.interruptionReason,
                tool.status,
              ),
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
              status: isLiveTurn
                ? normalizeLiveItemStatus(
                    tool.status,
                    tool.toolResult ? (tool.toolResult.success ? 'completed' : 'error') : 'running',
                  )
                : normalizeRecoveredToolStatus(
                    tool.status,
                    normalizedTurnStatus,
                    tool.toolResult,
                  ),
              orderIndex: tool.orderIndex,
              subagentSessionId: tool.subagentSessionId,
              subagentDialogTurnId: tool.subagentDialogTurnId,
              subagentModelId: tool.subagentModelId,
              subagentModelDisplayName: tool.subagentModelDisplayName,
              attemptId: tool.attemptId,
              attemptIndex: tool.attemptIndex,
            })),
          ...(round.thinkingItems || []).map((thinking: any) => ({
            id: thinking.id,
            type: 'thinking' as const,
            content: thinking.content,
            isStreaming: isLiveTurn ? thinking.isStreaming === true : false,
            isCollapsed: isLiveTurn
              ? (thinking.isCollapsed ?? thinking.isStreaming !== true)
              : (thinking.isCollapsed ?? true),
            timestamp: thinking.timestamp,
            status: isLiveTurn
              ? normalizeLiveItemStatus(
                  thinking.status,
                  thinking.isStreaming === true ? 'streaming' : 'completed',
                )
              : normalizeRecoveredThinkingStatus(thinking.status, normalizedTurnStatus),
            orderIndex: thinking.orderIndex,
            subagentSessionId: thinking.subagentSessionId,
            attemptId: thinking.attemptId,
            attemptIndex: thinking.attemptIndex,
          })),
        ].sort((a: any, b: any) => {
          const aIndex = a.orderIndex !== undefined ? a.orderIndex : a.timestamp || 0;
          const bIndex = b.orderIndex !== undefined ? b.orderIndex : b.timestamp || 0;
          
          return aIndex - bIndex;
        });

        const hydratedRound = synchronizeRoundAttempts({
          id: round.id,
          index: round.roundIndex ?? 0,
          roundGroupId: round.roundGroupId,
          renderHints: round.renderHints,
          items: flatItems,
          isStreaming:
            isLiveTurn &&
            (normalizedRoundStatus === 'pending' ||
              normalizedRoundStatus === 'streaming' ||
              normalizedRoundStatus === 'pending_confirmation'),
          isComplete:
            normalizedRoundStatus !== 'pending' &&
            normalizedRoundStatus !== 'streaming' &&
            normalizedRoundStatus !== 'pending_confirmation',
          status: normalizedRoundStatus,
          startTime: round.startTime ?? round.timestamp,
          endTime: round.endTime,
          durationMs: round.durationMs,
          providerId: round.providerId,
          modelConfigId: round.modelConfigId,
          effectiveModelName: round.effectiveModelName,
          firstChunkMs: round.firstChunkMs,
          firstVisibleOutputMs: round.firstVisibleOutputMs,
          streamDurationMs: round.streamDurationMs,
          attemptCount: round.attemptCount,
          failureCategory: round.failureCategory,
          tokenDetails: round.tokenDetails,
        });

        return hydratedRound;
      }),
      timestamp: turn.timestamp,
      status: normalizedTurnStatus,
      finishReason:
        typeof turn.finishReason === 'string'
          ? turn.finishReason
          : typeof turn.finish_reason === 'string'
            ? turn.finish_reason
            : undefined,
      hasFinalResponse:
        typeof turn.hasFinalResponse === 'boolean'
          ? turn.hasFinalResponse
          : typeof turn.has_final_response === 'boolean'
            ? turn.has_final_response
            : undefined,
      startTime: turn.startTime,
      endTime: turn.endTime,
      tokenUsage: rawTokenUsage
        ? {
            inputTokens: rawTokenUsage.inputTokens ?? rawTokenUsage.input_tokens,
            outputTokens: rawTokenUsage.outputTokens ?? rawTokenUsage.output_tokens,
            totalTokens: rawTokenUsage.totalTokens ?? rawTokenUsage.total_tokens,
            timestamp: rawTokenUsage.timestamp,
          }
        : undefined,
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
