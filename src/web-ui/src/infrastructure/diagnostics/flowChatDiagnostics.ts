import {
  appendFlowChatDiagnosticEntries,
  type FlowChatDiagnosticTransportEntry,
} from './flowChatDiagnosticsTransport';
import { isTauriRuntime } from '@/infrastructure/runtime';

const FLUSH_INTERVAL_MS = 150;
const FLUSH_BATCH_SIZE = 128;
const MAX_QUEUED_ENTRIES = 1024;

export interface FlowChatDiagnosticProbe {
  hypothesis: string;
  location: string;
  message: string;
  data?: () => Record<string, unknown>;
}

interface DroppedEntrySummary {
  count: number;
  firstSequence: number;
  lastSequence: number;
  lastTimestamp: string;
  lastPerformanceTimeMs: number;
}

class FlowChatDiagnosticsRecorder {
  private enabled = false;
  private sequence = 0;
  private queue: FlowChatDiagnosticTransportEntry[] = [];
  private flushTimer: number | null = null;
  private flushInFlight: Promise<void> | null = null;
  private flushRequested = false;
  private droppedEntrySummary: DroppedEntrySummary | null = null;

  isEnabled(): boolean {
    return this.enabled;
  }

  setEnabled(enabled: boolean): void {
    const supportedEnabled = enabled && isTauriRuntime();
    if (this.enabled === supportedEnabled) {
      return;
    }

    this.enabled = supportedEnabled;
    if (supportedEnabled) {
      window.addEventListener('pagehide', this.handlePageHide);
      this.trace({
        hypothesis: 'I',
        location: 'FlowChatDiagnosticsRecorder.setEnabled',
        message: 'Flow Chat diagnostics enabled',
      });
      return;
    }

    window.removeEventListener('pagehide', this.handlePageHide);
    this.clearFlushTimer();
    void this.flush();
  }

  trace(probe: FlowChatDiagnosticProbe): void {
    if (!this.enabled) {
      return;
    }

    let data: Record<string, unknown> | undefined;
    if (probe.data) {
      try {
        data = probe.data();
      } catch (error) {
        data = {
          probeDataError: error instanceof Error ? error.message : String(error),
        };
      }
    }

    const entry: FlowChatDiagnosticTransportEntry = {
      sequence: ++this.sequence,
      timestamp: new Date().toISOString(),
      performanceTimeMs: typeof performance === 'undefined' ? 0 : performance.now(),
      hypothesis: probe.hypothesis,
      location: probe.location,
      message: probe.message,
      ...(data ? { data } : {}),
    };

    if (this.queue.length >= MAX_QUEUED_ENTRIES) {
      const droppedEntry = this.queue.shift();
      if (droppedEntry) {
        this.recordDroppedEntries([droppedEntry]);
      }
    }
    this.queue.push(entry);

    if (this.queue.length >= FLUSH_BATCH_SIZE) {
      this.clearFlushTimer();
      void this.flush();
      return;
    }
    this.scheduleFlush();
  }

  flushNow(): Promise<void> {
    this.clearFlushTimer();
    return this.flush();
  }

  resetForTests(): void {
    this.enabled = false;
    this.sequence = 0;
    this.queue = [];
    this.flushRequested = false;
    this.droppedEntrySummary = null;
    this.clearFlushTimer();
    this.flushInFlight = null;
    if (typeof window !== 'undefined') {
      window.removeEventListener('pagehide', this.handlePageHide);
    }
  }

  private scheduleFlush(): void {
    if (this.flushTimer !== null || typeof window === 'undefined') {
      return;
    }
    this.flushTimer = window.setTimeout(() => {
      this.flushTimer = null;
      void this.flush();
    }, FLUSH_INTERVAL_MS);
  }

  private clearFlushTimer(): void {
    if (this.flushTimer === null || typeof window === 'undefined') {
      this.flushTimer = null;
      return;
    }
    window.clearTimeout(this.flushTimer);
    this.flushTimer = null;
  }

  private mergeDroppedEntrySummary(summary: DroppedEntrySummary): void {
    const current = this.droppedEntrySummary;
    if (!current) {
      this.droppedEntrySummary = summary;
      return;
    }

    this.droppedEntrySummary = {
      count: current.count + summary.count,
      firstSequence: Math.min(current.firstSequence, summary.firstSequence),
      lastSequence: Math.max(current.lastSequence, summary.lastSequence),
      lastTimestamp: summary.lastSequence >= current.lastSequence
        ? summary.lastTimestamp
        : current.lastTimestamp,
      lastPerformanceTimeMs: summary.lastSequence >= current.lastSequence
        ? summary.lastPerformanceTimeMs
        : current.lastPerformanceTimeMs,
    };
  }

  private recordDroppedEntries(entries: FlowChatDiagnosticTransportEntry[]): void {
    const firstEntry = entries[0];
    const lastEntry = entries.at(-1);
    if (!firstEntry || !lastEntry) {
      return;
    }

    this.mergeDroppedEntrySummary({
      count: entries.length,
      firstSequence: firstEntry.sequence,
      lastSequence: lastEntry.sequence,
      lastTimestamp: lastEntry.timestamp,
      lastPerformanceTimeMs: lastEntry.performanceTimeMs,
    });
  }

  private takeDroppedEntrySummary(): DroppedEntrySummary | null {
    const summary = this.droppedEntrySummary;
    this.droppedEntrySummary = null;
    return summary;
  }

  private createDroppedEntryMarker(
    summary: DroppedEntrySummary,
  ): FlowChatDiagnosticTransportEntry {
    return {
      sequence: summary.lastSequence,
      timestamp: summary.lastTimestamp,
      performanceTimeMs: summary.lastPerformanceTimeMs,
      hypothesis: 'I',
      location: 'FlowChatDiagnosticsRecorder.flush',
      message: 'Flow Chat diagnostic entries dropped before flush',
      data: {
        droppedEntries: summary.count,
        firstDroppedSequence: summary.firstSequence,
        lastDroppedSequence: summary.lastSequence,
      },
    };
  }

  private flush = (): Promise<void> => {
    this.flushRequested = true;
    if (this.flushInFlight) {
      return this.flushInFlight;
    }

    this.flushInFlight = this.drainQueue().finally(() => {
      this.flushInFlight = null;
      if (this.flushRequested) {
        void this.flush();
      }
    });
    return this.flushInFlight;
  };

  private drainQueue = async (): Promise<void> => {
    while (true) {
      this.flushRequested = false;
      if (this.queue.length === 0 && !this.droppedEntrySummary) {
        return;
      }

      const droppedSummary = this.takeDroppedEntrySummary();
      const normalBatchSize = droppedSummary
        ? FLUSH_BATCH_SIZE - 1
        : FLUSH_BATCH_SIZE;
      const normalEntries = this.queue.splice(0, normalBatchSize);
      const batch = droppedSummary
        ? [this.createDroppedEntryMarker(droppedSummary), ...normalEntries]
        : normalEntries;

      try {
        await appendFlowChatDiagnosticEntries(batch);
      } catch {
        if (droppedSummary) {
          this.mergeDroppedEntrySummary(droppedSummary);
        }
        this.recordDroppedEntries(normalEntries);
        this.flushRequested = false;
        if (this.enabled && this.queue.length > 0) {
          this.scheduleFlush();
        }
        return;
      }

      const hasPendingEntries = this.queue.length > 0 || Boolean(this.droppedEntrySummary);
      if (!hasPendingEntries) {
        return;
      }
      if (
        !this.enabled ||
        this.flushRequested ||
        this.queue.length >= FLUSH_BATCH_SIZE
      ) {
        continue;
      }

      this.scheduleFlush();
      return;
    }
  };

  private handlePageHide = (): void => {
    this.clearFlushTimer();
    void this.flush();
  };
}

export const flowChatDiagnostics = new FlowChatDiagnosticsRecorder();

export function setFlowChatDiagnosticsEnabled(enabled: boolean): void {
  flowChatDiagnostics.setEnabled(enabled);
}

export function isFlowChatDiagnosticsEnabled(): boolean {
  return flowChatDiagnostics.isEnabled();
}
