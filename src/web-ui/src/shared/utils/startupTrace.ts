import { createLogger } from './logger';
import { roundDurationMs, type LoggerLike } from './timing';

type NowFn = () => number;
type TraceData = Record<string, unknown>;

export type StartupTraceRequestType = 'tauri' | 'http';

export interface StartupTraceApiCall {
  type: StartupTraceRequestType;
  command: string;
  target?: string;
  durationMs: number;
  startedAtMs?: number;
  endedAtMs?: number;
  outcome?: 'success' | 'failure';
  cacheOutcome?: 'hit' | 'miss' | 'unknown';
  requestBytes?: number;
  responseBytes?: number;
  payloadEstimateDurationMs?: number;
  requestPayloadEstimateDurationMs?: number;
  responsePayloadEstimateDurationMs?: number;
  adapterInitDurationMs?: number;
  transportDurationMs?: number;
  invokeDurationMs?: number;
  activeRequestsAtStart?: number;
  activeRequestsAtEnd?: number;
  maxConcurrentRequests?: number;
  remote: boolean;
}

export interface StartupTraceOptions {
  enabled?: boolean;
  logger?: LoggerLike;
  traceId?: string;
  now?: NowFn;
  maxPhaseEvents?: number;
  maxApiCallRecords?: number;
  logEvents?: boolean;
  logSummary?: boolean;
}

export interface DeferredAnimationFrameTraceOptions {
  frameCount?: number;
  now?: NowFn;
  requestAnimationFrame?: (callback: (time: number) => void) => number;
}

export interface ReactRenderProfileTrace {
  component: string;
  phase: string;
  actualDurationMs: number;
  baseDurationMs?: number;
  startTimeMs?: number;
  commitTimeMs?: number;
  contentLength?: number;
  itemCount?: number;
  groupCount?: number;
  renderedCount?: number;
  turnId?: string;
  roundId?: string;
  itemId?: string;
  visibleGroupStartIndex?: number;
  visibleGroupEndIndex?: number;
  textItemCount?: number;
  toolItemCount?: number;
  visibleTextItemCount?: number;
  visibleToolItemCount?: number;
  criticalGroupCount?: number;
  exploreGroupCount?: number;
  hasCodeBlock?: boolean;
  hasTable?: boolean;
  isStreaming?: boolean;
  [key: string]: unknown;
}

interface CommandAggregate {
  command: string;
  count: number;
  successCount: number;
  failureCount: number;
  cacheHitCount: number;
  cacheMissCount: number;
  cacheUnknownCount: number;
  remoteCount: number;
  totalDurationMs: number;
  maxDurationMs: number;
  requestBytes: number;
  responseBytes: number;
}

interface PhaseRecord {
  traceId: string;
  phase: string;
  atMs: number;
  [key: string]: unknown;
}

export interface StartupTraceApiSummary {
  totalCount: number;
  successCount: number;
  failureCount: number;
  cacheHitCount: number;
  cacheMissCount: number;
  cacheUnknownCount: number;
  remoteCount: number;
  requestBytes: number;
  responseBytes: number;
  payloadEstimateDurationMs: number;
  byCommand: CommandAggregate[];
  calls: StartupTraceApiCallRecord[];
}

export interface StartupTraceApiCallRecord {
  traceId: string;
  type: StartupTraceRequestType;
  command: string;
  target?: string;
  startedAtMs?: number;
  endedAtMs?: number;
  durationMs: number;
  outcome: 'success' | 'failure';
  cacheOutcome: 'hit' | 'miss' | 'unknown';
  requestBytes: number;
  responseBytes: number;
  remote: boolean;
  payloadEstimateDurationMs?: number;
  requestPayloadEstimateDurationMs?: number;
  responsePayloadEstimateDurationMs?: number;
  adapterInitDurationMs?: number;
  transportDurationMs?: number;
  invokeDurationMs?: number;
  activeRequestsAtStart?: number;
  activeRequestsAtEnd?: number;
  maxConcurrentRequests?: number;
}

export interface StartupTraceSnapshot {
  traceId: string;
  phases: {
    count: number;
    events: PhaseRecord[];
  };
  api: StartupTraceApiSummary;
}

export interface StartupTraceDiagnostics {
  snapshot: () => StartupTraceSnapshot;
  flushSummary: (reason: string) => void;
}

declare global {
  // E2E and manual performance collection read this sanitized diagnostic surface.
  // It intentionally exposes no raw request payloads or workspace paths.
  var __BITFUN_STARTUP_TRACE__: StartupTraceDiagnostics | undefined;
  var __BITFUN_PERF_TRACE_ENABLED__: boolean | undefined;
  var __BITFUN_RENDER_PROFILE_ENABLED__: boolean | undefined;
}

const DEFAULT_MAX_ESTIMATED_BYTES = 64 * 1024;
const SENSITIVE_KEY_PATTERN =
  /(api[-_]?key|authorization|bearer|token|secret|password|credential|payload|request|response|args|remoteconnectionid|remote[_-]?connection[_-]?id|remotesshhost|remote[_-]?ssh[_-]?host|sshhost|ssh[_-]?host|workspacepath|workspace[_-]?path)/i;

function createTraceId(): string {
  const injectedTraceId = (globalThis as { __BITFUN_STARTUP_TRACE_ID__?: unknown })
    .__BITFUN_STARTUP_TRACE_ID__;
  if (typeof injectedTraceId === 'string' && injectedTraceId.trim().length > 0) {
    return injectedTraceId;
  }

  const cryptoLike = globalThis.crypto;
  if (cryptoLike && typeof cryptoLike.randomUUID === 'function') {
    return cryptoLike.randomUUID();
  }
  return `startup-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function optionalRounded(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value)
    ? roundDurationMs(value)
    : undefined;
}

function optionalString(value: unknown): string | undefined {
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}

function isSafeScalar(value: unknown): value is string | number | boolean | null {
  return (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  );
}

function sanitizeTraceData(data?: TraceData): TraceData | undefined {
  if (!data) {
    return undefined;
  }

  const sanitized: TraceData = {};
  for (const [key, value] of Object.entries(data)) {
    if (SENSITIVE_KEY_PATTERN.test(key)) {
      continue;
    }
    if (isSafeScalar(value)) {
      sanitized[key] = value;
      continue;
    }
    if (Array.isArray(value) && value.every(isSafeScalar)) {
      sanitized[key] = value;
    }
  }
  return sanitized;
}

function isNonLocalRemoteHost(value: unknown): boolean {
  if (typeof value !== 'string') {
    return false;
  }
  const normalized = value.trim().toLowerCase();
  if (!normalized) {
    return false;
  }
  return !(
    normalized === 'localhost' ||
    normalized.startsWith('localhost:') ||
    normalized === '127.0.0.1' ||
    normalized.startsWith('127.0.0.1:') ||
    normalized === '::1' ||
    normalized === '[::1]' ||
    normalized.startsWith('[::1]:')
  );
}

export function isRemoteTraceContext(
  remoteConnectionId?: unknown,
  remoteSshHost?: unknown
): boolean {
  if (typeof remoteConnectionId === 'string' && remoteConnectionId.trim().length > 0) {
    return true;
  }
  return isNonLocalRemoteHost(remoteSshHost);
}

function hasRemoteKey(key: string, value: unknown): boolean {
  const normalized = key.toLowerCase();
  if (
    normalized === 'remoteconnectionid' ||
    normalized === 'remote_connection_id'
  ) {
    return typeof value === 'string' && value.trim().length > 0;
  }
  if (
    normalized === 'remotesshhost' ||
    normalized === 'remote_ssh_host' ||
    normalized === 'sshhost' ||
    normalized === 'ssh_host'
  ) {
    return isNonLocalRemoteHost(value);
  }
  if (normalized === 'workspacekind' || normalized === 'workspace_kind') {
    return typeof value === 'string' && value.toLowerCase() === 'remote';
  }
  return false;
}

export function isRemoteTraceRequest(value: unknown, maxDepth = 4): boolean {
  if (!value || typeof value !== 'object' || maxDepth < 0) {
    return false;
  }

  if (Array.isArray(value)) {
    return value.some(item => isRemoteTraceRequest(item, maxDepth - 1));
  }

  for (const [key, nested] of Object.entries(value as Record<string, unknown>)) {
    if (hasRemoteKey(key, nested)) {
      return true;
    }
    if (nested && typeof nested === 'object' && isRemoteTraceRequest(nested, maxDepth - 1)) {
      return true;
    }
  }

  return false;
}

export function estimateJsonBytes(value: unknown, maxBytes = DEFAULT_MAX_ESTIMATED_BYTES): number {
  const seen = new WeakSet<object>();
  let total = 0;

  const add = (amount: number) => {
    total = Math.min(maxBytes, total + amount);
  };

  const visit = (current: unknown, depth: number) => {
    if (total >= maxBytes || depth < 0) {
      return;
    }

    if (current === null || current === undefined) {
      add(4);
      return;
    }

    if (typeof current === 'string') {
      add(current.length + 2);
      return;
    }

    if (typeof current === 'number' || typeof current === 'boolean') {
      add(String(current).length);
      return;
    }

    if (typeof current !== 'object') {
      add(String(current).length + 2);
      return;
    }

    if (seen.has(current)) {
      add(12);
      return;
    }
    seen.add(current);

    if (Array.isArray(current)) {
      add(2);
      for (const item of current) {
        add(1);
        visit(item, depth - 1);
        if (total >= maxBytes) {
          return;
        }
      }
      return;
    }

    add(2);
    for (const [key, nested] of Object.entries(current as Record<string, unknown>)) {
      add(key.length + 3);
      visit(nested, depth - 1);
      if (total >= maxBytes) {
        return;
      }
    }
  };

  visit(value, 8);
  return total;
}

export class StartupTrace {
  private readonly enabled: boolean;
  private readonly logger: LoggerLike;
  private readonly now: NowFn;
  private readonly maxPhaseEvents: number;
  private readonly maxApiCallRecords: number;
  private readonly logEvents: boolean;
  private readonly logSummary: boolean;
  readonly traceId: string;
  private phaseEvents = 0;
  private readonly phaseRecords: PhaseRecord[] = [];
  private apiCallEvents = 0;
  private readonly apiCallRecords: StartupTraceApiCallRecord[] = [];
  private readonly commandAggregates = new Map<string, CommandAggregate>();
  private totalApiCount = 0;
  private successfulApiCount = 0;
  private failedApiCount = 0;
  private cacheHitCount = 0;
  private cacheMissCount = 0;
  private cacheUnknownCount = 0;
  private remoteApiCount = 0;
  private requestBytes = 0;
  private responseBytes = 0;
  private payloadEstimateDurationMs = 0;

  constructor(options: StartupTraceOptions = {}) {
    this.enabled = options.enabled ?? true;
    this.logger = options.logger ?? createLogger('StartupTrace');
    this.traceId = options.traceId ?? createTraceId();
    this.now = options.now ?? (() => globalThis.performance?.now?.() ?? Date.now());
    this.maxPhaseEvents = options.maxPhaseEvents ?? 260;
    this.maxApiCallRecords = options.maxApiCallRecords ?? 240;
    this.logEvents = options.logEvents ?? false;
    this.logSummary = options.logSummary ?? false;
  }

  markPhase(phase: string, data?: TraceData): void {
    if (!this.enabled || this.phaseEvents >= this.maxPhaseEvents) {
      return;
    }
    this.phaseEvents += 1;
    const phaseRecord = {
      traceId: this.traceId,
      phase,
      atMs: roundDurationMs(this.now()),
      ...(sanitizeTraceData(data) ?? {}),
    };
    this.phaseRecords.push(phaseRecord);
    if (this.logEvents) {
      this.logger.debug('Startup trace event', phaseRecord);
    }
  }

  recordApiCall(call: StartupTraceApiCall): void {
    if (!this.enabled) {
      return;
    }

    const durationMs = roundDurationMs(call.durationMs);
    const startedAtMs = typeof call.startedAtMs === 'number'
      ? roundDurationMs(call.startedAtMs)
      : undefined;
    const endedAtMs = typeof call.endedAtMs === 'number'
      ? roundDurationMs(call.endedAtMs)
      : (startedAtMs !== undefined ? roundDurationMs(startedAtMs + durationMs) : undefined);
    const requestBytes = call.requestBytes ?? 0;
    const responseBytes = call.responseBytes ?? 0;
    const requestPayloadEstimateDurationMs = optionalRounded(call.requestPayloadEstimateDurationMs);
    const responsePayloadEstimateDurationMs = optionalRounded(call.responsePayloadEstimateDurationMs);
    const payloadEstimateDurationMs = optionalRounded(call.payloadEstimateDurationMs)
      ?? roundDurationMs((requestPayloadEstimateDurationMs ?? 0) + (responsePayloadEstimateDurationMs ?? 0));
    const succeeded = call.outcome !== 'failure';
    const cacheOutcome = call.cacheOutcome ?? 'unknown';
    this.totalApiCount += 1;
    this.successfulApiCount += succeeded ? 1 : 0;
    this.failedApiCount += succeeded ? 0 : 1;
    this.cacheHitCount += cacheOutcome === 'hit' ? 1 : 0;
    this.cacheMissCount += cacheOutcome === 'miss' ? 1 : 0;
    this.cacheUnknownCount += cacheOutcome === 'unknown' ? 1 : 0;
    this.remoteApiCount += call.remote ? 1 : 0;
    this.requestBytes += requestBytes;
    this.responseBytes += responseBytes;
    this.payloadEstimateDurationMs += payloadEstimateDurationMs;

    if (this.apiCallEvents < this.maxApiCallRecords) {
      this.apiCallEvents += 1;
      this.apiCallRecords.push({
        traceId: this.traceId,
        type: call.type,
        command: call.command,
        target: call.target,
        startedAtMs,
        endedAtMs,
        durationMs,
        outcome: succeeded ? 'success' : 'failure',
        cacheOutcome,
        requestBytes,
        responseBytes,
        remote: call.remote,
        payloadEstimateDurationMs,
        requestPayloadEstimateDurationMs,
        responsePayloadEstimateDurationMs,
        adapterInitDurationMs: optionalRounded(call.adapterInitDurationMs),
        transportDurationMs: optionalRounded(call.transportDurationMs),
        invokeDurationMs: optionalRounded(call.invokeDurationMs),
        activeRequestsAtStart: optionalRounded(call.activeRequestsAtStart),
        activeRequestsAtEnd: optionalRounded(call.activeRequestsAtEnd),
        maxConcurrentRequests: optionalRounded(call.maxConcurrentRequests),
      });
    }

    const existing = this.commandAggregates.get(call.command) ?? {
      command: call.command,
      count: 0,
      successCount: 0,
      failureCount: 0,
      cacheHitCount: 0,
      cacheMissCount: 0,
      cacheUnknownCount: 0,
      remoteCount: 0,
      totalDurationMs: 0,
      maxDurationMs: 0,
      requestBytes: 0,
      responseBytes: 0,
    };

    existing.count += 1;
    existing.successCount += succeeded ? 1 : 0;
    existing.failureCount += succeeded ? 0 : 1;
    existing.cacheHitCount += cacheOutcome === 'hit' ? 1 : 0;
    existing.cacheMissCount += cacheOutcome === 'miss' ? 1 : 0;
    existing.cacheUnknownCount += cacheOutcome === 'unknown' ? 1 : 0;
    existing.remoteCount += call.remote ? 1 : 0;
    existing.totalDurationMs = roundDurationMs(existing.totalDurationMs + durationMs);
    existing.maxDurationMs = Math.max(existing.maxDurationMs, durationMs);
    existing.requestBytes += requestBytes;
    existing.responseBytes += responseBytes;
    this.commandAggregates.set(call.command, existing);
  }

  private buildApiSummary(): StartupTraceApiSummary {
    const byCommand = Array.from(this.commandAggregates.values())
      .sort((left, right) => right.totalDurationMs - left.totalDurationMs)
      .map(item => ({
        ...item,
        totalDurationMs: roundDurationMs(item.totalDurationMs),
        maxDurationMs: roundDurationMs(item.maxDurationMs),
      }));

    return {
      totalCount: this.totalApiCount,
      successCount: this.successfulApiCount,
      failureCount: this.failedApiCount,
      cacheHitCount: this.cacheHitCount,
      cacheMissCount: this.cacheMissCount,
      cacheUnknownCount: this.cacheUnknownCount,
      remoteCount: this.remoteApiCount,
      requestBytes: this.requestBytes,
      responseBytes: this.responseBytes,
      payloadEstimateDurationMs: roundDurationMs(this.payloadEstimateDurationMs),
      byCommand,
      calls: this.apiCallRecords.map(record => ({ ...record })),
    };
  }

  getSnapshot(): StartupTraceSnapshot {
    return {
      traceId: this.traceId,
      phases: {
        count: this.phaseEvents,
        events: this.phaseRecords.map(record => ({ ...record })),
      },
      api: this.buildApiSummary(),
    };
  }

  flushSummary(reason: string): void {
    if (!this.enabled) {
      return;
    }

    if (this.logSummary) {
      const snapshot = this.getSnapshot();
      this.logger.info('Startup trace summary', {
        traceId: snapshot.traceId,
        reason,
        phases: snapshot.phases,
        api: {
          ...snapshot.api,
        },
      });
    }
  }
}

export function createStartupTrace(options: StartupTraceOptions = {}): StartupTrace {
  return new StartupTrace(options);
}

export const startupTrace = createStartupTrace();

export function isStartupRenderTraceEnabled(): boolean {
  return globalThis.__BITFUN_RENDER_PROFILE_ENABLED__ === true;
}

export function recordReactRenderProfile(
  trace: StartupTrace,
  profile: ReactRenderProfileTrace
): void {
  if (!isStartupRenderTraceEnabled()) {
    return;
  }

  trace.markPhase('react_render_profile', {
    component: profile.component,
    renderPhase: profile.phase,
    actualDurationMs: roundDurationMs(profile.actualDurationMs),
    baseDurationMs: optionalRounded(profile.baseDurationMs),
    startTimeMs: optionalRounded(profile.startTimeMs),
    commitTimeMs: optionalRounded(profile.commitTimeMs),
    contentLength: optionalRounded(profile.contentLength),
    itemCount: optionalRounded(profile.itemCount),
    groupCount: optionalRounded(profile.groupCount),
    renderedCount: optionalRounded(profile.renderedCount),
    turnId: optionalString(profile.turnId),
    roundId: optionalString(profile.roundId),
    itemId: optionalString(profile.itemId),
    visibleGroupStartIndex: optionalRounded(profile.visibleGroupStartIndex),
    visibleGroupEndIndex: optionalRounded(profile.visibleGroupEndIndex),
    textItemCount: optionalRounded(profile.textItemCount),
    toolItemCount: optionalRounded(profile.toolItemCount),
    visibleTextItemCount: optionalRounded(profile.visibleTextItemCount),
    visibleToolItemCount: optionalRounded(profile.visibleToolItemCount),
    criticalGroupCount: optionalRounded(profile.criticalGroupCount),
    exploreGroupCount: optionalRounded(profile.exploreGroupCount),
    hasCodeBlock: profile.hasCodeBlock,
    hasTable: profile.hasTable,
    isStreaming: profile.isStreaming,
  });
}

if (import.meta.env.DEV || globalThis.__BITFUN_PERF_TRACE_ENABLED__ === true) {
  globalThis.__BITFUN_STARTUP_TRACE__ = {
    snapshot: () => startupTrace.getSnapshot(),
    flushSummary: (reason: string) => startupTrace.flushSummary(reason),
  };
}

export function markPhaseAfterAnimationFrames(
  trace: StartupTrace,
  phase: string,
  data?: TraceData,
  options: DeferredAnimationFrameTraceOptions = {}
): void {
  const frameCount = Math.max(1, Math.floor(options.frameCount ?? 2));
  const now = options.now ?? (() => globalThis.performance?.now?.() ?? Date.now());
  const requestFrame = options.requestAnimationFrame ?? globalThis.requestAnimationFrame?.bind(globalThis);
  const startedAt = now();

  if (!requestFrame) {
    trace.markPhase(phase, {
      ...(data ?? {}),
      frameCount: 0,
      durationMs: 0,
    });
    return;
  }

  let remainingFrames = frameCount;
  const scheduleNextFrame = () => {
    requestFrame(() => {
      remainingFrames -= 1;
      if (remainingFrames <= 0) {
        trace.markPhase(phase, {
          ...(data ?? {}),
          frameCount,
          durationMs: roundDurationMs(now() - startedAt),
        });
        return;
      }
      scheduleNextFrame();
    });
  };

  scheduleNextFrame();
}
