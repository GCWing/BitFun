import type { DialogTurn } from '@/flow_chat/types/flow-chat';
import { createLogger } from '@/shared/utils/logger';
import { dispatchApi } from './dispatchApi';
import type { DispatchObserverJob } from './dispatchJobStore';
import {
  DISPATCH_TRANSCRIPT_SCHEMA_VERSION,
  type DispatchTranscriptCache,
} from './types';

const log = createLogger('DispatchTranscriptCache');

/**
 * Coalescing window for transcript writes.
 *
 * A streaming job produces many small projection updates per second. Writing
 * the whole transcript for each one would trade the replay cost this cache
 * removes for a file-write cost of the same order.
 */
const SAVE_THROTTLE_MS = 3000;

/**
 * Payloads are captured at a quiescent point and written later, so what is
 * pending here is always internally consistent even if the observer has moved
 * on since. Only the newest payload per job survives the throttle window.
 */
const pendingPayloads = new Map<string, DispatchTranscriptCache>();
const pendingTimers = new Map<string, ReturnType<typeof setTimeout>>();

/**
 * Read and validate this controller's cached transcript for a job.
 *
 * Every rejection path returns `null`, which puts the observer back on the
 * full-replay-from-byte-zero behavior it had before this cache existed. That
 * fallback is the reason none of these checks need to be recoverable.
 */
export async function loadDispatchTranscript(
  job: Pick<DispatchObserverJob, 'jobId' | 'sessionId'>,
): Promise<DispatchTranscriptCache | null> {
  let cached: DispatchTranscriptCache | null;
  try {
    cached = await dispatchApi.loadTranscript(job.jobId);
  } catch (error) {
    log.warn('Failed to read cached dispatch transcript', {
      jobId: job.jobId,
      error,
    });
    return null;
  }
  if (!cached || typeof cached !== 'object') {
    return null;
  }
  if (cached.schemaVersion !== DISPATCH_TRANSCRIPT_SCHEMA_VERSION) {
    // The projection rules changed since this was written. Replaying is the
    // only way to reproject under the current ones.
    return null;
  }
  if (cached.jobId !== job.jobId || cached.sessionId !== job.sessionId) {
    return null;
  }
  if (!Number.isFinite(cached.cursor) || cached.cursor <= 0) {
    return null;
  }
  if (!Array.isArray(cached.dialogTurns) || cached.dialogTurns.length === 0) {
    return null;
  }
  if (!Array.isArray(cached.appliedEventIds)) {
    return null;
  }
  return cached;
}

/**
 * Capture the current projection for a job so it can be written soon.
 *
 * Call this only where the transcript and the cursor agree — that is, after a
 * whole status page has been applied and committed. Capturing eagerly and
 * writing lazily is what keeps a throttled write from ever pairing a cursor
 * with turns from a different point in the stream.
 */
export function scheduleDispatchTranscriptSave(
  job: DispatchObserverJob,
  dialogTurns: DialogTurn[],
  cursor: number,
): void {
  if (cursor <= 0 || dialogTurns.length === 0) {
    return;
  }
  pendingPayloads.set(job.jobId, {
    schemaVersion: DISPATCH_TRANSCRIPT_SCHEMA_VERSION,
    jobId: job.jobId,
    sessionId: job.sessionId,
    cursor,
    dialogTurns,
    appliedEventIds: job.appliedEventIds,
    // Completeness travels with the transcript. Restoring turns without these
    // would render a truncated history as a whole one.
    eventLogComplete: job.eventLogComplete,
    historyTruncated: job.historyTruncated,
    omittedEventCount: job.omittedEventCount,
  });
  if (pendingTimers.has(job.jobId)) {
    return;
  }
  pendingTimers.set(
    job.jobId,
    setTimeout(() => {
      pendingTimers.delete(job.jobId);
      void flushDispatchTranscriptSave(job.jobId);
    }, SAVE_THROTTLE_MS),
  );
}

/** Write whatever was last captured for a job, if anything. */
export async function flushDispatchTranscriptSave(jobId: string): Promise<void> {
  const timer = pendingTimers.get(jobId);
  if (timer) {
    clearTimeout(timer);
    pendingTimers.delete(jobId);
  }
  const payload = pendingPayloads.get(jobId);
  if (!payload) {
    return;
  }
  pendingPayloads.delete(jobId);
  try {
    const saved = await dispatchApi.saveTranscript(jobId, payload);
    if (!saved) {
      log.debug('Dispatch transcript above the controller cache limit', { jobId });
    }
  } catch (error) {
    // Losing the cache costs a full replay on the next restart, nothing more.
    log.warn('Failed to cache dispatch transcript', { jobId, error });
  }
}

/**
 * Drop a job's cached transcript, on disk and in flight.
 *
 * Deleting a projection must not leave its content readable in the controller's
 * cache until retention gets around to it.
 */
export async function forgetDispatchTranscript(jobId: string): Promise<void> {
  const timer = pendingTimers.get(jobId);
  if (timer) {
    clearTimeout(timer);
    pendingTimers.delete(jobId);
  }
  pendingPayloads.delete(jobId);
  try {
    await dispatchApi.saveTranscript(jobId, null);
  } catch (error) {
    log.warn('Failed to drop cached dispatch transcript', { jobId, error });
  }
}

/** Cancel every pending write without flushing. Used when the observer stops. */
export function cancelDispatchTranscriptSaves(): void {
  for (const timer of pendingTimers.values()) {
    clearTimeout(timer);
  }
  pendingTimers.clear();
  pendingPayloads.clear();
}
