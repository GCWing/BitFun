/**
 * Event batcher
 * 
 * Uses requestAnimationFrame to batch high-frequency events and reduce UI updates
 * 
 * Design principles:
 * - Events with the same key are merged (accumulated or replaced)
 * - Batch processing triggered once per frame
 * - Supports different key generation strategies for normal and subagent events
 */

import { areSensitiveDiagnosticsEnabled, createLogger } from '@/shared/utils/logger';
import { elapsedMs, nowMs } from '@/shared/utils/timing';

const log = createLogger('EventBatcher');

export type MergeStrategy = 'accumulate' | 'replace';

export interface BatchedEvent<T = any> {
  key: string;
  payload: T;
  strategy: MergeStrategy;
  accumulator?: (existing: T, incoming: T) => T;
  sourceCount: number;
  timestamp: number;
}

export interface EventBatcherOptions {
  onFlush: (events: Array<{ key: string; payload: any }>) => void;
}

export interface BatchedEventLogSummary {
  rawEventCount: number;
  mergedEventCount: number;
  events: Array<{
    key: string;
    strategy: MergeStrategy;
    sourceCount: number;
    timestamp: number;
    eventType?: string;
    toolName?: string;
    paramsLength?: number;
  }>;
}

export interface SensitiveBatchedEventLogPayload {
  rawEventCount: number;
  mergedEventCount: number;
  mergedEvents: BatchedEvent[];
}

export function summarizeBatchedEventsForLog(bufferedEvents: BatchedEvent[]): BatchedEventLogSummary {
  return {
    rawEventCount: bufferedEvents.reduce((count, event) => count + event.sourceCount, 0),
    mergedEventCount: bufferedEvents.length,
    events: bufferedEvents.map(({ key, payload, strategy, sourceCount, timestamp }) => {
      const toolEvent = payload?.toolEvent;
      const params = toolEvent?.params;

      return {
        key,
        strategy,
        sourceCount,
        timestamp,
        eventType: toolEvent?.event_type,
        toolName: toolEvent?.tool_name,
        paramsLength: typeof params === 'string' ? params.length : undefined,
      };
    }),
  };
}

export function getBatchedEventsLogPayload(
  bufferedEvents: BatchedEvent[]
): BatchedEventLogSummary | SensitiveBatchedEventLogPayload {
  const rawEventCount = bufferedEvents.reduce((count, event) => count + event.sourceCount, 0);
  const mergedEventCount = bufferedEvents.length;

  if (!areSensitiveDiagnosticsEnabled()) {
    return summarizeBatchedEventsForLog(bufferedEvents);
  }

  return {
    rawEventCount,
    mergedEventCount,
    mergedEvents: bufferedEvents.map(({ key, payload, strategy, sourceCount, timestamp }) => ({
      key,
      payload,
      strategy,
      sourceCount,
      timestamp,
    })),
  };
}

export class EventBatcher {
  private buffer: Map<string, BatchedEvent> = new Map();
  private scheduled = false;
  private onFlush: (events: Array<{ key: string; payload: any }>) => void;
  private frameId: number | null = null;

  // Update frequency control to prevent UI blocking with many events
  private UPDATE_INTERVAL = 100; // Update every 100ms instead of every frame (16.67ms)
  private lastUpdateTime = 0;
  private timeoutId: ReturnType<typeof setTimeout> | null = null;

  constructor(options: EventBatcherOptions) {
    this.onFlush = options.onFlush;
  }

  add<T>(
    key: string,
    payload: T,
    strategy: MergeStrategy = 'replace',
    accumulator?: (existing: T, incoming: T) => T
  ): void {
    const existing = this.buffer.get(key);

    if (existing) {
      if (strategy === 'accumulate' && accumulator) {
        existing.payload = accumulator(existing.payload, payload);
        existing.timestamp = Date.now();
      } else {
        existing.payload = payload;
        existing.timestamp = Date.now();
      }
      existing.sourceCount += 1;
    } else {
      this.buffer.set(key, {
        key,
        payload,
        strategy,
        accumulator,
        sourceCount: 1,
        timestamp: Date.now()
      });
    }

    this.scheduleFlush();
  }

  private scheduleFlush(): void {
    if (this.scheduled) return;
    this.scheduled = true;

    const now = nowMs();
    const timeSinceLastUpdate = now - this.lastUpdateTime;

    if (timeSinceLastUpdate >= this.UPDATE_INTERVAL) {
      this.frameId = requestAnimationFrame(() => {
        this.flush();
        this.scheduled = false;
        this.frameId = null;
        this.lastUpdateTime = nowMs();
      });
    } else {
      const delay = this.UPDATE_INTERVAL - timeSinceLastUpdate;
      this.timeoutId = setTimeout(() => {
        this.frameId = requestAnimationFrame(() => {
          this.flush();
          this.scheduled = false;
          this.frameId = null;
          this.lastUpdateTime = nowMs();
        });
        this.timeoutId = null;
      }, delay);
    }
  }

  private flush(): void {
    if (this.buffer.size === 0) return;

    const startTime = nowMs();
    const bufferedEvents = Array.from(this.buffer.values());
    const logPayload = getBatchedEventsLogPayload(bufferedEvents);
    const { rawEventCount, mergedEventCount } = logPayload;

    const events = bufferedEvents.map(({ key, payload }) => ({
      key,
      payload
    }));

    log.trace('Flushing batched events', logPayload);

    this.buffer = new Map();
    this.onFlush(events);

    const durationMs = elapsedMs(startTime);
    if (durationMs > 10) {
      log.warn('Event batch processing took longer than expected', {
        rawEventCount,
        mergedEventCount,
        durationMs,
      });
    }
  }

  flushNow(): void {
    if (this.frameId !== null) {
      cancelAnimationFrame(this.frameId);
      this.frameId = null;
    }
    if (this.timeoutId !== null) {
      clearTimeout(this.timeoutId);
      this.timeoutId = null;
    }
    this.scheduled = false;
    this.flush();
  }

  getBufferSize(): number {
    return this.buffer.size;
  }

  clear(): void {
    if (this.frameId !== null) {
      cancelAnimationFrame(this.frameId);
      this.frameId = null;
    }
    if (this.timeoutId !== null) {
      clearTimeout(this.timeoutId);
      this.timeoutId = null;
    }
    this.buffer.clear();
    this.scheduled = false;
  }

  destroy(): void {
    this.clear();
  }
}

export interface SubagentParentInfo {
  sessionId: string;
  toolCallId: string;
  dialogTurnId: string;
}

export type ToolEventType =
  | 'EarlyDetected'
  | 'ParamsPartial'
  | 'Queued'
  | 'Waiting'
  | 'Started'
  | 'Progress'
  | 'Streaming'
  | 'StreamChunk'
  | 'ConfirmationNeeded'
  | 'Confirmed'
  | 'Rejected'
  | 'Completed'
  | 'Failed'
  | 'Cancelled';

interface BaseToolEvent<T extends ToolEventType> {
  event_type: T;
  tool_id: string;
  tool_name: string;
}

export type EarlyDetectedToolEvent = BaseToolEvent<'EarlyDetected'>;

export interface ParamsPartialToolEvent extends BaseToolEvent<'ParamsPartial'> {
  params: string;
}

export function normalizeParamsPartialFragment(params: unknown): string {
  return typeof params === 'string' ? params : '';
}

export interface QueuedToolEvent extends BaseToolEvent<'Queued'> {
  position: number;
}

export interface WaitingToolEvent extends BaseToolEvent<'Waiting'> {
  dependencies: string[];
}

export interface StartedToolEvent extends BaseToolEvent<'Started'> {
  params: unknown;
  timeout_seconds?: number;
}

export interface ProgressToolEvent extends BaseToolEvent<'Progress'> {
  message: string;
  percentage: number;
}

export interface StreamingToolEvent extends BaseToolEvent<'Streaming'> {
  chunks_received: number;
}

export interface StreamChunkToolEvent extends BaseToolEvent<'StreamChunk'> {
  data: unknown;
}

export interface ConfirmationNeededToolEvent extends BaseToolEvent<'ConfirmationNeeded'> {
  params: unknown;
}

export type ConfirmedToolEvent = BaseToolEvent<'Confirmed'>;

export type RejectedToolEvent = BaseToolEvent<'Rejected'>;

export interface CompletedToolEvent extends BaseToolEvent<'Completed'> {
  result: unknown;
  result_for_assistant?: string;
  duration_ms: number;
  queue_wait_ms?: number;
  preflight_ms?: number;
  confirmation_wait_ms?: number;
  execution_ms?: number;
}

export interface FailedToolEvent extends BaseToolEvent<'Failed'> {
  error: string;
  duration_ms?: number;
  queue_wait_ms?: number;
  preflight_ms?: number;
  confirmation_wait_ms?: number;
  execution_ms?: number;
}

export interface CancelledToolEvent extends BaseToolEvent<'Cancelled'> {
  reason: string;
  duration_ms?: number;
  queue_wait_ms?: number;
  preflight_ms?: number;
  confirmation_wait_ms?: number;
  execution_ms?: number;
}

export type FlowToolEvent =
  | EarlyDetectedToolEvent
  | ParamsPartialToolEvent
  | QueuedToolEvent
  | WaitingToolEvent
  | StartedToolEvent
  | ProgressToolEvent
  | StreamingToolEvent
  | StreamChunkToolEvent
  | ConfirmationNeededToolEvent
  | ConfirmedToolEvent
  | RejectedToolEvent
  | CompletedToolEvent
  | FailedToolEvent
  | CancelledToolEvent;

export interface TextChunkEventData {
  sessionId: string;
  turnId: string;
  roundId: string;
  text: string;
  contentType: 'text' | 'thinking';
  isThinkingEnd?: boolean;
  subagentParentInfo?: SubagentParentInfo;
}

export interface ToolEventData {
  sessionId: string;
  turnId: string;
  toolEvent: FlowToolEvent;
  subagentParentInfo?: SubagentParentInfo;
}

/**
 * Generate merge key for TextChunk events
 * 
 * Key structure:
 * - Normal text: text:{sessionId}:{roundId}:{contentType}
 * - Subagent text: subagent:text:{parentSessionId}:{parentToolId}:{subSessionId}:{roundId}:{contentType}
 */
export function generateTextChunkKey(data: TextChunkEventData): string {
  const { sessionId, roundId, contentType, subagentParentInfo } = data;

  if (subagentParentInfo) {
    const { sessionId: parentSessionId, toolCallId: parentToolId } = subagentParentInfo;
    return `subagent:text:${parentSessionId}:${parentToolId}:${sessionId}:${roundId}:${contentType}`;
  } else {
    return `text:${sessionId}:${roundId}:${contentType}`;
  }
}

/**
 * Generate merge key for ToolEvent events
 * 
 * Returns null if the event doesn't need batching (isolated event)
 * 
 * Key structure:
 * - Tool params: tool:params:{sessionId}:{toolUseId}
 * - Subagent tool params: subagent:tool:params:{parentSessionId}:{parentToolId}:{subToolUseId}
 * - Tool progress: tool:progress:{sessionId}:{toolUseId}
 * - Subagent tool progress: subagent:tool:progress:{parentSessionId}:{parentToolId}:{subToolUseId}
 */
export function generateToolEventKey(data: ToolEventData): { key: string; strategy: MergeStrategy } | null {
  const { sessionId, toolEvent, subagentParentInfo } = data;
  const toolUseId = toolEvent.tool_id;
  const eventType = toolEvent.event_type;

  const isolatedEvents: ToolEventType[] = ['EarlyDetected', 'Started', 'Completed', 'Failed', 'Cancelled', 'ConfirmationNeeded'];
  if (isolatedEvents.includes(eventType)) {
    return null;
  }

  if (subagentParentInfo) {
    const { sessionId: parentSessionId, toolCallId: parentToolId } = subagentParentInfo;

    if (eventType === 'ParamsPartial') {
      const toolName = (toolEvent as any).tool_name || '';
      const isWriteLike = ['write', 'write_notebook', 'file_write', 'Write'].includes(toolName);
      return {
        key: `subagent:tool:params:${parentSessionId}:${parentToolId}:${toolUseId}`,
        strategy: isWriteLike ? 'replace' : 'accumulate'
      };
    }
    if (eventType === 'Progress') {
      return {
        key: `subagent:tool:progress:${parentSessionId}:${parentToolId}:${toolUseId}`,
        strategy: 'replace'
      };
    }
  } else {
    if (eventType === 'ParamsPartial') {
      const toolName = (toolEvent as any).tool_name || '';
      const isWriteLike = ['write', 'write_notebook', 'file_write', 'Write'].includes(toolName);
      return {
        key: `tool:params:${sessionId}:${toolUseId}`,
        strategy: isWriteLike ? 'replace' : 'accumulate'
      };
    }
    if (eventType === 'Progress') {
      return {
        key: `tool:progress:${sessionId}:${toolUseId}`,
        strategy: 'replace'
      };
    }
  }

  return null;
}

/**
 * Parse event key to extract event type information
 */
export function parseEventKey(key: string): {
  isSubagent: boolean;
  eventType: 'text' | 'tool:params' | 'tool:progress';
  ids: Record<string, string>;
} | null {
  const parts = key.split(':');

  if (parts[0] === 'subagent') {
    // subagent:text:parentSessionId:parentToolId:subSessionId:roundId:contentType
    // subagent:tool:params:parentSessionId:parentToolId:subToolUseId
    // subagent:tool:progress:parentSessionId:parentToolId:subToolUseId
    if (parts[1] === 'text') {
      return {
        isSubagent: true,
        eventType: 'text',
        ids: {
          parentSessionId: parts[2],
          parentToolId: parts[3],
          subSessionId: parts[4],
          roundId: parts[5],
          contentType: parts[6]
        }
      };
    } else if (parts[1] === 'tool' && parts[2] === 'params') {
      return {
        isSubagent: true,
        eventType: 'tool:params',
        ids: {
          parentSessionId: parts[3],
          parentToolId: parts[4],
          subToolUseId: parts[5]
        }
      };
    } else if (parts[1] === 'tool' && parts[2] === 'progress') {
      return {
        isSubagent: true,
        eventType: 'tool:progress',
        ids: {
          parentSessionId: parts[3],
          parentToolId: parts[4],
          subToolUseId: parts[5]
        }
      };
    }
  } else {
    // text:sessionId:roundId:contentType
    // tool:params:sessionId:toolUseId
    // tool:progress:sessionId:toolUseId
    if (parts[0] === 'text') {
      return {
        isSubagent: false,
        eventType: 'text',
        ids: {
          sessionId: parts[1],
          roundId: parts[2],
          contentType: parts[3]
        }
      };
    } else if (parts[0] === 'tool' && parts[1] === 'params') {
      return {
        isSubagent: false,
        eventType: 'tool:params',
        ids: {
          sessionId: parts[2],
          toolUseId: parts[3]
        }
      };
    } else if (parts[0] === 'tool' && parts[1] === 'progress') {
      return {
        isSubagent: false,
        eventType: 'tool:progress',
        ids: {
          sessionId: parts[2],
          toolUseId: parts[3]
        }
      };
    }
  }

  return null;
}
