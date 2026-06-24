import { createLogger } from '@/shared/utils/logger';
import { elapsedMs, nowMs, roundDurationMs } from '@/shared/utils/timing';

const log = createLogger('HistorySessionDiagnostics');

const RECENT_EVENT_LIMIT = 30;
const WARN_EVENT_LIMIT = 15;
const MAX_SESSION_DIAGNOSTICS = 50;

export type HistorySessionDiagnosticValue =
  | string
  | number
  | boolean
  | null
  | undefined
  | HistorySessionDiagnosticValue[]
  | { [key: string]: HistorySessionDiagnosticValue };

export type HistorySessionDiagnosticData = Record<string, HistorySessionDiagnosticValue>;

interface HistorySessionDiagnosticEvent {
  event: string;
  ageMs: number;
  data?: HistorySessionDiagnosticData;
}

interface HistorySessionDiagnosticState {
  diagnosticId: string;
  sessionId: string;
  startedAtMs: number;
  events: HistorySessionDiagnosticEvent[];
  pendingHydrateStartedAtMs?: number;
  pendingHydrateData?: HistorySessionDiagnosticData;
  stalledWarned: boolean;
}

export interface HistorySessionLoadingStallSnapshot extends HistorySessionDiagnosticData {
  durationMs: number;
  historyState?: string;
  isHistorical?: boolean;
  isRemote?: boolean;
  activeSessionIdMatches?: boolean;
  hasRenderableContent?: boolean;
  dialogTurnCount?: number;
}

const diagnosticsBySession = new Map<string, HistorySessionDiagnosticState>();
let diagnosticSequence = 0;

function trimOldDiagnostics(): void {
  while (diagnosticsBySession.size > MAX_SESSION_DIAGNOSTICS) {
    const oldestSessionId = diagnosticsBySession.keys().next().value;
    if (!oldestSessionId) {
      return;
    }
    diagnosticsBySession.delete(oldestSessionId);
  }
}

function createDiagnosticId(sessionId: string): string {
  diagnosticSequence += 1;
  const prefix = sessionId.slice(0, 8) || 'session';
  return `${prefix}-${Math.round(nowMs())}-${diagnosticSequence}`;
}

function getOrCreateDiagnostic(sessionId: string): HistorySessionDiagnosticState {
  const existing = diagnosticsBySession.get(sessionId);
  if (existing) {
    return existing;
  }

  const created: HistorySessionDiagnosticState = {
    diagnosticId: createDiagnosticId(sessionId),
    sessionId,
    startedAtMs: nowMs(),
    events: [],
    stalledWarned: false,
  };
  diagnosticsBySession.set(sessionId, created);
  trimOldDiagnostics();
  return created;
}

function pushEvent(
  state: HistorySessionDiagnosticState,
  event: string,
  data?: HistorySessionDiagnosticData,
  options?: { logEvent?: boolean },
): void {
  const record: HistorySessionDiagnosticEvent = {
    event,
    ageMs: elapsedMs(state.startedAtMs),
    ...(data ? { data } : {}),
  };

  state.events.push(record);
  if (state.events.length > RECENT_EVENT_LIMIT) {
    state.events.splice(0, state.events.length - RECENT_EVENT_LIMIT);
  }

  if (options?.logEvent !== false) {
    log.debug('Historical session diagnostic event', {
      diagnosticId: state.diagnosticId,
      sessionId: state.sessionId,
      event,
      ageMs: record.ageMs,
      ...(data ?? {}),
    });
  }
}

function summarizeEvents(state: HistorySessionDiagnosticState): HistorySessionDiagnosticEvent[] {
  return state.events.slice(-WARN_EVENT_LIMIT).map(event => ({
    event: event.event,
    ageMs: event.ageMs,
    ...(event.data ? { data: event.data } : {}),
  }));
}

function findLastEvent(
  state: HistorySessionDiagnosticState,
  predicate: (event: HistorySessionDiagnosticEvent) => boolean,
): HistorySessionDiagnosticEvent | undefined {
  for (let index = state.events.length - 1; index >= 0; index -= 1) {
    const event = state.events[index];
    if (predicate(event)) {
      return event;
    }
  }
  return undefined;
}

export function beginHistorySessionDiagnostics(
  sessionId: string,
  event: string,
  data?: HistorySessionDiagnosticData,
): string {
  const state: HistorySessionDiagnosticState = {
    diagnosticId: createDiagnosticId(sessionId),
    sessionId,
    startedAtMs: nowMs(),
    events: [],
    stalledWarned: false,
  };
  diagnosticsBySession.set(sessionId, state);
  trimOldDiagnostics();
  pushEvent(state, event, data);
  return state.diagnosticId;
}

export function recordHistorySessionDiagnosticEvent(
  sessionId: string,
  event: string,
  data?: HistorySessionDiagnosticData,
  options?: { logEvent?: boolean },
): void {
  pushEvent(getOrCreateDiagnostic(sessionId), event, data, options);
}

export function markHistorySessionHydratePending(
  sessionId: string,
  data?: HistorySessionDiagnosticData,
): void {
  const state = getOrCreateDiagnostic(sessionId);
  state.pendingHydrateStartedAtMs = nowMs();
  state.pendingHydrateData = data;
  pushEvent(state, 'hydrate_pending_started', data);
}

export function clearHistorySessionHydratePending(
  sessionId: string,
  outcome: string,
  data?: HistorySessionDiagnosticData,
): void {
  const state = getOrCreateDiagnostic(sessionId);
  const pendingHydrateAgeMs = state.pendingHydrateStartedAtMs !== undefined
    ? elapsedMs(state.pendingHydrateStartedAtMs)
    : undefined;
  pushEvent(state, 'hydrate_pending_settled', {
    outcome,
    pendingHydrateAgeMs,
    ...(data ?? {}),
  });
  state.pendingHydrateStartedAtMs = undefined;
  state.pendingHydrateData = undefined;
}

export function warnHistorySessionLoadingLayerStalled(
  sessionId: string,
  snapshot: HistorySessionLoadingStallSnapshot,
): void {
  const state = getOrCreateDiagnostic(sessionId);
  if (state.stalledWarned) {
    return;
  }

  state.stalledWarned = true;
  pushEvent(state, 'loading_layer_stalled', snapshot, { logEvent: false });

  const lastHydrateEvent = findLastEvent(state, event => event.event.includes('hydrate'));
  const lastStoreEvent = findLastEvent(state, event => event.event.startsWith('store_'));
  const pendingHydrateAgeMs = state.pendingHydrateStartedAtMs !== undefined
    ? roundDurationMs(nowMs() - state.pendingHydrateStartedAtMs)
    : undefined;

  log.warn('Historical session loading layer stalled', {
    diagnosticId: state.diagnosticId,
    sessionId,
    ...snapshot,
  });
  log.warn('Historical session hydrate state at stall', {
    diagnosticId: state.diagnosticId,
    sessionId,
    hasPendingHydrate: state.pendingHydrateStartedAtMs !== undefined,
    pendingHydrateAgeMs,
    pendingHydrateData: state.pendingHydrateData,
    lastHydrateEvent: lastHydrateEvent?.event,
    lastStoreEvent: lastStoreEvent?.event,
    lastHydrateData: lastHydrateEvent?.data,
    lastStoreData: lastStoreEvent?.data,
  });
  log.warn('Historical session recent lifecycle events', {
    diagnosticId: state.diagnosticId,
    sessionId,
    events: summarizeEvents(state),
  });
}

export function resetHistorySessionDiagnosticsForTests(): void {
  diagnosticsBySession.clear();
  diagnosticSequence = 0;
}
